//! Postgres integration: writing SQL `NULL` from a change-set.
//!
//! # What is being pinned
//!
//! A record field is an [`ActiveValue`], which distinguishes *whether* a
//! column is written from *what* is written to it. These tests hold both
//! halves of that contract against a real database:
//!
//! - a column staged as `SetNull` is written as SQL `NULL`, in the type the
//!   column actually has;
//! - a column left `NotSet` is absent from the statement entirely, so an
//!   update touches nothing it was not asked to touch.
//!
//! The second half matters as much as the first. A change-set that nulled
//! every unset `Option` would clear columns the caller never mentioned —
//! worse than failing to clear the ones it did.
//!
//! [`ActiveValue`]: lifeguard::ActiveValue

use std::sync::Mutex;

use crate::context::get_test_context;
use lifeguard::executor::LifeError;
use lifeguard::test_helpers::TestDatabase;
use lifeguard::{ActiveModelTrait, ColumnTrait, LifeExecutor, LifeModelTrait};
use lifeguard_derive::{LifeModel, LifeRecord};
use uuid::Uuid;

static LOCK: Mutex<()> = Mutex::new(());

/// These tests assert while holding the lock, so a genuine failure poisons it
/// and every later test dies with `PoisonError` instead of its own message —
/// one real failure would look like ten mysterious ones. Ignore the poison
/// and keep the serialisation.
fn lock() -> std::sync::MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[derive(LifeModel, LifeRecord, Debug, Clone)]
#[table_name = "lg_nullable_update"]
pub struct Widget {
    #[primary_key]
    #[column_type = "UUID"]
    pub id: Uuid,

    /// Non-nullable: no `set_name_null()` is generated for it, so "clear this"
    /// is a compile error rather than a runtime surprise.
    #[column_type = "TEXT"]
    pub name: String,

    #[column_type = "TEXT"]
    #[nullable]
    pub note: Option<String>,

    /// Nullable with a DEFAULT — distinguishes "let the default apply" from
    /// "explicitly NULL" on INSERT.
    #[column_type = "TEXT"]
    #[nullable]
    pub tag: Option<String>,

    /// Non-string types prove the NULL is typed per column: a
    /// `Value::String(None)` bound here is a parameter-type error, not a NULL.
    #[column_type = "INTEGER"]
    #[nullable]
    pub count: Option<i32>,

    #[column_type = "UUID"]
    #[nullable]
    pub owner_id: Option<Uuid>,

    #[column_type = "TIMESTAMP WITH TIME ZONE"]
    #[nullable]
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

fn setup(executor: &dyn LifeExecutor) -> Result<(), LifeError> {
    executor.execute("DROP TABLE IF EXISTS lg_nullable_update CASCADE", &[])?;
    executor.execute(
        "CREATE TABLE lg_nullable_update (
            id UUID PRIMARY KEY,
            name TEXT NOT NULL,
            note TEXT,
            tag TEXT DEFAULT 'from-default',
            count INTEGER,
            owner_id UUID,
            expires_at TIMESTAMP WITH TIME ZONE
        )",
        &[],
    )?;
    Ok(())
}

/// Insert a fully populated row and return its id.
fn seed(executor: &dyn LifeExecutor) -> Uuid {
    let id = Uuid::new_v4();
    let mut record = WidgetRecord::new();
    record
        .set_id(id)
        .set_name("seeded".to_string())
        .set_note(Some("original note".to_string()))
        .set_tag(Some("original tag".to_string()))
        .set_count(Some(41))
        .set_owner_id(Some(Uuid::new_v4()))
        .set_expires_at(Some(chrono::Utc::now()));
    record.insert(executor).expect("seed insert");
    id
}

fn note_of(executor: &dyn LifeExecutor, id: Uuid) -> Option<String> {
    let row = executor
        .query_one(
            "SELECT note FROM lg_nullable_update WHERE id = $1",
            &[&id as &(dyn may_postgres::types::ToSql + Sync)],
        )
        .expect("select note");
    row.get(0)
}

fn fetch(executor: &dyn LifeExecutor, id: Uuid) -> WidgetModel {
    Entity::find()
        .filter(<Entity as LifeModelTrait>::Column::Id.eq(id))
        .find_one(&executor)
        .expect("query")
        .expect("row exists")
}

