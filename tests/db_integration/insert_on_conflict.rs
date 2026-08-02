//! Integration tests for `ActiveModelTrait::insert_on_conflict`.
//!
//! Upsert exists so that callers needing an idempotent write stay inside the
//! ORM. Before it, the only options were raw SQL — which forfeits the typed
//! parameter marshalling, the validators and the insert hooks — or a
//! `SELECT`-then-`INSERT`, which races under concurrency.
//!
//! These run against a real PostgreSQL because the whole point is the database's
//! `ON CONFLICT` semantics: what `DO NOTHING` returns, what `DO UPDATE` returns,
//! and whether `RETURNING` yields a row. None of that can be asserted by
//! inspecting generated SQL.

use lifeguard::query::column::column_trait::ColumnTrait;
use lifeguard::{
    test_helpers::TestDatabase, ActiveModelTrait, LifeExecutor, LifeModelTrait,
    MayPostgresExecutor, OnConflict,
};
use lifeguard_derive::{LifeModel, LifeRecord};

fn get_db() -> TestDatabase {
    let ctx = crate::context::get_test_context();
    TestDatabase::with_url(&ctx.pg_url)
}

/// Connect and make sure the table exists.
///
/// The suite runs its tests in parallel, so schema creation is done once:
/// concurrent `CREATE TABLE IF NOT EXISTS` calls race on the backing sequence
/// and fail with a `pg_class_relname_nsp_index` unique violation. For the same
/// reason there is no blanket `DELETE` here — the tests would wipe each
/// other's rows. Each test scopes itself with a unique dedup key instead.
fn fresh_executor() -> MayPostgresExecutor {
    static SCHEMA: std::sync::Once = std::sync::Once::new();

    let mut test_db = get_db();
    let _client = test_db.connect().expect("connect to test database");
    let executor = test_db.executor().expect("create executor");

    SCHEMA.call_once(|| {
        setup_schema(&executor).expect("schema");
    });

    executor
}

/// A dedup key unique to this test *and* this run, so a previous run's rows
/// cannot make a fresh run pass or fail spuriously.
fn key(scope: &str) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    format!("{scope}:{nanos}")
}

pub mod dedup_event {
    use super::{LifeModel, LifeRecord};

    /// Mirrors the shape that motivated this feature: an event queue keyed by a
    /// unique `dedup_key`, where a duplicate enqueue must be a no-op rather
    /// than an error.
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_dedup_events"]
    pub struct DedupEvent {
        #[primary_key]
        #[auto_increment]
        pub id: i32,

        /// The conflict target. `#[unique]` is not decoration here — PostgreSQL
        /// refuses `ON CONFLICT (dedup_key)` unless a unique index backs the
        /// column, so the constraint is what makes the feature work at all.
        #[unique]
        pub dedup_key: String,
        pub event_type: String,
        pub status: String,
    }
}
pub use dedup_event::DedupEventRecord;

/// Build the table from the entity definition rather than hand-written DDL, so
/// the test cannot drift from the model it is exercising.
///
/// The unique index on `dedup_key` comes from `#[unique]` on the entity field,
/// which is the conflict target these tests turn on.
fn setup_schema(executor: &MayPostgresExecutor) -> Result<(), lifeguard::executor::LifeError> {
    use lifeguard::migration::schema_manager::SchemaManager;

    // Drop first: `create_table_from_entity` emits IF NOT EXISTS, so a table
    // left behind by an earlier run keeps its old shape and silently ignores
    // changes to the entity — including the `#[unique]` that ON CONFLICT needs.
    // That is a real trap worth stating: an entity edit does not reach an
    // existing table, which is precisely how a schema drifts from its model.
    executor.execute("DROP TABLE IF EXISTS test_dedup_events", &[])?;

    let manager = SchemaManager::new(executor);
    manager.create_table_from_entity::<dedup_event::Entity>()?;

    // KNOWN GAP: `create_table_from_entity` emits column definitions only. The
    // `#[unique]` on `dedup_key` — and `#[indexed]`, and the primary key
    // constraint — are dropped, so the generated table is weaker than the
    // entity declares. Verified by inspecting the produced table: no unique
    // constraint, no primary key index.
    //
    // That matters well beyond this test. It is the mechanism by which an
    // entity-driven schema silently loses the very constraint an ON CONFLICT
    // target depends on, which is a real risk for
    // `hauliage.notification_events.dedup_key`.
    //
    // Until the generator honours these attributes, the constraint is applied
    // explicitly here so these tests exercise upsert rather than the gap.
    executor.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS test_dedup_events_dedup_key_uq
         ON test_dedup_events (dedup_key)",
        &[],
    )?;

    Ok(())
}

