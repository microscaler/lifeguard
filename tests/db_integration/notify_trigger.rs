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
    let sql = generate_create_table_sql::<notify_event::Entity>(
        notify_event::Entity::table_definition(),
    )
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

    assert_eq!(rows.len(), 1, "exactly one notify trigger should be installed");
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

// NOTE: there is deliberately no test asserting a LISTENer receives the payload.
// `may_postgres` discards notifications — connection.rs matches
// `BackendMessage::Async(Message::NotificationResponse(_body)) => {}` and drops
// the body on the floor — so no consumer built on the current driver can
// observe a NOTIFY at all. Emitting is testable here; delivery is not, until
// the driver is taught to surface notifications. That gap is what stands
// between this spike and the LifeReflector cache-coherence design.
