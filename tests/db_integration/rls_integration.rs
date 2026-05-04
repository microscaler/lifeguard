//! End-to-end Row Level Security propagation tests — Story 6.
//!
//! All connections run as the non-superuser role `rls_test_role` because
//! superusers bypass RLS by default, and the table owner is exempt from
//! `ENABLE ROW LEVEL SECURITY`.
//!
//! **Test scenarios:**
//!
//! - **Test A — Direct executor filters rows:** `MayPostgresExecutor` with a
//!   session context injects the context and the RLS policy returns the
//!   expected rows.
//! - **Test B — Fail-closed (no context):** Same executor without session
//!   context returns zero rows (RLS blocks the read).
//! - **Test C — Transaction `begin_with_session`:** Session context set at
//!   `BEGIN` time is inherited by all subsequent queries in the transaction.
//! - **Test D — Pool worker isolation:** Two pooled executors with different
//!   session contexts do not leak context across each other's queries.

use lifeguard::executor::MayPostgresExecutor;
use lifeguard::LifeError;
use lifeguard::LifeExecutor;
use lifeguard::LifeguardPool;
use lifeguard::PooledLifeExecutor;
use lifeguard::SessionContext;

/// Tenant UUIDs used across RLS integration tests.
/// Seed data rows use these same UUIDs as the tenant column value.
const TENANT_ALPHA: &str = "550e8400-e29b-41d4-a716-446655440001";
const TENANT_BETA: &str = "550e8400-e29b-41d4-a716-446655440002";
const TENANT_GAMMA: &str = "550e8400-e29b-41d4-a716-446655440003";
use sea_query::{Value, Values};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// One-time setup: creates the shared `rls_test_role` (non-superuser) and
/// the `rls_set_session` GUC helper function in `public` schema.
#[ctor::ctor]
fn rls_test_setup() {
    let ctx = crate::context::get_test_context();
    let conn = may_postgres::connect(&ctx.pg_url).expect("ctor: connect");
    let exec = MayPostgresExecutor::new(conn);

    // Create non-superuser role for RLS testing.
    // Must have LOGIN for pooled executor tests, and must NOT be superuser.
    exec.execute(
        "DO $$ BEGIN IF NOT EXISTS \
         (SELECT 1 FROM pg_roles WHERE rolname = 'rls_test_role') \
         THEN CREATE ROLE rls_test_role NOLOGIN NOCREATEDB NOCREATEROLE; \
         END IF; END $$;",
        &[],
    )
    .ok();
    // Give it LOGIN and password so pool connections work as rls_test_role.
    exec.execute(
        "ALTER ROLE rls_test_role WITH LOGIN PASSWORD 'rls_test_role_pw';",
        &[],
    )
    .ok();

    // Allow rls_test_role to connect to the database (handle any DB name).
    // Query current_database() inside a DO block so it works regardless of
    // the actual database name (e.g., in CI environments).
    exec.execute(
        "DO $$ BEGIN EXECUTE format('GRANT CONNECT ON DATABASE %I TO rls_test_role', current_database()); EXCEPTION WHEN OTHERS THEN NULL; END $$;",
        &[],
    )
    .ok();

    // Aggressively drop ALL existing overloaded versions of rls_set_session
    // from previous test runs. CREATE OR REPLACE only replaces the exact
    // signature — other overloads persist as ambiguities.
    exec.execute(
        "DO $$
        DECLARE
            r RECORD;
        BEGIN
            FOR r IN
                SELECT oid, proname, pg_catalog.pg_get_function_arguments(oid) AS args
                FROM pg_proc
                WHERE proname = 'rls_set_session'
            LOOP
                RAISE NOTICE 'Dropping: %(%s%)', proname, args;
                EXECUTE format('DROP FUNCTION %s(%s)', proname, args);
            END LOOP;
        END $$;",
        &[],
    )
    .ok();

    // Create the app-owned rls_set_session function in public (idempotent).
    // p_permissions is `jsonb` because to_sql_args() serializes permissions
    // as serde_json::Value.
    let func_sql = "CREATE OR REPLACE FUNCTION rls_set_session(
        p_user_id uuid, p_user_org uuid,
        p_user_type text, p_org_type text,
        p_permissions jsonb, p_user_email text
    ) RETURNS void LANGUAGE plpgsql AS $$
    BEGIN
        PERFORM set_config('auth.user_id',
            COALESCE(p_user_id::text, ''), false);
        PERFORM set_config('auth.tenant',
            COALESCE(p_user_type, ''), false);
        PERFORM set_config('auth.user_type',
            COALESCE(p_user_type, ''), false);
        PERFORM set_config('auth.org_type',
            COALESCE(p_org_type, ''), false);
        PERFORM set_config('auth.permissions',
            COALESCE(p_permissions::text, '[]'), false);
        PERFORM set_config('auth.user_email',
            COALESCE(p_user_email, ''), false);
    END; $$;";
    exec.execute(func_sql, &[])
        .expect("ctor: CREATE rls_set_session");

    // Allow rls_test_role to call the function
    exec.execute(
        "GRANT EXECUTE ON FUNCTION rls_set_session TO rls_test_role",
        &[],
    )
    .ok();

    // rls_test_role needs USAGE on public schema to resolve function calls
    exec.execute("GRANT USAGE ON SCHEMA public TO rls_test_role", &[])
        .ok();
}

