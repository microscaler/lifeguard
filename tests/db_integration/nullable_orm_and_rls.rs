//! Change-set semantics across the ORM surfaces that are not a plain
//! `update()`: soft-delete restore, identity-map / session flush, and Row
//! Level Security.
//!
//! `nullable_null_update.rs` covers the change-set → statement path directly.
//! These are the paths that build on it and have their own failure modes:
//!
//! - **Soft delete.** `delete()` stamps `deleted_at`; there is no `restore()`,
//!   because clearing `deleted_at` *is* the restore. Since `find()` filters on
//!   `deleted_at IS NULL`, a restore that failed to write would leave the row
//!   intact but invisible to every query the ORM generates.
//! - **Identity map / session flush.** Persistence goes through a
//!   caller-supplied `Record::update()`, and a change-set whose only staged
//!   column is a NULL still has to count as dirty — otherwise save-if-dirty
//!   code drops the write before any SQL exists.
//! - **RLS.** `USING` decides which rows an UPDATE may target and `WITH CHECK`
//!   what a row may become. A clear is a real write, so the policy adjudicates
//!   it: permitted clears land, forbidden ones are refused by the database
//!   rather than quietly skipped by the ORM.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use lifeguard::executor::MayPostgresExecutor;
use lifeguard::query::column::column_trait::ColumnTrait;
use lifeguard::session::ModelIdentityMap;
use lifeguard::test_helpers::TestDatabase;
use lifeguard::{ActiveModelTrait, LifeExecutor, LifeModelTrait, SessionContext};
use lifeguard_derive::{LifeModel, LifeRecord};

static LOCK: Mutex<()> = Mutex::new(());

/// Assertions run while the lock is held, so a real failure would poison it
/// and turn every later test into a `PoisonError` instead of its own message.
fn lock() -> std::sync::MutexGuard<'static, ()> {
    LOCK.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn db() -> TestDatabase {
    let ctx = crate::context::get_test_context();
    TestDatabase::with_url(&ctx.pg_url)
}

// ══════════════════════════ soft delete: restore ═════════════════════════════

pub mod docs {
    use super::{DateTime, LifeModel, LifeRecord, Utc};

    #[derive(LifeModel, LifeRecord, Debug, Clone)]
    #[table_name = "lg_null_soft_delete"]
    #[soft_delete]
    pub struct Doc {
        #[primary_key]
        #[auto_increment]
        pub id: i32,
        pub title: String,
        pub deleted_at: Option<DateTime<Utc>>,
    }
}
use docs::{DocRecord, Entity as DocEntity};

fn setup_docs(executor: &dyn LifeExecutor) {
    executor
        .execute("DROP TABLE IF EXISTS lg_null_soft_delete CASCADE", &[])
        .expect("drop");
    executor
        .execute(
            "CREATE TABLE lg_null_soft_delete (
                id SERIAL PRIMARY KEY,
                title TEXT NOT NULL,
                deleted_at TIMESTAMPTZ
            )",
            &[],
        )
        .expect("create");
}

/// Soft delete then restore, entirely through the ORM.
///
/// The restore half (`deleted_at = NULL`) is the operation the bug ate. Note
/// what makes this more than a duplicate of the plain-update test: `find()` on
/// a `#[soft_delete]` entity filters on `deleted_at IS NULL`, so a restore
/// that silently did nothing left the row permanently invisible to every
/// query the ORM generates — the data was intact but unreachable.
#[test]
fn soft_deleted_row_can_be_restored_through_the_orm() {
    let _guard = lock();
    let mut test_db = db();
    let executor = test_db.executor().expect("executor");
    setup_docs(&executor);

    let mut record = DocRecord::new();
    record.set_title("quarterly report".to_string());
    let doc = record.insert(&executor).expect("insert");

    // Soft delete: builds its own UPDATE with an explicit timestamp, so this
    // half was never affected by the bug.
    let record = DocRecord::from_model(&doc);
    record.delete(&executor).expect("soft delete");
    assert!(
        DocEntity::find()
            .filter(<DocEntity as LifeModelTrait>::Column::Id.eq(doc.id))
            .find_one(&executor)
            .expect("query")
            .is_none(),
        "soft-deleted rows are filtered out of find()"
    );

    // Restore. There is no `restore()` API — clearing `deleted_at` IS the
    // restore, and it used to be a no-op.
    let mut record = DocRecord::new();
    record.set_id(doc.id).set_deleted_at(None);
    record.update(&executor).expect("restore");

    let restored = DocEntity::find()
        .filter(<DocEntity as LifeModelTrait>::Column::Id.eq(doc.id))
        .find_one(&executor)
        .expect("query")
        .expect("row is visible again");
    assert!(restored.deleted_at.is_none());
    assert_eq!(restored.title, "quarterly report");
}

