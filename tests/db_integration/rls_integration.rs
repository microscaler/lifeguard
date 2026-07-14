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
const PLATFORM_TENANT: &str = "hauliage";
const RLS_SET_SESSION_SQL: &str = "CREATE OR REPLACE FUNCTION public.rls_set_session(
    p_tenant_id text, p_subject_id uuid, p_organization_id uuid,
    p_session_id text, p_roles jsonb, p_permissions jsonb,
    p_user_type text, p_org_type text
) RETURNS void LANGUAGE plpgsql AS $$
BEGIN
    PERFORM set_config('sesame.tenant_id', p_tenant_id, true);
    PERFORM set_config('sesame.subject_id', p_subject_id::text, true);
    PERFORM set_config('sesame.organization_id', p_organization_id::text, true);
    PERFORM set_config('sesame.session_id', p_session_id, true);
    PERFORM set_config('sesame.roles', p_roles::text, true);
    PERFORM set_config('sesame.permissions', p_permissions::text, true);
    PERFORM set_config('sesame.user_type', COALESCE(p_user_type, ''), true);
    PERFORM set_config('sesame.org_type', COALESCE(p_org_type, ''), true);
END; $$;";
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

    // Nextest may start the test binary concurrently to enumerate normal and
    // ignored tests. Serialize the shared role/function DDL across those
    // processes; PostgreSQL can otherwise race two CREATE OR REPLACE calls.
    exec.execute("SELECT pg_advisory_lock(7211915000)", &[])
        .expect("ctor: acquire RLS setup lock");

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
                SELECT p.oid, n.nspname, p.proname,
                       pg_catalog.pg_get_function_arguments(p.oid) AS args
                FROM pg_proc AS p
                JOIN pg_namespace AS n ON n.oid = p.pronamespace
                WHERE p.proname = 'rls_set_session'
            LOOP
                EXECUTE format('DROP FUNCTION %I.%I(%s)',
                    r.nspname, r.proname, r.args);
            END LOOP;
        END $$;",
        &[],
    )
    .ok();

    // Create the app-owned rls_set_session function in public (idempotent).
    // p_permissions is `jsonb` because to_sql_args() serializes permissions
    // as serde_json::Value.
    exec.execute(RLS_SET_SESSION_SQL, &[])
        .expect("ctor: CREATE rls_set_session");

    // Allow rls_test_role to call the function
    exec.execute(
        "GRANT EXECUTE ON FUNCTION public.rls_set_session TO rls_test_role",
        &[],
    )
    .ok();

    // rls_test_role needs USAGE on public schema to resolve function calls
    exec.execute("GRANT USAGE ON SCHEMA public TO rls_test_role", &[])
        .ok();

    exec.execute("SELECT pg_advisory_unlock(7211915000)", &[])
        .expect("ctor: release RLS setup lock");
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

fn session_context(
    organization_id: &str,
    classification: &str,
    permissions: Vec<String>,
) -> SessionContext {
    SessionContext {
        tenant_id: PLATFORM_TENANT.to_string(),
        subject_id: uuid::Uuid::new_v4(),
        organization_id: uuid::Uuid::parse_str(organization_id).expect("organization UUID"),
        session_id: format!("rls-test-{}", uuid::Uuid::new_v4()),
        roles: vec![classification.to_string()],
        permissions,
        user_type: Some(classification.to_string()),
        org_type: Some("tenant".to_string()),
    }
}

