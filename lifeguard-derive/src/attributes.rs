//! Attribute parsing utilities

use std::collections::HashSet;

use proc_macro2::Span;
use syn::{Attribute, ExprLit, Field, Lit};

use crate::utils;

/// Extract table name from struct attributes
pub fn extract_table_name(attrs: &[Attribute]) -> Option<String> {
    for attr in attrs {
        if attr.path().is_ident("table_name") {
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s), ..
                }) = &meta.value
                {
                    return Some(s.value());
                }
            }
        }
    }
    None
}

/// Extract schema name from struct attributes
pub fn extract_schema_name(attrs: &[Attribute]) -> Option<String> {
    for attr in attrs {
        if attr.path().is_ident("schema_name") {
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s), ..
                }) = &meta.value
                {
                    return Some(s.value());
                }
            }
        }
    }
    None
}

/// Extract column name from field attributes
pub fn extract_column_name(field: &Field) -> Option<String> {
    for attr in &field.attrs {
        if attr.path().is_ident("column_name") {
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s), ..
                }) = &meta.value
                {
                    return Some(s.value());
                }
            }
        }
    }
    None
}

/// Extract model name from struct attributes
/// Used by `DeriveEntity` to know which Model type to reference in `EntityTrait`
pub fn extract_model_name(attrs: &[Attribute]) -> Option<syn::Ident> {
    for attr in attrs {
        if attr.path().is_ident("model") {
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s), ..
                }) = &meta.value
                {
                    // Parse the string as an Ident
                    return syn::parse_str::<syn::Ident>(&s.value()).ok();
                }
            }
        }
    }
    None
}

/// Extract column enum name from struct attributes
/// Used by `DeriveEntity` to know which Column enum type to reference in `EntityTrait`
pub fn extract_column_enum_name(attrs: &[Attribute]) -> Option<syn::Ident> {
    for attr in attrs {
        if attr.path().is_ident("column") {
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s), ..
                }) = &meta.value
                {
                    // Parse the string as an Ident
                    return syn::parse_str::<syn::Ident>(&s.value()).ok();
                }
            }
        }
    }
    None
}

/// `#[cursor_tiebreak = "ColumnVariant"]` on the `LifeModel` struct (forwarded to `DeriveEntity`) —
/// opt-in primary-key column variant for cursor pagination `after_pk` / `before_pk` when the cursor column
/// is non-unique. Valid only with a single-column primary key.
pub fn extract_cursor_tiebreak(attrs: &[Attribute]) -> Option<syn::Ident> {
    for attr in attrs {
        if attr.path().is_ident("cursor_tiebreak") {
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s), ..
                }) = &meta.value
                {
                    return syn::parse_str::<syn::Ident>(&s.value()).ok();
                }
            }
        }
    }
    None
}

/// Check if field has a specific attribute
pub fn has_attribute(field: &Field, attr_name: &str) -> bool {
    field
        .attrs
        .iter()
        .any(|attr| attr.path().is_ident(attr_name))
}

/// Parse `#[validate(custom = path)]` on a model field (PRD V-5).
///
/// Multiple attributes or `custom = a, custom = b` in one `#[validate(...)]` are supported.
/// The generated code calls each path as `path(&sea_query::Value) -> Result<(), String>`.
pub fn parse_field_validate_custom_paths(field: &Field) -> syn::Result<Vec<syn::Path>> {
    let mut paths = Vec::new();
    for attr in &field.attrs {
        if !attr.path().is_ident("validate") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("custom") {
                let value = meta.value()?;
                let path: syn::Path = value.parse()?;
                paths.push(path);
                Ok(())
            } else {
                Err(meta.error(
                    "unknown `validate` item; expected `custom = path` (function `fn(&sea_query::Value) -> Result<(), String>`)",
                ))
            }
        })?;
    }
    Ok(paths)
}

/// Holds the configuration extracted from `#[has_many]`, `#[belongs_to]`, etc.
#[derive(Debug, Clone, Default)]
pub struct RelationAttribute {
    pub entity: String,
    pub from: Option<String>,
    pub to: Option<String>,
}

/// Extract all column attributes from a field
///
/// This struct is a placeholder for future functionality that will support
/// additional column attributes like unique, indexed, nullable, etc.
#[allow(dead_code)]
#[derive(Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct ColumnAttributes {
    pub is_primary_key: bool,
    pub column_name: Option<String>,
    pub column_type: Option<String>,
    pub default_value: Option<String>,
    pub default_expr: Option<String>,
    pub renamed_from: Option<String>,
    pub is_unique: bool,
    pub is_indexed: bool,
    pub is_nullable: bool,
    pub is_auto_increment: bool,
    pub enum_name: Option<String>,
    pub is_ignored: bool,
    pub select_as: Option<String>,
    pub save_as: Option<String>,
    pub comment: Option<String>,
    /// Foreign key constraint (e.g., "`chart_of_accounts(id)` ON DELETE SET NULL")
    pub foreign_key: Option<String>,
    /// CHECK constraint expression (column-level)
    pub check: Option<String>,
    pub has_many: Option<RelationAttribute>,
    pub belongs_to: Option<RelationAttribute>,
    pub has_one: Option<RelationAttribute>,
}

