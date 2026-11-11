//! SPIKE: integration tests for `#[notify(...)]`.
//!
//! The point of the feature is that a row change emits a NOTIFY without anybody
//! hand-writing PL/pgSQL. So these tests do not stop at the generated SQL string —
//! they apply it to a real database, read `pg_trigger` to confirm what was
//! actually installed, and run all three operations so a malformed function body
//! raises. A trigger that compiles but is never installed, or is installed
//! against the wrong events, would satisfy any weaker check.
//!
//! The API is expected to change with use; these tests are the thing that will
//! tell us how.

use lifeguard::{test_helpers::TestDatabase, LifeExecutor, MayPostgresExecutor};
use lifeguard_derive::{LifeModel, LifeRecord};
use lifeguard_migrate::sql_generator::generate_create_table_sql;

/// These tests share one table and one NOTIFY channel, because both are fixed at
/// compile time by the `#[notify(...)]` attribute on the entity — there is no
/// per-test channel to hand out. Run concurrently they drop each other's table
/// and consume each other's notifications, so they take this lock and run one at
/// a time. The serialisation is between these tests only; the rest of the suite
/// is unaffected.
static NOTIFY_TESTS: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Acquire the lock, ignoring poisoning: a panic in one test should surface as
/// that test's failure, not as a cascade of misleading failures in the others.
fn serialised() -> std::sync::MutexGuard<'static, ()> {
    NOTIFY_TESTS.lock().unwrap_or_else(|e| e.into_inner())
}

fn get_db() -> TestDatabase {
    let ctx = crate::context::get_test_context();
    TestDatabase::with_url(&ctx.pg_url)
}

pub mod notify_event {
    use super::{LifeModel, LifeRecord};

    /// Mirrors `hauliage.notification_events`: the queue whose `pg_notify`
    /// wake-up currently lives inside a hand-written stored procedure, and is
    /// the use case this spike is designed against.
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_notify_events"]
    #[notify(channel = "test_lifeguard_events", on = "insert,update,delete")]
    pub struct NotifyEvent {
        #[primary_key]
        #[column_type = "UUID"]
        pub id: uuid::Uuid,

        #[column_type = "VARCHAR(64)"]
        pub status: String,
    }
}
pub use notify_event::NotifyEventRecord;

/// Generate the DDL from the entity and apply it — the whole path under test.
fn setup(executor: &MayPostgresExecutor) -> Result<(), String> {
    let sql = generate_create_table_sql::<notify_event::Entity>(
        notify_event::Entity::table_definition(),
    )?;

    // The generated DDL is several statements (table, function, drop trigger,
    // create trigger). A prepared statement cannot carry more than one command,
    // so this has to go through the simple-query path.
    let ctx = crate::context::get_test_context();
    let client = may_postgres::connect(&ctx.pg_url).map_err(|e| format!("connect: {e}"))?;

    client
        .batch_execute("DROP TABLE IF EXISTS test_notify_events CASCADE")
        .map_err(|e| format!("drop: {e}"))?;

    client
        .batch_execute(&sql)
        .map_err(|e| format!("apply generated DDL: {e}\n---\n{sql}"))?;

    let _ = executor;
    Ok(())
}

#[test]
fn generated_ddl_contains_both_the_function_and_the_trigger() {
    let sql =
        generate_create_table_sql::<notify_event::Entity>(notify_event::Entity::table_definition())
            .expect("generate");

    // Both halves must be generated. Emitting only the trigger would leave the
    // function as hand-written SQL, which is the drift this feature removes.
    assert!(
        sql.contains("CREATE OR REPLACE FUNCTION"),
        "trigger function should be generated:\n{sql}"
    );
    assert!(
        sql.contains("CREATE TRIGGER lifeguard_notify_test_notify_events"),
        "trigger should be generated:\n{sql}"
    );
    assert!(
        sql.contains("AFTER INSERT OR UPDATE OR DELETE"),
        "all three declared operations should be present:\n{sql}"
    );
    // Re-applying a migration must not fail on an existing trigger.
    assert!(
        sql.contains("DROP TRIGGER IF EXISTS"),
        "trigger creation should be idempotent:\n{sql}"
    );
}