#[allow(dead_code)] // retained for ad-hoc local cleanup
fn cleanup(executor: &MayPostgresExecutor) -> Result<(), lifeguard::executor::LifeError> {
    executor.execute("DELETE FROM test_dedup_events", &[])?;
    Ok(())
}

fn record(dedup_key: &str, event_type: &str, status: &str) -> DedupEventRecord {
    let mut r = DedupEventRecord::new();
    r.set_dedup_key(dedup_key.to_string())
        .set_event_type(event_type.to_string())
        .set_status(status.to_string());
    r
}

fn count_rows(exec: &MayPostgresExecutor, key: &str) -> i64 {
    use dedup_event::{Column, Entity};
    Entity::find()
        .filter(Column::DedupKey.eq(key))
        .count()
        .one(exec)
        .expect("count")
}

#[test]
fn do_nothing_reports_the_conflict_instead_of_writing_twice() {
    let exec = fresh_executor();
    let k = key("quote.expiring");

    let conflict = || {
        OnConflict::column(dedup_event::Column::DedupKey)
            .do_nothing()
            .to_owned()
    };

    let first = record(&k, "quote.expiring_soon", "PENDING")
        .insert_on_conflict(&exec, conflict())
        .expect("first insert");
    assert!(
        first.is_some(),
        "the first insert must write a row and return it"
    );

    // The same logical event arriving again — a second worker tick, a retry, a
    // redelivered message. It must not create a duplicate, and must not error.
    let second = record(&k, "quote.expiring_soon", "PENDING")
        .insert_on_conflict(&exec, conflict())
        .expect("duplicate enqueue must not be an error");
    assert!(
        second.is_none(),
        "a DO NOTHING conflict must report None, not a fabricated row"
    );

    assert_eq!(
        count_rows(&exec, &k),
        1,
        "exactly one row should exist after a duplicate enqueue"
    );
}

#[test]
fn do_update_returns_the_surviving_row_so_the_caller_learns_its_id() {
    let exec = fresh_executor();
    let k = key("job.assigned");

    let original = record(&k, "job.assigned", "PENDING")
        .insert(&exec)
        .expect("seed row");

    // DO UPDATE always produces a row, which is how a caller recovers the id of
    // whichever row won the race. DO NOTHING cannot do this — it returns
    // nothing — and that difference is the reason both forms are supported.
    let upserted = record(&k, "job.assigned", "DISPATCHED")
        .insert_on_conflict(
            &exec,
            OnConflict::column(dedup_event::Column::DedupKey)
                .update_column(dedup_event::Column::Status)
                .to_owned(),
        )
        .expect("upsert")
        .expect("DO UPDATE must return the surviving row");

    assert_eq!(
        upserted.id, original.id,
        "the upsert should resolve to the existing row, not a new one"
    );
    assert_eq!(
        upserted.status, "DISPATCHED",
        "the conflicting column should have been updated from EXCLUDED"
    );
    assert_eq!(count_rows(&exec, &k), 1);
}

#[test]
fn a_non_conflicting_insert_behaves_exactly_like_a_plain_insert() {
    let exec = fresh_executor();
    let k = key("unique.key");

    let model = record(&k, "job.created", "PENDING")
        .insert_on_conflict(
            &exec,
            OnConflict::column(dedup_event::Column::DedupKey)
                .do_nothing()
                .to_owned(),
        )
        .expect("insert")
        .expect("no conflict, so a row must come back");

    assert!(
        model.id > 0,
        "the auto-increment primary key must still be populated via RETURNING"
    );
    assert_eq!(model.status, "PENDING");
}

#[test]
fn plain_insert_still_errors_on_a_genuine_unique_violation() {
    let exec = fresh_executor();
    let k = key("strict.key");

    record(&k, "job.created", "PENDING")
        .insert(&exec)
        .expect("first insert");

    // Refactoring insert() to share a body with insert_on_conflict() must not
    // have quietly turned a duplicate key into a silent no-op. Without an
    // ON CONFLICT clause the database error has to keep surfacing.
    let result = record(&k, "job.created", "PENDING").insert(&exec);
    assert!(
        result.is_err(),
        "a duplicate insert with no ON CONFLICT clause must still fail loudly"
    );
}