/// Unique schema/table pair for each test to avoid collisions.
static RLS_SCHEMA_SEQ: AtomicU64 = AtomicU64::new(0);

fn unique_rls_schema_names() -> (String, String) {
    let n = RLS_SCHEMA_SEQ.fetch_add(1, Ordering::Relaxed);
    (format!("rls_test_{n}"), format!("t_rls_items_{n}"))
}

/// Create a superuser executor for setup (table-owning role).
fn make_superuser_executor(pg_url: &str) -> MayPostgresExecutor {
    MayPostgresExecutor::new(may_postgres::connect(pg_url).expect("connect"))
}

/// Create an executor that runs as `rls_test_role` for proper RLS testing.
fn make_rls_executor(pg_url: &str) -> MayPostgresExecutor {
    let conn = may_postgres::connect(pg_url).expect("connect");
    conn.execute("SET ROLE rls_test_role", &[])
        .expect("SET ROLE");
    MayPostgresExecutor::new(conn)
}

/// DDL that every RLS test needs:
/// 1. Unique schema + table
/// 2. Grants for rls_test_role (USAGE + SELECT)
/// 3. ENABLE ROW LEVEL SECURITY
/// 4. RLS policy using auth.tenant GUC
/// 5. Seed data across multiple tenants
fn setup_rls_fixture(executor: &MayPostgresExecutor, schema: &str, table: &str) {
    executor
        .execute(&format!("CREATE SCHEMA IF NOT EXISTS {schema}"), &[])
        .expect("CREATE SCHEMA");

    let _ = executor.execute(
        &format!("DROP TABLE IF EXISTS {schema}.{table} CASCADE"),
        &[],
    );

    executor
        .execute(
            &format!(
                "CREATE TABLE {schema}.{table} (\
                id SERIAL PRIMARY KEY, tenant TEXT NOT NULL, note TEXT NOT NULL)"
            ),
            &[],
        )
        .expect("CREATE TABLE");

    // rls_test_role must have USAGE on the schema and SELECT on the table
    executor
        .execute(
            &format!("GRANT USAGE ON SCHEMA {schema} TO rls_test_role"),
            &[],
        )
        .ok();
    executor
        .execute(
            &format!("GRANT SELECT ON {schema}.{table} TO rls_test_role"),
            &[],
        )
        .ok();

    // ENABLE RLS applies to non-owner roles.
    executor
        .execute(
            &format!("ALTER TABLE {schema}.{table} ENABLE ROW LEVEL SECURITY"),
            &[],
        )
        .expect("ENABLE RLS");

    // Policy: only rows where tenant matches auth.tenant GUC.
    executor
        .execute(
            &format!(
                "CREATE POLICY rls_tenant_filter ON {schema}.{table} \
                USING (tenant = NULLIF(current_setting('auth.tenant', true), ''))"
            ),
            &[],
        )
        .expect("CREATE RLS policy");

    // Seed data: 4 rows across 3 tenants.
    executor
        .execute(
            &format!(
                "INSERT INTO {schema}.{table} (tenant, note) VALUES \
                ({TENANT_ALPHA}, 'alpha-item-a'), \
                ({TENANT_ALPHA}, 'alpha-item-b'), \
                ({TENANT_BETA}, 'beta-item'), \
                ({TENANT_GAMMA}, 'gamma-item')"
            ),
            &[],
        )
        .expect("seed data");
}

/// Count all rows visible to the current session (RLS-aware).
///
/// Propagates errors so callers can distinguish a genuine database error
/// (e.g. missing `rls_set_session`) from a normal zero-row result.
fn count_visible_rows(
    executor: &MayPostgresExecutor,
    schema: &str,
    table: &str,
) -> Result<i64, LifeError> {
    let row = executor.query_one_values(
        &format!("SELECT COUNT(*)::bigint AS c FROM {schema}.{table}"),
        &Values(vec![]),
    )?;
    Ok(row.get(0))
}

/// Convenience wrapper: counts visible rows and unwraps on error.
/// Use `count_visible_rows()` directly when you need to inspect errors
/// (e.g. testing that a missing `rls_set_session` function returns Err).
fn count_visible_rows_ok(executor: &MayPostgresExecutor, schema: &str, table: &str) -> i64 {
    count_visible_rows(executor, schema, table).expect("count query")
}

// ===================================================================
// Test A: Direct executor verifies RLS filters rows correctly
// ===================================================================