fn parse_relation_attr(attr: &Attribute) -> Result<RelationAttribute, syn::Error> {
    let mut rel = RelationAttribute::default();
    if let syn::Meta::List(meta_list) = &attr.meta {
        let nested = meta_list.parse_args_with(
            syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
        )?;
        for meta in nested {
            if let syn::Meta::NameValue(nv) = meta {
                if let syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(s),
                    ..
                }) = &nv.value
                {
                    if nv.path.is_ident("entity") {
                        rel.entity = s.value();
                    } else if nv.path.is_ident("from") {
                        rel.from = Some(s.value());
                    } else if nv.path.is_ident("to") {
                        rel.to = Some(s.value());
                    }
                }
            }
        }
    }
    if rel.entity.is_empty() {
        return Err(syn::Error::new_spanned(
            attr,
            "Relation attribute must specify an 'entity' (e.g., #[has_many(entity = \"post::Entity\")])",
        ));
    }
    Ok(rel)
}

/// Parse all column attributes from a field
///
/// Extracts all column-related attributes from a field and returns them
/// as a `ColumnAttributes` struct. Used by the `LifeModel` macro to generate
/// `ColumnTrait::def()` implementations.
///
/// Returns an error if invalid attribute values are found (e.g., empty strings
/// in `select_as` or `save_as` attributes).
#[allow(clippy::too_many_lines)]
pub fn parse_column_attributes(field: &Field) -> Result<ColumnAttributes, syn::Error> {
    let mut attrs = ColumnAttributes::default();

    // Debug: Check if attributes are being seen
    // For now, just process them normally

    for attr in &field.attrs {
        if attr.path().is_ident("primary_key") {
            attrs.is_primary_key = true;
        } else if attr.path().is_ident("column_name") {
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s), ..
                }) = &meta.value
                {
                    attrs.column_name = Some(s.value());
                }
            }
        } else if attr.path().is_ident("column_type") {
            // Parse name-value attribute: #[column_type = "VARCHAR(255)"]
            // Use the exact same pattern as extract_column_name which works
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s), ..
                }) = &meta.value
                {
                    attrs.column_type = Some(s.value());
                }
            }
        } else if attr.path().is_ident("default_value") {
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s), ..
                }) = &meta.value
                {
                    attrs.default_value = Some(s.value());
                }
            }
        } else if attr.path().is_ident("default_expr") {
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s), ..
                }) = &meta.value
                {
                    attrs.default_expr = Some(s.value());
                }
            }
        } else if attr.path().is_ident("renamed_from") {
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s), ..
                }) = &meta.value
                {
                    attrs.renamed_from = Some(s.value());
                }
            }
        } else if attr.path().is_ident("unique") {
            attrs.is_unique = true;
        } else if attr.path().is_ident("indexed") {
            attrs.is_indexed = true;
        } else if attr.path().is_ident("nullable") {
            attrs.is_nullable = true;
        } else if attr.path().is_ident("auto_increment") {
            attrs.is_auto_increment = true;
        } else if attr.path().is_ident("enum_name") {
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s), ..
                }) = &meta.value
                {
                    attrs.enum_name = Some(s.value());
                }
            }
        } else if attr.path().is_ident("ignore") || attr.path().is_ident("skip") {
            attrs.is_ignored = true;
        } else if attr.path().is_ident("select_as") {
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s), ..
                }) = &meta.value
                {
                    let value = s.value();
                    if value.is_empty() {
                        return Err(syn::Error::new_spanned(
                            &meta.value,
                            "Empty string not allowed in select_as attribute. select_as must contain a valid SQL expression."
                        ));
                    }
                    // Validate expression length to prevent memory issues with get_static_expr() caching
                    #[allow(clippy::items_after_statements)]
                    const MAX_EXPR_LENGTH: usize = 64 * 1024; // 64KB
                    if value.len() > MAX_EXPR_LENGTH {
                        return Err(syn::Error::new_spanned(
                            &meta.value,
                            format!(
                                "select_as expression is too long ({} bytes, max {} bytes). Very long expressions can cause memory issues with static string caching.",
                                value.len(), MAX_EXPR_LENGTH
                            )
                        ));
                    }
                    attrs.select_as = Some(value);
                }
            }
        } else if attr.path().is_ident("save_as") {
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s), ..
                }) = &meta.value
                {
                    let value = s.value();
                    if value.is_empty() {
                        return Err(syn::Error::new_spanned(
                            &meta.value,
                            "Empty string not allowed in save_as attribute. save_as must contain a valid SQL expression."
                        ));
                    }
                    // Validate expression length to prevent memory issues with get_static_expr() caching
                    #[allow(clippy::items_after_statements)]
                    const MAX_EXPR_LENGTH: usize = 64 * 1024; // 64KB
                    if value.len() > MAX_EXPR_LENGTH {
                        return Err(syn::Error::new_spanned(
                            &meta.value,
                            format!(
                                "save_as expression is too long ({} bytes, max {} bytes). Very long expressions can cause memory issues with static string caching.",
                                value.len(), MAX_EXPR_LENGTH
                            )
                        ));
                    }
                    attrs.save_as = Some(value);
                }
            }
        } else if attr.path().is_ident("has_many") {
            attrs.is_ignored = true; // Don't treat as DB column
            attrs.has_many = Some(parse_relation_attr(attr)?);
        } else if attr.path().is_ident("belongs_to") {
            attrs.is_ignored = true;
            attrs.belongs_to = Some(parse_relation_attr(attr)?);
        } else if attr.path().is_ident("has_one") {
            attrs.is_ignored = true;
            attrs.has_one = Some(parse_relation_attr(attr)?);
        } else if attr.path().is_ident("comment") {
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s), ..
                }) = &meta.value
                {
                    attrs.comment = Some(s.value());
                }
            }
        } else if attr.path().is_ident("foreign_key") {
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s), ..
                }) = &meta.value
                {
                    attrs.foreign_key = Some(s.value());
                }
            }
        } else if attr.path().is_ident("check") {
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s), ..
                }) = &meta.value
                {
                    attrs.check = Some(s.value());
                }
            }
        }
    }

    Ok(attrs)
}