/// DDL that every RLS test needs:
/// 1. Unique schema + table
/// 2. Grants for rls_test_role (USAGE + SELECT)
/// 3. ENABLE ROW LEVEL SECURITY
/// 4. RLS policy using the canonical Sesame organization GUC
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

    // Policy: only rows where tenant matches the active Sesame organization.
    executor
        .execute(
            &format!(
                "CREATE POLICY rls_tenant_filter ON {schema}.{table} \
                USING (tenant = NULLIF(current_setting('sesame.organization_id', true), ''))"
            ),
            &[],
        )
        .expect("CREATE RLS policy");

    // Seed data: 4 rows across 3 tenants.
    executor
        .execute(
            &format!(
                "INSERT INTO {schema}.{table} (tenant, note) VALUES \
                ('{TENANT_ALPHA}', 'alpha-item-a'), \
                ('{TENANT_ALPHA}', 'alpha-item-b'), \
                ('{TENANT_BETA}', 'beta-item'), \
                ('{TENANT_GAMMA}', 'gamma-item')"
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
    // The executor wraps context injection and this application query in one
    // transaction, allowing the helper to use transaction-local GUCs.
    let alpha_exec = make_rls_executor(&primary_url).with_session_context(session_context(
        TENANT_ALPHA,
        "alpha",
        vec!["read".into()],
    ));

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
// Test A2: Direct one-shot context does not leak after commit
// ===================================================================

#[test]
fn test_a2_direct_context_clears_after_one_shot_commit() {
    let ctx = crate::context::get_test_context();
    let primary_url = ctx.pg_url.clone();
    let (schema, table) = unique_rls_schema_names();

    let setup = make_superuser_executor(&primary_url);
    setup_rls_fixture(&setup, &schema, &table);

    // Client clones share one underlying connection. Use a contextual executor
    // first, then query through a context-free handle on that exact connection.
    let plain_exec = make_rls_executor(&primary_url);
    let alpha_exec = MayPostgresExecutor::new(plain_exec.client().clone())
        .with_session_context(session_context(TENANT_ALPHA, "alpha", vec!["read".into()]));

    assert_eq!(
        count_visible_rows_ok(&alpha_exec, &schema, &table),
        2,
        "A2: contextual operation should see alpha rows"
    );
    assert_eq!(
        count_visible_rows_ok(&plain_exec, &schema, &table),
        0,
        "A2: context must be cleared after the one-shot transaction commits"
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
    let tx = exec
        .begin_with_session(session_context(
            TENANT_BETA,
            "beta",
            vec!["read".into(), "write".into()],
        ))
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
            &Values(vec![Value::String(Some(TENANT_BETA.to_string()))]),
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
            &Values(vec![Value::String(Some(TENANT_ALPHA.to_string()))]),
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
    let exec_alpha = PooledLifeExecutor::new(pool.clone()).with_session_context(session_context(
        TENANT_ALPHA,
        "alpha",
        vec!["read".into()],
    ));

    let exec_gamma = PooledLifeExecutor::new(pool).with_session_context(session_context(
        TENANT_GAMMA,
        "gamma",
        vec!["read".into()],
    ));

    // Query alpha count via alpha executor.
    let count_alpha = exec_alpha
        .query_one_values(
            &format!("SELECT COUNT(*)::bigint AS c FROM {schema}.{table} WHERE tenant = $1"),
            &Values(vec![Value::String(Some(TENANT_ALPHA.to_string()))]),
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
            &Values(vec![Value::String(Some(TENANT_GAMMA.to_string()))]),
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
// Test E1: Empty roles and permissions retain tenant scoping
// ===================================================================

/// Roles and permissions may legitimately be empty, but the hard identity and
/// active-organization boundary remains mandatory and scoped.
#[test]
fn test_e1_empty_context_visible_rows() {
    let ctx = crate::context::get_test_context();
    let primary_url = ctx.pg_url.clone();
    let (schema, table) = unique_rls_schema_names();

    let conn = may_postgres::connect(&primary_url).expect("connect");
    let setup_exec = MayPostgresExecutor::new(conn);
    setup_rls_fixture(&setup_exec, &schema, &table);

    let mut context = session_context(TENANT_ALPHA, "member", vec![]);
    context.roles.clear();
    let empty_ctx = make_rls_executor(&primary_url).with_session_context(context);

    let count_empty = count_visible_rows_ok(&empty_ctx, &schema, &table);
    assert_eq!(
        count_empty, 2,
        "E1: empty authorization lists must retain active-organization scoping"
    );
}

// ===================================================================
// Test E3: Optional classifications may be absent
// ===================================================================

/// The tenant, subject, organization, and session boundary is always present.
/// Optional subject and organization classifications do not affect isolation.
#[test]
fn test_e3_permissions_only_context() {
    let ctx = crate::context::get_test_context();
    let primary_url = ctx.pg_url.clone();
    let (schema, table) = unique_rls_schema_names();

    let conn = may_postgres::connect(&primary_url).expect("connect");
    let setup_exec = MayPostgresExecutor::new(conn);
    setup_rls_fixture(&setup_exec, &schema, &table);

    let mut context = session_context(
        TENANT_BETA,
        "member",
        vec!["admin".to_string(), "read".to_string()],
    );
    context.user_type = None;
    context.org_type = None;
    let perms_only_ctx = make_rls_executor(&primary_url).with_session_context(context);

    let count_perms = count_visible_rows_ok(&perms_only_ctx, &schema, &table);
    assert_eq!(
        count_perms, 1,
        "E3: absent optional classifications must retain active-organization scoping"
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
    let special_perms_ctx = make_rls_executor(&primary_url).with_session_context(session_context(
        TENANT_ALPHA,
        "alpha",
        vec!["admin:write".to_string(), "read/write".to_string()],
    ));

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
    let ctx_alpha = make_rls_executor(&primary_url).with_session_context(session_context(
        TENANT_ALPHA,
        "alpha",
        vec![],
    ));

    let ctx_beta = make_rls_executor(&primary_url).with_session_context(session_context(
        TENANT_BETA,
        "beta",
        vec![],
    ));

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
        "DO $$\n        DECLARE\n            r RECORD;\n        BEGIN\n            FOR r IN\n                SELECT p.oid, n.nspname, p.proname,\n                       pg_catalog.pg_get_function_arguments(p.oid) AS args
                FROM pg_proc AS p
                JOIN pg_namespace AS n ON n.oid = p.pronamespace
                WHERE p.proname = 'rls_set_session'\n            LOOP\n                EXECUTE format('DROP FUNCTION %I.%I(%s)',\n                    r.nspname, r.proname, r.args);\n            END LOOP;\n        END $$;",
        &[],
    )
    .expect("drop all rls_set_session overloads");

    // Create an executor with session context.
    let exec_with_ctx = make_rls_executor(&primary_url).with_session_context(session_context(
        TENANT_ALPHA,
        "alpha",
        vec![],
    ));

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
        other => {
            panic!("E6: expected PostgresError, got {other:?} — the failure mode may have changed")
        }
    }

    // Recreate the function for subsequent tests.
    setup_exec
        .execute(RLS_SET_SESSION_SQL, &[])
        .expect("recreate rls_set_session for subsequent tests");

    // The failed helper call must have rolled its transaction back. A plain
    // handle sharing the same connection should be immediately usable.
    let plain_exec = MayPostgresExecutor::new(exec_with_ctx.client().clone());
    plain_exec
        .query_one("SELECT 1", &[])
        .expect("connection remains usable after context injection failure");
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
    // Begin a transaction with session context for "alpha".
    let tx = exec
        .begin_with_session(session_context(TENANT_ALPHA, "alpha", vec!["read".into()]))
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
    // Begin with Serializable isolation and session context.
    let tx = exec
        .begin_with_isolation_session(
            IsolationLevel::Serializable,
            session_context(TENANT_GAMMA, "gamma", vec![]),
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
    let mut tx = exec
        .begin_with_session(session_context(TENANT_BETA, "beta", vec![]))
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

    let exec_alpha = PooledLifeExecutor::new(pool.clone()).with_session_context(session_context(
        TENANT_ALPHA,
        "alpha",
        vec![],
    ));

    let exec_beta = PooledLifeExecutor::new(pool.clone()).with_session_context(session_context(
        TENANT_BETA,
        "beta",
        vec![],
    ));

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
// Test P2: Pool worker happy path — function exists
// ===================================================================

/// **P2 — Pool worker happy path.** When rls_set_session exists, the
/// pool worker should execute queries with RLS context normally.
#[test]
fn test_p2_pool_worker_with_function() {
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

    let exec = PooledLifeExecutor::new(pool).with_session_context(session_context(
        TENANT_ALPHA,
        "alpha",
        vec![],
    ));

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
// Test P2b: Pool worker fails when rls_set_session function is missing
// ===================================================================

/// **P2b — Pool worker fails when rls_set_session is missing.**
/// If the rls_set_session function is not present in the schema,
/// the pool worker should return an error for context-injected queries
/// (not silently execute without RLS context).
#[test]
fn test_p2b_pool_worker_missing_function() {
    let ctx = crate::context::get_test_context();
    let primary_url = ctx.pg_url.clone();
    let (schema, table) = unique_rls_schema_names();

    let conn = may_postgres::connect(&primary_url).expect("connect");
    let setup_exec = MayPostgresExecutor::new(conn);
    setup_rls_fixture(&setup_exec, &schema, &table);

    // Drop rls_set_session to simulate a missing function.
    // Must use r. prefix for loop variable references inside the DO block.
    setup_exec.execute(
        "DO $$\n        DECLARE\n            r RECORD;\n        BEGIN\n            FOR r IN\n                SELECT p.oid, n.nspname, p.proname,\n                       pg_catalog.pg_get_function_arguments(p.oid) AS args\n                FROM pg_proc AS p\n                JOIN pg_namespace AS n ON n.oid = p.pronamespace\n                WHERE p.proname = 'rls_set_session'\n            LOOP\n                EXECUTE format('DROP FUNCTION %I.%I(%s)',\n                    r.nspname, r.proname, r.args);\n            END LOOP;\n        END $$;",
        &[],
    )
    .expect("drop rls_set_session");

    // Build RLS URL for the pool.
    let rls_url = primary_url.replace("postgres:postgres@", "rls_test_role:rls_test_role_pw@");

    // Create a pool with 1 worker.
    let pool = Arc::new(LifeguardPool::new(&rls_url, 1, Vec::new(), 0).expect("create pool"));

    // Create a pooled executor WITH session context.
    let exec = PooledLifeExecutor::new(Arc::clone(&pool)).with_session_context(session_context(
        TENANT_ALPHA,
        "alpha",
        vec![],
    ));

    // The helper is missing, so the worker must reject the job before the
    // application query can run without a tenant context.
    let result = exec.query_one_values(
        &format!("SELECT COUNT(*)::bigint AS c FROM {schema}.{table}"),
        &Values(vec![]),
    );

    assert!(
        matches!(
            &result,
            Err(LifeError::Pool(msg))
                if msg.contains("rls_set_session") && msg.contains("not found")
        ),
        "P2b: expected a descriptive pool error for the missing helper"
    );

    // Recreate the function for subsequent tests (same body as ctor setup).
    setup_exec
        .execute(RLS_SET_SESSION_SQL, &[])
        .expect("recreate rls_set_session for subsequent tests");

    // The worker must have rolled back the failed contextual job rather than
    // leaving its only slot in an aborted transaction.
    PooledLifeExecutor::new(pool)
        .query_one_values("SELECT 1", &Values(vec![]))
        .expect("pool slot remains usable after context injection failure");
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

// ===================================================================
// Test P4: Pool worker context does not leak to its next job
// ===================================================================

#[test]
fn test_p4_pool_worker_context_clears_after_job_commit() {
    let ctx = crate::context::get_test_context();
    let primary_url = ctx.pg_url.clone();
    let (schema, table) = unique_rls_schema_names();

    let setup_exec = make_superuser_executor(&primary_url);
    setup_rls_fixture(&setup_exec, &schema, &table);

    let rls_url = primary_url.replace("postgres:postgres@", "rls_test_role:rls_test_role_pw@");
    // Exactly one worker guarantees both jobs use the same physical slot.
    let pool = Arc::new(LifeguardPool::new(&rls_url, 1, Vec::new(), 0).expect("create pool"));
    let alpha_exec = PooledLifeExecutor::new(Arc::clone(&pool))
        .with_session_context(session_context(TENANT_ALPHA, "alpha", vec![]));
    let plain_exec = PooledLifeExecutor::new(pool);
    let query = format!("SELECT COUNT(*)::bigint AS c FROM {schema}.{table}");

    let contextual_count = alpha_exec
        .query_one_values(&query, &Values(vec![]))
        .expect("contextual pool query")
        .get::<_, i64>(0);
    assert_eq!(contextual_count, 2, "P4: alpha job should see alpha rows");

    let plain_count = plain_exec
        .query_one_values(&query, &Values(vec![]))
        .expect("context-free pool query after contextual job")
        .get::<_, i64>(0);
    assert_eq!(
        plain_count, 0,
        "P4: context must be cleared before the worker accepts its next job"
    );
}

// ===================================================================
// Test P5: Pinned pooled transaction shares and clears RLS context
// ===================================================================

#[test]
fn test_p5_pinned_pool_transaction_context_lifecycle() {
    let ctx = crate::context::get_test_context();
    let primary_url = ctx.pg_url.clone();
    let (schema, table) = unique_rls_schema_names();

    let setup_exec = make_superuser_executor(&primary_url);
    setup_rls_fixture(&setup_exec, &schema, &table);

    let rls_url = primary_url.replace("postgres:postgres@", "rls_test_role:rls_test_role_pw@");
    // One slot proves all lifecycle checks reuse the same physical connection.
    let pool = Arc::new(LifeguardPool::new(&rls_url, 1, Vec::new(), 0).expect("create pool"));
    let alpha_context = session_context(TENANT_ALPHA, "alpha", vec!["read".into()]);
    let query = format!("SELECT COUNT(*)::bigint AS c FROM {schema}.{table}");

    let two_counts = pool
        .with_session_transaction(&alpha_context, |executor| {
            let first = executor
                .query_one_values(&query, &Values(vec![]))?
                .get::<_, i64>(0);
            let second = executor
                .query_one_values(&query, &Values(vec![]))?
                .get::<_, i64>(0);
            Ok((first, second))
        })
        .expect("pinned contextual transaction");
    assert_eq!(two_counts, (2, 2), "all statements share one RLS context");

    let application_error = pool.with_session_transaction(&alpha_context, |executor| {
        let visible = executor
            .query_one_values(&query, &Values(vec![]))?
            .get::<_, i64>(0);
        assert_eq!(visible, 2);
        Err::<(), _>(LifeError::Other("application failure".to_string()))
    });
    assert!(matches!(application_error, Err(LifeError::Other(_))));

    let panic_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: Result<(), LifeError> = pool.with_session_transaction(&alpha_context, |executor| {
            let visible = executor
                .query_one_values(&query, &Values(vec![]))?
                .get::<_, i64>(0);
            assert_eq!(visible, 2);
            panic!("intentional transaction unwind");
        });
    }));
    assert!(panic_result.is_err(), "the test closure must unwind");

    let plain_count = PooledLifeExecutor::new(pool)
        .query_one_values(&query, &Values(vec![]))
        .expect("context-free query after commit, error, and panic")
        .get::<_, i64>(0);
    assert_eq!(
        plain_count, 0,
        "commit, application error, and panic must all clear transaction-local context"
    );
}