#[test]
fn the_trigger_is_installed_and_fires_on_a_real_insert() {
    let _guard = serialised();

    let mut test_db = get_db();
    let _client = test_db.connect().expect("connect");
    let executor = test_db.executor().expect("executor");
    setup(&executor).expect("setup");

    // The trigger must exist, be enabled, and be bound to the operations the
    // entity declared. Reading pg_trigger rather than trusting the DDL string
    // means the assertion survives any future change in how the SQL is built.
    let rows = executor
        .query_all(
            "SELECT t.tgname, t.tgenabled, t.tgtype, p.proname
             FROM pg_trigger t
             JOIN pg_proc p ON p.oid = t.tgfoid
             JOIN pg_class c ON c.oid = t.tgrelid
             WHERE c.relname = 'test_notify_events' AND NOT t.tgisinternal",
            &[],
        )
        .expect("query pg_trigger");

    assert_eq!(
        rows.len(),
        1,
        "exactly one notify trigger should be installed"
    );
    let name: &str = rows[0].get(0);
    let enabled: i8 = rows[0].get(1);
    let tgtype: i16 = rows[0].get(2);
    let func: &str = rows[0].get(3);

    assert_eq!(name, "lifeguard_notify_test_notify_events");
    assert_eq!(func, "lifeguard_notify_test_notify_events");
    assert_eq!(enabled as u8 as char, 'O', "trigger should be enabled");

    // pg_trigger.tgtype bit flags: ROW=1<<0, INSERT=1<<2, DELETE=1<<3, UPDATE=1<<4.
    assert_eq!(tgtype & (1 << 0), 1, "should be a FOR EACH ROW trigger");
    assert_ne!(tgtype & (1 << 2), 0, "should fire on INSERT");
    assert_ne!(tgtype & (1 << 3), 0, "should fire on DELETE");
    assert_ne!(tgtype & (1 << 4), 0, "should fire on UPDATE");

    // Exercise all three operations. A malformed function body — a bad column
    // reference, or OLD used where only NEW exists — raises here and nowhere else.
    let id = uuid::Uuid::new_v4();
    executor
        .execute(
            "INSERT INTO test_notify_events (id, status) VALUES ($1, $2)",
            &[&id, &"PENDING".to_string()],
        )
        .expect("INSERT should fire the trigger without error");
    executor
        .execute(
            "UPDATE test_notify_events SET status = $2 WHERE id = $1",
            &[&id, &"SENT".to_string()],
        )
        .expect("UPDATE should fire the trigger without error");
    executor
        .execute("DELETE FROM test_notify_events WHERE id = $1", &[&id])
        .expect("DELETE should fire the trigger, reading the key from OLD");
}

/// End-to-end: a row change reaches a `LISTEN`ing connection.
///
/// This is the assertion the feature actually exists for — everything else here
/// only checks that the right SQL was installed. It could not be written when
/// `#[notify]` was first added, because the driver decoded `NotificationResponse`
/// and dropped it on the floor; `LISTEN` worked but nothing could ever observe a
/// payload. With that fixed upstream, the whole path is testable: entity
/// declaration -> generated trigger -> pg_notify -> listener.
#[test]
fn a_row_change_reaches_a_listening_connection() {
    let _guard = serialised();

    let mut test_db = get_db();
    let _client = test_db.connect().expect("connect");
    let executor = test_db.executor().expect("executor");
    setup(&executor).expect("setup");

    let ctx = crate::context::get_test_context();
    let listener = may_postgres::connect(&ctx.pg_url).expect("listener connection");
    listener
        .batch_execute("LISTEN test_lifeguard_events")
        .expect("listen");

    let id = uuid::Uuid::new_v4();
    executor
        .execute(
            "INSERT INTO test_notify_events (id, status) VALUES ($1, $2)",
            &[&id, &"PENDING".to_string()],
        )
        .expect("insert");

    let payload =
        wait_for_notification(&listener).expect("the INSERT should have produced a notification");

    // The payload shape is the contract both consumers rely on: cache coherence
    // needs the table and operation to invalidate the right key, a queue
    // consumer needs the row id.
    let parsed: serde_json::Value = serde_json::from_str(&payload)
        .unwrap_or_else(|e| panic!("payload not JSON: {e}: {payload}"));

    assert_eq!(parsed["table"], "test_notify_events");
    assert_eq!(parsed["op"], "INSERT");
    assert_eq!(parsed["id"], id.to_string());
    assert!(
        parsed["v"].as_i64().is_some_and(|v| v > 0),
        "payload carries a version stamp: {payload}"
    );
    assert!(
        parsed["ts"].as_i64().is_some_and(|t| t > 1_600_000_000_000),
        "payload carries a millisecond timestamp: {payload}"
    );
}