/// How derived `LifeRecord::validate_fields` combines per-field `#[validate(custom = ...)]` errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TableValidationStrategy {
    /// First failing field validator stops; remaining field validators are skipped.
    #[default]
    FailFast,
    /// Run every field validator and collect all errors.
    Aggregate,
}

/// Btree sort / nulls order parsed from `#[index]` (maps to `lifeguard::IndexBtree*` in codegen).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParsedBtreeSort {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParsedBtreeNulls {
    First,
    Last,
}

/// One key segment after full `#[index]` parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ParsedIndexKeyPart {
    Column {
        name: String,
        opclass: Option<String>,
        collate: Option<String>,
        sort: Option<ParsedBtreeSort>,
        nulls: Option<ParsedBtreeNulls>,
    },
    Expression {
        sql: String,
        coverage_columns: Vec<String>,
        opclass: Option<String>,
        collate: Option<String>,
        sort: Option<ParsedBtreeSort>,
        nulls: Option<ParsedBtreeNulls>,
    },
}

/// Parsed `#[index = "..."]` before codegen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedIndexSpec {
    pub name: String,
    pub columns: Vec<String>,
    /// Legacy verbatim key list when [`Self::key_parts`] is empty.
    pub key_list_sql: Option<String>,
    pub key_parts: Vec<ParsedIndexKeyPart>,
    pub unique: bool,
    pub partial_where: Option<String>,
    pub include_columns: Vec<String>,
}

/// Table-level attributes for entity definitions
#[derive(Debug, Clone, Default)]
pub struct TableAttributes {
    /// Table comment/documentation
    pub table_comment: Option<String>,
    /// Composite unique constraints (each entry is a vector of column names)
    pub composite_unique: Vec<Vec<String>>,
    /// Index definitions
    pub indexes: Vec<ParsedIndexSpec>,
    /// Table-level CHECK constraints
    /// Each entry is a tuple of (`constraint_name`, expression)
    /// If `constraint_name` is None, a default name will be generated from the table name
    pub check_constraints: Vec<(Option<String>, String)>,
    /// Skip `FromRow` generation (useful for SQL generation when types don't implement `FromSql`)
    pub skip_from_row: bool,
    /// Lifecycle hook: before insert
    pub before_insert: Option<String>,
    /// Lifecycle hook: after insert
    pub after_insert: Option<String>,
    /// Lifecycle hook: before update
    pub before_update: Option<String>,
    /// Lifecycle hook: after update
    pub after_update: Option<String>,
    /// Lifecycle hook: before delete
    pub before_delete: Option<String>,
    /// Lifecycle hook: after delete
    pub after_delete: Option<String>,
    /// Auto timestamp flag automatically handles `created_at` and `updated_at`
    pub auto_timestamp: bool,
    /// Soft delete flag intercepts DELETE operations and updates `deleted_at` instead
    pub soft_delete: bool,
    /// When set, the derived `LifeRecord` implements `ActiveModelBehavior::validation_strategy`
    /// so `validate_fields` matches `run_validators` for that strategy.
    pub validation_strategy: Option<TableValidationStrategy>,
    /// When true, every database column on the struct must appear in at least one of: `#[primary_key]`,
    /// `#[indexed]`, a table-level `#[index = "..."]` key or INCLUDE list, or `#[composite_unique = "..."]`.
    pub require_index_coverage: bool,
}