/// The explicit spelling of the same restore, and proof that soft delete still
/// *sets* a timestamp — the fix must not have turned `delete()` into a clear.
#[test]
fn soft_delete_still_stamps_and_set_null_still_restores() {
    let _guard = lock();
    let mut test_db = db();
    let executor = test_db.executor().expect("executor");
    setup_docs(&executor);

    let mut record = DocRecord::new();
    record.set_title("memo".to_string());
    let doc = record.insert(&executor).expect("insert");

    let record = DocRecord::from_model(&doc);
    record.delete(&executor).expect("soft delete");

    let row = executor
        .query_one(
            "SELECT deleted_at FROM lg_null_soft_delete WHERE id = $1",
            &[&doc.id as &(dyn may_postgres::types::ToSql + Sync)],
        )
        .expect("select");
    let stamped: Option<DateTime<Utc>> = row.get(0);
    assert!(stamped.is_some(), "delete() must still stamp deleted_at");

    let mut record = DocRecord::new();
    record.set_id(doc.id).set_deleted_at_null();
    record.update(&executor).expect("restore");

    let row = executor
        .query_one(
            "SELECT deleted_at FROM lg_null_soft_delete WHERE id = $1",
            &[&doc.id as &(dyn may_postgres::types::ToSql + Sync)],
        )
        .expect("select");
    let after: Option<DateTime<Utc>> = row.get(0);
    assert_eq!(after, None, "set_deleted_at_null must clear the stamp");
}

// ═══════════════════ identity map / unit-of-work flush ═══════════════════════

pub mod profiles {
    use super::{LifeModel, LifeRecord};

    #[derive(LifeModel, LifeRecord, Debug, Clone)]
    #[table_name = "lg_null_flush"]
    pub struct Profile {
        #[primary_key]
        #[auto_increment]
        pub id: i32,
        pub handle: String,
        pub nickname: Option<String>,
    }
}
use profiles::{Entity as ProfileEntity, ProfileRecord};

fn setup_profiles(executor: &dyn LifeExecutor) {
    executor
        .execute("DROP TABLE IF EXISTS lg_null_flush CASCADE", &[])
        .expect("drop");
    executor
        .execute(
            "CREATE TABLE lg_null_flush (
                id SERIAL PRIMARY KEY,
                handle TEXT NOT NULL,
                nickname TEXT
            )",
            &[],
        )
        .expect("create");
}

fn nickname_of(executor: &dyn LifeExecutor, id: i32) -> Option<String> {
    executor
        .query_one(
            "SELECT nickname FROM lg_null_flush WHERE id = $1",
            &[&id as &(dyn may_postgres::types::ToSql + Sync)],
        )
        .expect("select")
        .get(0)
}

/// A clear that goes out through `flush_dirty` must reach the database, and
/// must not be dropped as "nothing to do".
#[test]
fn identity_map_flush_persists_a_cleared_column() {
    let _guard = lock();
    let mut test_db = db();
    let executor = test_db.executor().expect("executor");
    setup_profiles(&executor);

    let mut record = ProfileRecord::new();
    record
        .set_handle("ada".to_string())
        .set_nickname(Some("the countess".to_string()));
    let profile = record.insert(&executor).expect("insert");

    let mut map = ModelIdentityMap::<ProfileEntity>::new();
    let cell = map.register_loaded(profile.clone());
    cell.borrow_mut().nickname = None;
    map.mark_dirty(&profile);

    map.flush_dirty(&executor, |ex, model| {
        let m = model.borrow();
        let mut record = ProfileRecord::new();
        record.set_id(m.id).set_nickname(m.nickname.clone());
        record.update(ex)?;
        Ok(())
    })
    .expect("flush_dirty");

    assert_eq!(
        nickname_of(&executor, profile.id),
        None,
        "a cleared column must survive the flush path"
    );
}

/// A record whose *only* pending change is a NULL still counts as dirty. If it
/// did not, save-if-dirty code would skip the write and the clear would be
/// lost before any SQL was generated.
#[test]
fn a_null_only_change_set_is_dirty_and_writes() {
    let _guard = lock();
    let mut test_db = db();
    let executor = test_db.executor().expect("executor");
    setup_profiles(&executor);

    let mut record = ProfileRecord::new();
    record
        .set_handle("grace".to_string())
        .set_nickname(Some("amazing".to_string()));
    let profile = record.insert(&executor).expect("insert");

    let mut record = ProfileRecord::new();
    record.set_id(profile.id);
    let before = record.dirty_fields().len();
    record.set_nickname_null();
    assert_eq!(
        record.dirty_fields().len(),
        before + 1,
        "staging a NULL must register as a change"
    );

    record.update(&executor).expect("update");
    assert_eq!(nickname_of(&executor, profile.id), None);
}

