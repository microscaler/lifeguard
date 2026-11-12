//! How Rust integer fields are stored in Postgres.
//!
//! Postgres has no unsigned or one-byte integer types, so `i8` and the unsigned types are stored
//! in the smallest signed column type that holds their whole range, read back as that signed
//! Rust type and converted with a checked `try_from`. This one table answers the question
//! everywhere in Lifeguard: the `LifeModel` column-type inference that generated migrations use,
//! the derived `FromRow`, `ColumnDefinition::from_rust_type`, and `lifeguard-codegen`. Because
//! they share it, a table created from a generated migration always decodes back into its model.
//!
//! `u64` is stored as `BIGINT`, whose maximum is `i64::MAX`: larger `u64` values cannot be
//! stored, and binding one as a query parameter is an error.

#![forbid(unsafe_code)]

/// How one Rust integer type is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntegerStorage {
    /// The field's Rust type.
    pub rust: &'static str,
    /// The Rust type the column decodes as (`i16`, `i32` or `i64`).
    pub read_as: &'static str,
    /// The Postgres column type: `SMALLINT`, `INTEGER` or `BIGINT`.
    pub sql: &'static str,
}

impl IntegerStorage {
    /// Whether the field is read as a different type and converted with `try_from`
    /// (`i8` and the unsigned types).
    #[must_use]
    pub fn converts(&self) -> bool {
        self.rust != self.read_as
    }
}

/// Every integer type Lifeguard maps.
pub const INTEGER_STORAGE: [IntegerStorage; 8] = [
    IntegerStorage {
        rust: "i8",
        read_as: "i16",
        sql: "SMALLINT",
    },
    IntegerStorage {
        rust: "i16",
        read_as: "i16",
        sql: "SMALLINT",
    },
    IntegerStorage {
        rust: "i32",
        read_as: "i32",
        sql: "INTEGER",
    },
    IntegerStorage {
        rust: "i64",
        read_as: "i64",
        sql: "BIGINT",
    },
    IntegerStorage {
        rust: "u8",
        read_as: "i16",
        sql: "SMALLINT",
    },
    IntegerStorage {
        rust: "u16",
        read_as: "i32",
        sql: "INTEGER",
    },
    IntegerStorage {
        rust: "u32",
        read_as: "i64",
        sql: "BIGINT",
    },
    IntegerStorage {
        rust: "u64",
        read_as: "i64",
        sql: "BIGINT",
    },
];

/// Storage for the integer type named exactly `rust_type` (`i8`, `i16`, `i32`, `i64`, `u8`,
/// `u16`, `u32` or `u64`). `None` for anything else, including `Option<...>`, `u128` and names
/// such as `u8x4`.
#[must_use]
pub fn integer_storage(rust_type: &str) -> Option<IntegerStorage> {
    INTEGER_STORAGE
        .iter()
        .copied()
        .find(|s| s.rust == rust_type)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Postgres type each read type decodes from.
    fn sql_for_read_as(read_as: &str) -> &'static str {
        match read_as {
            "i16" => "SMALLINT",
            "i32" => "INTEGER",
            "i64" => "BIGINT",
            _ => "",
        }
    }

    /// (min, max) of each type, widened to i128.
    fn range(t: &str) -> (i128, i128) {
        match t {
            "i8" => (i128::from(i8::MIN), i128::from(i8::MAX)),
            "i16" => (i128::from(i16::MIN), i128::from(i16::MAX)),
            "i32" => (i128::from(i32::MIN), i128::from(i32::MAX)),
            "i64" => (i128::from(i64::MIN), i128::from(i64::MAX)),
            "u8" => (0, i128::from(u8::MAX)),
            "u16" => (0, i128::from(u16::MAX)),
            "u32" => (0, i128::from(u32::MAX)),
            "u64" => (0, i128::from(u64::MAX)),
            _ => (0, -1),
        }
    }

    #[test]
    fn column_type_is_the_one_the_read_type_decodes_from() {
        for s in INTEGER_STORAGE {
            assert_eq!(s.sql, sql_for_read_as(s.read_as), "{s:?}");
        }
    }

    #[test]
    fn read_type_holds_the_whole_range_except_u64() {
        for s in INTEGER_STORAGE {
            let (lo, hi) = range(s.rust);
            let (rlo, rhi) = range(s.read_as);
            if s.rust == "u64" {
                // BIGINT cannot hold u64 values above i64::MAX; that limit is documented.
                assert!(hi > rhi);
            } else {
                assert!(rlo <= lo && hi <= rhi, "{s:?}");
            }
        }
    }

    #[test]
    fn only_i8_and_unsigned_types_convert() {
        let converting: Vec<&str> = INTEGER_STORAGE
            .iter()
            .filter(|s| s.converts())
            .map(|s| s.rust)
            .collect();
        assert_eq!(converting, ["i8", "u8", "u16", "u32", "u64"]);
    }

    #[test]
    fn lookup_is_exact() {
        assert_eq!(integer_storage("i8").map(|s| s.sql), Some("SMALLINT"));
        assert_eq!(integer_storage("i16").map(|s| s.sql), Some("SMALLINT"));
        assert_eq!(integer_storage("u8").map(|s| s.sql), Some("SMALLINT"));
        assert_eq!(integer_storage("u16").map(|s| s.sql), Some("INTEGER"));
        assert_eq!(integer_storage("u32").map(|s| s.sql), Some("BIGINT"));
        assert_eq!(integer_storage("u64").map(|s| s.sql), Some("BIGINT"));
        for other in ["Option<u8>", "u128", "i128", "u8x4", "usize", ""] {
            assert_eq!(integer_storage(other), None, "{other}");
        }
    }
}
