//! Type mapping utilities for converting column type strings to `SeaQuery` `ColumnDef`.
//!
//! This module provides the type mapping logic that converts string-based column type
//! definitions (e.g., "Integer", "String") into `SeaQuery`'s `ColumnDef` types.

use sea_query::ColumnDef;

/// Map a column type string to `SeaQuery` `ColumnDef` type
///
/// This function handles the conversion from string-based type names to `SeaQuery`'s
/// column type methods. It's used by `ColumnDefinition::to_column_def()`.
///
/// # Arguments
///
/// * `col_type` - The column type string (e.g., "Integer", "String", "Json")
/// * `def` - The `ColumnDef` to configure
///
/// # Type Mapping
///
/// Maps column type strings to `SeaQuery` column types:
/// - "Integer" / "i32" / "i64" → `.integer()` or `.big_integer()`
/// - "String" / "Text" → `.string()` or `.text()`
/// - "Boolean" / "bool" → `.boolean()`
/// - "Float" / "f32" → `.float()`
/// - "Double" / "f64" → `.double()`
/// - "Json" / "Jsonb" → `.json()`
/// - `"Timestamp"` / `"DateTime"` → `.timestamp()`
/// - "Date" → `.date()`
/// - "Time" → `.time()`
/// - "Uuid" → `.uuid()`
/// - "Binary" / "Bytes" → `.binary()`
pub(crate) fn apply_column_type(col_type: &str, def: &mut ColumnDef) {
    let col_type_lower = col_type.to_lowercase();
    match col_type_lower.as_str() {
        "integer" | "i32" | "int" => {
            def.integer();
        }
        "bigint" | "i64" | "big_integer" => {
            def.big_integer();
        }
        "smallint" | "i16" => {
            def.small_integer();
        }
        "tinyint" | "i8" => {
            def.tiny_integer();
        }
        "string" | "text" | "varchar" => {
            def.text();
        }
        "char" => {
            def.char(); // Fixed-length character type
        }
        "boolean" | "bool" => {
            def.boolean();
        }
        "float" | "f32" | "real" => {
            def.float();
        }
        "double" | "f64" | "double_precision" => {
            def.double();
        }
        "json" | "jsonb" => {
            def.json();
        }
        "timestamp" | "datetime" | "timestamptz" => {
            def.timestamp();
        }
        "date" => {
            def.date();
        }
        "time" | "timetz" => {
            def.time();
        }
        "uuid" => {
            def.uuid();
        }
        "binary" | "bytes" | "bytea" | "blob" => {
            def.binary();
        }
        "decimal" | "numeric" => {
            def.decimal_len(10, 2); // Default precision/scale, can be overridden
        }
        _ => {
            // Unknown type, default to text
            def.text();
        }
    }
}