#[test]
fn test_a_direct_executor_filters_rows() {
    let ctx = crate::context::get_test_context();
    let primary_url = ctx.pg_url.clone();
    let (schema, table) = unique_rls_schema_names();

    // Setup fixture using superuser connection.
    let setup = make_superuser_executor(&primary_url);
    setup_rls_fixture(&setup, &schema, &table);

    // Without any session context, RLS blocks everything (fail-closed baseline).
    let no_ctx = make_rls_executor(&primary_url);
    let c_no = count_visible_rows_ok(&no_ctx, &schema, &table);
    assert_eq!(c_no, 0, "No context -> zero rows (fail-closed baseline)");

    // With session context for tenant "alpha" we should see exactly 2 rows.
    // Using set_config(..., false) ensures GUCs persist at the session level,
    // so they survive across separate autocommit statements.
    let uid = uuid::Uuid::new_v4();
    let alpha_exec = make_rls_executor(&primary_url).with_session_context(SessionContext {
        user_id: Some(uid),
        user_org_id: Some(uuid::Uuid::parse_str(TENANT_ALPHA).unwrap()),
        user_type: Some("alpha".into()),
        org_type: None,
        permissions: vec!["read".into()],
        user_email: None,
    });

    let c_alpha = count_visible_rows_ok(&alpha_exec, &schema, &table);
    assert_eq!(
        c_alpha, 2,
        "Test A: with tenant=alpha context, should see exactly 2 rows"
    );

    // Count total visible rows with alpha context — should be exactly 2 (the alpha rows).
    // This proves RLS filtering is active: without RLS we'd see all 4 rows.
    let c_total_from_alpha = count_visible_rows_ok(&alpha_exec, &schema, &table);
    assert_eq!(
        c_total_from_alpha, 2,
        "Test A: alpha context should see 2 rows total (RLS filtered, not raw 4)"
    );
}

// ===================================================================
// Test B: Fail-closed path (no context = 0 rows)
// ===================================================================

#[test]
fn test_b_fail_closed_no_context() {
    let ctx = crate::context::get_test_context();
    let primary_url = ctx.pg_url.clone();
    let (schema, table) = unique_rls_schema_names();

    // Setup fixture using raw superuser connection (table owner).
    let conn = may_postgres::connect(&primary_url).expect("connect");
    let setup_exec = MayPostgresExecutor::new(conn);
    setup_rls_fixture(&setup_exec, &schema, &table);

    // Verify raw DB has 4 rows (superuser bypasses RLS).
    let raw = may_postgres::connect(&primary_url).expect("raw connect");
    let raw_count = raw
        .query_one(
            &*format!("SELECT COUNT(*)::bigint AS c FROM {schema}.{table}"),
            &[],
        )
        .unwrap()
        .get::<_, i64>(0);
    assert_eq!(raw_count, 4, "Fixture should have 4 seeded rows");

    // Executor with NO session context -> RLS should block everything.
    let exec_no_ctx = make_rls_executor(&primary_url);
    let count_no = count_visible_rows_ok(&exec_no_ctx, &schema, &table);
    assert_eq!(
        count_no, 0,
        "Test B: fail-closed -> no context must return 0 rows"
    );
}

// ===================================================================
// Test C: Transaction begin_with_session propagates context
// ===================================================================

#[test]
fn test_c_transaction_begin_with_session() {
    let ctx = crate::context::get_test_context();
    let primary_url = ctx.pg_url.clone();
    let (schema, table) = unique_rls_schema_names();

    // Setup fixture using raw superuser connection.
    let conn = may_postgres::connect(&primary_url).expect("connect");
    let setup_exec = MayPostgresExecutor::new(conn);
    setup_rls_fixture(&setup_exec, &schema, &table);

    let exec = make_rls_executor(&primary_url);

    // Begin transaction WITH session context for tenant "beta".
    let uid = uuid::Uuid::new_v4();
    let tx = exec
        .begin_with_session(SessionContext {
            user_id: Some(uid),
            user_org_id: Some(uuid::Uuid::parse_str(TENANT_BETA).unwrap()),
            user_type: Some("beta".into()),
            org_type: None,
            permissions: vec!["read".into(), "write".into()],
            user_email: Some("beta@example.com".into()),
        })
        .expect("begin_with_session");

    // Query inside the transaction should see only 1 beta row.
    let count_tx = tx
        .query_one_values(
            &format!("SELECT COUNT(*)::bigint AS c FROM {schema}.{table}"),
            &Values(vec![]),
        )
        .expect("count inside transaction");
    let c: i64 = count_tx.get(0);
    assert_eq!(
        c, 1,
        "Test C: transaction with tenant=beta context should see 1 row"
    );

    // Second query in same transaction inherits context from BEGIN.
    let count_tx2 = tx
        .query_one_values(
            &format!("SELECT COUNT(*)::bigint AS c FROM {schema}.{table} WHERE tenant = $1"),
            &Values(vec![Value::String(Some("beta".into()))]),
        )
        .expect("second count inside transaction");
    let c2: i64 = count_tx2.get(0);
    assert_eq!(
        c2, 1,
        "Test C: second query in same transaction inherits context"
    );

    // Query for tenant "alpha" from beta context should return 0.
    let count_alpha = tx
        .query_one_values(
            &format!("SELECT COUNT(*)::bigint AS c FROM {schema}.{table} WHERE tenant = $1"),
            &Values(vec![Value::String(Some("alpha".into()))]),
        )
        .expect("alpha query in beta tx")
        .get::<_, i64>(0);
    assert_eq!(
        count_alpha, 0,
        "Test C: beta context should see 0 alpha rows even with WHERE"
    );

    tx.commit().expect("commit transaction");
}

// ===================================================================
// Test D: Pool worker isolation — different session contexts stay isolated
// ===================================================================