// ─────────────────────────── positive: NULL is written ───────────────────────

/// `set_x(None)` on an update must write `NULL`. This is the exact call that
/// used to be silently dropped.
#[test]
fn set_none_writes_null_on_update() {
    let _guard = lock();
    let ctx = get_test_context();
    let mut db = TestDatabase::with_url(&ctx.pg_url);
    let executor = db.executor().expect("executor");
    setup(&executor).expect("setup");

    let id = seed(&executor);
    assert_eq!(note_of(&executor, id).as_deref(), Some("original note"));

    let mut record = WidgetRecord::new();
    record.set_id(id).set_note(None);
    record.update(&executor).expect("update");

    assert_eq!(
        note_of(&executor, id),
        None,
        "set_note(None) must clear the column, not leave the old value"
    );
}

/// The explicit spelling, which is the only option for a `T` + `#[nullable]`
/// field whose setter cannot take `None`.
#[test]
fn set_null_helper_writes_null() {
    let _guard = lock();
    let ctx = get_test_context();
    let mut db = TestDatabase::with_url(&ctx.pg_url);
    let executor = db.executor().expect("executor");
    setup(&executor).expect("setup");

    let id = seed(&executor);
    let mut record = WidgetRecord::new();
    record.set_id(id).set_note_null();
    record.update(&executor).expect("update");

    assert_eq!(note_of(&executor, id), None);
}

/// Every nullable column type must produce a correctly typed NULL. A wrongly
/// typed null parameter fails at the wire protocol, so this test would error
/// rather than merely assert.
#[test]
fn typed_nulls_work_for_every_column_type() {
    let _guard = lock();
    let ctx = get_test_context();
    let mut db = TestDatabase::with_url(&ctx.pg_url);
    let executor = db.executor().expect("executor");
    setup(&executor).expect("setup");

    let id = seed(&executor);
    let mut record = WidgetRecord::new();
    record
        .set_id(id)
        .set_note(None)
        .set_count(None)
        .set_owner_id(None)
        .set_expires_at(None);
    record.update(&executor).expect("update with typed nulls");

    let model = fetch(&executor, id);
    assert_eq!(model.note, None, "TEXT");
    assert_eq!(model.count, None, "INTEGER");
    assert_eq!(model.owner_id, None, "UUID");
    assert_eq!(model.expires_at, None, "TIMESTAMPTZ");
    assert_eq!(model.name, "seeded", "untouched NOT NULL column survives");
}

/// The everyday shape: read the row, clear one field, write it back.
#[test]
fn from_model_then_clear_one_field() {
    let _guard = lock();
    let ctx = get_test_context();
    let mut db = TestDatabase::with_url(&ctx.pg_url);
    let executor = db.executor().expect("executor");
    setup(&executor).expect("setup");

    let id = seed(&executor);
    let model = fetch(&executor, id);

    let mut record = WidgetRecord::from_model(&model);
    record.set_expires_at(None);
    record.update(&executor).expect("update");

    let after = fetch(&executor, id);
    assert_eq!(after.expires_at, None, "cleared");
    assert_eq!(
        after.note.as_deref(),
        Some("original note"),
        "other fields round-trip unchanged"
    );
}

/// On INSERT, an explicit NULL must beat the column DEFAULT — otherwise
/// "explicitly none" and "unspecified" collapse into the same row.
#[test]
fn insert_explicit_null_overrides_column_default() {
    let _guard = lock();
    let ctx = get_test_context();
    let mut db = TestDatabase::with_url(&ctx.pg_url);
    let executor = db.executor().expect("executor");
    setup(&executor).expect("setup");

    let explicit = Uuid::new_v4();
    let mut record = WidgetRecord::new();
    record
        .set_id(explicit)
        .set_name("explicit".to_string())
        .set_tag_null();
    record.insert(&executor).expect("insert with explicit null");
    assert_eq!(
        fetch(&executor, explicit).tag,
        None,
        "explicit NULL must not fall back to the DEFAULT"
    );

    let defaulted = Uuid::new_v4();
    let mut record = WidgetRecord::new();
    record.set_id(defaulted).set_name("defaulted".to_string());
    record.insert(&executor).expect("insert without tag");
    assert_eq!(
        fetch(&executor, defaulted).tag.as_deref(),
        Some("from-default"),
        "an untouched column must still take the DEFAULT"
    );
}

