//! A table created from the generated migration SQL decodes back into its model. The column type
//! inferred for each narrow or unsigned integer field (`i8`, `i16` and `u8` SMALLINT, `u16`
//! INTEGER, `u32` and `u64` BIGINT, plain and `Option`) is the type the derived `FromRow` reads,
//! so the round trip works, and a stored value outside the field's range is a row error.

use lifeguard::FromRow;
use lifeguard_migrate::sql_generator::generate_create_table_sql;

pub mod integer_round_trip {
    use lifeguard_derive::LifeModel;

    #[derive(LifeModel)]
    #[table_name = "lg_integer_round_trip"]
    pub struct IntegerRoundTrip {
        #[primary_key]
        pub id: i32,
        pub tiny: i8,
        pub short: i16,
        pub small: u8,
        pub medium: u16,
        pub large: u32,
        pub huge: u64,
        pub opt_tiny: Option<i8>,
        pub opt_short: Option<i16>,
        pub opt_small: Option<u8>,
        pub opt_medium: Option<u16>,
        pub opt_large: Option<u32>,
        pub opt_huge: Option<u64>,
    }
}

use integer_round_trip::{Entity, IntegerRoundTripModel};

const COLUMNS: &str = "id, tiny, short, small, medium, large, huge, \
                       opt_tiny, opt_short, opt_small, opt_medium, opt_large, opt_huge";

fn postgres_url() -> Option<String> {
    std::env::var("TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .or_else(|_| std::env::var("LIFEGUARD_DATABASE_URL"))
        .ok()
        .filter(|s| !s.trim().is_empty())
}

fn generated_sql() -> String {
    generate_create_table_sql::<Entity>(Entity::table_definition()).expect("generate SQL")
}

#[test]
fn generated_migration_uses_the_column_types_from_row_reads() {
    let sql = generated_sql();
    for (col, ty) in [
        ("tiny", "SMALLINT"),
        ("short", "SMALLINT"),
        ("small", "SMALLINT"),
        ("medium", "INTEGER"),
        ("large", "BIGINT"),
        ("huge", "BIGINT"),
        ("opt_tiny", "SMALLINT"),
        ("opt_short", "SMALLINT"),
        ("opt_small", "SMALLINT"),
        ("opt_medium", "INTEGER"),
        ("opt_large", "BIGINT"),
        ("opt_huge", "BIGINT"),
    ] {
        assert!(
            sql.contains(&format!("    {col} {ty}")),
            "{col} {ty} in:\n{sql}"
        );
    }
}

#[test]
fn migration_created_table_round_trips_through_from_row() {
    let Some(url) = postgres_url() else {
        eprintln!(
            "migration_created_table_round_trips_through_from_row: skipped (set TEST_DATABASE_URL, DATABASE_URL, or LIFEGUARD_DATABASE_URL)"
        );
        return;
    };
    let schema = format!("lg_int_{}", uuid::Uuid::new_v4().simple());
    let client = may_postgres::connect(&url).expect("connect");
    client
        .batch_execute(&format!(
            "CREATE SCHEMA {schema}; SET search_path TO {schema};"
        ))
        .expect("scratch schema");
    // Row 1: zeros and NULLs. Row 2: each field's maximum (BIGINT tops out at i64::MAX, the
    // largest storable u64). Row 3: each signed field's minimum. Rows 4 and 5: an i8 column
    // holding a value outside -128..=127, plain and Option.
    let insert = format!(
        "INSERT INTO lg_integer_round_trip ({COLUMNS}) VALUES
           (1, 0, 0, 0, 0, 0, 0, NULL, NULL, NULL, NULL, NULL, NULL),
           (2, 127, 32767, 255, 65535, 4294967295, 9223372036854775807,
               127, 32767, 255, 65535, 4294967295, 9223372036854775807),
           (3, -128, -32768, 0, 0, 0, 0, -128, -32768, 0, 0, 0, 0),
           (4, 128, 0, 0, 0, 0, 0, NULL, NULL, NULL, NULL, NULL, NULL),
           (5, 0, 0, 0, 0, 0, 0, -129, NULL, NULL, NULL, NULL, NULL)"
    );
    let applied = client
        .batch_execute(&generated_sql())
        .and_then(|()| client.batch_execute(&insert));
    let rows =
        applied.and_then(|()| client.query("SELECT * FROM lg_integer_round_trip ORDER BY id", &[]));
    client
        .batch_execute(&format!("DROP SCHEMA {schema} CASCADE"))
        .ok();
    let rows = rows.expect("apply the generated migration, insert and select");
    let decoded: Vec<_> = rows.iter().map(IntegerRoundTripModel::from_row).collect();
    let mut it = decoded.into_iter();
    let zero = it.next().expect("row 1").expect("decode row 1");
    let max = it.next().expect("row 2").expect("decode row 2");
    let min = it.next().expect("row 3").expect("decode row 3");
    assert!(
        it.next().expect("row 4").is_err(),
        "128 must not decode into i8"
    );
    assert!(
        it.next().expect("row 5").is_err(),
        "-129 must not decode into Option<i8>"
    );

    assert_eq!(
        (
            zero.tiny,
            zero.short,
            zero.small,
            zero.medium,
            zero.large,
            zero.huge
        ),
        (0, 0, 0, 0, 0, 0)
    );
    assert_eq!(
        (zero.opt_tiny, zero.opt_short, zero.opt_small),
        (None, None, None)
    );
    assert_eq!(
        (zero.opt_medium, zero.opt_large, zero.opt_huge),
        (None, None, None)
    );
    let huge = u64::try_from(i64::MAX).expect("fits");
    assert_eq!(
        (max.tiny, max.short, max.small, max.medium, max.large, max.huge),
        (i8::MAX, i16::MAX, u8::MAX, u16::MAX, u32::MAX, huge)
    );
    assert_eq!(
        (max.opt_tiny, max.opt_short, max.opt_small),
        (Some(i8::MAX), Some(i16::MAX), Some(u8::MAX))
    );
    assert_eq!(
        (max.opt_medium, max.opt_large, max.opt_huge),
        (Some(u16::MAX), Some(u32::MAX), Some(huge))
    );
    assert_eq!(
        (min.tiny, min.short, min.opt_tiny, min.opt_short),
        (i8::MIN, i16::MIN, Some(i8::MIN), Some(i16::MIN))
    );
}
