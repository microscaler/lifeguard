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
    pub is_unique: bool,
    pub is_indexed: bool,
    pub is_nullable: bool,
    pub is_auto_increment: bool,
    pub enum_name: Option<String>,
}

impl Default for ColumnAttributes {
    fn default() -> Self {
        Self {
            is_primary_key: false,
            column_name: None,
            column_type: None,
            default_value: None,
            is_unique: false,
            is_indexed: false,
            is_nullable: false,
            is_auto_increment: false,
            enum_name: None,
        }
    }
}

/// Parse all column attributes from a field
/// 
/// This function is a placeholder for future functionality that will parse
/// all column attributes at once. Currently, attributes are parsed individually
/// as needed.
#[allow(dead_code)]
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
        }
    }
    
    attrs
}