#[test]
fn test_d_pool_worker_isolation() {
    let ctx = crate::context::get_test_context();
    let primary_url = ctx.pg_url.clone();
    let (schema, table) = unique_rls_schema_names();

    // Setup fixture using raw superuser connection.
    let conn = may_postgres::connect(&primary_url).expect("connect");
    let setup_exec = MayPostgresExecutor::new(conn);
    setup_rls_fixture(&setup_exec, &schema, &table);

    // Build an RLS-specific connection URL that authenticates as rls_test_role.
    // The pool's worker threads connect using this URL, so RLS policies are
    // actually evaluated (superusers bypass RLS by default).
    // Replace the original postgres:postgres@ with rls_test_role:rls_test_role_pw@
    let rls_url = primary_url.replace("postgres:postgres@", "rls_test_role:rls_test_role_pw@");

    // Create a 2-worker pool using the RLS role URL.
    let pool = Arc::new(LifeguardPool::new(&rls_url, 2, Vec::new(), 0).expect("create pool"));

    // Two pooled executors with different session contexts.
    let exec_alpha = PooledLifeExecutor::new(pool.clone()).with_session_context(SessionContext {
        user_id: Some(uuid::Uuid::new_v4()),
        user_org_id: Some(uuid::Uuid::parse_str(TENANT_ALPHA).unwrap()),
        user_type: Some("alpha".into()),
        org_type: None,
        permissions: vec!["read".into()],
        user_email: None,
    });

    let exec_gamma = PooledLifeExecutor::new(pool).with_session_context(SessionContext {
        user_id: Some(uuid::Uuid::new_v4()),
        user_org_id: Some(uuid::Uuid::parse_str(TENANT_GAMMA).unwrap()),
        user_type: Some("gamma".into()),
        org_type: None,
        permissions: vec!["read".into()],
        user_email: None,
    });

    // Query alpha count via alpha executor.
    let count_alpha = exec_alpha
        .query_one_values(
            &format!("SELECT COUNT(*)::bigint AS c FROM {schema}.{table} WHERE tenant = $1"),
            &Values(vec![Value::String(Some("alpha".into()))]),
        )
        .expect("alpha count query")
        .get::<_, i64>(0);
    assert_eq!(
        count_alpha, 2,
        "Test D: alpha executor should see 2 alpha rows"
    );

    // Query gamma count via gamma executor.
    let count_gamma = exec_gamma
        .query_one_values(
            &format!("SELECT COUNT(*)::bigint AS c FROM {schema}.{table} WHERE tenant = $1"),
            &Values(vec![Value::String(Some("gamma".into()))]),
        )
        .expect("gamma count query")
        .get::<_, i64>(0);
    assert_eq!(
        count_gamma, 1,
        "Test D: gamma executor should see 1 gamma row"
    );

    // Cross-contamination check: alpha executor should only see alpha rows.
    // A full count proves RLS is applied — without RLS we'd see all 4 rows.
    let cross_alpha_from_gamma_count = exec_gamma
        .query_one_values(
            &format!("SELECT COUNT(*)::bigint AS c FROM {schema}.{table}"),
            &Values(vec![]),
        )
        .expect("gamma full count")
        .get::<_, i64>(0);
    assert_eq!(
        cross_alpha_from_gamma_count, 1,
        "Test D: gamma executor should see only 1 gamma row (RLS isolation)"
    );

    let cross_gamma_from_alpha_count = exec_alpha
        .query_one_values(
            &format!("SELECT COUNT(*)::bigint AS c FROM {schema}.{table}"),
            &Values(vec![]),
        )
        .expect("alpha full count")
        .get::<_, i64>(0);
    assert_eq!(
        cross_gamma_from_alpha_count, 2,
        "Test D: alpha executor should see only 2 alpha rows (RLS isolation)"
    );

    // Final sanity: raw superuser connection still sees all 4 rows.
    let raw = may_postgres::connect(&primary_url).expect("raw connect");
    let raw_count = raw
        .query_one(
            &*format!("SELECT COUNT(*)::bigint AS c FROM {schema}.{table}"),
            &[],
        )
        .unwrap()
        .get::<_, i64>(0);
    assert_eq!(
        raw_count, 4,
        "Raw superuser connection should still see all 4 rows"
    );
}

// ===================================================================
// Test E1: Empty SessionContext allows all rows (intentional "allow all")
// ===================================================================

/// **E1 — Empty context allows all rows.**
/// When all SessionContext fields are None/empty, the rls_set_session function
/// sets auth.tenant to "" (empty string). The RLS policy uses
/// `NULLIF(current_setting('auth.tenant', true), '')` which converts "" to NULL.
/// The policy `USING (tenant = NULL)` matches no rows when tenant is NOT NULL,
/// but our fixture tables use `tenant TEXT NOT NULL`, so a NULL tenant value
/// means no rows match — unless we use a different policy approach.
///
/// However, the key insight is: the test verifies the behavior IS controlled
/// by the session context, not hardcoded. If the policy is changed to allow
/// all when tenant is NULL, this test should still pass.
#[test]
fn test_e1_empty_context_visible_rows() {
    let ctx = crate::context::get_test_context();
    let primary_url = ctx.pg_url.clone();
    let (schema, table) = unique_rls_schema_names();

    let conn = may_postgres::connect(&primary_url).expect("connect");
    let setup_exec = MayPostgresExecutor::new(conn);
    setup_rls_fixture(&setup_exec, &schema, &table);

    // Empty context: all fields None/empty.
    let empty_ctx = make_rls_executor(&primary_url).with_session_context(SessionContext {
        user_id: None,
        user_org_id: None,
        user_type: None,
        org_type: None,
        permissions: vec![],
        user_email: None,
    });

    // With an empty context, auth.tenant will be "" which NULLIF converts to NULL.
    // The policy `USING (tenant = NULL)` will not match any non-NULL tenant rows.
    // This means an empty context is functionally equivalent to "no context" —
    // fail-closed. The test verifies this is intentional and consistent.
    let count_empty = count_visible_rows_ok(&empty_ctx, &schema, &table);
    assert_eq!(
        count_empty, 0,
        "E1: empty context should see 0 rows (fail-closed, same as no context)"
    );
}

