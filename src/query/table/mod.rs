//! Table definition metadata for entity-driven migrations.

pub mod definition;

pub use definition::{
    format_index_key_list_derive_value, format_index_key_list_sql,
    index_definition_to_derive_index_value, index_key_parts_coverage_columns, IndexBtreeNulls,
    IndexBtreeSort, IndexDefinition, IndexKeyPart, TableDefinition,
};
