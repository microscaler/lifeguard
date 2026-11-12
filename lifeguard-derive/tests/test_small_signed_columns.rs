//! `i8` and `i16` fields are stored as `SMALLINT` (Postgres has no one-byte integer type), so
//! the column type a generated migration infers is the one `FromRow` reads.

use lifeguard_derive::LifeModel;

#[derive(LifeModel)]
#[table_name = "small_signed"]
pub struct SmallSigned {
    #[primary_key]
    pub id: i32,
    pub tiny: i8,
    pub short: i16,
    pub opt_tiny: Option<i8>,
    pub opt_short: Option<i16>,
}

#[test]
fn i8_and_i16_columns_are_smallint() {
    for (col, sql, nullable) in [
        (Column::Id, "INTEGER", false),
        (Column::Tiny, "SMALLINT", false),
        (Column::Short, "SMALLINT", false),
        (Column::OptTiny, "SMALLINT", true),
        (Column::OptShort, "SMALLINT", true),
    ] {
        let def = col.column_def();
        assert_eq!(def.column_type.as_deref(), Some(sql), "{col:?}");
        assert_eq!(def.nullable, nullable, "{col:?}");
    }
}
