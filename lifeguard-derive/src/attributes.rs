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
        }
    }
}

/// Parse all column attributes from a field
/// 
/// Extracts all column-related attributes from a field and returns them
/// as a `ColumnAttributes` struct. Used by the `LifeModel` macro to generate
/// `ColumnTrait::def()` implementations.
pub fn parse_column_attributes(field: &Field) -> ColumnAttributes {
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
                    attrs.select_as = Some(s.value());
                }
            }
        } else if attr.path().is_ident("save_as") {
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(ExprLit {
                    lit: Lit::Str(s),
                    ..
                }) = &meta.value {
                    attrs.save_as = Some(s.value());
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
        }
    }
    
    attrs
}