/// The operation is reported accurately, so a listener can tell an insert from
/// an update or a delete — which is what makes cache invalidation possible
/// rather than just "something changed".
#[test]
fn each_operation_is_reported_with_its_own_op_and_key() {
    let _guard = serialised();

    let mut test_db = get_db();
    let _client = test_db.connect().expect("connect");
    let executor = test_db.executor().expect("executor");
    setup(&executor).expect("setup");

    let ctx = crate::context::get_test_context();
    let listener = may_postgres::connect(&ctx.pg_url).expect("listener connection");
    listener
        .batch_execute("LISTEN test_lifeguard_events")
        .expect("listen");

    let id = uuid::Uuid::new_v4();
    executor
        .execute(
            "INSERT INTO test_notify_events (id, status) VALUES ($1, $2)",
            &[&id, &"PENDING".to_string()],
        )
        .expect("insert");
    executor
        .execute(
            "UPDATE test_notify_events SET status = $2 WHERE id = $1",
            &[&id, &"SENT".to_string()],
        )
        .expect("update");
    executor
        .execute("DELETE FROM test_notify_events WHERE id = $1", &[&id])
        .expect("delete");

    let mut ops = Vec::new();
    let mut versions = Vec::new();
    for _ in 0..3 {
        let payload = wait_for_notification(&listener).expect("three notifications expected");
        let parsed: serde_json::Value = serde_json::from_str(&payload).expect("json payload");
        // DELETE has no NEW row, so the key must come from OLD. Getting this
        // wrong would surface here as a null id.
        assert_eq!(
            parsed["id"],
            id.to_string(),
            "every operation should report the changed row's key: {payload}"
        );
        ops.push(parsed["op"].as_str().unwrap_or_default().to_string());
        versions.push(parsed["v"].as_i64().expect("version stamp"));
    }

    assert_eq!(ops, vec!["INSERT", "UPDATE", "DELETE"]);
    // Successive writes to one row carry increasing versions; consumers rely on
    // this to discard stale events for a key.
    assert!(
        versions.windows(2).all(|w| w[0] < w[1]),
        "versions must increase per key: {versions:?}"
    );
}

/// Poll for a notification, giving the connection round trips to surface it.
///
/// Notifications arrive asynchronously: the server sends them when it chooses
/// and the connection coroutine decodes them during I/O, so a test that read the
/// queue once would race.
fn wait_for_notification(client: &may_postgres::Client) -> Option<String> {
    use std::time::Duration;

    for _ in 0..50 {
        if let Some(notification) = client.notifications().pop() {
            return Some(notification.payload().to_string());
        }
        client.batch_execute("SELECT 1").ok()?;
        std::thread::sleep(Duration::from_millis(20));
    }
    None
}

// ---------------------------------------------------------------------------------------------
// granularity = "statement": one notification per statement, keys batched in `ids`.

pub mod notify_stmt {
    use super::{LifeModel, LifeRecord};

    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_notify_stmt"]
    #[notify(
        channel = "test_lifeguard_stmt",
        on = "insert,update,delete",
        granularity = "statement"
    )]
    pub struct NotifyStmt {
        #[primary_key]
        pub id: i64,
        pub qty: i32,
    }
}

fn setup_stmt() -> may_postgres::Client {
    let sql =
        generate_create_table_sql::<notify_stmt::Entity>(notify_stmt::Entity::table_definition())
            .expect("generate");
    assert!(sql.contains("FOR EACH STATEMENT"), "{sql}");
    let ctx = crate::context::get_test_context();
    let client = may_postgres::connect(&ctx.pg_url).expect("connect");
    client
        .batch_execute("DROP TABLE IF EXISTS test_notify_stmt CASCADE")
        .expect("drop");
    client
        .batch_execute(&sql)
        .unwrap_or_else(|e| panic!("apply generated DDL: {e}\n---\n{sql}"));
    client
}

