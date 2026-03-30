//! Integration tests for the Coroutine Streaming and Cursor Pagination architectures.
//!
//! Validates `CursorPaginator` static slice boundaries and `SelectQueryStreamEx` receiver queues natively.

use lifeguard::query::traits::LifeModelTrait;
use lifeguard::query::SelectQueryStreamEx;
use lifeguard::ActiveModelTrait;
use lifeguard::{test_helpers::TestDatabase, LifeExecutor, MayPostgresExecutor};
use lifeguard_derive::{LifeModel, LifeRecord};
use sea_query::{Expr, ExprTrait, Order};

fn get_db() -> TestDatabase {
    let ctx = crate::context::get_test_context();
    TestDatabase::with_url(&ctx.pg_url)
}

#[derive(LifeModel, LifeRecord, Clone, Debug)]
#[table_name = "test_stream_cursors"]
#[cursor_tiebreak = "Id"]
pub struct DataPoint {
    #[primary_key]
    #[auto_increment]
    pub id: i32,
    pub name: String,
    pub val: i32,
}

fn setup_schema(executor: &MayPostgresExecutor) {
    executor
        .execute("DROP TABLE IF EXISTS test_stream_cursors CASCADE", &[])
        .unwrap();
    executor
        .execute(
            r"
        CREATE TABLE test_stream_cursors (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            val INTEGER NOT NULL
        )
        ",
            &[],
        )
        .unwrap();
}

#[test]
fn test_pagination_and_streaming() {
    let mut db = get_db();
    let _client = db.connect().expect("Failed to connect to testing database");
    let executor = db.executor().expect("Failed to extract executor interface");

    // Boot schema
    setup_schema(&executor);

    // Insert 10 ordered elements (id 1..10)
    for i in 1..=10 {
        let mut rec = DataPointRecord::new();
        rec.set_name(format!("Point {i}"));
        rec.set_val(i * 10);
        rec.save(&executor).expect("Database insertion crashed");
    }

    // ----------------------------------------------------------------------------------
    // PHASE 1: Verify `CursorPaginator` Keysets!
    // ----------------------------------------------------------------------------------
    // Objective: Fetch logically after ID 3, explicitly bounded by limit 4. (IDs: 4, 5, 6, 7)
    let cursor_results = Entity::find()
        .cursor_by("id")
        .after(3)
        .first(4)
        .fetch(&executor)
        .expect("Cursor query failed to compute structurally");

    assert_eq!(cursor_results.len(), 4, "Cursor slice bound improperly");
    assert_eq!(cursor_results[0].id, 4);
    assert_eq!(cursor_results[3].id, 7);

    // Fetch Last bounds natively (requires Desc sorting implicitly!)
    let backwards_results = Entity::find()
        .cursor_by("id")
        .before(9)
        .last(2)
        .fetch(&executor) // Should explicitly extract 8, 7! natively sorted DESC internally!
        .unwrap();

    // Wait, last(2) parses DESC, so it evaluates `WHERE id < 9 ORDER BY id DESC LIMIT 2`.
    // Records 8 and 7 should be returned!
    assert_eq!(backwards_results.len(), 2);
    assert_eq!(
        backwards_results[0].id, 8,
        "Expected backwards lookup resolving backwards natively limit slices"
    );
    assert_eq!(backwards_results[1].id, 7);

    // Duplicate `val` with distinct ids: keyset on `val` alone is ambiguous; (val, id) tie-break fixes it.
    for _ in 0..3 {
        let mut rec = DataPointRecord::new();
        rec.set_name("dup".into());
        rec.set_val(99);
        rec.save(&executor).expect("dup val insert");
    }
    let dupes: Vec<_> = Entity::find()
        .filter(Expr::col("val").eq(99))
        .order_by("id", Order::Asc)
        .all(&executor)
        .expect("list dup val rows");
    assert_eq!(dupes.len(), 3, "three rows share val=99");
    let mid_id = dupes[1].id;
    let tie = Entity::find()
        .cursor_by("val")
        .after(99)
        .after_pk(mid_id)
        .first(2)
        .fetch(&executor)
        .expect("tie-break cursor");
    assert_eq!(tie.len(), 2);
    assert_eq!(tie[0].id, dupes[2].id, "next same-val row after mid pk");
    assert_eq!(
        tie[1].id, 10,
        "then higher val from original seed (val=100)"
    );

    // ----------------------------------------------------------------------------------
    // PHASE 2: Verify Coroutine Channel Streaming!
    // ----------------------------------------------------------------------------------
    // Objective: Open an asynchronous transaction channel, streaming perfectly 3 packets until 10 row exhaustion!
    // Chunks should resolve dynamically as [3, 3, 3, 1] elements natively!
    let receiver = Entity::find().stream_all(&executor, 3);

    let mut chunk_counts = Vec::new();
    let mut total_records = 0;

    while let Ok(res) = receiver.recv() {
        let chunk = res.expect("Receiver yielded database error inside thread");
        assert!(
            !chunk.is_empty(),
            "Stream emitted an empty chunk instead of gracefully terminating!"
        );
        chunk_counts.push(chunk.len());
        total_records += chunk.len();
    }

    assert_eq!(
        total_records, 13,
        "Failed to stream the exact number of rows via coroutine loop closure"
    );
    assert_eq!(
        chunk_counts,
        vec![3, 3, 3, 3, 1],
        "Mismatched FETCH chunk alignment distributions inside transaction!"
    );
}