/// Parse table-level attributes from struct attributes
///
/// # Parameters
/// * `attrs` - Struct attributes to parse
/// * `valid_columns` - Set of valid column names that exist on the struct (for validation)
#[allow(dead_code)] // Used by macro expansion
#[allow(clippy::too_many_lines)] // Single attribute-dispatch loop; splitting would obscure control flow
pub fn parse_table_attributes(
    attrs: &[Attribute],
    valid_columns: &std::collections::HashSet<String>,
) -> Result<TableAttributes, syn::Error> {
    let mut table_attrs = TableAttributes::default();

    for attr in attrs {
        if attr.path().is_ident("skip_from_row") {
            // Skip FromRow generation - useful for SQL generation when types don't implement FromSql
            table_attrs.skip_from_row = true;
        } else if attr.path().is_ident("table_comment") {
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s), ..
                }) = &meta.value
                {
                    table_attrs.table_comment = Some(s.value());
                }
            }
        } else if attr.path().is_ident("composite_unique") {
            // Parse comma-separated column names: #[composite_unique = "col1, col2, col3"]
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s), ..
                }) = &meta.value
                {
                    let columns: Vec<String> = s
                        .value()
                        .split(',')
                        .map(|col| col.trim().to_string())
                        .filter(|col| !col.is_empty())
                        .collect();
                    // Validate that all columns exist
                    for col in &columns {
                        if !valid_columns.contains(col) {
                            return Err(syn::Error::new_spanned(
                                attr,
                                format!("Column '{}' in composite_unique does not exist on this struct. Available columns: {}",
                                    col,
                                    valid_columns.iter().map(String::as_str).collect::<Vec<_>>().join(", "))
                            ));
                        }
                    }
                    if !columns.is_empty() {
                        table_attrs.composite_unique.push(columns);
                    }
                }
            }
        } else if attr.path().is_ident("index") {
            // Parse index definition: #[index = "name(columns) WHERE condition"]
            // Format: "idx_name(col1, col2) WHERE col1 IS NOT NULL"
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s), ..
                }) = &meta.value
                {
                    let index_def = parse_index_definition(&s.value())?;
                    // Validate that all columns in the index exist
                    for col in &index_def.columns {
                        if !valid_columns.contains(col) {
                            return Err(syn::Error::new_spanned(
                                attr,
                                format!("Column '{}' in index '{}' does not exist on this struct. Available columns: {}",
                                    col,
                                    index_def.name,
                                    valid_columns.iter().map(String::as_str).collect::<Vec<_>>().join(", "))
                            ));
                        }
                    }
                    for col in &index_def.include_columns {
                        if !valid_columns.contains(col) {
                            return Err(syn::Error::new_spanned(
                                attr,
                                format!("INCLUDE column '{}' in index '{}' does not exist on this struct. Available columns: {}",
                                    col,
                                    index_def.name,
                                    valid_columns.iter().map(String::as_str).collect::<Vec<_>>().join(", "))
                            ));
                        }
                    }
                    table_attrs.indexes.push(index_def);
                }
            }
        } else if attr.path().is_ident("check") {
            // Table-level CHECK constraint
            // Format: #[check = "name: expression"] or #[check = "expression"]
            // If "name:" prefix is present, use it as the constraint name
            // Otherwise, generate a default name from the table name
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s), ..
                }) = &meta.value
                {
                    let value = s.value();
                    // Check if format is "name: expression"
                    if let Some(colon_pos) = value.find(':') {
                        let name = value[..colon_pos].trim().to_string();
                        let expr = value[colon_pos + 1..].trim().to_string();
                        if !name.is_empty() && !expr.is_empty() {
                            table_attrs.check_constraints.push((Some(name), expr));
                        } else {
                            // Invalid format, treat as expression only
                            table_attrs.check_constraints.push((None, value));
                        }
                    } else {
                        // No name specified, use expression only
                        table_attrs.check_constraints.push((None, value));
                    }
                }
            }
        } else if attr.path().is_ident("auto_timestamp") {
            table_attrs.auto_timestamp = true;
        } else if attr.path().is_ident("soft_delete") {
            table_attrs.soft_delete = true;
        } else if attr.path().is_ident("before_insert") {
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s), ..
                }) = &meta.value
                {
                    table_attrs.before_insert = Some(s.value());
                }
            } else if let Ok(meta) = attr.meta.require_list() {
                table_attrs.before_insert = Some(meta.tokens.to_string());
            }
        } else if attr.path().is_ident("after_insert") {
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s), ..
                }) = &meta.value
                {
                    table_attrs.after_insert = Some(s.value());
                }
            } else if let Ok(meta) = attr.meta.require_list() {
                table_attrs.after_insert = Some(meta.tokens.to_string());
            }
        } else if attr.path().is_ident("before_update") {
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s), ..
                }) = &meta.value
                {
                    table_attrs.before_update = Some(s.value());
                }
            } else if let Ok(meta) = attr.meta.require_list() {
                table_attrs.before_update = Some(meta.tokens.to_string());
            }
        } else if attr.path().is_ident("after_update") {
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s), ..
                }) = &meta.value
                {
                    table_attrs.after_update = Some(s.value());
                }
            } else if let Ok(meta) = attr.meta.require_list() {
                table_attrs.after_update = Some(meta.tokens.to_string());
            }
        } else if attr.path().is_ident("before_delete") {
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s), ..
                }) = &meta.value
                {
                    table_attrs.before_delete = Some(s.value());
                }
            } else if let Ok(meta) = attr.meta.require_list() {
                table_attrs.before_delete = Some(meta.tokens.to_string());
            }
        } else if attr.path().is_ident("after_delete") {
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s), ..
                }) = &meta.value
                {
                    table_attrs.after_delete = Some(s.value());
                }
            } else if let Ok(meta) = attr.meta.require_list() {
                table_attrs.after_delete = Some(meta.tokens.to_string());
            }
        } else if attr.path().is_ident("validation_strategy") {
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s), ..
                }) = &meta.value
                {
                    let v = s.value().to_ascii_lowercase();
                    table_attrs.validation_strategy = Some(match v.as_str() {
                        "aggregate" => TableValidationStrategy::Aggregate,
                        "fail_fast" | "failfast" => TableValidationStrategy::FailFast,
                        _ => {
                            return Err(syn::Error::new_spanned(
                                attr,
                                "validation_strategy must be \"aggregate\" or \"fail_fast\"",
                            ));
                        }
                    });
                }
            }
        } else if attr.path().is_ident("require_index_coverage") {
            match &attr.meta {
                syn::Meta::Path(_) => table_attrs.require_index_coverage = true,
                _ => {
                    return Err(syn::Error::new_spanned(
                        attr,
                        "require_index_coverage must be a unit attribute: #[require_index_coverage]",
                    ));
                }
            }
        }
    }

    Ok(table_attrs)
}

