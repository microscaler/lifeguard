//! Integration tests for [`lifeguard::FromRow`] on `serde_json::Value` (PRD: JSF panic safety P1.1).

use lifeguard::test_helpers::TestDatabase;
use lifeguard::FromRow;
use lifeguard::LifeExecutor;

fn get_db() -> TestDatabase {
    let ctx = crate::context::get_test_context();
    TestDatabase::with_url(&ctx.pg_url)
}

#[test]
fn json_value_from_row_accepts_valid_json_text() {
    let mut test_db = get_db();
    let executor = test_db.executor().expect("executor");
    let row = executor
        .query_one(r#"SELECT '{"x":1}'::text AS j"#, &[])
        .expect("query_one");
    let v = serde_json::Value::from_row(&row).expect("from_row");
    assert_eq!(v["x"], 1);
}

#[test]
fn json_value_from_row_rejects_invalid_json_text() {
    let mut test_db = get_db();
    let executor = test_db.executor().expect("executor");
    let row = executor
        .query_one("SELECT $1::text", &[&"not-json{{{"])
        .expect("query_one");
    assert!(
        serde_json::Value::from_row(&row).is_err(),
        "invalid JSON text must yield Err, not panic"
    );
}