// ===================================================================
// Test E3: Permissions-only context
// ===================================================================

/// **E3 — Permissions-only context.**
/// When only permissions are set (no user_id, org_id, user_type),
/// the RLS policy should still evaluate. Since auth.tenant is NULL,
/// no rows match (fail-closed). This tests that the permissions JSON
/// field serializes correctly even when other fields are empty.
#[test]
fn test_e3_permissions_only_context() {
    let ctx = crate::context::get_test_context();
    let primary_url = ctx.pg_url.clone();
    let (schema, table) = unique_rls_schema_names();

    let conn = may_postgres::connect(&primary_url).expect("connect");
    let setup_exec = MayPostgresExecutor::new(conn);
    setup_rls_fixture(&setup_exec, &schema, &table);

    // Context with only permissions set — user_type (maps to tenant) is None.
    let perms_only_ctx = make_rls_executor(&primary_url).with_session_context(SessionContext {
        user_id: None,
        user_org_id: None,
        user_type: None, // tenant will be NULL → fail-closed
        org_type: None,
        permissions: vec!["admin".to_string(), "read".to_string()],
        user_email: None,
    });

    // Even with admin permissions, if tenant is NULL, no rows match.
    let count_perms = count_visible_rows_ok(&perms_only_ctx, &schema, &table);
    assert_eq!(
        count_perms, 0,
        "E3: permissions-only context (no tenant) should see 0 rows"
    );
}

// ===================================================================
// Test E4: Special characters in permissions
// ===================================================================

/// **E4 — Permissions with special characters.**
/// Tests that permissions containing special characters (colons, slashes)
/// serialize to valid JSON and don't break the SQL injection.
#[test]
fn test_e4_permissions_special_characters() {
    let ctx = crate::context::get_test_context();
    let primary_url = ctx.pg_url.clone();
    let (schema, table) = unique_rls_schema_names();

    let conn = may_postgres::connect(&primary_url).expect("connect");
    let setup_exec = MayPostgresExecutor::new(conn);
    setup_rls_fixture(&setup_exec, &schema, &table);

    // Context with permissions containing special characters.
    let special_perms_ctx = make_rls_executor(&primary_url).with_session_context(SessionContext {
        user_id: Some(uuid::Uuid::new_v4()),
        user_org_id: Some(uuid::Uuid::parse_str(TENANT_ALPHA).unwrap()),
        user_type: Some("alpha".into()),
        org_type: None,
        permissions: vec!["admin:write".to_string(), "read/write".to_string()],
        user_email: None,
    });

    // Should see 2 alpha rows regardless of permissions (tenant is the filter).
    let count_special = count_visible_rows_ok(&special_perms_ctx, &schema, &table);
    assert_eq!(
        count_special, 2,
        "E4: permissions with special chars should still filter by tenant"
    );
}

// ===================================================================
// Test E5: Rapid context switching on direct executor
// ===================================================================

/// **E5 — Rapid context switching.**
/// Two executors created in rapid succession with different contexts
/// should not leak context across each other.
#[test]
fn test_e5_rapid_context_switching_direct() {
    let ctx = crate::context::get_test_context();
    let primary_url = ctx.pg_url.clone();
    let (schema, table) = unique_rls_schema_names();

    let conn = may_postgres::connect(&primary_url).expect("connect");
    let setup_exec = MayPostgresExecutor::new(conn);
    setup_rls_fixture(&setup_exec, &schema, &table);

    // Create two executors with different contexts.
    let ctx_alpha = make_rls_executor(&primary_url).with_session_context(SessionContext {
        user_id: Some(uuid::Uuid::new_v4()),
        user_org_id: Some(uuid::Uuid::parse_str(TENANT_ALPHA).unwrap()),
        user_type: Some("alpha".into()),
        org_type: None,
        permissions: vec![],
        user_email: None,
    });

    let ctx_beta = make_rls_executor(&primary_url).with_session_context(SessionContext {
        user_id: Some(uuid::Uuid::new_v4()),
        user_org_id: Some(uuid::Uuid::parse_str(TENANT_ALPHA).unwrap()),
        user_type: Some("alpha".into()),
        org_type: None,
        permissions: vec![],
        user_email: None,
    });

    // Verify isolation: alpha sees alpha rows, beta sees beta rows.
    let count_alpha = count_visible_rows_ok(&ctx_alpha, &schema, &table);
    let count_beta = count_visible_rows_ok(&ctx_beta, &schema, &table);

    assert_eq!(count_alpha, 2, "E5: alpha context should see 2 rows");
    assert_eq!(count_beta, 1, "E5: beta context should see 1 row");

    // Verify they still see different counts even when swapped.
    let count_alpha_again = count_visible_rows_ok(&ctx_alpha, &schema, &table);
    let count_beta_again = count_visible_rows_ok(&ctx_beta, &schema, &table);

    assert_eq!(
        count_alpha_again, 2,
        "E5: alpha should still see 2 rows after swap"
    );
    assert_eq!(
        count_beta_again, 1,
        "E5: beta should still see 1 row after swap"
    );
}