/// An `Unchanged` column is a value we hold, so a copy-insert writes it —
/// `NotSet` is what lets a column DEFAULT apply, and the two must not be
/// confused on the insert path either.
#[test]
fn insert_writes_unchanged_values_but_not_untouched_ones() {
    let _guard = lock();
    let ctx = get_test_context();
    let mut db = TestDatabase::with_url(&ctx.pg_url);
    let executor = db.executor().expect("executor");
    setup(&executor).expect("setup");

    let original = fetch(&executor, seed(&executor));

    // Copy the row under a new id: everything is `Unchanged` except the id.
    let mut record = WidgetRecord::from_model(&original);
    let copy_id = Uuid::new_v4();
    record.set_id(copy_id);
    record.insert(&executor).expect("copy insert");

    let copy = fetch(&executor, copy_id);
    assert_eq!(copy.name, original.name, "Unchanged values are inserted");
    assert_eq!(copy.note, original.note);
    assert_eq!(copy.count, original.count);
    assert_eq!(copy.tag, original.tag, "not silently replaced by the DEFAULT");

    // A record that never touches `tag` gets the DEFAULT instead.
    let fresh_id = Uuid::new_v4();
    let mut record = WidgetRecord::new();
    record.set_id(fresh_id).set_name("fresh".to_string());
    record.insert(&executor).expect("fresh insert");
    assert_eq!(
        fetch(&executor, fresh_id).tag.as_deref(),
        Some("from-default")
    );
}

/// A staged NULL is a change, so it must show up in change tracking — code
/// that skips a save when `!is_dirty()` would otherwise drop the clear.
#[test]
fn staged_null_counts_as_dirty() {
    let mut record = WidgetRecord::new();
    assert!(!record.is_dirty(), "empty record is clean");

    record.set_note(None);
    assert!(record.is_dirty(), "staging a NULL is a change");
    assert!(record.dirty_fields().iter().any(|f| f == "note"));
    assert_eq!(record.null_columns(), vec![<Entity as LifeModelTrait>::Column::Note]);
}

// ─────────────────── negative: NULL is NOT written ───────────────────────────

/// The contract that must survive the fix: a field you never touch is left
/// alone. If the fix over-reached, every unset `Option` would be nulled and
/// partial updates would destroy data.
#[test]
fn untouched_column_is_left_alone() {
    let _guard = lock();
    let ctx = get_test_context();
    let mut db = TestDatabase::with_url(&ctx.pg_url);
    let executor = db.executor().expect("executor");
    setup(&executor).expect("setup");

    let id = seed(&executor);
    let mut record = WidgetRecord::new();
    // Touch only `count`; every other nullable column is unset.
    record.set_id(id).set_count(Some(7));
    record.update(&executor).expect("update");

    let model = fetch(&executor, id);
    assert_eq!(model.count, Some(7));
    assert_eq!(
        model.note.as_deref(),
        Some("original note"),
        "an unset Option must NOT be written as NULL"
    );
    assert_eq!(model.tag.as_deref(), Some("original tag"));
    assert!(model.owner_id.is_some());
    assert!(model.expires_at.is_some());
}

/// Last write wins: staging a NULL and then a value must store the value.
#[test]
fn value_after_null_wins() {
    let _guard = lock();
    let ctx = get_test_context();
    let mut db = TestDatabase::with_url(&ctx.pg_url);
    let executor = db.executor().expect("executor");
    setup(&executor).expect("setup");

    let id = seed(&executor);
    let mut record = WidgetRecord::new();
    record
        .set_id(id)
        .set_note(None)
        .set_note(Some("replaced".to_string()));
    assert!(
        record.null_columns().is_empty(),
        "assigning a value must un-stage the NULL"
    );
    record.update(&executor).expect("update");

    assert_eq!(note_of(&executor, id).as_deref(), Some("replaced"));
}

