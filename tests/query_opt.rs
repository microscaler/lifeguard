//! `LifeExecutor::query_opt` (zero or one row) on every built-in executor, and how empty results
//! feed `lifeguard_query_errors_total`: an empty result is never a query error.
//!
//! Its own test binary (process) so no other test changes the error counter while this one reads it.

#![allow(dead_code)] // shared `context` helpers this binary does not use
#![allow(clippy::pedantic)]

mod context;

use lifeguard::{LifeExecutor, LifeguardPool, MayPostgresExecutor, PooledLifeExecutor};
use sea_query::Values;
use std::sync::Arc;

const NONE_SQL: &str = "SELECT 1::int4 WHERE false";
const ONE_SQL: &str = "SELECT 7::int4";
const MANY_SQL: &str = "SELECT generate_series(1, 3)::int4";

/// Sum of every `lifeguard_query_errors_total` series (all `pool_tier` labels).
fn query_errors_total() -> f64 {
    lifeguard::metrics::prometheus_scrape_text()
        .lines()
        .filter(|l| l.starts_with("lifeguard_query_errors_total"))
        .filter_map(|l| l.rsplit(' ').next()?.parse::<f64>().ok())
        .sum()
}

/// Zero, one and many rows through `query_opt`; `query_one` on an empty result. Returns how many
/// of these calls should have counted as query errors (only the many-rows one).
fn exercise(exec: &dyn LifeExecutor, label: &str) -> f64 {
    assert!(
        exec.query_opt(NONE_SQL, &[]).expect(label).is_none(),
        "{label}: no row is Ok(None)"
    );
    let row = exec.query_opt(ONE_SQL, &[]).expect(label).expect(label);
    assert_eq!(row.get::<_, i32>(0), 7, "{label}");
    assert!(
        exec.query_opt(MANY_SQL, &[]).is_err(),
        "{label}: more than one row is an error"
    );

    let Err(err) = exec.query_one(NONE_SQL, &[]) else {
        panic!("{label}: query_one on no row is still an error");
    };
    assert!(
        err.to_string().contains("no rows"),
        "{label}: query_one no-row error names it: {err}"
    );
    assert_eq!(
        exec.query_one(ONE_SQL, &[]).expect(label).get::<_, i32>(0),
        7,
        "{label}"
    );
    1.0
}

#[test]
fn query_opt_on_every_executor_and_empty_results_are_not_query_errors() {
    let ctx = context::get_test_context();
    let start = query_errors_total();
    let mut expected = 0.0;

    // Direct client.
    let direct = MayPostgresExecutor::new(may_postgres::connect(&ctx.pg_url).expect("connect"));
    expected += exercise(&direct, "direct");
    assert_eq!(query_errors_total() - start, expected, "direct");

    // Transaction.
    let tx = direct.begin().expect("begin");
    expected += exercise(&tx, "transaction");
    tx.rollback().expect("rollback");
    assert_eq!(query_errors_total() - start, expected, "transaction");

    // Pool workers (values path and the empty-params `&dyn ToSql` path).
    let pooled = PooledLifeExecutor::new(Arc::new(
        LifeguardPool::new(&ctx.pg_url, 1, Vec::new(), 0).expect("pool"),
    ));
    expected += exercise(&pooled, "pooled");
    assert!(pooled
        .query_opt_values(NONE_SQL, &Values(Vec::new()))
        .expect("pooled values")
        .is_none());
    assert_eq!(query_errors_total() - start, expected, "pooled");

    // A real database error is still counted.
    assert!(direct
        .query_opt("SELECT * FROM no_such_table_query_opt", &[])
        .is_err());
    assert_eq!(
        query_errors_total() - start,
        expected + 1.0,
        "real errors still count"
    );
}
