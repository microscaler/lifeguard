//! Table definition metadata for entity-driven migrations.
//!
//! This module provides `TableDefinition` which stores table-level metadata
//! including composite unique constraints, indexes, CHECK constraints, and table comments.

/// Btree key sort direction for index metadata (matches PostgreSQL `ASC` / `DESC`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IndexBtreeSort {
    #[default]
    Asc,
    Desc,
}

/// Btree `NULLS FIRST` / `NULLS LAST` for index metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexBtreeNulls {
    First,
    Last,
}

/// One btree index key segment (column reference or expression).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexKeyPart {
    /// Simple column key with optional collation, operator class, and ordering.
    Column {
        name: String,
        opclass: Option<String>,
        collate: Option<String>,
        sort: Option<IndexBtreeSort>,
        nulls: Option<IndexBtreeNulls>,
    },
    /// Functional / expression key; `coverage_columns` must list table columns for derive validation.
    Expression {
        sql: String,
        coverage_columns: Vec<String>,
        opclass: Option<String>,
        collate: Option<String>,
        sort: Option<IndexBtreeSort>,
        nulls: Option<IndexBtreeNulls>,
    },
}

impl IndexKeyPart {
    /// Column names this segment contributes for `#[require_index_coverage]` / SQL validation.
    pub fn coverage_column_names(&self) -> Vec<&str> {
        match self {
            IndexKeyPart::Column { name, .. } => vec![name.as_str()],
            IndexKeyPart::Expression {
                coverage_columns, ..
            } => coverage_columns.iter().map(String::as_str).collect(),
        }
    }
}

/// Append optional ` COLLATE …`, operator class, `ASC`/`DESC`, `NULLS …` after a leading SQL fragment.
fn push_index_key_suffixes(
    out: &mut String,
    collate: &Option<String>,
    opclass: &Option<String>,
    sort: Option<IndexBtreeSort>,
    nulls: Option<IndexBtreeNulls>,
) {
    if let Some(c) = collate {
        out.push_str(" COLLATE ");
        if c.chars().any(|ch| ch.is_whitespace() || ch == '"') || !is_simple_sql_ident(c) {
            out.push('"');
            out.push_str(&c.replace('"', "\"\""));
            out.push('"');
        } else {
            out.push_str(c);
        }
    }
    if let Some(o) = opclass {
        out.push(' ');
        out.push_str(o);
    }
    match sort {
        Some(IndexBtreeSort::Desc) => out.push_str(" DESC"),
        Some(IndexBtreeSort::Asc) => out.push_str(" ASC"),
        None => {}
    }
    match nulls {
        Some(IndexBtreeNulls::First) => out.push_str(" NULLS FIRST"),
        Some(IndexBtreeNulls::Last) => out.push_str(" NULLS LAST"),
        None => {}
    }
}

fn is_simple_sql_ident(s: &str) -> bool {
    let mut it = s.chars();
    let Some(first) = it.next() else {
        return false;
    };
    if !first.is_ascii_alphabetic() && first != '_' {
        return false;
    }
    it.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Format btree key list SQL (parentheses **not** included) from structured parts.
pub fn format_index_key_list_sql(parts: &[IndexKeyPart]) -> String {
    let mut out = String::new();
    for (i, p) in parts.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        match p {
            IndexKeyPart::Column {
                name,
                opclass,
                collate,
                sort,
                nulls,
            } => {
                out.push_str(name);
                push_index_key_suffixes(&mut out, collate, opclass, *sort, *nulls);
            }
            IndexKeyPart::Expression {
                sql,
                opclass,
                collate,
                sort,
                nulls,
                ..
            } => {
                out.push_str(sql);
                push_index_key_suffixes(&mut out, collate, opclass, *sort, *nulls);
            }
        }
    }
    out
}

/// Format key list for `#[index = "name(…)"]` — expression segments append ` | coverage_columns`.
pub fn format_index_key_list_derive_value(parts: &[IndexKeyPart]) -> String {
    let mut out = String::new();
    for (i, p) in parts.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        match p {
            IndexKeyPart::Column {
                name,
                opclass,
                collate,
                sort,
                nulls,
            } => {
                out.push_str(name);
                push_index_key_suffixes(&mut out, collate, opclass, *sort, *nulls);
            }
            IndexKeyPart::Expression {
                sql,
                coverage_columns,
                opclass,
                collate,
                sort,
                nulls,
            } => {
                out.push_str(sql);
                push_index_key_suffixes(&mut out, collate, opclass, *sort, *nulls);
                out.push_str(" | ");
                out.push_str(&coverage_columns.join(", "));
            }
        }
    }
    out
}

/// Ordered, deduped column names referenced by index key parts (for validation lists).
pub fn index_key_parts_coverage_columns(parts: &[IndexKeyPart]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for p in parts {
        for c in p.coverage_column_names() {
            if seen.insert(c.to_string()) {
                out.push(c.to_string());
            }
        }
    }
    out
}