// ════════════════════════════ Row Level Security ═════════════════════════════

static RLS_SEQ: AtomicU64 = AtomicU64::new(0);

pub mod items {
    use super::{LifeModel, LifeRecord};

    #[derive(LifeModel, LifeRecord, Debug, Clone)]
    #[table_name = "lg_null_rls_items"]
    #[schema_name = "public"]
    pub struct Item {
        #[primary_key]
        #[auto_increment]
        pub id: i32,
        pub org_id: String,
        pub secret: Option<String>,
    }
}
use items::ItemRecord;

/// Executor running as the non-superuser `rls_test_role` (superusers and the
/// table owner bypass RLS, which would make these tests vacuous). The role and
/// the `rls_set_session` GUC helper are created by `rls_integration`'s `ctor`.
fn rls_executor(pg_url: &str) -> MayPostgresExecutor {
    let conn = may_postgres::connect(pg_url).expect("connect");
    conn.execute("SET ROLE rls_test_role", &[])
        .expect("SET ROLE");
    MayPostgresExecutor::new(conn)
}

fn context_for(org_id: &str) -> SessionContext {
    SessionContext {
        tenant_id: "hauliage".to_string(),
        subject_id: uuid::Uuid::new_v4(),
        organization_id: uuid::Uuid::parse_str(org_id).expect("org uuid"),
        session_id: format!("null-rls-{}", uuid::Uuid::new_v4()),
        roles: vec!["member".to_string()],
        permissions: vec![],
        user_type: Some("member".to_string()),
        org_type: Some("tenant".to_string()),
    }
}

/// Table + policy. `with_check` controls whether the policy also constrains
/// what a row may become, which is what turns a NULL write into a rejection.
fn setup_rls_items(superuser: &MayPostgresExecutor, with_check: bool) {
    let _ = RLS_SEQ.fetch_add(1, Ordering::Relaxed);
    superuser
        .execute("DROP TABLE IF EXISTS public.lg_null_rls_items CASCADE", &[])
        .expect("drop");
    superuser
        .execute(
            "CREATE TABLE public.lg_null_rls_items (
                id SERIAL PRIMARY KEY,
                org_id TEXT NOT NULL,
                secret TEXT
            )",
            &[],
        )
        .expect("create");
    superuser
        .execute(
            "GRANT SELECT, INSERT, UPDATE ON public.lg_null_rls_items TO rls_test_role",
            &[],
        )
        .expect("grant table");
    superuser
        .execute(
            "GRANT USAGE, SELECT ON SEQUENCE public.lg_null_rls_items_id_seq TO rls_test_role",
            &[],
        )
        .expect("grant sequence");
    superuser
        .execute(
            "ALTER TABLE public.lg_null_rls_items ENABLE ROW LEVEL SECURITY",
            &[],
        )
        .expect("enable rls");

    let policy = if with_check {
        // The row must stay in the caller's org AND keep a secret. Nulling
        // `secret` is therefore a policy violation — reachable through the
        // ordinary `set_secret_null()` path rather than an escape hatch.
        "CREATE POLICY org_isolation ON public.lg_null_rls_items
             USING (org_id = current_setting('sesame.organization_id', true))
             WITH CHECK (org_id = current_setting('sesame.organization_id', true)
                         AND secret IS NOT NULL)"
    } else {
        "CREATE POLICY org_isolation ON public.lg_null_rls_items
             USING (org_id = current_setting('sesame.organization_id', true))"
    };
    superuser.execute(policy, &[]).expect("create policy");
}

fn seed_item(superuser: &MayPostgresExecutor, org_id: &str, secret: &str) -> i32 {
    superuser
        .query_one(
            "INSERT INTO public.lg_null_rls_items (org_id, secret) VALUES ($1, $2) RETURNING id",
            &[
                &org_id as &(dyn may_postgres::types::ToSql + Sync),
                &secret as &(dyn may_postgres::types::ToSql + Sync),
            ],
        )
        .expect("seed")
        .get(0)
}

fn secret_of(superuser: &MayPostgresExecutor, id: i32) -> Option<String> {
    superuser
        .query_one(
            "SELECT secret FROM public.lg_null_rls_items WHERE id = $1",
            &[&id as &(dyn may_postgres::types::ToSql + Sync)],
        )
        .expect("select")
        .get(0)
}

const ORG_A: &str = "550e8400-e29b-41d4-a716-446655440001";
const ORG_B: &str = "550e8400-e29b-41d4-a716-446655440002";