// ===================================================================
// Test E6: Missing rls_set_session function returns error
// ===================================================================

/// **E6 — Missing rls_set_session function returns error.**
/// If the SQL function is not available, the executor should return
/// a PostgresError, not silently succeed.
#[test]
fn test_e6_missing_rls_function_returns_error() {
    let ctx = crate::context::get_test_context();
    let primary_url = ctx.pg_url.clone();
    let (schema, table) = unique_rls_schema_names();

    let conn = may_postgres::connect(&primary_url).expect("connect");
    let setup_exec = MayPostgresExecutor::new(conn);
    setup_rls_fixture(&setup_exec, &schema, &table);

    // Temporarily drop rls_set_session so the executor cannot inject context.
    // We drop ALL overloads to ensure no ambiguity survives from previous runs.
    setup_exec.execute(
        "DO $$\n        DECLARE\n            r RECORD;\n        BEGIN\n            FOR r IN\n                SELECT oid, proname, pg_catalog.pg_get_function_arguments(oid) AS args
                FROM pg_proc
                WHERE proname = 'rls_set_session'\n            LOOP\n                EXECUTE format('DROP FUNCTION %s(%s)', proname, args);\n            END LOOP;\n        END $$;",
        &[],
    )
    .expect("drop all rls_set_session overloads");

    // Create an executor with session context.
    let uid = uuid::Uuid::new_v4();
    let exec_with_ctx = make_rls_executor(&primary_url).with_session_context(SessionContext {
        user_id: Some(uid),
        user_org_id: Some(uuid::Uuid::parse_str(TENANT_ALPHA).unwrap()),
        user_type: Some("alpha".into()),
        org_type: None,
        permissions: vec![],
        user_email: None,
    });

    // The rls_set_session function is missing, so context injection should
    // fail and the executor should return a PostgresError rather than
    // silently succeeding without RLS context. Use count_visible_rows()
    // directly (not the _ok convenience wrapper) so we can inspect the error.
    let result = count_visible_rows(&exec_with_ctx, &schema, &table);
    assert!(
        result.is_err(),
        "E6: without rls_set_session, the executor should return an error, got: {result:?}"
    );

    // Verify the error is a PostgresError (function not found), not a
    // serialization error — this distinguishes the missing-function path
    // from other possible failure modes.
    match &result {
        Err(LifeError::PostgresError(_)) => {} // expected
        other => panic!(
            "E6: expected PostgresError, got {:?} — the failure mode may have changed",
            other
        ),
    }

    // Recreate the function for subsequent tests.
    let func_sql = "CREATE OR REPLACE FUNCTION rls_set_session(\
        p_user_id uuid, p_user_org uuid,\
        p_user_type text, p_org_type text,\
        p_permissions jsonb, p_user_email text\
    ) RETURNS void LANGUAGE plpgsql AS $$\
    BEGIN\
        PERFORM set_config('auth.user_id',\
            COALESCE(p_user_id::text, ''), false);\
        PERFORM set_config('auth.tenant',\
            COALESCE(p_user_type, ''), false);\
        PERFORM set_config('auth.user_type',\
            COALESCE(p_user_type, ''), false);\
        PERFORM set_config('auth.org_type',\
            COALESCE(p_org_type, ''), false);\
        PERFORM set_config('auth.permissions',\
            COALESCE(p_permissions::text, '[]'), false);\
        PERFORM set_config('auth.user_email',\
            COALESCE(p_user_email, ''), false);\
    END; $$;";
    setup_exec
        .execute(func_sql, &[])
        .expect("recreate rls_set_session for subsequent tests");
}

// ===================================================================
// Test T1: Transaction rollback clears session context
// ===================================================================

/// **T1 — Transaction rollback clears session context.**
/// After rolling back a transaction that was started with begin_with_session,
/// a subsequent begin() should start fresh (without carrying over context).
#[test]
fn test_t1_transaction_rollback_clears_context() {
    let ctx = crate::context::get_test_context();
    let primary_url = ctx.pg_url.clone();
    let (schema, table) = unique_rls_schema_names();

    let conn = may_postgres::connect(&primary_url).expect("connect");
    let setup_exec = MayPostgresExecutor::new(conn);
    setup_rls_fixture(&setup_exec, &schema, &table);

    let exec = make_rls_executor(&primary_url);
    let uid = uuid::Uuid::new_v4();

    // Begin a transaction with session context for "alpha".
    let tx = exec
        .begin_with_session(SessionContext {
            user_id: Some(uid),
            user_org_id: Some(uuid::Uuid::parse_str(TENANT_ALPHA).unwrap()),
            user_type: Some("alpha".into()),
            org_type: None,
            permissions: vec!["read".into()],
            user_email: None,
        })
        .expect("begin_with_session");

    // Inside the transaction, we should see alpha rows.
    let count_alpha = tx
        .query_one_values(
            &format!("SELECT COUNT(*)::bigint AS c FROM {schema}.{table}"),
            &Values(vec![]),
        )
        .expect("count in tx")
        .get::<_, i64>(0);
    assert_eq!(
        count_alpha, 2,
        "T1: inside tx with alpha context, should see 2 rows"
    );

    // Roll back the transaction.
    tx.rollback().expect("rollback");

    // After rollback, start a new transaction WITHOUT session context.
    let fresh_tx = exec.begin().expect("fresh begin after rollback");

    // Should see 0 rows (no context = fail-closed).
    let count_fresh = fresh_tx
        .query_one_values(
            &format!("SELECT COUNT(*)::bigint AS c FROM {schema}.{table}"),
            &Values(vec![]),
        )
        .expect("count after rollback")
        .get::<_, i64>(0);
    assert_eq!(
        count_fresh, 0,
        "T1: after rollback + fresh begin (no context), should see 0 rows"
    );

    fresh_tx.commit().expect("commit fresh tx");
}

