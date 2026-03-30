//! Idle liveness probes on pool workers (PRD R4.2): `SELECT 1` while no work is queued.

use crate::context::get_test_context;
use lifeguard::{LifeExecutor, LifeguardPool, LifeguardPoolSettings, PooledLifeExecutor};
use sea_query::Values;
use std::sync::Arc;
use std::time::Duration;

#[test]
fn pooled_idle_liveness_probe_keeps_connection_usable() {
    let ctx = get_test_context();
    let url = ctx.pg_url.clone();
    let settings = LifeguardPoolSettings {
        idle_liveness_interval: Some(Duration::from_millis(400)),
        ..LifeguardPoolSettings::default()
    };
    let pool = Arc::new(
        LifeguardPool::new_with_settings(&url, 1, vec![], 0, &settings)
            .expect("LifeguardPool::new_with_settings primary-only"),
    );
    let ex = PooledLifeExecutor::new(pool);
    ex.query_one_values("SELECT 1", &Values(Vec::new()))
        .expect("first query");
    std::thread::sleep(Duration::from_millis(900));
    ex.query_one_values("SELECT 1", &Values(Vec::new()))
        .expect("second query after idle window (probe should have run)");
}