/// Enforces [`TableAttributes::require_index_coverage`] after table attributes are parsed.
pub fn validate_require_index_coverage(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
    valid_columns: &HashSet<String>,
    table_attrs: &TableAttributes,
    span: proc_macro2::Span,
) -> syn::Result<()> {
    if !table_attrs.require_index_coverage {
        return Ok(());
    }
    let mut covered: HashSet<String> = HashSet::new();
    for field in fields {
        let Some(field_name) = &field.ident else {
            continue;
        };
        if has_attribute(field, "skip") || has_attribute(field, "ignore") {
            continue;
        }
        let col = extract_column_name(field)
            .unwrap_or_else(|| utils::snake_case(&field_name.to_string()));
        let col_attrs = parse_column_attributes(field)?;
        if col_attrs.is_primary_key || col_attrs.is_indexed {
            covered.insert(col);
        }
    }
    for idx in &table_attrs.indexes {
        for c in &idx.columns {
            covered.insert(c.clone());
        }
        for c in &idx.include_columns {
            covered.insert(c.clone());
        }
    }
    for group in &table_attrs.composite_unique {
        for c in group {
            covered.insert(c.clone());
        }
    }
    let mut missing: Vec<String> = valid_columns.difference(&covered).cloned().collect();
    missing.sort();
    if !missing.is_empty() {
        return Err(syn::Error::new(
            span,
            format!(
                "require_index_coverage: column(s) not covered by #[primary_key], #[indexed], #[index], or #[composite_unique]: {}",
                missing.join(", ")
            ),
        ));
    }
    Ok(())
}

fn index_simple_ident(segment: &str) -> bool {
    let s = segment.trim();
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_alphabetic() && first != '_' {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Split `expr | col1, col2` (space-pipe-space) into key text and coverage column list text.
fn split_index_expr_and_coverage(inner: &str) -> Result<(&str, Option<&str>), syn::Error> {
    let inner = inner.trim();
    if let Some(pos) = inner.find(" | ") {
        let left = inner[..pos].trim();
        let right = inner[pos + 3..].trim();
        if left.is_empty() {
            return Err(syn::Error::new(
                Span::call_site(),
                "Invalid index definition: missing expression before ` | ` in index key list",
            ));
        }
        if right.is_empty() {
            return Err(syn::Error::new(
                Span::call_site(),
                "Invalid index definition: after ` | ` list at least one table column for validation (e.g. \"lower(email) | email\")",
            ));
        }
        Ok((left, Some(right)))
    } else {
        Ok((inner, None))
    }
}

fn strip_suffix_ci<'a>(s: &'a str, suffix: &str) -> Option<&'a str> {
    let ls = s.len();
    let lu = suffix.len();
    if ls >= lu && s[ls - lu..].eq_ignore_ascii_case(suffix) {
        Some(s[..ls - lu].trim_end())
    } else {
        None
    }
}