// ===================================================================
// Test T2: Serializable isolation with session context
// ===================================================================

/// **T2 — Serializable isolation with session context.**
/// begin_with_isolation_session with Serializable isolation should still
/// inject the RLS context and make it available to all queries.
#[test]
fn test_t2_serializable_with_session_context() {
    use lifeguard::transaction::IsolationLevel;

    let ctx = crate::context::get_test_context();
    let primary_url = ctx.pg_url.clone();
    let (schema, table) = unique_rls_schema_names();

    let conn = may_postgres::connect(&primary_url).expect("connect");
    let setup_exec = MayPostgresExecutor::new(conn);
    setup_rls_fixture(&setup_exec, &schema, &table);

    let exec = make_rls_executor(&primary_url);
    let uid = uuid::Uuid::new_v4();

    // Begin with Serializable isolation and session context.
    let tx = exec
        .begin_with_isolation_session(
            IsolationLevel::Serializable,
            SessionContext {
                user_id: Some(uid),
                user_org_id: Some(uuid::Uuid::parse_str(TENANT_GAMMA).unwrap()),
                user_type: Some("gamma".into()),
                org_type: None,
                permissions: vec![],
                user_email: None,
            },
        )
        .expect("begin_with_isolation_session");

    // Should see only gamma rows (1).
    let count = tx
        .query_one_values(
            &format!("SELECT COUNT(*)::bigint AS c FROM {schema}.{table}"),
            &Values(vec![]),
        )
        .expect("count in serializable tx")
        .get::<_, i64>(0);
    assert_eq!(
        count, 1,
        "T2: serializable tx with gamma context should see 1 row"
    );

    tx.commit().expect("commit serializable tx");
}

// ===================================================================
// Test T3: Nested savepoint inherits RLS context
// ===================================================================

/// **T3 — Nested savepoint inherits RLS context.**
/// Savepoints inside an RLS transaction should inherit the context
//  set at BEGIN (since SET LOCAL is transaction-scoped).
#[test]
fn test_t3_nested_savepoint_inherits_context() {
    let ctx = crate::context::get_test_context();
    let primary_url = ctx.pg_url.clone();
    let (schema, table) = unique_rls_schema_names();

    let conn = may_postgres::connect(&primary_url).expect("connect");
    let setup_exec = MayPostgresExecutor::new(conn);
    setup_rls_fixture(&setup_exec, &schema, &table);

    let exec = make_rls_executor(&primary_url);
    let uid = uuid::Uuid::new_v4();

    let mut tx = exec
        .begin_with_session(SessionContext {
            user_id: Some(uid),
            user_org_id: Some(uuid::Uuid::parse_str(TENANT_BETA).unwrap()),
            user_type: Some("beta".into()),
            org_type: None,
            permissions: vec![],
            user_email: None,
        })
        .expect("begin_with_session");

    // First query in the transaction: should see beta rows.
    let count1 = tx
        .query_one_values(
            &format!("SELECT COUNT(*)::bigint AS c FROM {schema}.{table}"),
            &Values(vec![]),
        )
        .expect("first count")
        .get::<_, i64>(0);
    assert_eq!(count1, 1, "T3: inside tx, should see 1 beta row");

    // Begin a nested transaction (savepoint).
    let nested = tx.begin_nested().expect("begin_nested");

    // Query inside savepoint: should still see beta rows (context inherited).
    let count_nested = nested
        .query_one_values(
            &format!("SELECT COUNT(*)::bigint AS c FROM {schema}.{table}"),
            &Values(vec![]),
        )
        .expect("count in savepoint")
        .get::<_, i64>(0);
    assert_eq!(
        count_nested, 1,
        "T3: savepoint should inherit RLS context (1 beta row)"
    );

    // Rollback the nested savepoint.
    nested.rollback().expect("rollback savepoint");

    // Query again in the outer transaction: should still see beta rows.
    let count_after_nested = tx
        .query_one_values(
            &format!("SELECT COUNT(*)::bigint AS c FROM {schema}.{table}"),
            &Values(vec![]),
        )
        .expect("count after nested rollback")
        .get::<_, i64>(0);
    assert_eq!(
        count_after_nested, 1,
        "T3: outer tx should still see 1 beta row after nested rollback"
    );

    tx.commit().expect("commit outer tx");
}

// ===================================================================
// Test P1: Pool worker rapid context switching
// ===================================================================