/// A clear under RLS must apply to your own row.
#[test]
fn rls_clear_applies_to_a_visible_row() {
    let _guard = lock();
    let ctx = crate::context::get_test_context();
    let superuser = MayPostgresExecutor::new(may_postgres::connect(&ctx.pg_url).expect("connect"));
    setup_rls_items(&superuser, false);
    let mine = seed_item(&superuser, ORG_A, "my secret");

    let executor = rls_executor(&ctx.pg_url).with_session_context(context_for(ORG_A));

    let mut record = ItemRecord::new();
    record.set_id(mine).set_secret(None);
    record.update(&executor).expect("update own row");

    assert_eq!(
        secret_of(&superuser, mine),
        None,
        "clearing a column on a row the policy admits must take effect"
    );
}

/// …and must NOT reach a row the policy hides. RLS scoping is enforced by the
/// `USING` clause on the UPDATE, so now that the SET clause is real, the
/// interesting question is whether the row filter still holds. It does: the
/// statement matches zero rows and the ORM reports `RecordNotFound` rather
/// than silently "succeeding".
#[test]
fn rls_clear_cannot_reach_another_orgs_row() {
    let _guard = lock();
    let ctx = crate::context::get_test_context();
    let superuser = MayPostgresExecutor::new(may_postgres::connect(&ctx.pg_url).expect("connect"));
    setup_rls_items(&superuser, false);
    let theirs = seed_item(&superuser, ORG_B, "their secret");

    let executor = rls_executor(&ctx.pg_url).with_session_context(context_for(ORG_A));

    let mut record = ItemRecord::new();
    record.set_id(theirs).set_secret(None);
    let result = record.update(&executor);

    assert!(
        result.is_err(),
        "an update that matches no visible row must not report success"
    );
    assert_eq!(
        secret_of(&superuser, theirs).as_deref(),
        Some("their secret"),
        "another org's data must be untouched"
    );
}

/// The behaviour change RLS users should know about: a clear is now a real
/// write, so a `WITH CHECK` policy adjudicates it.
///
/// Before the fix the SET clause was dropped, so the policy never saw the
/// write: Postgres was asked to change nothing, happily changed nothing, and
/// the caller was told it had cleared a column the database would never have
/// let it clear. Silent divergence between what the application believed and
/// what the row contained — the worst possible outcome for a security
/// boundary. Now it fails loudly.
#[test]
fn rls_with_check_adjudicates_a_clear_instead_of_it_being_skipped() {
    let _guard = lock();
    let ctx = crate::context::get_test_context();
    let superuser = MayPostgresExecutor::new(may_postgres::connect(&ctx.pg_url).expect("connect"));
    setup_rls_items(&superuser, true);
    let mine = seed_item(&superuser, ORG_A, "my secret");

    let executor = rls_executor(&ctx.pg_url).with_session_context(context_for(ORG_A));

    let mut record = ItemRecord::new();
    record.set_id(mine).set_secret_null();
    let err = record
        .update(&executor)
        .expect_err("the policy must adjudicate the clear, not have it silently dropped");

    // Assert on WHICH error. Before the fix this call also failed — but with
    // a syntax error, because dropping the only SET clause left `UPDATE ...
    // WHERE`. "It errored" would therefore pass for entirely the wrong
    // reason; the point is that Postgres evaluated the policy and refused.
    let message = err.to_string();
    assert!(
        message.contains("row-level security policy"),
        "expected an RLS policy violation, got: {message}"
    );
    assert_eq!(
        secret_of(&superuser, mine).as_deref(),
        Some("my secret"),
        "a rejected clear must leave the row intact"
    );
}

/// The same clear succeeds when the policy permits it — the rejection above is
/// the policy talking, not the ORM refusing to emit NULLs under RLS.
#[test]
fn rls_permits_a_clear_the_policy_allows() {
    let _guard = lock();
    let ctx = crate::context::get_test_context();
    let superuser = MayPostgresExecutor::new(may_postgres::connect(&ctx.pg_url).expect("connect"));
    setup_rls_items(&superuser, false);
    let mine = seed_item(&superuser, ORG_A, "my secret");

    let executor = rls_executor(&ctx.pg_url).with_session_context(context_for(ORG_A));

    let mut record = ItemRecord::new();
    record.set_id(mine).set_secret_null();
    record.update(&executor).expect("clear permitted by policy");

    assert_eq!(secret_of(&superuser, mine), None);
    assert_eq!(
        superuser
            .query_one(
                "SELECT org_id FROM public.lg_null_rls_items WHERE id = $1",
                &[&mine as &(dyn may_postgres::types::ToSql + Sync)],
            )
            .expect("select")
            .get::<_, String>(0),
        ORG_A,
        "only the targeted column changed"
    );
}