/// Build the string value for `#[index = "..."]` from a fully specified [`IndexDefinition`].
///
/// Caller escapes quotes for Rust source. Omits `UNIQUE` / `INCLUDE` / `WHERE`; use for key-only
/// reconstruction or testing round-trips.
pub fn index_definition_to_derive_index_value(idx: &IndexDefinition) -> String {
    let inner = if !idx.key_parts.is_empty() {
        format_index_key_list_derive_value(&idx.key_parts)
    } else if let Some(ref k) = idx.key_list_sql {
        k.clone()
    } else {
        idx.columns.join(", ")
    };
    let mut s = format!("{}({inner})", idx.name);
    if !idx.include_columns.is_empty() {
        s.push_str(" INCLUDE (");
        s.push_str(&idx.include_columns.join(", "));
        s.push(')');
    }
    if let Some(ref w) = idx.partial_where {
        s.push_str(" WHERE ");
        s.push_str(w);
    }
    s
}

/// Table definition metadata
///
/// Stores information about table-level constraints, indexes, and metadata.
/// This is used for entity-driven migration generation.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TableDefinition {
    /// Table comment/documentation
    pub table_comment: Option<String>,
    /// Composite unique constraints (multi-column unique)
    /// Each entry is a vector of column names
    pub composite_unique: Vec<Vec<String>>,
    /// Index definitions (key columns, optional INCLUDE, unique, partial WHERE).
    pub indexes: Vec<IndexDefinition>,
    /// Table-level `CHECK` constraints
    /// Each entry is a tuple of (`constraint_name`, `expression`)
    /// If `constraint_name` is `None`, a default name will be generated from the table name
    pub check_constraints: Vec<(Option<String>, String)>,
    /// Whether this entity is a PostgreSQL VIEW instead of a BASE TABLE
    pub is_view: bool,
    /// The select query backing the view (used for schema generation)
    pub view_query: Option<String>,
}

/// Index definition metadata
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexDefinition {
    /// Index name
    pub name: String,
    /// Column names used for validation, `#[require_index_coverage]`, and deduping `#[indexed]`.
    ///
    /// When [`Self::key_parts`] is non-empty, this should match
    /// [`index_key_parts_coverage_columns`] (derive and infer-schema maintain that).
    pub columns: Vec<String>,
    /// Legacy verbatim btree key list when [`Self::key_parts`] is empty (e.g. `lower(email)`).
    pub key_list_sql: Option<String>,
    /// Structured btree key segments; when non-empty, SQL generation uses these instead of
    /// [`Self::key_list_sql`] / [`Self::columns`].
    pub key_parts: Vec<IndexKeyPart>,
    /// PostgreSQL **`INCLUDE`** payload columns (non-key columns stored in the index)
    pub include_columns: Vec<String>,
    /// Whether this is a unique index
    pub unique: bool,
    /// Partial index WHERE clause (if any)
    pub partial_where: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_key_parts_column_with_opclass_sort_nulls() {
        let parts = vec![IndexKeyPart::Column {
            name: "slug".into(),
            opclass: Some("text_pattern_ops".into()),
            collate: None,
            sort: Some(IndexBtreeSort::Asc),
            nulls: Some(IndexBtreeNulls::Last),
        }];
        assert_eq!(
            format_index_key_list_sql(&parts),
            "slug text_pattern_ops ASC NULLS LAST"
        );
    }

    #[test]
    fn format_key_parts_expression_with_coverage_separate() {
        let parts = vec![
            IndexKeyPart::Expression {
                sql: "lower(email)".into(),
                coverage_columns: vec!["email".into()],
                opclass: None,
                collate: None,
                sort: None,
                nulls: None,
            },
            IndexKeyPart::Column {
                name: "id".into(),
                opclass: None,
                collate: None,
                sort: Some(IndexBtreeSort::Desc),
                nulls: None,
            },
        ];
        assert_eq!(
            format_index_key_list_sql(&parts),
            "lower(email), id DESC"
        );
    }

    #[test]
    fn format_derive_value_appends_coverage_for_expression() {
        let parts = vec![IndexKeyPart::Expression {
            sql: "lower(email)".into(),
            coverage_columns: vec!["email".into()],
            opclass: None,
            collate: None,
            sort: None,
            nulls: None,
        }];
        assert_eq!(format_index_key_list_derive_value(&parts), "lower(email) | email");
        assert_eq!(format_index_key_list_sql(&parts), "lower(email)");
    }

    #[test]
    fn derive_value_round_trip_shape() {
        let idx = IndexDefinition {
            name: "idx_t".into(),
            columns: vec!["a".into(), "b".into()],
            key_list_sql: None,
            key_parts: vec![
                IndexKeyPart::Column {
                    name: "a".into(),
                    opclass: None,
                    collate: None,
                    sort: None,
                    nulls: None,
                },
                IndexKeyPart::Column {
                    name: "b".into(),
                    opclass: None,
                    collate: None,
                    sort: Some(IndexBtreeSort::Desc),
                    nulls: Some(IndexBtreeNulls::First),
                },
            ],
            include_columns: vec![],
            unique: false,
            partial_where: None,
        };
        assert_eq!(
            index_definition_to_derive_index_value(&idx),
            "idx_t(a, b DESC NULLS FIRST)"
        );
    }
}