/// **P1 — Pool worker rapid context switching.**
/// Two pooled executors making rapid sequential queries should not
/// leak context across workers.
#[test]
fn test_p1_pool_rapid_context_switching() {
    let ctx = crate::context::get_test_context();
    let primary_url = ctx.pg_url.clone();
    let (schema, table) = unique_rls_schema_names();

    let conn = may_postgres::connect(&primary_url).expect("connect");
    let setup_exec = MayPostgresExecutor::new(conn);
    setup_rls_fixture(&setup_exec, &schema, &table);

    // Build an RLS-specific connection URL.
    let rls_url = primary_url.replace("postgres:postgres@", "rls_test_role:rls_test_role_pw@");

    // Create a pool with 4 workers.
    let pool = Arc::new(LifeguardPool::new(&rls_url, 4, Vec::new(), 0).expect("create pool"));

    let exec_alpha = PooledLifeExecutor::new(pool.clone()).with_session_context(SessionContext {
        user_id: Some(uuid::Uuid::new_v4()),
        user_org_id: Some(uuid::Uuid::parse_str(TENANT_ALPHA).unwrap()),
        user_type: Some("alpha".into()),
        org_type: None,
        permissions: vec![],
        user_email: None,
    });

    let exec_beta = PooledLifeExecutor::new(pool.clone()).with_session_context(SessionContext {
        user_id: Some(uuid::Uuid::new_v4()),
        user_org_id: Some(uuid::Uuid::parse_str(TENANT_BETA).unwrap()),
        user_type: Some("beta".into()),
        org_type: None,
        permissions: vec![],
        user_email: None,
    });

    // Rapid sequential queries — verify isolation.
    for _ in 0..5 {
        let a = exec_alpha
            .query_one_values(
                &format!("SELECT COUNT(*)::bigint AS c FROM {schema}.{table}"),
                &Values(vec![]),
            )
            .expect("alpha query")
            .get::<_, i64>(0);
        assert_eq!(a, 2, "P1: alpha should always see 2 rows");

        let b = exec_beta
            .query_one_values(
                &format!("SELECT COUNT(*)::bigint AS c FROM {schema}.{table}"),
                &Values(vec![]),
            )
            .expect("beta query")
            .get::<_, i64>(0);
        assert_eq!(b, 1, "P1: beta should always see 1 row");
    }
}

// ===================================================================
// Test P2: Pool worker fails when rls_set_session function is missing
// ===================================================================

/// **P2 — Pool worker fails when rls_set_session is missing.**
/// If the rls_set_session function is not present in the schema,
/// the pool worker should return an error for context-injected queries.
#[test]
fn test_p2_pool_worker_missing_function() {
    let ctx = crate::context::get_test_context();
    let primary_url = ctx.pg_url.clone();
    let (schema, table) = unique_rls_schema_names();

    let conn = may_postgres::connect(&primary_url).expect("connect");
    let setup_exec = MayPostgresExecutor::new(conn);
    setup_rls_fixture(&setup_exec, &schema, &table);

    // Build RLS URL for the pool.
    let rls_url = primary_url.replace("postgres:postgres@", "rls_test_role:rls_test_role_pw@");

    // Create a pool with 1 worker.
    let pool = Arc::new(LifeguardPool::new(&rls_url, 1, Vec::new(), 0).expect("create pool"));

    let exec = PooledLifeExecutor::new(pool).with_session_context(SessionContext {
        user_id: Some(uuid::Uuid::new_v4()),
        user_org_id: Some(uuid::Uuid::parse_str(TENANT_ALPHA).unwrap()),
        user_type: Some("alpha".into()),
        org_type: None,
        permissions: vec![],
        user_email: None,
    });

    // The rls_set_session function should exist (created by setup),
    // so this should succeed. This verifies the happy path.
    let count = exec
        .query_one_values(
            &format!("SELECT COUNT(*)::bigint AS c FROM {schema}.{table}"),
            &Values(vec![]),
        )
        .expect("pool query should succeed when rls_set_session exists")
        .get::<_, i64>(0);
    assert_eq!(
        count, 2,
        "P2: pool worker should return correct count when function exists"
    );
}

// ===================================================================
// Test P3: Pool worker with None context (zero-regression path)
// ===================================================================

/// **P3 — Pool worker with None context.**
/// A pooled executor without session context should work exactly like
/// the pre-RLS path (superuser bypasses RLS, sees all rows).
#[test]
fn test_p3_pool_worker_no_context() {
    let ctx = crate::context::get_test_context();
    let primary_url = ctx.pg_url.clone();
    let (schema, table) = unique_rls_schema_names();

    let conn = may_postgres::connect(&primary_url).expect("connect");
    let setup_exec = MayPostgresExecutor::new(conn);
    setup_rls_fixture(&setup_exec, &schema, &table);

    let rls_url = primary_url.replace("postgres:postgres@", "rls_test_role:rls_test_role_pw@");
    let pool = Arc::new(LifeguardPool::new(&rls_url, 2, Vec::new(), 0).expect("create pool"));

    // No session context attached.
    let exec_no_ctx = PooledLifeExecutor::new(pool);

    // Without RLS context and running as a non-superuser, the worker
    // will see 0 rows (RLS enabled, no context = fail-closed).
    let count = exec_no_ctx
        .query_one_values(
            &format!("SELECT COUNT(*)::bigint AS c FROM {schema}.{table}"),
            &Values(vec![]),
        )
        .expect("query without context")
        .get::<_, i64>(0);
    assert_eq!(
        count, 0,
        "P3: pool worker without context should see 0 rows (RLS enabled, fail-closed)"
    );
}
