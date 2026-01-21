//! Attribute parsing utilities

use syn::{Attribute, Field, ExprLit, Lit};

/// Extract table name from struct attributes
pub fn extract_table_name(attrs: &[Attribute]) -> Option<String> {
    for attr in attrs {
        if attr.path().is_ident("table_name") {
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s),
                    ..
                }) = &meta.value {
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
                    lit: Lit::Str(s),
                    ..
                }) = &meta.value {
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
                    lit: Lit::Str(s),
                    ..
                }) = &meta.value {
                    return Some(s.value());
                }
            }
        }
    }
    None
}

/// Extract model name from struct attributes
/// Used by DeriveEntity to know which Model type to reference in EntityTrait
pub fn extract_model_name(attrs: &[Attribute]) -> Option<syn::Ident> {
    for attr in attrs {
        if attr.path().is_ident("model") {
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s),
                    ..
                }) = &meta.value {
                    // Parse the string as an Ident
                    return syn::parse_str::<syn::Ident>(&s.value()).ok();
                }
            }
        }
    }
    None
}

/// Extract column enum name from struct attributes
/// Used by DeriveEntity to know which Column enum type to reference in EntityTrait
pub fn extract_column_enum_name(attrs: &[Attribute]) -> Option<syn::Ident> {
    for attr in attrs {
        if attr.path().is_ident("column") {
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s),
                    ..
                }) = &meta.value {
                    // Parse the string as an Ident
                    return syn::parse_str::<syn::Ident>(&s.value()).ok();
                }
            }
        }
    }
    None
}

/// Check if field has a specific attribute
pub fn has_attribute(field: &Field, attr_name: &str) -> bool {
    field.attrs.iter().any(|attr| attr.path().is_ident(attr_name))
}

/// Extract all column attributes from a field
/// 
/// This struct is a placeholder for future functionality that will support
/// additional column attributes like unique, indexed, nullable, etc.
#[allow(dead_code)]
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
    /// Foreign key constraint (e.g., "chart_of_accounts(id) ON DELETE SET NULL")
    pub foreign_key: Option<String>,
    /// CHECK constraint expression (column-level)
    pub check: Option<String>,
}

impl Default for ColumnAttributes {
    fn default() -> Self {
        Self {
            is_primary_key: false,
            column_name: None,
            column_type: None,
            default_value: None,
            default_expr: None,
            renamed_from: None,
            is_unique: false,
            is_indexed: false,
            is_nullable: false,
            is_auto_increment: false,
            enum_name: None,
            is_ignored: false,
            select_as: None,
            save_as: None,
            comment: None,
            foreign_key: None,
            check: None,
        }
    }
}

/// Parse all column attributes from a field
/// 
/// Extracts all column-related attributes from a field and returns them
/// as a `ColumnAttributes` struct. Used by the `LifeModel` macro to generate
/// `ColumnTrait::def()` implementations.
/// 
/// Returns an error if invalid attribute values are found (e.g., empty strings
/// in select_as or save_as attributes).
pub fn parse_column_attributes(field: &Field) -> Result<ColumnAttributes, syn::Error> {
    let mut attrs = ColumnAttributes::default();
    
    for attr in &field.attrs {
        if attr.path().is_ident("primary_key") {
            attrs.is_primary_key = true;
        } else if attr.path().is_ident("column_name") {
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s),
                    ..
                }) = &meta.value {
                    attrs.column_name = Some(s.value());
                }
            }
        } else if attr.path().is_ident("column_type") {
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s),
                    ..
                }) = &meta.value {
                    attrs.column_type = Some(s.value());
                }
            }
        } else if attr.path().is_ident("default_value") {
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s),
                    ..
                }) = &meta.value {
                    attrs.default_value = Some(s.value());
                }
            }
        } else if attr.path().is_ident("default_expr") {
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s),
                    ..
                }) = &meta.value {
                    attrs.default_expr = Some(s.value());
                }
            }
        } else if attr.path().is_ident("renamed_from") {
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s),
                    ..
                }) = &meta.value {
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
                    lit: Lit::Str(s),
                    ..
                }) = &meta.value {
                    attrs.enum_name = Some(s.value());
                }
            }
        } else if attr.path().is_ident("ignore") || attr.path().is_ident("skip") {
            attrs.is_ignored = true;
        } else if attr.path().is_ident("select_as") {
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s),
                    ..
                }) = &meta.value {
                    let value = s.value();
                    if value.is_empty() {
                        return Err(syn::Error::new_spanned(
                            &meta.value,
                            "Empty string not allowed in select_as attribute. select_as must contain a valid SQL expression."
                        ));
                    }
                    // Validate expression length to prevent memory issues with get_static_expr() caching
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
                    lit: Lit::Str(s),
                    ..
                }) = &meta.value {
                    let value = s.value();
                    if value.is_empty() {
                        return Err(syn::Error::new_spanned(
                            &meta.value,
                            "Empty string not allowed in save_as attribute. save_as must contain a valid SQL expression."
                        ));
                    }
                    // Validate expression length to prevent memory issues with get_static_expr() caching
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
        } else if attr.path().is_ident("comment") {
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s),
                    ..
                }) = &meta.value {
                    attrs.comment = Some(s.value());
                }
            }
        } else if attr.path().is_ident("foreign_key") {
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s),
                    ..
                }) = &meta.value {
                    attrs.foreign_key = Some(s.value());
                }
            }
        } else if attr.path().is_ident("check") {
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s),
                    ..
                }) = &meta.value {
                    attrs.check = Some(s.value());
                }
            }
        }
    }
    
    Ok(attrs)
}