fn parse_coverage_column_list(cov: &str) -> Result<Vec<String>, syn::Error> {
    let columns: Vec<String> = cov
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if columns.is_empty() {
        return Err(syn::Error::new(
            Span::call_site(),
            "Invalid index definition: column list after ` | ` is empty",
        ));
    }
    for c in &columns {
        if !index_simple_ident(c) {
            return Err(syn::Error::new(
                Span::call_site(),
                format!(
                    "Invalid index definition: coverage column `{c}` must be a simple identifier"
                ),
            ));
        }
    }
    Ok(columns)
}

fn parse_quoted_or_ident_collate(tail: &str) -> Result<(String, String), syn::Error> {
    let tail = tail.trim_start();
    if tail.is_empty() {
        return Err(syn::Error::new(
            Span::call_site(),
            "Invalid index definition: missing collation after COLLATE",
        ));
    }
    if tail.starts_with('"') {
        let bytes = tail.as_bytes();
        let mut i = 1usize;
        let mut out = String::new();
        while i < bytes.len() {
            if bytes[i] == b'"' {
                if i + 1 < bytes.len() && bytes[i + 1] == b'"' {
                    out.push('"');
                    i += 2;
                    continue;
                }
                let rest = tail[i + 1..].trim_start().to_string();
                return Ok((out, rest));
            }
            out.push(char::from(bytes[i]));
            i += 1;
        }
        return Err(syn::Error::new(
            Span::call_site(),
            "Invalid index definition: unterminated quoted collation",
        ));
    }
    let mut it = tail.split_whitespace();
    let c = it.next().ok_or_else(|| {
        syn::Error::new(Span::call_site(), "Invalid index definition: empty collation")
    })?;
    let rest = it.collect::<Vec<_>>().join(" ");
    Ok((c.to_string(), rest))
}

fn split_column_collate_opclass(s: &str) -> Result<(String, Option<String>, Option<String>), syn::Error> {
    let s = s.trim();
    let lower = s.to_ascii_lowercase();
    if let Some(pos) = lower.find(" collate ") {
        let head = s[..pos].trim();
        if head.split_whitespace().nth(1).is_some() {
            return Err(syn::Error::new(
                Span::call_site(),
                "Invalid index definition: only a single column name may appear before COLLATE",
            ));
        }
        if !index_simple_ident(head) {
            return Err(syn::Error::new(
                Span::call_site(),
                format!("Invalid index definition: invalid column name `{head}` before COLLATE"),
            ));
        }
        let tail = s[pos + " COLLATE ".len()..].trim_start();
        let (coll, rest) = parse_quoted_or_ident_collate(tail)?;
        let rest = rest.trim();
        let opclass = if rest.is_empty() {
            None
        } else {
            Some(rest.to_string())
        };
        return Ok((head.to_string(), Some(coll), opclass));
    }
    let mut it = s.split_whitespace();
    let col = it.next().ok_or_else(|| {
        syn::Error::new(Span::call_site(), "Invalid index definition: empty key segment")
    })?;
    if !index_simple_ident(col) {
        return Err(syn::Error::new(
            Span::call_site(),
            format!(
                "Invalid index definition: `{col}` is not a simple column name; use `expr | cols` for expressions"
            ),
        ));
    }
    let rest: String = it.collect::<Vec<_>>().join(" ");
    let opclass = if rest.is_empty() {
        None
    } else {
        Some(rest)
    };
    Ok((col.to_string(), None, opclass))
}

fn strip_trailing_sort_nulls(mut s: &str) -> (&str, Option<ParsedBtreeSort>, Option<ParsedBtreeNulls>) {
    let mut nulls = None;
    if let Some(r) = strip_suffix_ci(s, " NULLS FIRST") {
        nulls = Some(ParsedBtreeNulls::First);
        s = r.trim_end();
    } else if let Some(r) = strip_suffix_ci(s, " NULLS LAST") {
        nulls = Some(ParsedBtreeNulls::Last);
        s = r.trim_end();
    }
    let mut sort = None;
    if let Some(r) = strip_suffix_ci(s, " ASC") {
        sort = Some(ParsedBtreeSort::Asc);
        s = r.trim_end();
    } else if let Some(r) = strip_suffix_ci(s, " DESC") {
        sort = Some(ParsedBtreeSort::Desc);
        s = r.trim_end();
    }
    (s, sort, nulls)
}

