//! LifeModel derive macro implementation
//!
//! Based on SeaORM's expand_derive_entity_model pattern (v2.0.0-rc.28)
//! Generates Entity, Column, PrimaryKey, Model, FromRow, and LifeModelTrait

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DataStruct, DeriveInput, Fields, Ident, GenericArgument, PathArguments, Type};

use crate::attributes;
use crate::type_conversion;
use crate::utils;

/// Extract the inner type from Option<T>
/// Returns None if the type is not Option<T> or if extraction fails
fn extract_option_inner_type(ty: &Type) -> Option<&Type> {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Option" {
                // Extract inner type from generic arguments
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(inner_type)) = args.args.first() {
                        return Some(inner_type);
                    }
                }
            }
        }
    }
    None
}

/// Derive macro for `LifeModel` - generates Entity, Model, Column, PrimaryKey, and FromRow
///
/// This macro follows SeaORM's pattern exactly:
/// 1. Generates Entity struct with #[derive(DeriveEntity)] (triggers nested expansion)
/// 2. Generates Column enum
/// 3. Generates PrimaryKey enum  
/// 4. Generates Model struct
/// 5. Generates FromRow implementation for Model
/// 6. DeriveEntity (nested) generates LifeModelTrait for Entity
pub fn derive_life_model(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    // Extract struct name and table name
    let struct_name = &input.ident;
    let table_name = attributes::extract_table_name(&input.attrs)
        .unwrap_or_else(|| utils::snake_case(&struct_name.to_string()));
    let table_name_lit = syn::LitStr::new(&table_name, struct_name.span());

    // Extract fields
    let fields = match &input.data {
        Data::Struct(DataStruct {
            fields: Fields::Named(fields),
            ..
        }) => &fields.named,
        _ => {
            return syn::Error::new_spanned(
                &input.ident,
                "LifeModel can only be derived for structs with named fields",
            )
            .to_compile_error()
            .into();
        }
    };

    // Generate Model name
    let model_name = Ident::new(&format!("{}Model", struct_name), struct_name.span());
    let model_name_lit = syn::LitStr::new(&model_name.to_string(), model_name.span());

    // Process fields to generate:
    // - Column enum variants
    // - PrimaryKey enum variants
    // - Model struct fields
    // - FromRow field extraction
    // - ModelTrait get() match arms
    // - Primary key field tracking
    let mut column_variants = Vec::new();
    let mut primary_key_variants = Vec::new();
    let mut primary_key_variant_idents = Vec::new(); // Store (variant identifier, auto_increment) tuples for trait implementations
    let mut model_fields = Vec::new();
    let mut from_row_fields = Vec::new();
    let mut iden_impls = Vec::new();
    let mut model_get_match_arms = Vec::new();
    let mut model_set_match_arms = Vec::new();
    let mut primary_key_value_expr: Option<proc_macro2::TokenStream> = None;
    // Track primary key metadata for PrimaryKeyTrait
    let mut primary_key_type: Option<&Type> = None; // Keep for backward compatibility (first key only)
    let mut primary_key_types: Vec<&Type> = Vec::new(); // Track all primary key types for tuple ValueType
    let mut _primary_key_auto_increment = false; // Reserved for future PrimaryKeyTrait implementation
    let mut primary_key_to_column_mappings = Vec::new();

    for field in fields.iter() {
        let field_name = field.ident.as_ref().unwrap();
        let field_type = &field.ty;
        let column_name = attributes::extract_column_name(field)
            .unwrap_or_else(|| utils::snake_case(&field_name.to_string()));

        // Check if primary key
        let is_primary_key = attributes::has_attribute(field, "primary_key");
        let is_auto_increment = attributes::has_attribute(field, "auto_increment");

        // Generate Column enum variant (PascalCase)
        let column_variant = Ident::new(
            &utils::pascal_case(&field_name.to_string()),
            field_name.span(),
        );
        column_variants.push(quote! {
            #column_variant,
        });

        // Generate Iden implementation
        let column_name_str = column_name.as_str();
        iden_impls.push(quote! {
            Column::#column_variant => #column_name_str,
        });

        // Generate PrimaryKey variant if primary key
        if is_primary_key {
            primary_key_variants.push(quote! {
                #column_variant,
            });
            primary_key_variant_idents.push((column_variant.clone(), is_auto_increment)); // Store (identifier, auto_increment) for trait implementations
            
            // Track primary key metadata for PrimaryKeyTrait
            if primary_key_type.is_none() {
                primary_key_type = Some(field_type);
                _primary_key_auto_increment = is_auto_increment; // Keep for backward compatibility, but per-variant tracking is used
            }
            // Track all primary key types for tuple ValueType support
            primary_key_types.push(field_type);
            
            // Track mapping for PrimaryKeyToColumn
            primary_key_to_column_mappings.push(quote! {
                PrimaryKey::#column_variant => Column::#column_variant,
            });
            
            // Track primary key field for ModelTrait::get_primary_key_value()
            // Generate the value conversion expression now
            if primary_key_value_expr.is_none() {
                let pk_value_expr = match field_type {
                    syn::Type::Path(syn::TypePath {
                        path: syn::Path { segments, .. },
                        ..
                    }) => {
                        // Check if this is Option<T> first (using segments.last() like extract_option_inner_type)
                        // In syn's representation, Option<i32> is a single path segment with generic arguments,
                        // so segments.len() is 1, not 2. We need to check the last segment for "Option".
                        if let Some(last_segment) = segments.last() {
                            if last_segment.ident == "Option" {
                                // Handle Option<T> for primary key - extract inner type from generic arguments
                                if let Some(inner_type) = extract_option_inner_type(field_type) {
                                    type_conversion::generate_option_field_to_value_with_default(field_name, inner_type)
                                } else {
                                    quote! { sea_query::Value::String(None) }
                                }
                            } else {
                                // Not Option, use direct field-to-value conversion
                                type_conversion::generate_field_to_value(field_name, field_type)
                            }
                        } else {
                            quote! { sea_query::Value::String(None) }
                        }
                    }
                    _ => quote! { sea_query::Value::String(None) },
                };
                primary_key_value_expr = Some(pk_value_expr);
            }
        }

        // Generate Model field with serde rename attribute to match to_json() behavior
        // This ensures from_json() and to_json() use the same JSON key names (database column names)
        let column_name_lit = syn::LitStr::new(&column_name, field_name.span());
        model_fields.push(quote! {
            #[serde(rename = #column_name_lit)]
            pub #field_name: #field_type,
        });

        // Generate ModelTrait::get() match arm
        // Convert field value to sea_query::Value
        let field_value_to_value = match field_type {
            syn::Type::Path(syn::TypePath {
                path: syn::Path { segments, .. },
                ..
            }) => {
                // Check if this is Option<T> first (using segments.last() like extract_option_inner_type)
                // In syn's representation, Option<i32> is a single path segment with generic arguments,
                // so segments.len() is 1, not 2. We need to check the last segment for "Option".
                if let Some(last_segment) = segments.last() {
                    if last_segment.ident == "Option" {
                        // Handle Option<T> - extract inner type from generic arguments
                        if let Some(inner_type) = extract_option_inner_type(field_type) {
                            type_conversion::generate_option_field_to_value_with_default(field_name, inner_type)
                        } else {
                            quote! { sea_query::Value::String(None) }
                        }
                    } else {
                        // Not Option, use direct field-to-value conversion
                        type_conversion::generate_field_to_value(field_name, field_type)
                    }
                } else {
                    quote! { sea_query::Value::String(None) }
                }
            }
            _ => quote! { sea_query::Value::String(None) },
        };

        model_get_match_arms.push(quote! {
            Column::#column_variant => #field_value_to_value,
        });

        // Generate ModelTrait::set() match arm
        // Convert sea_query::Value to field value
        let value_to_field_value = match field_type {
            syn::Type::Path(syn::TypePath {
                path: syn::Path { segments, .. },
                ..
            }) => {
                // Check if this is Option<T> first
                if let Some(last_segment) = segments.last() {
                    if last_segment.ident == "Option" {
                        // Handle Option<T> - extract inner type
                        if let Some(inner_type) = extract_option_inner_type(field_type) {
                            if let Type::Path(inner_path) = inner_type {
                                // Check for serde_json::Value
                                let is_json_value = inner_path.path.segments.len() == 2
                                    && inner_path.path.segments.first().map(|s| s.ident.to_string()) == Some("serde_json".to_string())
                                    && inner_path.path.segments.last().map(|s| s.ident.to_string()) == Some("Value".to_string());
                                
                                if is_json_value {
                                    quote! {
                                        match value {
                                            sea_query::Value::Json(Some(v)) => {
                                                self.#field_name = Some(*v);
                                                Ok(())
                                            }
                                            sea_query::Value::Json(None) => {
                                                self.#field_name = None;
                                                Ok(())
                                            }
                                            _ => Err(lifeguard::ModelError::InvalidValueType {
                                                column: stringify!(#column_variant).to_string(),
                                                expected: "Json".to_string(),
                                                actual: format!("{:?}", value),
                                            })
                                        }
                                    }
                                } else if let Some(inner_segment) = inner_path.path.segments.last() {
                                    let inner_ident = inner_segment.ident.to_string();
                                    match inner_ident.as_str() {
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
                                                _ => Err(lifeguard::ModelError::InvalidValueType {
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
                                                _ => Err(lifeguard::ModelError::InvalidValueType {
                                                    column: stringify!(#column_variant).to_string(),
                                                    expected: "BigInt".to_string(),
                                                    actual: format!("{:?}", value),
                                                })
                                            }
                                        },
                                        "i16" => quote! {
                                            match value {
                                                sea_query::Value::SmallInt(Some(v)) => {
                                                    self.#field_name = Some(v);
                                                    Ok(())
                                                }
                                                sea_query::Value::SmallInt(None) => {
                                                    self.#field_name = None;
                                                    Ok(())
                                                }
                                                _ => Err(lifeguard::ModelError::InvalidValueType {
                                                    column: stringify!(#column_variant).to_string(),
                                                    expected: "SmallInt".to_string(),
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
                                                _ => Err(lifeguard::ModelError::InvalidValueType {
                                                    column: stringify!(#column_variant).to_string(),
                                                    expected: "String".to_string(),
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
                                                _ => Err(lifeguard::ModelError::InvalidValueType {
                                                    column: stringify!(#column_variant).to_string(),
                                                    expected: "Bool".to_string(),
                                                    actual: format!("{:?}", value),
                                                })
                                            }
                                        },
                                        "u8" => quote! {
                                            match value {
                                                sea_query::Value::SmallInt(Some(v)) => {
                                                    self.#field_name = Some(v as u8);
                                                    Ok(())
                                                }
                                                sea_query::Value::SmallInt(None) => {
                                                    self.#field_name = None;
                                                    Ok(())
                                                }
                                                _ => Err(lifeguard::ModelError::InvalidValueType {
                                                    column: stringify!(#column_variant).to_string(),
                                                    expected: "SmallInt".to_string(),
                                                    actual: format!("{:?}", value),
                                                })
                                            }
                                        },
                                        "u16" => quote! {
                                            match value {
                                                sea_query::Value::Int(Some(v)) => {
                                                    self.#field_name = Some(v as u16);
                                                    Ok(())
                                                }
                                                sea_query::Value::Int(None) => {
                                                    self.#field_name = None;
                                                    Ok(())
                                                }
                                                _ => Err(lifeguard::ModelError::InvalidValueType {
                                                    column: stringify!(#column_variant).to_string(),
                                                    expected: "Int".to_string(),
                                                    actual: format!("{:?}", value),
                                                })
                                            }
                                        },
                                        "u32" => quote! {
                                            match value {
                                                sea_query::Value::BigInt(Some(v)) => {
                                                    self.#field_name = Some(v as u32);
                                                    Ok(())
                                                }
                                                sea_query::Value::BigInt(None) => {
                                                    self.#field_name = None;
                                                    Ok(())
                                                }
                                                _ => Err(lifeguard::ModelError::InvalidValueType {
                                                    column: stringify!(#column_variant).to_string(),
                                                    expected: "BigInt".to_string(),
                                                    actual: format!("{:?}", value),
                                                })
                                            }
                                        },
                                        "u64" => quote! {
                                            match value {
                                                sea_query::Value::BigInt(Some(v)) => {
                                                    self.#field_name = Some(v as u64);
                                                    Ok(())
                                                }
                                                sea_query::Value::BigInt(None) => {
                                                    self.#field_name = None;
                                                    Ok(())
                                                }
                                                _ => Err(lifeguard::ModelError::InvalidValueType {
                                                    column: stringify!(#column_variant).to_string(),
                                                    expected: "BigInt".to_string(),
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
                                                _ => Err(lifeguard::ModelError::InvalidValueType {
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
                                                _ => Err(lifeguard::ModelError::InvalidValueType {
                                                    column: stringify!(#column_variant).to_string(),
                                                    expected: "Double".to_string(),
                                                    actual: format!("{:?}", value),
                                                })
                                            }
                                        },
                                        _ => quote! {
                                            Err(lifeguard::ModelError::InvalidValueType {
                                                column: stringify!(#column_variant).to_string(),
                                                expected: "supported type".to_string(),
                                                actual: format!("{:?}", value),
                                            })
                                        },
                                    }
                                } else {
                                    quote! {
                                        Err(lifeguard::ModelError::InvalidValueType {
                                            column: stringify!(#column_variant).to_string(),
                                            expected: "supported type".to_string(),
                                            actual: format!("{:?}", value),
                                        })
                                    }
                                }
                            } else {
                                quote! {
                                    Err(lifeguard::ModelError::InvalidValueType {
                                        column: stringify!(#column_variant).to_string(),
                                        expected: "supported type".to_string(),
                                        actual: format!("{:?}", value),
                                    })
                                }
                            }
                        } else {
                            quote! {
                                Err(lifeguard::ModelError::InvalidValueType {
                                    column: stringify!(#column_variant).to_string(),
                                    expected: "supported type".to_string(),
                                    actual: format!("{:?}", value),
                                })
                            }
                        }
                    } else {
                        // Not Option, check for serde_json::Value or primitive types
                        let is_json_value = segments.len() == 2
                            && segments.first().map(|s| s.ident.to_string()) == Some("serde_json".to_string())
                            && segments.last().map(|s| s.ident.to_string()) == Some("Value".to_string());
                        
                        if is_json_value {
                            quote! {
                                match value {
                                    sea_query::Value::Json(Some(v)) => {
                                        self.#field_name = *v;
                                        Ok(())
                                    }
                                    sea_query::Value::Json(None) => {
                                        Err(lifeguard::ModelError::InvalidValueType {
                                            column: stringify!(#column_variant).to_string(),
                                            expected: "Json(Some(_))".to_string(),
                                            actual: "Json(None)".to_string(),
                                        })
                                    }
                                    _ => Err(lifeguard::ModelError::InvalidValueType {
                                        column: stringify!(#column_variant).to_string(),
                                        expected: "Json".to_string(),
                                        actual: format!("{:?}", value),
                                    })
                                }
                            }
                        } else if let Some(segment) = segments.first() {
                            let ident_str = segment.ident.to_string();
                            match ident_str.as_str() {
                                "i32" => quote! {
                                    match value {
                                        sea_query::Value::Int(Some(v)) => {
                                            self.#field_name = v;
                                            Ok(())
                                        }
                                        sea_query::Value::Int(None) => {
                                            Err(lifeguard::ModelError::InvalidValueType {
                                                column: stringify!(#column_variant).to_string(),
                                                expected: "Int(Some(_))".to_string(),
                                                actual: "Int(None)".to_string(),
                                            })
                                        }
                                        _ => Err(lifeguard::ModelError::InvalidValueType {
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
                                            Err(lifeguard::ModelError::InvalidValueType {
                                                column: stringify!(#column_variant).to_string(),
                                                expected: "BigInt(Some(_))".to_string(),
                                                actual: "BigInt(None)".to_string(),
                                            })
                                        }
                                        _ => Err(lifeguard::ModelError::InvalidValueType {
                                            column: stringify!(#column_variant).to_string(),
                                            expected: "BigInt".to_string(),
                                            actual: format!("{:?}", value),
                                        })
                                    }
                                },
                                "i16" => quote! {
                                    match value {
                                        sea_query::Value::SmallInt(Some(v)) => {
                                            self.#field_name = v;
                                            Ok(())
                                        }
                                        sea_query::Value::SmallInt(None) => {
                                            Err(lifeguard::ModelError::InvalidValueType {
                                                column: stringify!(#column_variant).to_string(),
                                                expected: "SmallInt(Some(_))".to_string(),
                                                actual: "SmallInt(None)".to_string(),
                                            })
                                        }
                                        _ => Err(lifeguard::ModelError::InvalidValueType {
                                            column: stringify!(#column_variant).to_string(),
                                            expected: "SmallInt".to_string(),
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
                                            Err(lifeguard::ModelError::InvalidValueType {
                                                column: stringify!(#column_variant).to_string(),
                                                expected: "String(Some(_))".to_string(),
                                                actual: "String(None)".to_string(),
                                            })
                                        }
                                        _ => Err(lifeguard::ModelError::InvalidValueType {
                                            column: stringify!(#column_variant).to_string(),
                                            expected: "String".to_string(),
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
                                            Err(lifeguard::ModelError::InvalidValueType {
                                                column: stringify!(#column_variant).to_string(),
                                                expected: "Bool(Some(_))".to_string(),
                                                actual: "Bool(None)".to_string(),
                                            })
                                        }
                                        _ => Err(lifeguard::ModelError::InvalidValueType {
                                            column: stringify!(#column_variant).to_string(),
                                            expected: "Bool".to_string(),
                                            actual: format!("{:?}", value),
                                        })
                                    }
                                },
                                "u8" => quote! {
                                    match value {
                                        sea_query::Value::SmallInt(Some(v)) => {
                                            self.#field_name = v as u8;
                                            Ok(())
                                        }
                                        sea_query::Value::SmallInt(None) => {
                                            Err(lifeguard::ModelError::InvalidValueType {
                                                column: stringify!(#column_variant).to_string(),
                                                expected: "SmallInt(Some(_))".to_string(),
                                                actual: "SmallInt(None)".to_string(),
                                            })
                                        }
                                        _ => Err(lifeguard::ModelError::InvalidValueType {
                                            column: stringify!(#column_variant).to_string(),
                                            expected: "SmallInt".to_string(),
                                            actual: format!("{:?}", value),
                                        })
                                    }
                                },
                                "u16" => quote! {
                                    match value {
                                        sea_query::Value::Int(Some(v)) => {
                                            self.#field_name = v as u16;
                                            Ok(())
                                        }
                                        sea_query::Value::Int(None) => {
                                            Err(lifeguard::ModelError::InvalidValueType {
                                                column: stringify!(#column_variant).to_string(),
                                                expected: "Int(Some(_))".to_string(),
                                                actual: "Int(None)".to_string(),
                                            })
                                        }
                                        _ => Err(lifeguard::ModelError::InvalidValueType {
                                            column: stringify!(#column_variant).to_string(),
                                            expected: "Int".to_string(),
                                            actual: format!("{:?}", value),
                                        })
                                    }
                                },
                                "u32" => quote! {
                                    match value {
                                        sea_query::Value::BigInt(Some(v)) => {
                                            self.#field_name = v as u32;
                                            Ok(())
                                        }
                                        sea_query::Value::BigInt(None) => {
                                            Err(lifeguard::ModelError::InvalidValueType {
                                                column: stringify!(#column_variant).to_string(),
                                                expected: "BigInt(Some(_))".to_string(),
                                                actual: "BigInt(None)".to_string(),
                                            })
                                        }
                                        _ => Err(lifeguard::ModelError::InvalidValueType {
                                            column: stringify!(#column_variant).to_string(),
                                            expected: "BigInt".to_string(),
                                            actual: format!("{:?}", value),
                                        })
                                    }
                                },
                                "u64" => quote! {
                                    match value {
                                        sea_query::Value::BigInt(Some(v)) => {
                                            self.#field_name = v as u64;
                                            Ok(())
                                        }
                                        sea_query::Value::BigInt(None) => {
                                            Err(lifeguard::ModelError::InvalidValueType {
                                                column: stringify!(#column_variant).to_string(),
                                                expected: "BigInt(Some(_))".to_string(),
                                                actual: "BigInt(None)".to_string(),
                                            })
                                        }
                                        _ => Err(lifeguard::ModelError::InvalidValueType {
                                            column: stringify!(#column_variant).to_string(),
                                            expected: "BigInt".to_string(),
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
                                            Err(lifeguard::ModelError::InvalidValueType {
                                                column: stringify!(#column_variant).to_string(),
                                                expected: "Float(Some(_))".to_string(),
                                                actual: "Float(None)".to_string(),
                                            })
                                        }
                                        _ => Err(lifeguard::ModelError::InvalidValueType {
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
                                            Err(lifeguard::ModelError::InvalidValueType {
                                                column: stringify!(#column_variant).to_string(),
                                                expected: "Double(Some(_))".to_string(),
                                                actual: "Double(None)".to_string(),
                                            })
                                        }
                                        _ => Err(lifeguard::ModelError::InvalidValueType {
                                            column: stringify!(#column_variant).to_string(),
                                            expected: "Double".to_string(),
                                            actual: format!("{:?}", value),
                                        })
                                    }
                                },
                                _ => quote! {
                                    Err(lifeguard::ModelError::InvalidValueType {
                                        column: stringify!(#column_variant).to_string(),
                                        expected: "supported type".to_string(),
                                        actual: format!("{:?}", value),
                                    })
                                },
                            }
                        } else {
                            quote! {
                                Err(lifeguard::ModelError::InvalidValueType {
                                    column: stringify!(#column_variant).to_string(),
                                    expected: "supported type".to_string(),
                                    actual: format!("{:?}", value),
                                })
                            }
                        }
                    }
                } else {
                    quote! {
                        Err(lifeguard::ModelError::InvalidValueType {
                            column: stringify!(#column_variant).to_string(),
                            expected: "supported type".to_string(),
                            actual: format!("{:?}", value),
                        })
                    }
                }
            }
            _ => quote! {
                Err(lifeguard::ModelError::InvalidValueType {
                    column: stringify!(#column_variant).to_string(),
                    expected: "supported type".to_string(),
                    actual: format!("{:?}", value),
                })
            },
        };

        model_set_match_arms.push(quote! {
            Column::#column_variant => #value_to_field_value,
        });

        // Generate FromRow field extraction
        let column_name_str = column_name.as_str();
        let get_expr = {
            // Handle unsigned integer types
            let is_unsigned = match field_type {
                syn::Type::Path(syn::TypePath {
                    path: syn::Path { segments, .. },
                    ..
                }) => {
                    if let Some(segment) = segments.first() {
                        let ident_str = segment.ident.to_string();
                        matches!(ident_str.as_str(), "u8" | "u16" | "u32" | "u64")
                    } else {
                        false
                    }
                }
                _ => false,
            };

            if is_unsigned {
                let signed_type = match field_type {
                    syn::Type::Path(syn::TypePath {
                        path: syn::Path { segments, .. },
                        ..
                    }) => {
                        if let Some(segment) = segments.first() {
                            match segment.ident.to_string().as_str() {
                                "u8" => quote! { i16 },
                                "u16" => quote! { i32 },
                                "u32" | "u64" => quote! { i64 },
                                _ => quote! { i32 },
                            }
                        } else {
                            quote! { i32 }
                        }
                    }
                    _ => quote! { i32 },
                };

                quote! {
                    {
                        let val: #signed_type = row.try_get::<&str, #signed_type>(#column_name_str)?;
                        val as #field_type
                    }
                }
            } else {
                quote! {
                    row.try_get::<&str, #field_type>(#column_name_str)?
                }
            }
        };

        from_row_fields.push(quote! {
            #field_name: #get_expr,
        });
    }

    // Generate primary key value expression for ModelTrait
    let pk_value_impl = primary_key_value_expr
        .as_ref()
        .map(|expr| {
            quote! {
                #expr
            }
        })
        .unwrap_or_else(|| {
            quote! {
                // WARNING: No primary key found for this entity.
                // get_primary_key_value() returns String(None) when no primary key is defined.
                // Consider adding a #[primary_key] attribute to one of the fields.
                sea_query::Value::String(None)
            }
        });

    // Generate PrimaryKeyTrait and PrimaryKeyToColumn implementations (if primary key exists)
    let primary_key_trait_impls = if !primary_key_variant_idents.is_empty() && primary_key_type.is_some() {
        // Generate ValueType - tuple for composite keys, single type for single keys
        let value_type = if primary_key_types.len() == 1 {
            // Single primary key - extract inner type if Option<T>
            let pk_type = primary_key_types[0];
            if let Some(inner_type) = extract_option_inner_type(pk_type) {
                // Option<T> -> use inner type T
                quote! { #inner_type }
            } else {
                // Non-Option type -> use as-is
                quote! { #pk_type }
            }
        } else {
            // Composite primary key - generate tuple type
            let tuple_types: Vec<proc_macro2::TokenStream> = primary_key_types.iter().map(|pk_type| {
                if let Some(inner_type) = extract_option_inner_type(pk_type) {
                    // Option<T> -> use inner type T
                    quote! { #inner_type }
                } else {
                    // Non-Option type -> use as-is
                    quote! { #pk_type }
                }
            }).collect();
            quote! { (#(#tuple_types),*) }
        };
        
        // Generate auto_increment match arms
        // Each variant uses its own auto_increment value, supporting composite primary keys
        // with mixed auto_increment settings
        let auto_increment_arms = primary_key_variant_idents.iter().map(|(variant, auto_inc)| {
            if *auto_inc {
                quote! {
                    PrimaryKey::#variant => true,
                }
            } else {
                quote! {
                    PrimaryKey::#variant => false,
                }
            }
        });
        
        // Generate PrimaryKeyArity implementation
        // Determine arity at macro expansion time based on number of primary key variants
        // Lifeguard enhancement: granular arity variants for better type safety
        let primary_key_arity_impl = match primary_key_variant_idents.len() {
            1 => quote! {
                lifeguard::PrimaryKeyArity::Single
            },
            2 => quote! {
                lifeguard::PrimaryKeyArity::Tuple2
            },
            3 => quote! {
                lifeguard::PrimaryKeyArity::Tuple3
            },
            4 => quote! {
                lifeguard::PrimaryKeyArity::Tuple4
            },
            5 => quote! {
                lifeguard::PrimaryKeyArity::Tuple5
            },
            _ => quote! {
                lifeguard::PrimaryKeyArity::Tuple6Plus
            },
        };
        
        quote! {
            // Implement PrimaryKeyTrait
            impl lifeguard::PrimaryKeyTrait for PrimaryKey {
                type ValueType = #value_type;
                
                fn auto_increment(self) -> bool {
                    match self {
                        #(#auto_increment_arms)*
                    }
                }
            }
            
            // Implement PrimaryKeyToColumn
            impl lifeguard::PrimaryKeyToColumn for PrimaryKey {
                type Column = Column;
                
                fn to_column(self) -> Self::Column {
                    match self {
                        #(#primary_key_to_column_mappings)*
                    }
                }
            }
            
            // Implement PrimaryKeyArityTrait
            impl lifeguard::PrimaryKeyArityTrait for PrimaryKey {
                fn arity() -> lifeguard::PrimaryKeyArity {
                    #primary_key_arity_impl
                }
            }
        }
    } else {
        quote! {
            // No primary key defined - PrimaryKeyTrait, PrimaryKeyToColumn, and PrimaryKeyArityTrait not implemented
        }
    };

    // Generate Entity with nested DeriveEntity (like SeaORM)
    // This triggers nested expansion where DeriveEntity generates LifeModelTrait
    let expanded = quote! {
        // STEP 1: Generate Column enum FIRST (before Entity, so DeriveEntity can reference it)
        // Make it pub so it's visible to DeriveEntity during nested expansion
        #[doc = " Generated by lifeguard-derive"]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Column {
            #(#column_variants)*
        }

        // Implement Iden for Column
        impl sea_query::Iden for Column {
            fn unquoted(&self) -> &str {
                match self {
                    #(#iden_impls)*
                }
            }
        }

        // Implement IdenStatic for Column (required by LifeModelTrait::Column)
        impl sea_query::IdenStatic for Column {
            fn as_str(&self) -> &'static str {
                match self {
                    #(#iden_impls)*
                }
            }
        }

        // Note: ColumnTrait is implemented via blanket impl<T: IntoColumnRef> ColumnTrait for T {}
        // The default def() method returns ColumnDefinition::default().
        // To get actual column metadata, use ColumnDefinition::from_rust_type() directly
        // or extend ColumnTrait in the future with a more specific implementation.

        // Create a type alias to ensure Column is fully resolved before DeriveEntity expands
        // This helps the compiler resolve Column during nested macro expansion
        type _ColumnAlias = Column;

        // STEP 2: Generate Entity struct (unit struct, like SeaORM)
        #[doc = " Generated by lifeguard-derive"]
        #[derive(Copy, Clone, Debug, lifeguard_derive::DeriveEntity)]
        #[table_name = #table_name_lit]
        #[model = #model_name_lit]
        pub struct Entity;

        // Table name constant (for convenience, matches Entity::table_name())
        impl Entity {
            pub const TABLE_NAME: &'static str = #table_name_lit;
        }

        // NOTE: LifeEntityName, Iden, IdenStatic, Default, and LifeModelTrait are all
        // generated by DeriveEntity (nested expansion via #[derive(DeriveEntity)] above)
        // Do NOT generate them here to avoid conflicts

        // STEP 3: Generate PrimaryKey enum
        #[doc = " Generated by lifeguard-derive"]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum PrimaryKey {
            #(#primary_key_variants)*
        }

        // STEP 4: Generate PrimaryKeyTrait and PrimaryKeyToColumn implementations
        #primary_key_trait_impls

        // STEP 5: Generate Model struct (like SeaORM's expand_derive_model)
        // Note: Serialize/Deserialize are added for JSON support (core feature)
        #[doc = " Generated by lifeguard-derive"]
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
        pub struct #model_name {
            #(#model_fields)*
        }

        // STEP 6: Generate FromRow implementation (automatic, no separate derive needed)
        #[automatically_derived]
        impl lifeguard::FromRow for #model_name {
            fn from_row(row: &may_postgres::Row) -> Result<Self, may_postgres::Error> {
                Ok(Self {
                    #(#from_row_fields)*
                })
            }
        }

        // STEP 7: Generate ModelTrait implementation
        // NOTE: We use Column directly instead of Entity::Column to avoid E0223 errors
        // during macro expansion. Entity::Column will be available after DeriveEntity expands.
        #[automatically_derived]
        impl lifeguard::ModelTrait for #model_name {
            type Entity = Entity;

            fn get(&self, column: Column) -> sea_query::Value {
                match column {
                    #(#model_get_match_arms)*
                    // Note: Match is exhaustive - all Column variants must have corresponding fields
                    // This is enforced at compile time by Rust
                }
            }

            fn set(
                &mut self,
                column: Column,
                value: sea_query::Value,
            ) -> Result<(), lifeguard::ModelError> {
                match column {
                    #(#model_set_match_arms)*
                    // Note: Match is exhaustive - all Column variants must have corresponding fields
                    // This is enforced at compile time by Rust
                }
            }

            fn get_primary_key_value(&self) -> sea_query::Value {
                #pk_value_impl
            }
        }

        // STEP 8: LifeModelTrait is generated by DeriveEntity (nested expansion)
        // This happens in a separate expansion phase, allowing proper type resolution
        // DeriveEntity sets both type Model and type Column using the identifiers passed via attributes
    };

    TokenStream::from(expanded)
}
