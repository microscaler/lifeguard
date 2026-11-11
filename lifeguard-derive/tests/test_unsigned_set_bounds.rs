//! `ModelTrait::set` on unsigned fields converts with a checked
//! `try_from`: values at the bounds of the target type are stored exactly,
//! and a negative or out-of-range value is an error instead of a silent
//! wrap-around.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use lifeguard::ModelTrait;
use lifeguard_derive::LifeModel;
use sea_query::Value;

#[derive(LifeModel)]
#[table_name = "unsigned_bounds"]
// The generated FromRow does not map Option<unsigned> columns yet; this test
// only exercises ModelTrait::set, so skip it.
#[skip_from_row]
pub struct Counters {
    #[primary_key]
    pub id: i32,
    pub small: u8,
    pub medium: u16,
    pub large: u32,
    pub huge: u64,
    pub opt_small: Option<u8>,
    pub opt_medium: Option<u16>,
    pub opt_large: Option<u32>,
    pub opt_huge: Option<u64>,
}

fn model() -> CountersModel {
    CountersModel {
        id: 1,
        small: 7,
        medium: 7,
        large: 7,
        huge: 7,
        opt_small: Some(7),
        opt_medium: Some(7),
        opt_large: Some(7),
        opt_huge: Some(7),
    }
}

/// Setting `v` must fail and leave the model unchanged.
fn rejects(m: &mut CountersModel, col: Column, v: Value) {
    let before = m.get(col);
    let err = m
        .set(col, v.clone())
        .expect_err(&format!("{col:?} must reject {v:?}"));
    assert!(err.to_string().contains("range 0..="), "{err}");
    assert_eq!(m.get(col), before, "{col:?} changed on a rejected set");
}

#[test]
fn u8_bounds() {
    let mut m = model();
    m.set(Column::Small, Value::SmallInt(Some(0))).unwrap();
    assert_eq!(m.small, 0);
    m.set(Column::Small, Value::SmallInt(Some(255))).unwrap();
    assert_eq!(m.small, 255);
    rejects(&mut m, Column::Small, Value::SmallInt(Some(256)));
    rejects(&mut m, Column::Small, Value::SmallInt(Some(-1)));
    rejects(&mut m, Column::Small, Value::SmallInt(Some(i16::MIN)));
}

#[test]
fn u16_bounds() {
    let mut m = model();
    m.set(Column::Medium, Value::Int(Some(0))).unwrap();
    assert_eq!(m.medium, 0);
    m.set(Column::Medium, Value::Int(Some(65_535))).unwrap();
    assert_eq!(m.medium, u16::MAX);
    rejects(&mut m, Column::Medium, Value::Int(Some(65_536)));
    rejects(&mut m, Column::Medium, Value::Int(Some(-1)));
}

#[test]
fn u32_bounds() {
    let mut m = model();
    m.set(Column::Large, Value::BigInt(Some(0))).unwrap();
    assert_eq!(m.large, 0);
    m.set(Column::Large, Value::BigInt(Some(i64::from(u32::MAX))))
        .unwrap();
    assert_eq!(m.large, u32::MAX);
    rejects(
        &mut m,
        Column::Large,
        Value::BigInt(Some(i64::from(u32::MAX) + 1)),
    );
    rejects(&mut m, Column::Large, Value::BigInt(Some(-1)));
}

#[test]
fn u64_bounds() {
    let mut m = model();
    m.set(Column::Huge, Value::BigInt(Some(0))).unwrap();
    assert_eq!(m.huge, 0);
    m.set(Column::Huge, Value::BigInt(Some(i64::MAX))).unwrap();
    assert_eq!(m.huge, i64::MAX as u64);
    // Previously these wrapped to 18446744073709551615 and 9223372036854775808.
    rejects(&mut m, Column::Huge, Value::BigInt(Some(-1)));
    rejects(&mut m, Column::Huge, Value::BigInt(Some(i64::MIN)));
}

#[test]
fn optional_unsigned_bounds() {
    let mut m = model();
    m.set(Column::OptSmall, Value::SmallInt(Some(255))).unwrap();
    assert_eq!(m.opt_small, Some(255));
    rejects(&mut m, Column::OptSmall, Value::SmallInt(Some(256)));
    rejects(&mut m, Column::OptSmall, Value::SmallInt(Some(-1)));

    m.set(Column::OptMedium, Value::Int(Some(65_535))).unwrap();
    assert_eq!(m.opt_medium, Some(u16::MAX));
    rejects(&mut m, Column::OptMedium, Value::Int(Some(65_536)));
    rejects(&mut m, Column::OptMedium, Value::Int(Some(-1)));

    m.set(Column::OptLarge, Value::BigInt(Some(i64::from(u32::MAX))))
        .unwrap();
    assert_eq!(m.opt_large, Some(u32::MAX));
    rejects(
        &mut m,
        Column::OptLarge,
        Value::BigInt(Some(i64::from(u32::MAX) + 1)),
    );
    rejects(&mut m, Column::OptLarge, Value::BigInt(Some(-1)));

    m.set(Column::OptHuge, Value::BigInt(Some(i64::MAX)))
        .unwrap();
    assert_eq!(m.opt_huge, Some(i64::MAX as u64));
    m.set(Column::OptHuge, Value::BigInt(Some(0))).unwrap();
    assert_eq!(m.opt_huge, Some(0));
    rejects(&mut m, Column::OptHuge, Value::BigInt(Some(-1)));

    // NULL still clears an optional field.
    m.set(Column::OptHuge, Value::BigInt(None)).unwrap();
    assert_eq!(m.opt_huge, None);
}