/// `from_model` seeds values without staging writes.
///
/// Every column arrives `Unchanged`, so an update from a loaded row emits a
/// *minimal* statement: the columns you set, and nothing else. A row read,
/// lightly edited and written back therefore cannot clobber a concurrent
/// edit to a column this caller never looked at.
#[test]
fn from_model_stages_nothing_and_updates_are_minimal() {
    let _guard = lock();
    let ctx = get_test_context();
    let mut db = TestDatabase::with_url(&ctx.pg_url);
    let executor = db.executor().expect("executor");
    setup(&executor).expect("setup");

    let id = seed(&executor);
    let model = fetch(&executor, id);

    let mut record = WidgetRecord::from_model(&model);
    assert_eq!(
        record.dirty_fields().len(),
        0,
        "a loaded row is not a pending write"
    );
    assert!(record.null_columns().is_empty());

    // Simulate a concurrent writer touching a column we are not editing.
    executor
        .execute(
            "UPDATE lg_nullable_update SET note = 'changed by someone else' WHERE id = $1",
            &[&id as &(dyn may_postgres::types::ToSql + Sync)],
        )
        .expect("concurrent update");

    record.set_count(Some(99));
    assert_eq!(record.dirty_fields(), vec!["count".to_string()]);
    record.update(&executor).expect("update");

    let after = fetch(&executor, id);
    assert_eq!(after.count, Some(99), "our edit landed");
    assert_eq!(
        after.note.as_deref(),
        Some("changed by someone else"),
        "a column we never set must not be rewritten from our stale snapshot"
    );

    // Clearing from a loaded row still works — it just has to be asked for.
    let mut record = WidgetRecord::from_model(&after);
    record.set_note(None);
    record.update(&executor).expect("update");
    assert_eq!(note_of(&executor, id), None);
}

/// `take()` removes a column from the change-set entirely; it is not a way to
/// stage a NULL.
#[test]
fn take_unstages_a_null() {
    let _guard = lock();
    let ctx = get_test_context();
    let mut db = TestDatabase::with_url(&ctx.pg_url);
    let executor = db.executor().expect("executor");
    setup(&executor).expect("setup");

    let id = seed(&executor);
    let mut record = WidgetRecord::new();
    record.set_id(id).set_note_null();
    let _ = record.take(<Entity as LifeModelTrait>::Column::Note);
    assert!(record.null_columns().is_empty());
    record.set_count(Some(1));
    record.update(&executor).expect("update");

    assert_eq!(
        note_of(&executor, id).as_deref(),
        Some("original note"),
        "a taken column must not be written at all"
    );
}

/// `reset()` clears staged NULLs along with everything else.
#[test]
fn reset_clears_staged_nulls() {
    let _guard = lock();
    let ctx = get_test_context();
    let mut db = TestDatabase::with_url(&ctx.pg_url);
    let executor = db.executor().expect("executor");
    setup(&executor).expect("setup");

    let id = seed(&executor);
    let mut record = WidgetRecord::new();
    record.set_id(id).set_note_null();
    record.reset();
    assert!(record.null_columns().is_empty());
    assert!(!record.is_dirty());

    record.set_id(id).set_count(Some(3));
    record.update(&executor).expect("update");
    assert_eq!(
        note_of(&executor, id).as_deref(),
        Some("original note"),
        "a reset NULL must not be resurrected"
    );
}

/// An `F`-style expression and a staged NULL are mutually exclusive; the last
/// one set wins, in both directions.
#[test]
fn expression_and_null_are_mutually_exclusive() {
    let _guard = lock();
    let ctx = get_test_context();
    let mut db = TestDatabase::with_url(&ctx.pg_url);
    let executor = db.executor().expect("executor");
    setup(&executor).expect("setup");

    // NULL then expression → the expression runs.
    let id = seed(&executor);
    let mut record = WidgetRecord::new();
    record.set_id(id).set_count_null();
    record.set_count_expr(<Entity as LifeModelTrait>::Column::Count.f_add(1));
    assert!(record.null_columns().is_empty(), "expression clears the NULL");
    record.update(&executor).expect("update");
    assert_eq!(fetch(&executor, id).count, Some(42), "41 + 1");

    // Expression then NULL → the NULL wins.
    let id = seed(&executor);
    let mut record = WidgetRecord::new();
    record.set_id(id);
    record.set_count_expr(<Entity as LifeModelTrait>::Column::Count.f_add(1));
    record.set_count_null();
    record.update(&executor).expect("update");
    assert_eq!(fetch(&executor, id).count, None, "NULL clears the expression");
}
