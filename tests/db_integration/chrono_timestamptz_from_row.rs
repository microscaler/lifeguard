//! Integration tests: `timestamptz` ↔ `chrono::DateTime<Utc>` (`FromRow` + insert round-trip).
//!
//! PRD: `docs/COMPLETE_CHRONO_TASKS.md` iteration B (B6), test matrix T1/T2.
//!
//! Each test uses a **distinct table name** so parallel integration tests do not race on
//! `CREATE TABLE` (PostgreSQL registers a composite type per table name).

use chrono::{DateTime, TimeZone, Utc};
use lifeguard::executor::LifeError;
use lifeguard::query::column::column_trait::ColumnTrait;
use lifeguard::test_helpers::TestDatabase;
use lifeguard::{ActiveModelTrait, LifeExecutor, LifeModelTrait};
use lifeguard_derive::{LifeModel, LifeRecord};
use may_postgres::types::ToSql;

fn get_db() -> TestDatabase {
    let ctx = crate::context::get_test_context();
    TestDatabase::with_url(&ctx.pg_url)
}

pub mod chrono_ts_read {
    use super::*;

    #[derive(LifeModel, LifeRecord)]
    #[table_name = "lg_chrono_ts_read"]
    pub struct TsUtcRow {
        #[primary_key]
        pub id: i32,
        pub ts: DateTime<Utc>,
    }
}

pub mod chrono_ts_insert {
    use super::*;

    #[derive(LifeModel, LifeRecord)]
    #[table_name = "lg_chrono_ts_insert"]
    pub struct TsUtcRow {
        #[primary_key]
        pub id: i32,
        pub ts: DateTime<Utc>,
    }
}

pub mod chrono_ts_opt {
    use super::*;

    #[derive(LifeModel, LifeRecord)]
    #[table_name = "lg_chrono_ts_opt"]
    pub struct TsUtcOptRow {
        #[primary_key]
        pub id: i32,
        pub ts: Option<DateTime<Utc>>,
    }
}

fn setup_read(executor: &dyn LifeExecutor) -> Result<(), LifeError> {
    executor.execute(
        r"
        CREATE TABLE IF NOT EXISTS lg_chrono_ts_read (
            id INT PRIMARY KEY,
            ts TIMESTAMPTZ NOT NULL
        )
        ",
        &[],
    )?;
    Ok(())
}

fn setup_insert(executor: &dyn LifeExecutor) -> Result<(), LifeError> {
    executor.execute(
        r"
        CREATE TABLE IF NOT EXISTS lg_chrono_ts_insert (
            id INT PRIMARY KEY,
            ts TIMESTAMPTZ NOT NULL
        )
        ",
        &[],
    )?;
    Ok(())
}

fn setup_opt(executor: &dyn LifeExecutor) -> Result<(), LifeError> {
    executor.execute(
        r"
        CREATE TABLE IF NOT EXISTS lg_chrono_ts_opt (
            id INT PRIMARY KEY,
            ts TIMESTAMPTZ
        )
        ",
        &[],
    )?;
    Ok(())
}

fn cleanup_read(executor: &dyn LifeExecutor) -> Result<(), LifeError> {
    executor.execute("DELETE FROM lg_chrono_ts_read", &[])?;
    Ok(())
}

fn cleanup_insert(executor: &dyn LifeExecutor) -> Result<(), LifeError> {
    executor.execute("DELETE FROM lg_chrono_ts_insert", &[])?;
    Ok(())
}

fn cleanup_opt(executor: &dyn LifeExecutor) -> Result<(), LifeError> {
    executor.execute("DELETE FROM lg_chrono_ts_opt", &[])?;
    Ok(())
}

#[test]
fn timestamptz_select_into_datetime_utc_via_from_row() {
    let mut test_db = get_db();
    let executor = test_db.executor().expect("executor");
    setup_read(&executor).expect("setup");
    cleanup_read(&executor).expect("cleanup");

    let t = Utc.with_ymd_and_hms(2020, 6, 15, 14, 30, 45).unwrap();
    let params: [&dyn ToSql; 2] = [&1i32, &t];
    executor
        .execute(
            "INSERT INTO lg_chrono_ts_read (id, ts) VALUES ($1, $2)",
            &params,
        )
        .expect("insert");

    let found = chrono_ts_read::Entity::find()
        .filter(chrono_ts_read::Column::Id.eq(1))
        .find_one(&executor)
        .expect("find_one");
    let model = found.expect("row");
    assert_eq!(model.ts, t);
}

#[test]
fn timestamptz_insert_via_record_round_trip() {
    let mut test_db = get_db();
    let executor = test_db.executor().expect("executor");
    setup_insert(&executor).expect("setup");
    cleanup_insert(&executor).expect("cleanup");

    let t = Utc::now();
    let mut rec = chrono_ts_insert::TsUtcRowRecord::new();
    rec.set_id(42);
    rec.set_ts(t);
    let model = rec.insert(&executor).expect("insert");
    assert_eq!(model.id, 42);
    assert_eq!(model.ts, t);

    let found = chrono_ts_insert::Entity::find()
        .filter(chrono_ts_insert::Column::Id.eq(42))
        .find_one(&executor)
        .expect("find_one");
    assert_eq!(found.expect("row").ts, t);
}

#[test]
fn timestamptz_nullable_some_and_none() {
    let mut test_db = get_db();
    let executor = test_db.executor().expect("executor");
    setup_opt(&executor).expect("setup");
    cleanup_opt(&executor).expect("cleanup");

    let t = Utc.with_ymd_and_hms(2021, 3, 1, 0, 0, 0).unwrap();
    executor
        .execute(
            "INSERT INTO lg_chrono_ts_opt (id, ts) VALUES ($1, NULL)",
            &[&1i32],
        )
        .expect("insert null");
    let params: [&dyn ToSql; 2] = [&2i32, &t];
    executor
        .execute(
            "INSERT INTO lg_chrono_ts_opt (id, ts) VALUES ($1, $2)",
            &params,
        )
        .expect("insert some");

    let m1 = chrono_ts_opt::Entity::find()
        .filter(chrono_ts_opt::Column::Id.eq(1))
        .find_one(&executor)
        .expect("find 1")
        .expect("row 1");
    assert!(m1.ts.is_none());

    let m2 = chrono_ts_opt::Entity::find()
        .filter(chrono_ts_opt::Column::Id.eq(2))
        .find_one(&executor)
        .expect("find 2")
        .expect("row 2");
    assert_eq!(m2.ts, Some(t));
}