/// Table-level attributes for entity definitions
#[derive(Debug, Clone, Default)]
pub struct TableAttributes {
    /// Table comment/documentation
    pub table_comment: Option<String>,
    /// Composite unique constraints (each entry is a vector of column names)
    pub composite_unique: Vec<Vec<String>>,
    /// Index definitions (name, columns, unique, partial_where)
    pub indexes: Vec<(String, Vec<String>, bool, Option<String>)>,
    /// Table-level CHECK constraints
    pub check_constraints: Vec<String>,
}

/// Parse table-level attributes from struct attributes
#[allow(dead_code)] // Used by macro expansion
pub fn parse_table_attributes(attrs: &[Attribute]) -> Result<TableAttributes, syn::Error> {
    let mut table_attrs = TableAttributes::default();
    
    for attr in attrs {
        if attr.path().is_ident("table_comment") {
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s),
                    ..
                }) = &meta.value {
                    table_attrs.table_comment = Some(s.value());
                }
            }
        } else if attr.path().is_ident("composite_unique") {
            // Parse array of column names: #[composite_unique = ["col1", "col2"]]
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Array(array_expr) = &meta.value {
                    let mut columns = Vec::new();
                    for elem in &array_expr.elems {
                        if let syn::Expr::Lit(ExprLit {
                            lit: Lit::Str(s),
                            ..
                        }) = elem {
                            columns.push(s.value());
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
                    lit: Lit::Str(s),
                    ..
                }) = &meta.value {
                    let index_def = parse_index_definition(&s.value())?;
                    table_attrs.indexes.push(index_def);
                }
            }
        } else if attr.path().is_ident("check") {
            // Table-level CHECK constraint
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s),
                    ..
                }) = &meta.value {
                    table_attrs.check_constraints.push(s.value());
                }
            }
        }
    }
    
    Ok(table_attrs)
}

/// Parse index definition string
/// Format: "idx_name(col1, col2) WHERE col1 IS NOT NULL"
/// Returns: (name, columns, unique, partial_where)
#[allow(dead_code)] // Used by parse_table_attributes
fn parse_index_definition(def: &str) -> Result<(String, Vec<String>, bool, Option<String>), syn::Error> {
    let def = def.trim();
    let mut unique = false;
    
    // Check for UNIQUE prefix
    let def = if def.starts_with("UNIQUE ") {
        unique = true;
        &def[7..]
    } else {
        def
    };
    
    // Parse: "idx_name(col1, col2) WHERE condition"
    let where_pos = def.find(" WHERE ");
    let (index_part, where_clause) = if let Some(pos) = where_pos {
        (def[..pos].trim(), Some(def[pos + 7..].trim().to_string()))
    } else {
        (def, None)
    };
    
    // Parse: "idx_name(col1, col2)"
    let paren_pos = index_part.find('(');
    if let Some(pos) = paren_pos {
        let name = index_part[..pos].trim().to_string();
        let columns_str = &index_part[pos + 1..];
        let columns_str = columns_str.strip_suffix(')')
            .ok_or_else(|| syn::Error::new_spanned(
                &def,
                "Invalid index definition: missing closing parenthesis"
            ))?;
        
        let columns: Vec<String> = columns_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        
        Ok((name, columns, unique, where_clause))
    } else {
        // No columns specified - single column index
        Ok((index_part.to_string(), Vec::new(), unique, where_clause))
    }
}
