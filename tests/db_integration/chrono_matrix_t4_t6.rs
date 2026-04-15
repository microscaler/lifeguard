//! PRD test matrix T4–T6 (`docs/COMPLETE_CHRONO_TASKS.md`):
//!
//! - **T4:** PostgreSQL `DATE` ↔ `chrono::NaiveDate` (insert + `FromRow`).
//! - **T5:** `TIMESTAMPTZ` ↔ `chrono::DateTime<chrono::Local>` round-trip.
//! - **T6:** Mixed `UUID` + `JSONB` + `DateTime<Utc>` + `SMALLINT` (`i16`) in one row (bind / typing).
//!
//! Each scenario uses a **distinct table name** so parallel `db_integration` tests do not race.

use chrono::{DateTime, Local, NaiveDate, TimeZone, Utc};
use lifeguard::executor::LifeError;
use lifeguard::query::column::column_trait::ColumnTrait;
use lifeguard::test_helpers::TestDatabase;
use lifeguard::{ActiveModelTrait, LifeExecutor, LifeModelTrait};
use lifeguard_derive::{LifeModel, LifeRecord};
use may_postgres::types::ToSql;
use serde_json::json;
use uuid::Uuid;

fn get_db() -> TestDatabase {
    let ctx = crate::context::get_test_context();
    TestDatabase::with_url(&ctx.pg_url)
}

// --- T4 --------------------------------------------------------------------

pub mod t4_date {
    use super::*;

    #[derive(LifeModel, LifeRecord)]
    #[table_name = "lg_chrono_t4_date"]
    pub struct DateRow {
        #[primary_key]
        pub id: i32,
        pub day: NaiveDate,
    }
}

fn setup_t4(executor: &dyn LifeExecutor) -> Result<(), LifeError> {
    executor.execute(
        r"
        CREATE TABLE IF NOT EXISTS lg_chrono_t4_date (
            id INT PRIMARY KEY,
            day DATE NOT NULL
        )
        ",
        &[],
    )?;
    Ok(())
}

fn cleanup_t4(executor: &dyn LifeExecutor) -> Result<(), LifeError> {
    executor.execute("DELETE FROM lg_chrono_t4_date", &[])?;
    Ok(())
}

#[test]
fn t4_date_naive_round_trip() {
    let mut test_db = get_db();
    let executor = test_db.executor().expect("executor");
    setup_t4(&executor).expect("setup");
    cleanup_t4(&executor).expect("cleanup");

    let day = NaiveDate::from_ymd_opt(2024, 7, 4).expect("date");
    let params: [&dyn ToSql; 2] = [&7i32, &day];
    executor
        .execute(
            "INSERT INTO lg_chrono_t4_date (id, day) VALUES ($1, $2)",
            &params,
        )
        .expect("insert raw");

    let found = t4_date::Entity::find()
        .filter(t4_date::Column::Id.eq(7))
        .find_one(&executor)
        .expect("find_one");
    assert_eq!(found.expect("row").day, day);

    let mut rec = t4_date::DateRowRecord::new();
    rec.set_id(8);
    rec.set_day(day);
    let model = rec.insert(&executor).expect("record insert");
    assert_eq!(model.day, day);

    let found2 = t4_date::Entity::find()
        .filter(t4_date::Column::Id.eq(8))
        .find_one(&executor)
        .expect("find 8");
    assert_eq!(found2.expect("row 8").day, day);
}

// --- T5 --------------------------------------------------------------------

pub mod t5_local {
    use super::*;

    #[derive(LifeModel, LifeRecord)]
    #[table_name = "lg_chrono_t5_local"]
    pub struct LocalTsRow {
        #[primary_key]
        pub id: i32,
        pub ts: DateTime<Local>,
    }
}

fn setup_t5(executor: &dyn LifeExecutor) -> Result<(), LifeError> {
    executor.execute(
        r"
        CREATE TABLE IF NOT EXISTS lg_chrono_t5_local (
            id INT PRIMARY KEY,
            ts TIMESTAMPTZ NOT NULL
        )
        ",
        &[],
    )?;
    Ok(())
}

fn cleanup_t5(executor: &dyn LifeExecutor) -> Result<(), LifeError> {
    executor.execute("DELETE FROM lg_chrono_t5_local", &[])?;
    Ok(())
}

#[test]
fn t5_timestamptz_datetime_local_round_trip() {
    let mut test_db = get_db();
    let executor = test_db.executor().expect("executor");
    setup_t5(&executor).expect("setup");
    cleanup_t5(&executor).expect("cleanup");

    let utc = Utc.with_ymd_and_hms(2022, 12, 31, 23, 59, 1).unwrap();
    let local: DateTime<Local> = utc.with_timezone(&Local);

    let mut rec = t5_local::LocalTsRowRecord::new();
    rec.set_id(100);
    rec.set_ts(local);
    let model = rec.insert(&executor).expect("insert");
    assert_eq!(model.ts, local);

    let found = t5_local::Entity::find()
        .filter(t5_local::Column::Id.eq(100))
        .find_one(&executor)
        .expect("find");
    assert_eq!(found.expect("row").ts, local);
}

// --- T6 --------------------------------------------------------------------

pub mod t6_mixed {
    use super::*;

    #[derive(LifeModel, LifeRecord)]
    #[table_name = "lg_chrono_t6_mixed"]
    pub struct MixedRow {
        #[primary_key]
        pub id: i32,
        pub lane: i16,
        pub uid: Uuid,
        pub meta: serde_json::Value,
        pub at: DateTime<Utc>,
    }
}

fn setup_t6(executor: &dyn LifeExecutor) -> Result<(), LifeError> {
    executor.execute(
        r"
        CREATE TABLE IF NOT EXISTS lg_chrono_t6_mixed (
            id INT PRIMARY KEY,
            lane SMALLINT NOT NULL,
            uid UUID NOT NULL,
            meta JSONB NOT NULL,
            at TIMESTAMPTZ NOT NULL
        )
        ",
        &[],
    )?;
    Ok(())
}

fn cleanup_t6(executor: &dyn LifeExecutor) -> Result<(), LifeError> {
    executor.execute("DELETE FROM lg_chrono_t6_mixed", &[])?;
    Ok(())
}

#[test]
fn t6_mixed_uuid_json_datetime_utc_i16_round_trip() {
    let mut test_db = get_db();
    let executor = test_db.executor().expect("executor");
    setup_t6(&executor).expect("setup");
    cleanup_t6(&executor).expect("cleanup");

    let uid = Uuid::nil();
    let meta = json!({ "n": 42, "s": "x" });
    let at = Utc.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();

    let mut rec = t6_mixed::MixedRowRecord::new();
    rec.set_id(1);
    rec.set_lane(-42);
    rec.set_uid(uid);
    rec.set_meta(meta.clone());
    rec.set_at(at);
    let model = rec.insert(&executor).expect("insert");
    assert_eq!(model.lane, -42);
    assert_eq!(model.uid, uid);
    assert_eq!(model.meta, meta);
    assert_eq!(model.at, at);

    let found = t6_mixed::Entity::find()
        .filter(t6_mixed::Column::Id.eq(1))
        .find_one(&executor)
        .expect("find");
    let row = found.expect("row");
    assert_eq!(row.lane, -42);
    assert_eq!(row.uid, uid);
    assert_eq!(row.meta, meta);
    assert_eq!(row.at, at);
}