fn parse_structured_column_segment(seg: &str) -> Result<ParsedIndexKeyPart, syn::Error> {
    let (core, sort, nulls) = strip_trailing_sort_nulls(seg.trim());
    let (name, collate, opclass) = split_column_collate_opclass(core)?;
    Ok(ParsedIndexKeyPart::Column {
        name,
        opclass,
        collate,
        sort,
        nulls,
    })
}

fn peel_trailing_opclass_from_expr(sql: &str) -> (String, Option<String>) {
    let sql = sql.trim();
    let words: Vec<&str> = sql.split_whitespace().collect();
    if words.len() < 2 {
        return (sql.to_string(), None);
    }
    let last = words[words.len() - 1];
    if last.contains("_ops") || last.ends_with("_ops") {
        let opc = last.to_string();
        let head = words[..words.len() - 1].join(" ");
        return (head, Some(opc));
    }
    (sql.to_string(), None)
}

fn parse_expression_left(left: &str) -> (String, Option<String>, Option<String>, Option<ParsedBtreeSort>, Option<ParsedBtreeNulls>) {
    let (core, sort, nulls) = strip_trailing_sort_nulls(left);
    let lower = core.to_ascii_lowercase();
    if let Some(pos) = lower.find(" collate ") {
        let head = core[..pos].trim().to_string();
        let tail = core[pos + " COLLATE ".len()..].trim_start();
        if let Ok((coll, rest)) = parse_quoted_or_ident_collate(tail) {
            let rest = rest.trim();
            let opclass = if rest.is_empty() {
                None
            } else {
                Some(rest.to_string())
            };
            return (head, Some(coll), opclass, sort, nulls);
        }
    }
    let (sql, opclass) = peel_trailing_opclass_from_expr(core);
    (sql, None, opclass, sort, nulls)
}

fn parse_expression_key_segment(seg: &str) -> Result<ParsedIndexKeyPart, syn::Error> {
    let (left, cov_opt) = split_index_expr_and_coverage(seg)?;
    let cov = cov_opt.ok_or_else(|| {
        syn::Error::new(
            Span::call_site(),
            "Invalid index definition: expression segment must use \"expr | col1, col2\"",
        )
    })?;
    let coverage_columns = parse_coverage_column_list(cov)?;
    let (sql, collate, opclass, sort, nulls) = parse_expression_left(left);
    if sql.is_empty() {
        return Err(syn::Error::new(
            Span::call_site(),
            "Invalid index definition: empty expression before ` | `",
        ));
    }
    Ok(ParsedIndexKeyPart::Expression {
        sql,
        coverage_columns,
        opclass,
        collate,
        sort,
        nulls,
    })
}

fn split_index_key_segments(inner: &str) -> Vec<String> {
    let inner = inner.trim();
    if inner.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    for (i, c) in inner.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                let chunk = inner[start..i].trim();
                if !chunk.is_empty() {
                    out.push(chunk.to_string());
                }
                start = i + 1;
            }
            _ => {}
        }
    }
    let chunk = inner[start..].trim();
    if !chunk.is_empty() {
        out.push(chunk.to_string());
    }
    out
}

fn merge_columns_from_key_parts(parts: &[ParsedIndexKeyPart]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for p in parts {
        let names: Vec<String> = match p {
            ParsedIndexKeyPart::Column { name, .. } => vec![name.clone()],
            ParsedIndexKeyPart::Expression {
                coverage_columns, ..
            } => coverage_columns.clone(),
        };
        for c in names {
            if seen.insert(c.clone()) {
                out.push(c);
            }
        }
    }
    out
}

fn classify_index_key_list(inner: &str) -> Result<(Vec<ParsedIndexKeyPart>, Vec<String>), syn::Error> {
    let segments = split_index_key_segments(inner);
    if segments.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let mut key_parts = Vec::with_capacity(segments.len());
    for seg in segments {
        let seg = seg.trim();
        let (_, cov) = split_index_expr_and_coverage(seg)?;
        let part = if cov.is_some() {
            parse_expression_key_segment(seg)?
        } else if seg.contains('(') || seg.contains(')') {
            return Err(syn::Error::new(
                Span::call_site(),
                "Invalid index definition: expression or parenthesized keys must use \"expr | col1, col2\" — right side lists table columns for validation (example: \"lower(email) | email\")",
            ));
        } else {
            parse_structured_column_segment(seg)?
        };
        key_parts.push(part);
    }
    let columns = merge_columns_from_key_parts(&key_parts);
    Ok((key_parts, columns))
}