/// Every notification that arrives within a short quiet period.
fn drain_notifications(client: &may_postgres::Client) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    let mut quiet = 0;
    while quiet < 10 {
        let mut got = false;
        while let Some(n) = client.notifications().pop() {
            out.push(serde_json::from_str(n.payload()).expect("payload is JSON"));
            got = true;
        }
        quiet = if got { 0 } else { quiet + 1 };
        client.batch_execute("SELECT 1").expect("poll");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    out
}

fn ids_of(notes: &[serde_json::Value], op: &str) -> Vec<i64> {
    let mut ids: Vec<i64> = notes
        .iter()
        .filter(|n| n["op"] == op)
        .flat_map(|n| n["ids"].as_array().cloned().unwrap_or_default())
        .map(|v| v.as_i64().expect("integer key"))
        .collect();
    ids.sort_unstable();
    ids
}

#[test]
fn statement_granularity_sends_one_notification_per_statement() {
    let _guard = serialised();
    let writer = setup_stmt();
    let ctx = crate::context::get_test_context();
    let listener = may_postgres::connect(&ctx.pg_url).expect("listener");
    listener
        .batch_execute("LISTEN test_lifeguard_stmt")
        .expect("listen");

    // 500 rows in one statement: one notification, all keys.
    writer
        .batch_execute(
            "INSERT INTO test_notify_stmt (id, qty) SELECT g, 0 FROM generate_series(1, 500) g",
        )
        .expect("insert");
    let notes = drain_notifications(&listener);
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert_eq!(notes[0]["table"], "test_notify_stmt");
    assert!(
        notes[0]["id"].is_null(),
        "statement payloads carry ids, not id"
    );
    assert!(notes[0]["v"].as_i64().is_some_and(|v| v > 0));
    assert!(notes[0]["ts"]
        .as_i64()
        .is_some_and(|t| t > 1_600_000_000_000));
    assert_eq!(ids_of(&notes, "INSERT"), (1..=500).collect::<Vec<_>>());

    // An UPDATE that changes keys reports the old and the new ones.
    writer
        .batch_execute("UPDATE test_notify_stmt SET id = id + 1000 WHERE id <= 2")
        .expect("update");
    let notes = drain_notifications(&listener);
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert_eq!(ids_of(&notes, "UPDATE"), vec![1, 2, 1001, 1002]);

    // A statement that touches nothing sends nothing.
    writer
        .batch_execute("DELETE FROM test_notify_stmt WHERE id < 0")
        .expect("empty delete");
    assert!(drain_notifications(&listener).is_empty());

    // Many keys: split into several notifications, each under 8000 bytes, none lost.
    writer
        .batch_execute(
            "INSERT INTO test_notify_stmt (id, qty) \
             SELECT g, 0 FROM generate_series(100000000000, 100000003999) g",
        )
        .expect("wide insert");
    let notes = drain_notifications(&listener);
    assert!(
        notes.len() > 1,
        "4000 13-digit keys need more than one payload"
    );
    assert!(notes.iter().all(|n| n.to_string().len() < 8000));
    assert_eq!(
        ids_of(&notes, "INSERT"),
        (100_000_000_000..100_000_004_000).collect::<Vec<_>>()
    );

    // DELETE reports the old keys.
    writer
        .batch_execute("DELETE FROM test_notify_stmt WHERE id BETWEEN 3 AND 5")
        .expect("delete");
    assert_eq!(
        ids_of(&drain_notifications(&listener), "DELETE"),
        vec![3, 4, 5]
    );

    // Installed as three statement-level triggers, no row trigger.
    let n: i64 = writer
        .query_one(
            "SELECT count(*) FROM pg_trigger t JOIN pg_class c ON c.oid = t.tgrelid \
             WHERE c.relname = 'test_notify_stmt' AND NOT t.tgisinternal AND (t.tgtype & 1) = 0",
            &[],
        )
        .expect("pg_trigger")
        .get(0);
    assert_eq!(n, 3);
}
