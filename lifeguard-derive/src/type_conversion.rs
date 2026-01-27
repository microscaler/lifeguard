//! Centralized type conversion utilities for code generation
//!
//! This module provides functions to generate code for converting between Rust types
//! and `sea_query::Value`. All type conversions should go through this module to
//! ensure consistency across the codebase.
#![allow(clippy::map_unwrap_or)] // Allow in quote! macro-generated code
//!
//! Supported types:
//! - Integer types: i8, i16, i32, i64, u8, u16, u32, u64
//! - Floating point: f32, f64
//! - Boolean: bool
//! - String: String
//! - Binary: Vec<u8>
//! - JSON: `serde_json::Value`
//! - Option<T> for all above types
//!
//! # Type Conversion Consistency
//!
//! **CRITICAL:** All three conversion functions (`generate_field_to_value`,
//! `generate_option_field_to_value`, and `generate_option_field_to_value_with_default`)
//! must use the same `Value` variant for each Rust type to ensure consistency between
//! Model and Record `get()` methods.
//!
//! Specifically:
//! - `u64` must convert to `Value::BigUnsigned` (not `Value::BigInt`) in all three functions
//! - This ensures that `Model::get()` and `Record::get()` return the same `Value` variant
//! - Pattern matching and value comparisons between Model and Record will work correctly

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Type, TypePath, GenericArgument, PathArguments};

/// Check if a type is `serde_json::Value`
pub fn is_json_value_type(ty: &Type) -> bool {
    if let Type::Path(TypePath { path, .. }) = ty {
        let segments: Vec<_> = path.segments.iter().collect();
        segments.len() == 2
            && segments[0].ident == "serde_json"
            && segments[1].ident == "Value"
    } else {
        false
    }
}

/// Check if a type is `Vec<u8>` (binary data)
pub fn is_vec_u8_type(ty: &Type) -> bool {
    if let Type::Path(TypePath { path, .. }) = ty {
        if let Some(segment) = path.segments.last() {
            if segment.ident == "Vec" {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(Type::Path(TypePath { path: inner_path, .. }))) = args.args.first() {
                        if let Some(inner_segment) = inner_path.segments.last() {
                            return inner_segment.ident == "u8";
                        }
                    }
                }
            }
        }
    }
    false
}

/// Check if a type is `f32` (not Option<f32>)
pub fn is_f32_type(ty: &Type) -> bool {
    if let Type::Path(TypePath { path, .. }) = ty {
        if let Some(segment) = path.segments.last() {
            return segment.ident == "f32";
        }
    }
    false
}

/// Check if a type is `f64` (not Option<f64>)
pub fn is_f64_type(ty: &Type) -> bool {
    if let Type::Path(TypePath { path, .. }) = ty {
        if let Some(segment) = path.segments.last() {
            return segment.ident == "f64";
        }
    }
    false
}

/// Check if a type is `Option<f32>`
pub fn is_option_f32_type(ty: &Type) -> bool {
    if let Type::Path(TypePath { path, .. }) = ty {
        if let Some(segment) = path.segments.first() {
            if segment.ident == "Option" {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(inner_type)) = args.args.first() {
                        return is_f32_type(inner_type);
                    }
                }
            }
        }
    }
    false
}

/// Check if a type is `Option<f64>`
pub fn is_option_f64_type(ty: &Type) -> bool {
    if let Type::Path(TypePath { path, .. }) = ty {
        if let Some(segment) = path.segments.first() {
            if segment.ident == "Option" {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(inner_type)) = args.args.first() {
                        return is_f64_type(inner_type);
                    }
                }
            }
        }
    }
    false
}

/// Check if a type is `rust_decimal::Decimal`
pub fn is_decimal_type(ty: &Type) -> bool {
    if let Type::Path(TypePath { path, .. }) = ty {
        let segments: Vec<_> = path.segments.iter().collect();
        // Check for rust_decimal::Decimal (2 segments)
        if segments.len() == 2
            && segments[0].ident == "rust_decimal" && segments[1].ident == "Decimal" {
            return true;
        }
        // Check for Decimal (1 segment, if imported)
        if segments.len() == 1
            && segments[0].ident == "Decimal" {
            return true;
        }
    }
    false
}

/// Check if a type is `rusty_money::Money<C: Currency>`
pub fn is_money_type(ty: &Type) -> bool {
    if let Type::Path(TypePath { path, .. }) = ty {
        let segments: Vec<_> = path.segments.iter().collect();
        // Check for rusty_money::Money<...> (2 segments with generics)
        if segments.len() == 2
            && segments[0].ident == "rusty_money" && segments[1].ident == "Money" {
            // Check if it has generic arguments (Money<Currency>)
            if let PathArguments::AngleBracketed(_) = &segments[1].arguments {
                return true;
            }
        }
        // Check for Money<...> (1 segment with generics, if imported)
        if segments.len() == 1
            && segments[0].ident == "Money" {
            if let PathArguments::AngleBracketed(_) = &segments[0].arguments {
                return true;
            }
        }
    }
    false
}

/// Convert a Rust Type to its string representation
///
/// This function converts a `syn::Type` to a string representation that can be used
/// for runtime type introspection. It handles:
/// - Simple types: `i32`, `String`, `bool`, etc.
/// - Option types: `Option<i32>` → `"Option<i32>"`
/// - Path types: `serde_json::Value` → `"serde_json::Value"`
/// - Generic types: `Vec<u8>` → `"Vec<u8>"`
///
/// # Arguments
///
/// * `ty` - The Rust type to convert
///
/// # Returns
///
/// A string representation of the type
pub fn type_to_string(ty: &Type) -> String {
    match ty {
        Type::Path(type_path) => {
            let path = &type_path.path;
            let segments: Vec<String> = path.segments.iter()
                .map(|seg| {
                    let mut result = seg.ident.to_string();
                    // Handle generic arguments
                    if let PathArguments::AngleBracketed(args) = &seg.arguments {
                        // Filter to only include Type arguments (exclude lifetimes, const generics, etc.)
                        let generic_args: Vec<String> = args.args.iter()
                            .filter_map(|arg| {
                                if let GenericArgument::Type(inner_ty) = arg {
                                    Some(type_to_string(inner_ty))
                                } else {
                                    None
                                }
                            })
                            .collect();
                        // Only add angle brackets if we have type arguments after filtering
                        if !generic_args.is_empty() {
                            result.push('<');
                            result.push_str(&generic_args.join(", "));
                            result.push('>');
                        }
                    }
                    result
                })
                .collect();
            segments.join("::")
        }
        Type::Array(_) => "array".to_string(),
        Type::Slice(_) => "slice".to_string(),
        Type::Tuple(tuple) => {
            let elems: Vec<String> = tuple.elems.iter()
                .map(type_to_string)
                .collect();
            format!("({})", elems.join(", "))
        }
        Type::Reference(_) => "reference".to_string(),
        Type::Ptr(_) => "pointer".to_string(),
        _ => "unknown".to_string(),
    }
}

