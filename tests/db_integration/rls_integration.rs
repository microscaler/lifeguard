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
use lifeguard::LifeExecutor;
use lifeguard::LifeguardPool;
use lifeguard::PooledLifeExecutor;
use lifeguard::SessionContext;
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
    exec.execute(
        "DO $$ BEGIN EXECUTE 'GRANT CONNECT ON DATABASE postgres TO rls_test_role'; EXCEPTION WHEN OTHERS THEN NULL; END $$;",
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
        PERFORM set_config('auth.user_org',
            COALESCE(p_user_org::text, ''), false);
        PERFORM set_config('auth.tenant',
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
                ('alpha', 'alpha-item-a'), \
                ('alpha', 'alpha-item-b'), \
                ('beta', 'beta-item'), \
                ('gamma', 'gamma-item')"
            ),
            &[],
        )
        .expect("seed data");
}

/// Count all rows visible to the current session (RLS-aware).
fn count_visible_rows(executor: &MayPostgresExecutor, schema: &str, table: &str) -> i64 {
    let row = executor
        .query_one_values(
            &format!("SELECT COUNT(*)::bigint AS c FROM {schema}.{table}"),
            &Values(vec![]),
        )
        .expect("count query");
    row.get(0)
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
    let c_no = count_visible_rows(&no_ctx, &schema, &table);
    assert_eq!(c_no, 0, "No context -> zero rows (fail-closed baseline)");

    // With session context for tenant "alpha" we should see exactly 2 rows.
    // Using set_config(..., false) ensures GUCs persist at the session level,
    // so they survive across separate autocommit statements.
    let uid = uuid::Uuid::new_v4();
    let alpha_exec = make_rls_executor(&primary_url).with_session_context(SessionContext {
        user_id: Some(uid),
        user_org_id: None,
        user_type: Some("alpha".into()),
        org_type: None,
        permissions: vec!["read".into()],
        user_email: None,
    });

    let c_alpha = count_visible_rows(&alpha_exec, &schema, &table);
    assert_eq!(
        c_alpha, 2,
        "Test A: with tenant=alpha context, should see exactly 2 rows"
    );

    // Count total visible rows with alpha context — should be exactly 2 (the alpha rows).
    // This proves RLS filtering is active: without RLS we'd see all 4 rows.
    let c_total_from_alpha = count_visible_rows(&alpha_exec, &schema, &table);
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
    let count_no = count_visible_rows(&exec_no_ctx, &schema, &table);
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
            user_org_id: None,
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
        user_org_id: None,
        user_type: Some("alpha".into()),
        org_type: None,
        permissions: vec!["read".into()],
        user_email: None,
    });

    let exec_gamma = PooledLifeExecutor::new(pool).with_session_context(SessionContext {
        user_id: Some(uuid::Uuid::new_v4()),
        user_org_id: None,
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