/// Parse index definition string
/// Formats: `idx_name(col1, col2)`, `idx_name(col1) INCLUDE (col3) WHERE …`, `UNIQUE idx …`,
/// expression keys: `idx(lower(email) | email)`, opclass: `idx(slug text_pattern_ops | slug)`.
#[allow(dead_code)] // Used by parse_table_attributes
fn parse_index_definition(def: &str) -> Result<ParsedIndexSpec, syn::Error> {
    let def = def.trim();
    let mut unique = false;

    // Check for UNIQUE prefix
    let def = if let Some(stripped) = def.strip_prefix("UNIQUE ") {
        unique = true;
        stripped
    } else {
        def
    };

    // Parse: "… WHERE condition"
    let where_pos = def.find(" WHERE ");
    let (main_part, where_clause) = if let Some(pos) = where_pos {
        (def[..pos].trim(), Some(def[pos + 7..].trim().to_string()))
    } else {
        (def, None)
    };

    // Optional: "key_part INCLUDE (a, b)"
    let main_lower = main_part.to_ascii_lowercase();
    let include_idx = main_lower.find(" include ");
    let (key_part, include_columns) = if let Some(i) = include_idx {
        let kp = main_part[..i].trim();
        let after = main_part[i + " include ".len()..].trim();
        let inner = after.strip_prefix('(').and_then(|s| s.strip_suffix(')')).ok_or_else(|| {
            syn::Error::new(
                Span::call_site(),
                "Invalid index definition: INCLUDE must be followed by (col1, col2, ...)",
            )
        })?;
        let inc: Vec<String> = inner
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        (kp, inc)
    } else {
        (main_part, Vec::new())
    };

    // Parse: "idx_name(col1, col2)"
    let paren_pos = key_part.find('(');
    if let Some(pos) = paren_pos {
        let name = key_part[..pos].trim().to_string();
        let columns_str = &key_part[pos + 1..];
        let columns_str = columns_str.strip_suffix(')').ok_or_else(|| {
            syn::Error::new(
                Span::call_site(),
                "Invalid index definition: missing closing parenthesis",
            )
        })?;

        let (key_parts, columns) = classify_index_key_list(columns_str)?;

        Ok(ParsedIndexSpec {
            name,
            columns,
            key_list_sql: None,
            key_parts,
            unique,
            partial_where: where_clause,
            include_columns,
        })
    } else {
        // No columns specified - single column index
        Ok(ParsedIndexSpec {
            name: key_part.to_string(),
            columns: Vec::new(),
            key_list_sql: None,
            key_parts: Vec::new(),
            unique,
            partial_where: where_clause,
            include_columns,
        })
    }
}

#[cfg(test)]
mod index_definition_parse_tests {
    use super::*;

    #[test]
    fn parse_simple_composite_columns() {
        let p = parse_index_definition("idx_a_b(a, b)").expect("parse");
        assert_eq!(p.name, "idx_a_b");
        assert_eq!(p.columns, vec!["a", "b"]);
        assert!(p.key_list_sql.is_none());
        assert_eq!(p.key_parts.len(), 2);
    }

    #[test]
    fn parse_expression_with_coverage() {
        let p = parse_index_definition("idx(lower(email) | email)").expect("parse");
        assert_eq!(p.name, "idx");
        assert!(p.key_list_sql.is_none());
        assert_eq!(p.columns, vec!["email"]);
        assert!(matches!(
            &p.key_parts[0],
            ParsedIndexKeyPart::Expression { sql, coverage_columns, .. }
            if sql == "lower(email)" && coverage_columns == &["email".to_string()]
        ));
    }

    #[test]
    fn parse_opclass_column_without_pipe() {
        let p = parse_index_definition("idx_slug(slug text_pattern_ops)").expect("parse");
        assert_eq!(p.name, "idx_slug");
        assert_eq!(p.columns, vec!["slug"]);
        assert!(matches!(
            &p.key_parts[0],
            ParsedIndexKeyPart::Column { name, opclass, .. }
            if name == "slug" && opclass.as_deref() == Some("text_pattern_ops")
        ));
    }

    #[test]
    fn parse_column_desc_nulls_first() {
        let p = parse_index_definition("idx_t(x DESC NULLS FIRST)").expect("parse");
        assert_eq!(p.columns, vec!["x"]);
        assert!(matches!(
            &p.key_parts[0],
            ParsedIndexKeyPart::Column { name, sort, nulls, .. }
            if name == "x"
                && *sort == Some(ParsedBtreeSort::Desc)
                && *nulls == Some(ParsedBtreeNulls::First)
        ));
    }

    #[test]
    fn parse_unique_include_where() {
        let p = parse_index_definition(
            "UNIQUE idx_t(title) INCLUDE (body) WHERE active = true",
        )
        .expect("parse");
        assert!(p.unique);
        assert_eq!(p.columns, vec!["title"]);
        assert_eq!(p.include_columns, vec!["body"]);
        assert_eq!(p.partial_where.as_deref(), Some("active = true"));
        assert_eq!(p.key_parts.len(), 1);
    }

    #[test]
    fn rejects_expression_without_coverage() {
        let err = parse_index_definition("idx(lower(email))")
            .expect_err("expression index without coverage should fail");
        assert!(
            err.to_string().contains("expression or parenthesized keys"),
            "{}",
            err
        );
    }
}