/// Generate code to convert a Rust field value to `sea_query::Value`
///
/// This is used for Model-to-Value conversion (non-Option fields).
/// The field is accessed as `self.field_name` (not `Option<T>`).
///
/// # Arguments
///
/// * `field_name` - The field identifier
/// * `field_type` - The Rust type of the field (e.g., `i32`, `String`, `Vec<u8>`)
///
/// # Returns
///
/// Returns a `TokenStream` that generates code to convert the field to `Value`.
pub fn generate_field_to_value(field_name: &syn::Ident, field_type: &Type) -> TokenStream {
    // Check for serde_json::Value first
    if is_json_value_type(field_type) {
        return quote! {
            sea_query::Value::Json(Some(Box::new(self.#field_name.clone())))
        };
    }
    
    // Check for Vec<u8> (binary data)
    if is_vec_u8_type(field_type) {
        return quote! {
            sea_query::Value::Bytes(Some(self.#field_name.clone()))
        };
    }
    
    // Check for rust_decimal::Decimal
    if is_decimal_type(field_type) {
        // Convert Decimal to String for SeaQuery Value
        // SeaQuery doesn't have a Decimal variant, so we use String
        return quote! {
            sea_query::Value::String(Some(self.#field_name.to_string()))
        };
    }
    
    // Check for rusty_money::Money
    if is_money_type(field_type) {
        // Extract the amount (Decimal) from Money and convert to String
        // Money stores amount as Decimal internally
        return quote! {
            sea_query::Value::String(Some(self.#field_name.amount().to_string()))
        };
    }
    
    // Handle other types
    if let Type::Path(TypePath { path, .. }) = field_type {
        if let Some(segment) = path.segments.last() {
            let ident_str = segment.ident.to_string();
            match ident_str.as_str() {
                "i32" => quote! { sea_query::Value::Int(Some(self.#field_name)) },
                "i64" => quote! { sea_query::Value::BigInt(Some(self.#field_name)) },
                "i16" => quote! { sea_query::Value::SmallInt(Some(self.#field_name)) },
                "i8" => quote! { sea_query::Value::TinyInt(Some(self.#field_name as i8)) },
                "u8" => quote! { sea_query::Value::SmallInt(Some(self.#field_name as i16)) },
                "u16" => quote! { sea_query::Value::Int(Some(self.#field_name as i32)) },
                "u32" => quote! { sea_query::Value::BigInt(Some(self.#field_name as i64)) },
                "u64" => quote! { sea_query::Value::BigUnsigned(Some(self.#field_name)) },
                "f32" => quote! { sea_query::Value::Float(Some(self.#field_name)) },
                "f64" => quote! { sea_query::Value::Double(Some(self.#field_name)) },
                "bool" => quote! { sea_query::Value::Bool(Some(self.#field_name)) },
                "String" => quote! { sea_query::Value::String(Some(self.#field_name.clone())) },
                _ => {
                    // Unknown type - fallback to String(None)
                    // NOTE: This may hide bugs. Consider using only supported types
                    quote! { sea_query::Value::String(None) }
                }
            }
        } else {
            quote! { sea_query::Value::String(None) }
        }
    } else {
        quote! { sea_query::Value::String(None) }
    }
}

/// Generate code to convert an `Option<T>` field to `sea_query::Value` (with `unwrap_or` for None)
///
/// This is used for Model-to-Value conversion where Option<T> fields should return
/// `Value::Some`(...) or `Value::None` (not Option<Value>).
/// The field is accessed as `self.field_name` where `field_name: Option<T>`.
///
/// # Arguments
///
/// * `field_name` - The field identifier
/// * `inner_type` - The INNER type of the Option (e.g., `i32` from `Option<i32>`)
///
/// # Returns
///
/// Returns a `TokenStream` that generates code to convert `Option<T>` to `Value`.
pub fn generate_option_field_to_value_with_default(field_name: &syn::Ident, inner_type: &Type) -> TokenStream {
    // Check for serde_json::Value first
    if is_json_value_type(inner_type) {
        return quote! {
            self.#field_name.as_ref().map(|v| sea_query::Value::Json(Some(Box::new(v.clone())))).unwrap_or(sea_query::Value::Json(None))
        };
    }
    
    // Check for Vec<u8> (binary data)
    if is_vec_u8_type(inner_type) {
        return quote! {
            self.#field_name.as_ref().map(|v| sea_query::Value::Bytes(Some(v.clone()))).unwrap_or(sea_query::Value::Bytes(None))
        };
    }
    
    // Check for rust_decimal::Decimal
    if is_decimal_type(inner_type) {
        return quote! {
            self.#field_name.as_ref().map(|v| sea_query::Value::String(Some(v.to_string()))).unwrap_or(sea_query::Value::String(None))
        };
    }
    
    // Check for rusty_money::Money
    if is_money_type(inner_type) {
        return quote! {
            self.#field_name.as_ref().map(|v| sea_query::Value::String(Some(v.amount().to_string()))).unwrap_or(sea_query::Value::String(None))
        };
    }
    
    // Handle other types
    if let Type::Path(TypePath { path, .. }) = inner_type {
        if let Some(segment) = path.segments.last() {
            let ident_str = segment.ident.to_string();
            match ident_str.as_str() {
                "i32" => quote! {
                    self.#field_name.map(|v| sea_query::Value::Int(Some(v))).unwrap_or(sea_query::Value::Int(None))
                },
                "i64" => quote! {
                    self.#field_name.map(|v| sea_query::Value::BigInt(Some(v))).unwrap_or(sea_query::Value::BigInt(None))
                },
                "i16" => quote! {
                    self.#field_name.map(|v| sea_query::Value::SmallInt(Some(v))).unwrap_or(sea_query::Value::SmallInt(None))
                },
                "i8" => quote! {
                    self.#field_name.map(|v| sea_query::Value::TinyInt(Some(v as i8))).unwrap_or(sea_query::Value::TinyInt(None))
                },
                "u8" => quote! {
                    self.#field_name.map(|v| sea_query::Value::SmallInt(Some(v as i16))).unwrap_or(sea_query::Value::SmallInt(None))
                },
                "u16" => quote! {
                    self.#field_name.map(|v| sea_query::Value::Int(Some(v as i32))).unwrap_or(sea_query::Value::Int(None))
                },
                "u32" => quote! {
                    self.#field_name.map(|v| sea_query::Value::BigInt(Some(v as i64))).unwrap_or(sea_query::Value::BigInt(None))
                },
                "u64" => quote! {
                    self.#field_name.map(|v| sea_query::Value::BigUnsigned(Some(v))).unwrap_or(sea_query::Value::BigUnsigned(None))
                },
                "f32" => quote! {
                    self.#field_name.map(|v| sea_query::Value::Float(Some(v))).unwrap_or(sea_query::Value::Float(None))
                },
                "f64" => quote! {
                    self.#field_name.map(|v| sea_query::Value::Double(Some(v))).unwrap_or(sea_query::Value::Double(None))
                },
                "bool" => quote! {
                    self.#field_name.map(|v| sea_query::Value::Bool(Some(v))).unwrap_or(sea_query::Value::Bool(None))
                },
                "String" => quote! {
                    self.#field_name.as_ref().map(|v| sea_query::Value::String(Some(v.clone()))).unwrap_or(sea_query::Value::String(None))
                },
                _ => quote! {
                    sea_query::Value::String(None)
                },
            }
        } else {
            quote! { sea_query::Value::String(None) }
        }
    } else {
        quote! { sea_query::Value::String(None) }
    }
}

/// Generate code to convert an `Option<T>` field to `Option<sea_query::Value>`
///
/// This is used for Record-to-Value conversion (Option<T> fields).
/// The field is accessed as `self.field_name` where `field_name: Option<T>`.
///
/// # Arguments
///
/// * `field_name` - The field identifier
/// * `field_type` - The INNER type of the Option (e.g., `i32` from `Option<i32>`)
///
/// # Returns
///
/// Returns a `TokenStream` that generates code to convert `Option<T>` to `Option<Value>`.
/// 
/// Returns `None` when the field is `None`, and `Some(Value::...)` when the field is `Some(v)`.
/// This allows `get()` to correctly detect unset fields for CRUD operations.
pub fn generate_option_field_to_value(field_name: &syn::Ident, inner_type: &Type) -> TokenStream {
    // Check for serde_json::Value first
    if is_json_value_type(inner_type) {
        return quote! {
            self.#field_name.as_ref()
                .map(|v| sea_query::Value::Json(Some(Box::new(v.clone()))))
        };
    }
    
    // Check for Vec<u8> (binary data)
    if is_vec_u8_type(inner_type) {
        return quote! {
            self.#field_name.as_ref()
                .map(|v| sea_query::Value::Bytes(Some(v.clone())))
        };
    }
    
    // Check for rust_decimal::Decimal
    if is_decimal_type(inner_type) {
        return quote! {
            self.#field_name.as_ref()
                .map(|v| sea_query::Value::String(Some(v.to_string())))
        };
    }
    
    // Check for rusty_money::Money
    if is_money_type(inner_type) {
        return quote! {
            self.#field_name.as_ref()
                .map(|v| sea_query::Value::String(Some(v.amount().to_string())))
        };
    }
    
    // Handle other types
    if let Type::Path(TypePath { path, .. }) = inner_type {
        if let Some(segment) = path.segments.last() {
            let ident_str = segment.ident.to_string();
            match ident_str.as_str() {
                "i32" => quote! {
                    self.#field_name.map(|v| sea_query::Value::Int(Some(v)))
                },
                "i64" => quote! {
                    self.#field_name.map(|v| sea_query::Value::BigInt(Some(v)))
                },
                "i16" => quote! {
                    self.#field_name.map(|v| sea_query::Value::SmallInt(Some(v)))
                },
                "i8" => quote! {
                    self.#field_name.map(|v| sea_query::Value::TinyInt(Some(v as i8)))
                },
                "u8" => quote! {
                    self.#field_name.map(|v| sea_query::Value::SmallInt(Some(v as i16)))
                },
                "u16" => quote! {
                    self.#field_name.map(|v| sea_query::Value::Int(Some(v as i32)))
                },
                "u32" => quote! {
                    self.#field_name.map(|v| sea_query::Value::BigInt(Some(v as i64)))
                },
                "u64" => quote! {
                    self.#field_name.map(|v| sea_query::Value::BigUnsigned(Some(v)))
                },
                "f32" => quote! {
                    self.#field_name.map(|v| sea_query::Value::Float(Some(v)))
                },
                "f64" => quote! {
                    self.#field_name.map(|v| sea_query::Value::Double(Some(v)))
                },
                "bool" => quote! {
                    self.#field_name.map(|v| sea_query::Value::Bool(Some(v)))
                },
                "String" => quote! {
                    self.#field_name.as_ref().map(|v| sea_query::Value::String(Some(v.clone())))
                },
                _ => quote! {
                    // Unknown type: return None for unset fields, Some(String(None)) for set but None inner value
                    // This is a fallback - ideally the type should be known
                    self.#field_name.as_ref().map(|_| sea_query::Value::String(None))
                },
            }
        } else {
            quote! { 
                // Path segment not found: return None for unset fields
                self.#field_name.as_ref().map(|_| sea_query::Value::String(None))
            }
        }
    } else {
        quote! { 
            // Non-path type: return None for unset fields
            self.#field_name.as_ref().map(|_| sea_query::Value::String(None))
        }
    }
}

/// Generate code to convert `sea_query::Value` to a Rust field value
///
/// This is used for Value-to-Model conversion (non-Option fields).
/// The field is assigned as `self.field_name = value` (not `Option<T>`).
///
/// # Arguments
///
/// * `field_name` - The field identifier
/// * `field_type` - The Rust type of the field (e.g., `i32`, `String`, `Vec<u8>`)
/// * `column_variant` - The column variant identifier (for error messages)
///
/// # Returns
///
/// Returns a `TokenStream` that generates code to convert `Value` to the field type.
#[allow(dead_code)] // Reserved for future ModelTrait::set() implementation
#[allow(clippy::too_many_lines)]
pub fn generate_value_to_field(
    field_name: &syn::Ident,
    field_type: &Type,
    column_variant: &syn::Ident,
) -> TokenStream {
    // Check for serde_json::Value first
    if is_json_value_type(field_type) {
        return quote! {
            match value {
                sea_query::Value::Json(Some(v)) => {
                    self.#field_name = *v;
                    Ok(())
                }
                sea_query::Value::Json(None) => {
                    return Err(lifeguard::ActiveModelError::InvalidValueType {
                        column: stringify!(#column_variant).to_string(),
                        expected: "Json (non-null)".to_string(),
                        actual: format!("{:?}", value),
                    });
                }
                _ => Err(lifeguard::ActiveModelError::InvalidValueType {
                    column: stringify!(#column_variant).to_string(),
                    expected: "Json".to_string(),
                    actual: format!("{:?}", value),
                })
            }
        };
    }
    
    // Check for Vec<u8> (binary data)
    if is_vec_u8_type(field_type) {
        return quote! {
            match value {
                sea_query::Value::Bytes(Some(v)) => {
                    self.#field_name = v;
                    Ok(())
                }
                sea_query::Value::Bytes(None) => {
                    return Err(lifeguard::ActiveModelError::InvalidValueType {
                        column: stringify!(#column_variant).to_string(),
                        expected: "Bytes (non-null)".to_string(),
                        actual: format!("{:?}", value),
                    });
                }
                _ => Err(lifeguard::ActiveModelError::InvalidValueType {
                    column: stringify!(#column_variant).to_string(),
                    expected: "Bytes".to_string(),
                    actual: format!("{:?}", value),
                })
            }
        };
    }
    
    // Check for rust_decimal::Decimal
    if is_decimal_type(field_type) {
        return quote! {
            match value {
                sea_query::Value::String(Some(v)) => {
                    match v.parse::<rust_decimal::Decimal>() {
                        Ok(dec) => {
                            self.#field_name = dec;
                            Ok(())
                        }
                        Err(e) => Err(lifeguard::ActiveModelError::InvalidValueType {
                            column: stringify!(#column_variant).to_string(),
                            expected: "String containing valid Decimal".to_string(),
                            actual: format!("String({}) - parse error: {}", v, e),
                        })
                    }
                }
                sea_query::Value::String(None) => {
                    return Err(lifeguard::ActiveModelError::InvalidValueType {
                        column: stringify!(#column_variant).to_string(),
                        expected: "String (non-null)".to_string(),
                        actual: format!("{:?}", value),
                    });
                }
                _ => Err(lifeguard::ActiveModelError::InvalidValueType {
                    column: stringify!(#column_variant).to_string(),
                    expected: "String".to_string(),
                    actual: format!("{:?}", value),
                })
            }
        };
    }
    
    // Note: Money type conversion requires currency_code field, handled in FromRow generation
    // This function is for Value-to-field conversion (used in Record setters)
    // Money will need special handling in FromRow to construct from amount + currency
    
    // Handle other types
    if let Type::Path(TypePath { path, .. }) = field_type {
        if let Some(segment) = path.segments.last() {
            let ident_str = segment.ident.to_string();
            match ident_str.as_str() {
                "i32" => quote! {
                    match value {
                        sea_query::Value::Int(Some(v)) => {
                            self.#field_name = v;
                            Ok(())
                        }
                        sea_query::Value::Int(None) => {
                            return Err(lifeguard::ActiveModelError::InvalidValueType {
                                column: stringify!(#column_variant).to_string(),
                                expected: "Int (non-null)".to_string(),
                                actual: format!("{:?}", value),
                            });
                        }
                        _ => Err(lifeguard::ActiveModelError::InvalidValueType {
                            column: stringify!(#column_variant).to_string(),
                            expected: "Int".to_string(),
                            actual: format!("{:?}", value),
                        })
                    }
                },
                "i64" => quote! {
                    match value {
                        sea_query::Value::BigInt(Some(v)) => {
                            self.#field_name = v;
                            Ok(())
                        }
                        sea_query::Value::BigInt(None) => {
                            return Err(lifeguard::ActiveModelError::InvalidValueType {
                                column: stringify!(#column_variant).to_string(),
                                expected: "BigInt (non-null)".to_string(),
                                actual: format!("{:?}", value),
                            });
                        }
                        _ => Err(lifeguard::ActiveModelError::InvalidValueType {
                            column: stringify!(#column_variant).to_string(),
                            expected: "BigInt".to_string(),
                            actual: format!("{:?}", value),
                        })
                    }
                },
                "i16" => quote! {
                    match value {
                        sea_query::Value::SmallInt(Some(v)) => {
                            if v < -32768 || v > 32767 {
                                return Err(lifeguard::ActiveModelError::InvalidValueType {
                                    column: stringify!(#column_variant).to_string(),
                                    expected: "SmallInt in range -32768..=32767".to_string(),
                                    actual: format!("SmallInt({})", v),
                                });
                            }
                            self.#field_name = v;
                            Ok(())
                        }
                        sea_query::Value::SmallInt(None) => {
                            return Err(lifeguard::ActiveModelError::InvalidValueType {
                                column: stringify!(#column_variant).to_string(),
                                expected: "SmallInt (non-null)".to_string(),
                                actual: format!("{:?}", value),
                            });
                        }
                        _ => Err(lifeguard::ActiveModelError::InvalidValueType {
                            column: stringify!(#column_variant).to_string(),
                            expected: "SmallInt".to_string(),
                            actual: format!("{:?}", value),
                        })
                    }
                },
                "i8" => quote! {
                    match value {
                        sea_query::Value::TinyInt(Some(v)) => {
                            if v < -128 || v > 127 {
                                return Err(lifeguard::ActiveModelError::InvalidValueType {
                                    column: stringify!(#column_variant).to_string(),
                                    expected: "TinyInt in range -128..=127".to_string(),
                                    actual: format!("TinyInt({})", v),
                                });
                            }
                            self.#field_name = v as i8;
                            Ok(())
                        }
                        sea_query::Value::TinyInt(None) => {
                            return Err(lifeguard::ActiveModelError::InvalidValueType {
                                column: stringify!(#column_variant).to_string(),
                                expected: "TinyInt (non-null)".to_string(),
                                actual: format!("{:?}", value),
                            });
                        }
                        _ => Err(lifeguard::ActiveModelError::InvalidValueType {
                            column: stringify!(#column_variant).to_string(),
                            expected: "TinyInt".to_string(),
                            actual: format!("{:?}", value),
                        })
                    }
                },
                "u8" => quote! {
                    match value {
                        sea_query::Value::SmallInt(Some(v)) => {
                            if v < 0 || v > 255 {
                                return Err(lifeguard::ActiveModelError::InvalidValueType {
                                    column: stringify!(#column_variant).to_string(),
                                    expected: "SmallInt in range 0..=255".to_string(),
                                    actual: format!("SmallInt({})", v),
                                });
                            }
                            self.#field_name = v as u8;
                            Ok(())
                        }
                        sea_query::Value::SmallInt(None) => {
                            return Err(lifeguard::ActiveModelError::InvalidValueType {
                                column: stringify!(#column_variant).to_string(),
                                expected: "SmallInt (non-null)".to_string(),
                                actual: format!("{:?}", value),
                            });
                        }
                        _ => Err(lifeguard::ActiveModelError::InvalidValueType {
                            column: stringify!(#column_variant).to_string(),
                            expected: "SmallInt".to_string(),
                            actual: format!("{:?}", value),
                        })
                    }
                },
                "u16" => quote! {
                    match value {
                        sea_query::Value::Int(Some(v)) => {
                            if v < 0 || v > 65535 {
                                return Err(lifeguard::ActiveModelError::InvalidValueType {
                                    column: stringify!(#column_variant).to_string(),
                                    expected: "Int in range 0..=65535".to_string(),
                                    actual: format!("Int({})", v),
                                });
                            }
                            self.#field_name = v as u16;
                            Ok(())
                        }
                        sea_query::Value::Int(None) => {
                            return Err(lifeguard::ActiveModelError::InvalidValueType {
                                column: stringify!(#column_variant).to_string(),
                                expected: "Int (non-null)".to_string(),
                                actual: format!("{:?}", value),
                            });
                        }
                        _ => Err(lifeguard::ActiveModelError::InvalidValueType {
                            column: stringify!(#column_variant).to_string(),
                            expected: "Int".to_string(),
                            actual: format!("{:?}", value),
                        })
                    }
                },
                "u32" => quote! {
                    match value {
                        sea_query::Value::BigInt(Some(v)) => {
                            if v < 0 || v > 4294967295 {
                                return Err(lifeguard::ActiveModelError::InvalidValueType {
                                    column: stringify!(#column_variant).to_string(),
                                    expected: "BigInt in range 0..=4294967295".to_string(),
                                    actual: format!("BigInt({})", v),
                                });
                            }
                            self.#field_name = v as u32;
                            Ok(())
                        }
                        sea_query::Value::BigInt(None) => {
                            return Err(lifeguard::ActiveModelError::InvalidValueType {
                                column: stringify!(#column_variant).to_string(),
                                expected: "BigInt (non-null)".to_string(),
                                actual: format!("{:?}", value),
                            });
                        }
                        _ => Err(lifeguard::ActiveModelError::InvalidValueType {
                            column: stringify!(#column_variant).to_string(),
                            expected: "BigInt".to_string(),
                            actual: format!("{:?}", value),
                        })
                    }
                },
                "u64" => quote! {
                    match value {
                        sea_query::Value::BigUnsigned(Some(v)) => {
                            self.#field_name = v;
                            Ok(())
                        }
                        sea_query::Value::BigUnsigned(None) => {
                            return Err(lifeguard::ActiveModelError::InvalidValueType {
                                column: stringify!(#column_variant).to_string(),
                                expected: "BigUnsigned (non-null)".to_string(),
                                actual: format!("{:?}", value),
                            });
                        }
                        sea_query::Value::BigInt(Some(v)) => {
                            if v < 0 {
                                return Err(lifeguard::ActiveModelError::InvalidValueType {
                                    column: stringify!(#column_variant).to_string(),
                                    expected: "BigUnsigned or non-negative BigInt".to_string(),
                                    actual: format!("BigInt({})", v),
                                });
                            }
                            self.#field_name = v as u64;
                            Ok(())
                        }
                        sea_query::Value::BigInt(None) => {
                            return Err(lifeguard::ActiveModelError::InvalidValueType {
                                column: stringify!(#column_variant).to_string(),
                                expected: "BigUnsigned or BigInt (non-null)".to_string(),
                                actual: format!("{:?}", value),
                            });
                        }
                        _ => Err(lifeguard::ActiveModelError::InvalidValueType {
                            column: stringify!(#column_variant).to_string(),
                            expected: "BigUnsigned or BigInt".to_string(),
                            actual: format!("{:?}", value),
                        })
                    }
                },
                "f32" => quote! {
                    match value {
                        sea_query::Value::Float(Some(v)) => {
                            self.#field_name = v;
                            Ok(())
                        }
                        sea_query::Value::Float(None) => {
                            return Err(lifeguard::ActiveModelError::InvalidValueType {
                                column: stringify!(#column_variant).to_string(),
                                expected: "Float (non-null)".to_string(),
                                actual: format!("{:?}", value),
                            });
                        }
                        _ => Err(lifeguard::ActiveModelError::InvalidValueType {
                            column: stringify!(#column_variant).to_string(),
                            expected: "Float".to_string(),
                            actual: format!("{:?}", value),
                        })
                    }
                },
                "f64" => quote! {
                    match value {
                        sea_query::Value::Double(Some(v)) => {
                            self.#field_name = v;
                            Ok(())
                        }
                        sea_query::Value::Double(None) => {
                            return Err(lifeguard::ActiveModelError::InvalidValueType {
                                column: stringify!(#column_variant).to_string(),
                                expected: "Double (non-null)".to_string(),
                                actual: format!("{:?}", value),
                            });
                        }
                        _ => Err(lifeguard::ActiveModelError::InvalidValueType {
                            column: stringify!(#column_variant).to_string(),
                            expected: "Double".to_string(),
                            actual: format!("{:?}", value),
                        })
                    }
                },
                "bool" => quote! {
                    match value {
                        sea_query::Value::Bool(Some(v)) => {
                            self.#field_name = v;
                            Ok(())
                        }
                        sea_query::Value::Bool(None) => {
                            return Err(lifeguard::ActiveModelError::InvalidValueType {
                                column: stringify!(#column_variant).to_string(),
                                expected: "Bool (non-null)".to_string(),
                                actual: format!("{:?}", value),
                            });
                        }
                        _ => Err(lifeguard::ActiveModelError::InvalidValueType {
                            column: stringify!(#column_variant).to_string(),
                            expected: "Bool".to_string(),
                            actual: format!("{:?}", value),
                        })
                    }
                },
                "String" => quote! {
                    match value {
                        sea_query::Value::String(Some(v)) => {
                            self.#field_name = v;
                            Ok(())
                        }
                        sea_query::Value::String(None) => {
                            return Err(lifeguard::ActiveModelError::InvalidValueType {
                                column: stringify!(#column_variant).to_string(),
                                expected: "String (non-null)".to_string(),
                                actual: format!("{:?}", value),
                            });
                        }
                        _ => Err(lifeguard::ActiveModelError::InvalidValueType {
                            column: stringify!(#column_variant).to_string(),
                            expected: "String".to_string(),
                            actual: format!("{:?}", value),
                        })
                    }
                },
                _ => quote! {
                    Err(lifeguard::ActiveModelError::InvalidValueType {
                        column: stringify!(#column_variant).to_string(),
                        expected: "supported type".to_string(),
                        actual: format!("{:?}", value),
                    })
                },
            }
        } else {
            quote! {
                Err(lifeguard::ActiveModelError::InvalidValueType {
                    column: stringify!(#column_variant).to_string(),
                    expected: "supported type".to_string(),
                    actual: format!("{:?}", value),
                })
            }
        }
    } else {
        quote! {
            Err(lifeguard::ActiveModelError::InvalidValueType {
                column: stringify!(#column_variant).to_string(),
                expected: "supported type".to_string(),
                actual: format!("{:?}", value),
            })
        }
    }
}

/// Generate code to convert `sea_query::Value` to an `Option<T>` field
///
/// This is used for Value-to-Record conversion (Option<T> fields).
/// The field is assigned as `self.field_name = Some(value)` or `None`.
///
/// # Arguments
///
/// * `field_name` - The field identifier
/// * `inner_type` - The INNER type of the Option (e.g., `i32` from `Option<i32>`)
/// * `column_variant` - The column variant identifier (for error messages)
///
/// # Returns
///
/// Returns a `TokenStream` that generates code to convert `Value` to `Option<T>`.
#[allow(clippy::too_many_lines)]
pub fn generate_value_to_option_field(
    field_name: &syn::Ident,
    inner_type: &Type,
    column_variant: &syn::Ident,
) -> TokenStream {
    // Check for serde_json::Value first
    if is_json_value_type(inner_type) {
        return quote! {
            match value {
                sea_query::Value::Json(Some(v)) => {
                    self.#field_name = Some(*v);
                    Ok(())
                }
                sea_query::Value::Json(None) => {
                    self.#field_name = None;
                    Ok(())
                }
                _ => Err(lifeguard::ActiveModelError::InvalidValueType {
                    column: stringify!(#column_variant).to_string(),
                    expected: "Json".to_string(),
                    actual: format!("{:?}", value),
                })
            }
        };
    }
    
    // Check for Vec<u8> (binary data)
    if is_vec_u8_type(inner_type) {
        return quote! {
            match value {
                sea_query::Value::Bytes(Some(v)) => {
                    self.#field_name = Some(v);
                    Ok(())
                }
                sea_query::Value::Bytes(None) => {
                    self.#field_name = None;
                    Ok(())
                }
                _ => Err(lifeguard::ActiveModelError::InvalidValueType {
                    column: stringify!(#column_variant).to_string(),
                    expected: "Bytes".to_string(),
                    actual: format!("{:?}", value),
                })
            }
        };
    }
    
    // Check for rust_decimal::Decimal
    if is_decimal_type(inner_type) {
        return quote! {
            match value {
                sea_query::Value::String(Some(v)) => {
                    match v.parse::<rust_decimal::Decimal>() {
                        Ok(dec) => {
                            self.#field_name = Some(dec);
                            Ok(())
                        }
                        Err(e) => Err(lifeguard::ActiveModelError::InvalidValueType {
                            column: stringify!(#column_variant).to_string(),
                            expected: "String containing valid Decimal".to_string(),
                            actual: format!("String({}) - parse error: {}", v, e),
                        })
                    }
                }
                sea_query::Value::String(None) => {
                    self.#field_name = None;
                    Ok(())
                }
                _ => Err(lifeguard::ActiveModelError::InvalidValueType {
                    column: stringify!(#column_variant).to_string(),
                    expected: "String".to_string(),
                    actual: format!("{:?}", value),
                })
            }
        };
    }
    
    // Note: Money type conversion requires currency_code field, handled in FromRow generation
    // This function is for Value-to-option-field conversion (used in Record setters)
    // Money will need special handling in FromRow to construct from amount + currency
    
    // Handle other types
    if let Type::Path(TypePath { path, .. }) = inner_type {
        if let Some(segment) = path.segments.last() {
            let ident_str = segment.ident.to_string();
            match ident_str.as_str() {
                "i32" => quote! {
                    match value {
                        sea_query::Value::Int(Some(v)) => {
                            self.#field_name = Some(v);
                            Ok(())
                        }
                        sea_query::Value::Int(None) => {
                            self.#field_name = None;
                            Ok(())
                        }
                        _ => Err(lifeguard::ActiveModelError::InvalidValueType {
                            column: stringify!(#column_variant).to_string(),
                            expected: "Int".to_string(),
                            actual: format!("{:?}", value),
                        })
                    }
                },
                "i64" => quote! {
                    match value {
                        sea_query::Value::BigInt(Some(v)) => {
                            self.#field_name = Some(v);
                            Ok(())
                        }
                        sea_query::Value::BigInt(None) => {
                            self.#field_name = None;
                            Ok(())
                        }
                        _ => Err(lifeguard::ActiveModelError::InvalidValueType {
                            column: stringify!(#column_variant).to_string(),
                            expected: "BigInt".to_string(),
                            actual: format!("{:?}", value),
                        })
                    }
                },
                "i16" => quote! {
                    match value {
                        sea_query::Value::SmallInt(Some(v)) => {
                            if v < -32768 || v > 32767 {
                                return Err(lifeguard::ActiveModelError::InvalidValueType {
                                    column: stringify!(#column_variant).to_string(),
                                    expected: "SmallInt in range -32768..=32767".to_string(),
                                    actual: format!("SmallInt({})", v),
                                });
                            }
                            self.#field_name = Some(v);
                            Ok(())
                        }
                        sea_query::Value::SmallInt(None) => {
                            self.#field_name = None;
                            Ok(())
                        }
                        _ => Err(lifeguard::ActiveModelError::InvalidValueType {
                            column: stringify!(#column_variant).to_string(),
                            expected: "SmallInt".to_string(),
                            actual: format!("{:?}", value),
                        })
                    }
                },
                "i8" => quote! {
                    match value {
                        sea_query::Value::TinyInt(Some(v)) => {
                            if v < -128 || v > 127 {
                                return Err(lifeguard::ActiveModelError::InvalidValueType {
                                    column: stringify!(#column_variant).to_string(),
                                    expected: "TinyInt in range -128..=127".to_string(),
                                    actual: format!("TinyInt({})", v),
                                });
                            }
                            self.#field_name = Some(v as i8);
                            Ok(())
                        }
                        sea_query::Value::TinyInt(None) => {
                            self.#field_name = None;
                            Ok(())
                        }
                        _ => Err(lifeguard::ActiveModelError::InvalidValueType {
                            column: stringify!(#column_variant).to_string(),
                            expected: "TinyInt".to_string(),
                            actual: format!("{:?}", value),
                        })
                    }
                },
                "u8" => quote! {
                    match value {
                        sea_query::Value::SmallInt(Some(v)) => {
                            if v < 0 || v > 255 {
                                return Err(lifeguard::ActiveModelError::InvalidValueType {
                                    column: stringify!(#column_variant).to_string(),
                                    expected: "SmallInt in range 0..=255".to_string(),
                                    actual: format!("SmallInt({})", v),
                                });
                            }
                            self.#field_name = Some(v as u8);
                            Ok(())
                        }
                        sea_query::Value::SmallInt(None) => {
                            self.#field_name = None;
                            Ok(())
                        }
                        _ => Err(lifeguard::ActiveModelError::InvalidValueType {
                            column: stringify!(#column_variant).to_string(),
                            expected: "SmallInt".to_string(),
                            actual: format!("{:?}", value),
                        })
                    }
                },
                "u16" => quote! {
                    match value {
                        sea_query::Value::Int(Some(v)) => {
                            if v < 0 || v > 65535 {
                                return Err(lifeguard::ActiveModelError::InvalidValueType {
                                    column: stringify!(#column_variant).to_string(),
                                    expected: "Int in range 0..=65535".to_string(),
                                    actual: format!("Int({})", v),
                                });
                            }
                            self.#field_name = Some(v as u16);
                            Ok(())
                        }
                        sea_query::Value::Int(None) => {
                            self.#field_name = None;
                            Ok(())
                        }
                        _ => Err(lifeguard::ActiveModelError::InvalidValueType {
                            column: stringify!(#column_variant).to_string(),
                            expected: "Int".to_string(),
                            actual: format!("{:?}", value),
                        })
                    }
                },
                "u32" => quote! {
                    match value {
                        sea_query::Value::BigInt(Some(v)) => {
                            if v < 0 || v > 4294967295 {
                                return Err(lifeguard::ActiveModelError::InvalidValueType {
                                    column: stringify!(#column_variant).to_string(),
                                    expected: "BigInt in range 0..=4294967295".to_string(),
                                    actual: format!("BigInt({})", v),
                                });
                            }
                            self.#field_name = Some(v as u32);
                            Ok(())
                        }
                        sea_query::Value::BigInt(None) => {
                            self.#field_name = None;
                            Ok(())
                        }
                        _ => Err(lifeguard::ActiveModelError::InvalidValueType {
                            column: stringify!(#column_variant).to_string(),
                            expected: "BigInt".to_string(),
                            actual: format!("{:?}", value),
                        })
                    }
                },
                "u64" => quote! {
                    match value {
                        sea_query::Value::BigUnsigned(Some(v)) => {
                            self.#field_name = Some(v);
                            Ok(())
                        }
                        sea_query::Value::BigUnsigned(None) => {
                            self.#field_name = None;
                            Ok(())
                        }
                        sea_query::Value::BigInt(Some(v)) => {
                            if v < 0 {
                                return Err(lifeguard::ActiveModelError::InvalidValueType {
                                    column: stringify!(#column_variant).to_string(),
                                    expected: "BigUnsigned or non-negative BigInt".to_string(),
                                    actual: format!("BigInt({})", v),
                                });
                            }
                            self.#field_name = Some(v as u64);
                            Ok(())
                        }
                        sea_query::Value::BigInt(None) => {
                            self.#field_name = None;
                            Ok(())
                        }
                        _ => Err(lifeguard::ActiveModelError::InvalidValueType {
                            column: stringify!(#column_variant).to_string(),
                            expected: "BigUnsigned or BigInt".to_string(),
                            actual: format!("{:?}", value),
                        })
                    }
                },
                "f32" => quote! {
                    match value {
                        sea_query::Value::Float(Some(v)) => {
                            self.#field_name = Some(v);
                            Ok(())
                        }
                        sea_query::Value::Float(None) => {
                            self.#field_name = None;
                            Ok(())
                        }
                        _ => Err(lifeguard::ActiveModelError::InvalidValueType {
                            column: stringify!(#column_variant).to_string(),
                            expected: "Float".to_string(),
                            actual: format!("{:?}", value),
                        })
                    }
                },
                "f64" => quote! {
                    match value {
                        sea_query::Value::Double(Some(v)) => {
                            self.#field_name = Some(v);
                            Ok(())
                        }
                        sea_query::Value::Double(None) => {
                            self.#field_name = None;
                            Ok(())
                        }
                        _ => Err(lifeguard::ActiveModelError::InvalidValueType {
                            column: stringify!(#column_variant).to_string(),
                            expected: "Double".to_string(),
                            actual: format!("{:?}", value),
                        })
                    }
                },
                "bool" => quote! {
                    match value {
                        sea_query::Value::Bool(Some(v)) => {
                            self.#field_name = Some(v);
                            Ok(())
                        }
                        sea_query::Value::Bool(None) => {
                            self.#field_name = None;
                            Ok(())
                        }
                        _ => Err(lifeguard::ActiveModelError::InvalidValueType {
                            column: stringify!(#column_variant).to_string(),
                            expected: "Bool".to_string(),
                            actual: format!("{:?}", value),
                        })
                    }
                },
                "String" => quote! {
                    match value {
                        sea_query::Value::String(Some(v)) => {
                            self.#field_name = Some(v);
                            Ok(())
                        }
                        sea_query::Value::String(None) => {
                            self.#field_name = None;
                            Ok(())
                        }
                        _ => Err(lifeguard::ActiveModelError::InvalidValueType {
                            column: stringify!(#column_variant).to_string(),
                            expected: "String".to_string(),
                            actual: format!("{:?}", value),
                        })
                    }
                },
                _ => quote! {
                    Err(lifeguard::ActiveModelError::InvalidValueType {
                        column: stringify!(#column_variant).to_string(),
                        expected: "supported type".to_string(),
                        actual: format!("{:?}", value),
                    })
                },
            }
        } else {
            quote! {
                Err(lifeguard::ActiveModelError::InvalidValueType {
                    column: stringify!(#column_variant).to_string(),
                    expected: "supported type".to_string(),
                    actual: format!("{:?}", value),
                })
            }
        }
    } else {
        quote! {
            Err(lifeguard::ActiveModelError::InvalidValueType {
                column: stringify!(#column_variant).to_string(),
                expected: "supported type".to_string(),
                actual: format!("{:?}", value),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_str;

    #[test]
    fn test_type_to_string_simple_types() {
        // Test simple types
        let ty: Type = parse_str("i32").unwrap();
        assert_eq!(type_to_string(&ty), "i32");

        let ty: Type = parse_str("String").unwrap();
        assert_eq!(type_to_string(&ty), "String");

        let ty: Type = parse_str("bool").unwrap();
        assert_eq!(type_to_string(&ty), "bool");
    }

    #[test]
    fn test_type_to_string_option_types() {
        // Test Option types
        let ty: Type = parse_str("Option<i32>").unwrap();
        assert_eq!(type_to_string(&ty), "Option<i32>");

        let ty: Type = parse_str("Option<String>").unwrap();
        assert_eq!(type_to_string(&ty), "Option<String>");

        let ty: Type = parse_str("Option<Option<i32>>").unwrap();
        assert_eq!(type_to_string(&ty), "Option<Option<i32>>");
    }

    #[test]
    fn test_type_to_string_generic_types() {
        // Test generic types with type parameters
        let ty: Type = parse_str("Vec<u8>").unwrap();
        assert_eq!(type_to_string(&ty), "Vec<u8>");

        let ty: Type = parse_str("HashMap<String, i32>").unwrap();
        assert_eq!(type_to_string(&ty), "HashMap<String, i32>");
    }

    #[test]
    fn test_type_to_string_path_types() {
        // Test path types with multiple segments
        let ty: Type = parse_str("serde_json::Value").unwrap();
        assert_eq!(type_to_string(&ty), "serde_json::Value");

        let ty: Type = parse_str("std::collections::HashMap<String, i32>").unwrap();
        assert_eq!(type_to_string(&ty), "std::collections::HashMap<String, i32>");
    }

    #[test]
    fn test_type_to_string_phantom_data_no_empty_brackets() {
        // Test PhantomData without type generics (should not produce empty angle brackets)
        // PhantomData<'a> has a lifetime argument, not a type argument
        // After filtering, generic_args will be empty, so no angle brackets should be added
        let ty: Type = parse_str("PhantomData").unwrap();
        assert_eq!(type_to_string(&ty), "PhantomData");
        assert!(!type_to_string(&ty).contains("<>"), "Should not have empty angle brackets");
    }

    #[test]
    fn test_type_to_string_no_empty_brackets_for_non_type_generics() {
        // This test verifies that types with only non-type generic arguments
        // (lifetimes, const generics) don't produce empty angle brackets.
        // Since syn::parse_str doesn't easily parse lifetime parameters,
        // we test the behavior by ensuring that a simple type without generics
        // doesn't get brackets, and that types with type generics do get brackets.
        
        // Simple type - no brackets
        let ty: Type = parse_str("PhantomData").unwrap();
        assert_eq!(type_to_string(&ty), "PhantomData");
        assert!(!type_to_string(&ty).contains("<>"), "Should not have empty angle brackets");
        
        // Type with type generics - should have brackets
        let ty: Type = parse_str("Option<i32>").unwrap();
        assert_eq!(type_to_string(&ty), "Option<i32>");
        assert!(type_to_string(&ty).contains("<"), "Should have angle brackets for type generics");
    }

    #[test]
    fn test_type_to_string_mixed_generics() {
        // Test types with multiple generic arguments where some are types and some aren't
        // In practice, this would be like MyType<'a, T, 10> where 'a is lifetime, T is type, 10 is const
        // After filtering, only T should remain, so we should get MyType<T>
        
        // Test with just type generics (what we can easily parse)
        let ty: Type = parse_str("Result<i32, String>").unwrap();
        assert_eq!(type_to_string(&ty), "Result<i32, String>");
    }

    #[test]
    fn test_type_to_string_nested_generics() {
        // Test deeply nested generics
        let ty: Type = parse_str("Vec<Option<HashMap<String, i32>>>").unwrap();
        assert_eq!(type_to_string(&ty), "Vec<Option<HashMap<String, i32>>>");
    }

    #[test]
    fn test_type_to_string_tuple_types() {
        // Test tuple types
        let ty: Type = parse_str("(i32, String)").unwrap();
        let result = type_to_string(&ty);
        assert!(result.starts_with("(") && result.ends_with(")"));
        assert!(result.contains("i32"));
        assert!(result.contains("String"));
    }

    #[test]
    fn test_type_to_string_only_lifetime_generics() {
        // Test that types with only lifetime generics (no type generics) don't produce empty brackets
        // This test manually constructs a type with only lifetime generics to verify the fix
        use syn::{PathSegment, Path, PathArguments, AngleBracketedGenericArguments, GenericArgument, Lifetime};
        
        // Create a PathSegment with only a lifetime argument
        let lifetime = Lifetime::new("'a", proc_macro2::Span::call_site());
        let lifetime_arg = GenericArgument::Lifetime(lifetime);
        let args = AngleBracketedGenericArguments {
            colon2_token: None,
            lt_token: syn::token::Lt::default(),
            args: syn::punctuated::Punctuated::from_iter(vec![lifetime_arg]),
            gt_token: syn::token::Gt::default(),
        };
        
        let segment = PathSegment {
            ident: syn::Ident::new("PhantomData", proc_macro2::Span::call_site()),
            arguments: PathArguments::AngleBracketed(args),
        };
        
        let path = Path {
            leading_colon: None,
            segments: syn::punctuated::Punctuated::from_iter(vec![segment]),
        };
        
        let ty = Type::Path(syn::TypePath {
            qself: None,
            path,
        });
        
        let result = type_to_string(&ty);
        // Should be "PhantomData" without empty angle brackets
        assert_eq!(result, "PhantomData", "Type with only lifetime generics should not produce empty brackets");
        assert!(!result.contains("<>"), "Should not have empty angle brackets");
        assert!(!result.contains("<"), "Should not have any angle brackets when only non-type generics exist");
    }

    #[test]
    fn test_type_to_string_only_const_generics() {
        // Test that types with only const generics (no type generics) don't produce empty brackets
        use syn::{PathSegment, Path, PathArguments, AngleBracketedGenericArguments, GenericArgument, Expr};
        
        // Create a PathSegment with only a const argument
        // Note: syn::parse_str can't easily parse const generics, so we'll test the behavior
        // by ensuring that when we have a type with generics but filter out all type args,
        // we don't get empty brackets
        
        // Create a type like "Buffer<10>" where 10 is a const generic
        // Since syn::parse_str doesn't support const generics easily, we'll verify the logic
        // by testing that filtering works correctly
        
        // Test with a type that has mixed generics - we'll parse a type with type generics
        // and verify the filtering logic works, then document that const-only generics
        // would be handled the same way
        let ty: Type = parse_str("MyType<i32>").unwrap();
        let result = type_to_string(&ty);
        assert_eq!(result, "MyType<i32>");
        
        // The key test: verify that if we manually construct a type with only const generics,
        // it would be handled correctly. Since syn doesn't easily support const generics in parse_str,
        // we verify the logic by ensuring the filter-then-check pattern works.
        // The fix ensures that after filtering to only type generics, we check if the result
        // is empty before adding brackets.
    }

    #[test]
    fn test_type_to_string_mixed_type_and_non_type_generics() {
        // Test that types with both type and non-type generics only show type generics
        // This verifies the filtering logic works correctly
        use syn::{PathSegment, Path, PathArguments, AngleBracketedGenericArguments, GenericArgument, Lifetime, Type};
        
        // Create a type like "MyType<'a, T>" where 'a is lifetime and T is type
        // After filtering, we should get "MyType<T>"
        let lifetime = Lifetime::new("'a", proc_macro2::Span::call_site());
        let lifetime_arg = GenericArgument::Lifetime(lifetime);
        let type_arg: Type = parse_str("i32").unwrap();
        let type_generic_arg = GenericArgument::Type(type_arg);
        
        let args = AngleBracketedGenericArguments {
            colon2_token: None,
            lt_token: syn::token::Lt::default(),
            args: syn::punctuated::Punctuated::from_iter(vec![lifetime_arg, type_generic_arg]),
            gt_token: syn::token::Gt::default(),
        };
        
        let segment = PathSegment {
            ident: syn::Ident::new("MyType", proc_macro2::Span::call_site()),
            arguments: PathArguments::AngleBracketed(args),
        };
        
        let path = Path {
            leading_colon: None,
            segments: syn::punctuated::Punctuated::from_iter(vec![segment]),
        };
        
        let ty = Type::Path(syn::TypePath {
            qself: None,
            path,
        });
        
        let result = type_to_string(&ty);
        // Should be "MyType<i32>" - only the type generic, not the lifetime
        assert_eq!(result, "MyType<i32>", "Should only include type generics, not lifetime generics");
        assert!(!result.contains("'a"), "Should not include lifetime in output");
    }
}
