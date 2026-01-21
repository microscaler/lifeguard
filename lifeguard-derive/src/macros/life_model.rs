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

    // Extract struct name, table name, and schema name
    let struct_name = &input.ident;
    let table_name = attributes::extract_table_name(&input.attrs)
        .unwrap_or_else(|| utils::snake_case(&struct_name.to_string()));
    let table_name_lit = syn::LitStr::new(&table_name, struct_name.span());
    let schema_name = attributes::extract_schema_name(&input.attrs);
    let schema_attr = schema_name.as_ref().map(|s| {
        let schema_lit = syn::LitStr::new(s, struct_name.span());
        quote! { #[schema_name = #schema_lit] }
    });
    
    // Parse table-level attributes (composite_unique, index, check, table_comment)
    let table_attrs = match attributes::parse_table_attributes(&input.attrs) {
        Ok(attrs) => attrs,
        Err(e) => return e.to_compile_error().into(),
    };

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
    let mut column_variant_idents = Vec::new(); // Store identifiers for all_columns() method
    let mut primary_key_variants = Vec::new();
    let mut primary_key_variant_idents = Vec::new(); // Store (variant identifier, auto_increment) tuples for trait implementations
    let mut primary_key_field_names = Vec::new(); // Store field names for value extraction
    let mut model_fields = Vec::new();
    let mut from_row_fields = Vec::new();
    let mut iden_impls = Vec::new();
    
    // Generate table definition expression
    let table_comment_expr = table_attrs.table_comment.as_ref().map(|tc| {
        let tc_lit = syn::LitStr::new(tc, struct_name.span());
        quote! { Some(#tc_lit.to_string()) }
    }).unwrap_or_else(|| quote! { None });
    
    // Generate composite unique constraints
    let composite_unique_expr = if table_attrs.composite_unique.is_empty() {
        quote! { Vec::new() }
    } else {
        let unique_vecs: Vec<_> = table_attrs.composite_unique.iter().map(|cols| {
            let col_lits: Vec<_> = cols.iter().map(|c| {
                let c_lit = syn::LitStr::new(c, struct_name.span());
                quote! { #c_lit.to_string() }
            }).collect();
            quote! { vec![#(#col_lits),*] }
        }).collect();
        quote! { vec![#(#unique_vecs),*] }
    };
    
    // Generate index definitions
    let indexes_expr = if table_attrs.indexes.is_empty() {
        quote! { Vec::new() }
    } else {
        let index_defs: Vec<_> = table_attrs.indexes.iter().map(|(name, cols, unique, where_clause)| {
            let name_lit = syn::LitStr::new(name, struct_name.span());
            let col_lits: Vec<_> = cols.iter().map(|c| {
                let c_lit = syn::LitStr::new(c, struct_name.span());
                quote! { #c_lit.to_string() }
            }).collect();
            let unique_lit = syn::LitBool::new(*unique, struct_name.span());
            let where_expr = where_clause.as_ref().map(|w| {
                let w_lit = syn::LitStr::new(w, struct_name.span());
                quote! { Some(#w_lit.to_string()) }
            }).unwrap_or_else(|| quote! { None });
            quote! {
                lifeguard::IndexDefinition {
                    name: #name_lit.to_string(),
                    columns: vec![#(#col_lits),*],
                    unique: #unique_lit,
                    partial_where: #where_expr,
                }
            }
        }).collect();
        quote! { vec![#(#index_defs),*] }
    };
    
    // Generate CHECK constraints
    let check_constraints_expr = if table_attrs.check_constraints.is_empty() {
        quote! { Vec::new() }
    } else {
        let check_lits: Vec<_> = table_attrs.check_constraints.iter().map(|c| {
            let c_lit = syn::LitStr::new(c, struct_name.span());
            quote! { #c_lit.to_string() }
        }).collect();
        quote! { vec![#(#check_lits),*] }
    };
    
    let table_definition_expr = quote! {
        lifeguard::TableDefinition {
            table_comment: #table_comment_expr,
            composite_unique: #composite_unique_expr,
            indexes: #indexes_expr,
            check_constraints: #check_constraints_expr,
        }
    };
    let mut model_get_match_arms = Vec::new();
    let mut model_set_match_arms = Vec::new();
    let mut get_by_column_name_match_arms: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut get_value_type_match_arms: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut primary_key_value_expr: Option<proc_macro2::TokenStream> = None;
    // Track primary key metadata for PrimaryKeyTrait
    let mut primary_key_type: Option<&Type> = None; // Keep for backward compatibility (first key only)
    let mut primary_key_types: Vec<&Type> = Vec::new(); // Track all primary key types for tuple ValueType
    let mut _primary_key_auto_increment = false; // Reserved for future PrimaryKeyTrait implementation
    let mut primary_key_to_column_mappings = Vec::new();
    // Track column definitions for ColumnTrait::def() implementations
    let mut column_def_match_arms = Vec::new();
    let mut enum_type_name_match_arms = Vec::new();

    for field in fields.iter() {
        let field_name = field.ident.as_ref().unwrap();
        let field_type = &field.ty;
        let column_name = attributes::extract_column_name(field)
            .unwrap_or_else(|| utils::snake_case(&field_name.to_string()));

        // Extract all column attributes
        let col_attrs = match attributes::parse_column_attributes(field) {
            Ok(attrs) => attrs,
            Err(err) => return err.to_compile_error().into(),
        };
        let is_primary_key = col_attrs.is_primary_key;
        let is_auto_increment = col_attrs.is_auto_increment;
        let is_ignored = col_attrs.is_ignored;

        // Validate: primary key fields cannot be skipped/ignored
        if is_primary_key && is_ignored {
            // Find the skip/ignore attribute to use its span for better error location
            if let Some(attr) = field.attrs.iter()
                .find(|attr| attr.path().is_ident("skip") || attr.path().is_ident("ignore")) {
                return syn::Error::new_spanned(
                    attr,
                    "Field cannot have both `#[primary_key]` and `#[skip]` (or `#[ignore]`) attributes. Primary key fields must be included in database operations.",
                )
                .to_compile_error()
                .into();
            } else {
                // Fallback to field name if attribute not found (shouldn't happen)
                return syn::Error::new_spanned(
                    field_name,
                    "Field cannot have both `#[primary_key]` and `#[skip]` (or `#[ignore]`) attributes. Primary key fields must be included in database operations.",
                )
                .to_compile_error()
                .into();
            }
        }

        // Skip ignored fields - they're not mapped to database columns
        // But we still need to add them to the Model struct and FromRow
        if is_ignored {
            // Still include in Model struct
            model_fields.push(quote! {
                pub #field_name: #field_type,
            });
            // Add to FromRow with default value (since they're not in database)
            // Use Default::default() if available, otherwise use a placeholder
            // For Option<T>, use None; for other types, try Default::default()
            let default_expr = if extract_option_inner_type(field_type).is_some() {
                quote! { None }
            } else {
                quote! { <#field_type as Default>::default() }
            };
            from_row_fields.push(quote! {
                #field_name: #default_expr,
            });
            // Don't generate Column enum variant, Iden, etc. for ignored fields
            continue;
        }
        
        // For non-ignored fields, add to Model struct with serde attributes

        // Generate Column enum variant (PascalCase)
        let column_variant = Ident::new(
            &utils::pascal_case(&field_name.to_string()),
            field_name.span(),
        );
        column_variant_idents.push(column_variant.clone()); // Store identifier for all_columns()
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
            primary_key_field_names.push(field_name.clone()); // Store field name for value extraction
            
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
        // Also add custom deserializers for f32/f64 to handle NaN/infinity string representations
        let column_name_lit = syn::LitStr::new(&column_name, field_name.span());
        
        // Check if this is a float type that needs custom deserialization
        let deserialize_attr = if type_conversion::is_f32_type(field_type) {
            Some(quote! {
                #[serde(deserialize_with = "lifeguard::deserialize_f32")]
            })
        } else if type_conversion::is_f64_type(field_type) {
            Some(quote! {
                #[serde(deserialize_with = "lifeguard::deserialize_f64")]
            })
        } else if type_conversion::is_option_f32_type(field_type) {
            Some(quote! {
                #[serde(deserialize_with = "lifeguard::deserialize_option_f32")]
            })
        } else if type_conversion::is_option_f64_type(field_type) {
            Some(quote! {
                #[serde(deserialize_with = "lifeguard::deserialize_option_f64")]
            })
        } else {
            None
        };
        
        model_fields.push(quote! {
            #[serde(rename = #column_name_lit)]
            #deserialize_attr
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
        
        // Generate get_by_column_name match arm
        // Note: column_name_lit is already defined above (line 180)
        get_by_column_name_match_arms.push(quote! {
            #column_name_lit => Some(self.get(Column::#column_variant)),
        });

        // Generate get_value_type match arm
        let type_string = type_conversion::type_to_string(field_type);
        let type_string_lit = syn::LitStr::new(&type_string, field_name.span());
        get_value_type_match_arms.push(quote! {
            Column::#column_variant => Some(#type_string_lit),
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
        
        // Determine nullability from Option<T> or #[nullable] attribute
        let is_nullable = col_attrs.is_nullable || extract_option_inner_type(field_type).is_some();
        
        let get_expr = {
            // Check for special types that need custom handling
            // First, extract the inner type if it's Option<T>
            let inner_type = extract_option_inner_type(field_type).unwrap_or(field_type);
            
            // Get type name string for comparison
            let type_name = match inner_type {
                syn::Type::Path(syn::TypePath {
                    path: syn::Path { segments, .. },
                    ..
                }) => {
                    // Get full type path (e.g., "uuid::Uuid", "chrono::NaiveDateTime")
                    let mut path_parts = Vec::new();
                    for segment in segments {
                        path_parts.push(segment.ident.to_string());
                    }
                    path_parts.join("::")
                }
                _ => String::new(),
            };
            
            // Check if this is uuid::Uuid or chrono::NaiveDateTime
            let (is_uuid, is_naive_datetime) = match inner_type {
                syn::Type::Path(syn::TypePath {
                    path: syn::Path { segments, .. },
                    ..
                }) => {
                    let last_seg = segments.last().map(|s| s.ident.to_string());
                    let is_uuid = last_seg.as_ref().map(|s| s == "Uuid").unwrap_or(false) ||
                                  type_name.contains("Uuid");
                    let is_naive_datetime = last_seg.as_ref().map(|s| s == "NaiveDateTime").unwrap_or(false) ||
                                           type_name.contains("NaiveDateTime");
                    (is_uuid, is_naive_datetime)
                }
                _ => (false, false),
            };
            
            // Handle uuid::Uuid - get as string and parse
            // Note: We use explicit error handling to avoid type inference issues with ?
            if is_uuid {
                if is_nullable {
                    quote! {
                        {
                            let uuid_str: Option<String> = match row.try_get(#column_name_str) {
                                Ok(v) => v,
                                Err(e) => return Err(e),
                            };
                            match uuid_str {
                                None => None,
                                Some(s) => {
                                    match uuid::Uuid::parse_str(&s) {
                                        Ok(u) => Some(u),
                                        Err(_) => return Err(may_postgres::Error::__private_api_timeout()),
                                    }
                                }
                            }
                        }
                    }
                } else {
                    quote! {
                        {
                            let uuid_str: String = match row.try_get(#column_name_str) {
                                Ok(v) => v,
                                Err(e) => return Err(e),
                            };
                            match uuid::Uuid::parse_str(&uuid_str) {
                                Ok(u) => u,
                                Err(_) => return Err(may_postgres::Error::__private_api_timeout()),
                            }
                        }
                    }
                }
            }
            // Handle chrono::NaiveDateTime - get as string and parse
            else if is_naive_datetime {
                if is_nullable {
                    quote! {
                        {
                            let dt_str: Option<String> = match row.try_get(#column_name_str) {
                                Ok(v) => v,
                                Err(e) => return Err(e),
                            };
                            match dt_str {
                                None => None,
                                Some(s) => {
                                    let dt = chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S%.f")
                                        .or_else(|_| chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S"))
                                        .or_else(|_| chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%dT%H:%M:%S%.f"))
                                        .or_else(|_| chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%dT%H:%M:%S"));
                                    match dt {
                                        Ok(d) => Some(d),
                                        Err(_) => return Err(may_postgres::Error::__private_api_timeout()),
                                    }
                                }
                            }
                        }
                    }
                } else {
                    quote! {
                        {
                            let dt_str: String = match row.try_get(#column_name_str) {
                                Ok(v) => v,
                                Err(e) => return Err(e),
                            };
                            let dt = chrono::NaiveDateTime::parse_from_str(&dt_str, "%Y-%m-%d %H:%M:%S%.f")
                                .or_else(|_| chrono::NaiveDateTime::parse_from_str(&dt_str, "%Y-%m-%d %H:%M:%S"))
                                .or_else(|_| chrono::NaiveDateTime::parse_from_str(&dt_str, "%Y-%m-%dT%H:%M:%S%.f"))
                                .or_else(|_| chrono::NaiveDateTime::parse_from_str(&dt_str, "%Y-%m-%dT%H:%M:%S"));
                            match dt {
                                Ok(d) => d,
                                Err(_) => return Err(may_postgres::Error::__private_api_timeout()),
                            }
                        }
                    }
                }
            }
            // Handle unsigned integer types
            else {
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
            }
        };

        from_row_fields.push(quote! {
            #field_name: #get_expr,
        });

        // Generate ColumnTrait::def() match arm
        // Determine nullability from Option<T> or #[nullable] attribute
        // Use extract_option_inner_type to properly detect Option<T> types
        let is_nullable = col_attrs.is_nullable || extract_option_inner_type(field_type).is_some();
        
        // Build ColumnDefinition struct literal
        let column_type_expr = col_attrs.column_type.as_ref().map(|ct| {
            let ct_lit = syn::LitStr::new(ct, field_name.span());
            quote! { Some(#ct_lit.to_string()) }
        }).unwrap_or_else(|| quote! { None });
        
        let default_value_expr = col_attrs.default_value.as_ref().map(|dv| {
            let dv_lit = syn::LitStr::new(dv, field_name.span());
            quote! { Some(#dv_lit.to_string()) }
        }).unwrap_or_else(|| quote! { None });
        
        let default_expr_expr = col_attrs.default_expr.as_ref().map(|de| {
            let de_lit = syn::LitStr::new(de, field_name.span());
            quote! { Some(#de_lit.to_string()) }
        }).unwrap_or_else(|| quote! { None });
        
        let renamed_from_expr = col_attrs.renamed_from.as_ref().map(|rf| {
            let rf_lit = syn::LitStr::new(rf, field_name.span());
            quote! { Some(#rf_lit.to_string()) }
        }).unwrap_or_else(|| quote! { None });
        
        let select_as_expr = col_attrs.select_as.as_ref().map(|sa| {
            let sa_lit = syn::LitStr::new(sa, field_name.span());
            quote! { Some(#sa_lit.to_string()) }
        }).unwrap_or_else(|| quote! { None });
        
        let save_as_expr = col_attrs.save_as.as_ref().map(|sa| {
            let sa_lit = syn::LitStr::new(sa, field_name.span());
            quote! { Some(#sa_lit.to_string()) }
        }).unwrap_or_else(|| quote! { None });
        
        let comment_expr = col_attrs.comment.as_ref().map(|c| {
            let c_lit = syn::LitStr::new(c, field_name.span());
            quote! { Some(#c_lit.to_string()) }
        }).unwrap_or_else(|| quote! { None });
        
        let foreign_key_expr = col_attrs.foreign_key.as_ref().map(|fk| {
            let fk_lit = syn::LitStr::new(fk, field_name.span());
            quote! { Some(#fk_lit.to_string()) }
        }).unwrap_or_else(|| quote! { None });
        
        let check_expr = col_attrs.check.as_ref().map(|c| {
            let c_lit = syn::LitStr::new(c, field_name.span());
            quote! { Some(#c_lit.to_string()) }
        }).unwrap_or_else(|| quote! { None });
        
        // Extract boolean attributes for use in quote! macro
        let is_unique_attr = col_attrs.is_unique;
        let is_indexed_attr = col_attrs.is_indexed;
        let is_auto_increment_attr = col_attrs.is_auto_increment;
        
        column_def_match_arms.push(quote! {
            Column::#column_variant => lifeguard::ColumnDefinition {
                column_type: #column_type_expr,
                nullable: #is_nullable,
                default_value: #default_value_expr,
                default_expr: #default_expr_expr,
                renamed_from: #renamed_from_expr,
                select_as: #select_as_expr,
                save_as: #save_as_expr,
                comment: #comment_expr,
                unique: #is_unique_attr,
                indexed: #is_indexed_attr,
                auto_increment: #is_auto_increment_attr,
                foreign_key: #foreign_key_expr,
                check: #check_expr,
            },
        });
        
        // Generate ColumnTrait::enum_type_name() match arm if enum_name is present
        if let Some(ref enum_name) = col_attrs.enum_name {
            let enum_name_lit = syn::LitStr::new(enum_name, field_name.span());
            enum_type_name_match_arms.push(quote! {
                Column::#column_variant => Some(#enum_name_lit.to_string()),
            });
        } else {
            enum_type_name_match_arms.push(quote! {
                Column::#column_variant => None,
            });
        }
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
    
    // Generate get_primary_key_identity() implementation
    let pk_identity_impl = if primary_key_variant_idents.is_empty() {
        // No primary key - return empty Identity with arity 0 to match get_primary_key_values()
        // Using Many(vec![]) ensures arity() returns 0, matching the empty vec![] from get_primary_key_values()
        quote! {
            lifeguard::Identity::Many(vec![])
        }
    } else {
        // Generate Identity based on number of primary keys
        // Convert Column enum variants to DynIden using column name strings
        match primary_key_variant_idents.len() {
            1 => {
                let col = &primary_key_variant_idents[0].0;
                // Get column name from IdenStatic::as_str()
                quote! {
                    {
                        use sea_query::IdenStatic;
                        lifeguard::Identity::Unary(sea_query::DynIden::from(Column::#col.as_str()))
                    }
                }
            }
            2 => {
                let col1 = &primary_key_variant_idents[0].0;
                let col2 = &primary_key_variant_idents[1].0;
                quote! {
                    {
                        use sea_query::IdenStatic;
                        lifeguard::Identity::Binary(
                            sea_query::DynIden::from(Column::#col1.as_str()),
                            sea_query::DynIden::from(Column::#col2.as_str())
                        )
                    }
                }
            }
            3 => {
                let col1 = &primary_key_variant_idents[0].0;
                let col2 = &primary_key_variant_idents[1].0;
                let col3 = &primary_key_variant_idents[2].0;
                quote! {
                    {
                        use sea_query::IdenStatic;
                        lifeguard::Identity::Ternary(
                            sea_query::DynIden::from(Column::#col1.as_str()),
                            sea_query::DynIden::from(Column::#col2.as_str()),
                            sea_query::DynIden::from(Column::#col3.as_str())
                        )
                    }
                }
            }
            _n => {
                // 4 or more keys - use Many variant
                let cols: Vec<_> = primary_key_variant_idents.iter().map(|(col, _)| {
                    quote! { sea_query::DynIden::from(Column::#col.as_str()) }
                }).collect();
                quote! {
                    {
                        use sea_query::IdenStatic;
                        lifeguard::Identity::Many(vec![#(#cols),*])
                    }
                }
            }
        }
    };
    
    // Generate get_primary_key_values() implementation
    // Reuse the same conversion logic as get_primary_key_value() for consistency
    let pk_values_impl = if primary_key_field_names.is_empty() {
        // No primary key - return empty vector
        quote! {
            vec![]
        }
    } else {
        // Generate code to extract all primary key values
        // We need to match the field types and use the same conversion as get_primary_key_value()
        // For now, use a simpler approach: collect all primary key values
        let mut value_exprs = Vec::new();
        for (idx, field_name) in primary_key_field_names.iter().enumerate() {
            // Get the field type for this primary key
            if idx < primary_key_types.len() {
                let field_type = primary_key_types[idx];
                // Use the same conversion logic as get_primary_key_value()
                // Check if it's Option<T> and handle accordingly
                if let Some(inner_type) = extract_option_inner_type(field_type) {
                    // Option<T> - use the same conversion as get() method
                    value_exprs.push(type_conversion::generate_option_field_to_value_with_default(field_name, inner_type));
                } else {
                    // Non-Option - use direct conversion
                    value_exprs.push(type_conversion::generate_field_to_value(field_name, field_type));
                }
            } else {
                // Fallback if types don't match (shouldn't happen)
                value_exprs.push(quote! { sea_query::Value::String(None) });
            }
        }
        quote! {
            vec![#(#value_exprs),*]
        }
    };

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

        // NOTE: We can't generate `impl ColumnTrait for Column` because it conflicts
        // with the blanket impl `impl<T: IntoColumnRef> ColumnTrait for T {}`.
        // Rust doesn't allow overriding blanket impls with specific impls.
        // 
        // For now, we'll generate helper functions that can be used to get column metadata.
        // Users can call these functions directly, or we can work on a better solution later.
        // 
        // TODO: Consider using specialization (when stable) or a different trait design
        // to allow macro-generated impls to override default trait methods.
        //
        // Alternative: Generate a separate trait or use associated constants/functions
        // that the default ColumnTrait implementations can call.
        
        // Generate helper functions for column definitions (workaround for blanket impl conflict)
        impl Column {
            /// Get column definition metadata (generated by LifeModel macro)
            pub fn column_def(self) -> lifeguard::ColumnDefinition {
                match self {
                    #(#column_def_match_arms)*
                }
            }
        }
        
        // Implement ColumnDefHelper trait to allow generic code to call column_def()
        impl lifeguard::query::column::column_trait::ColumnDefHelper for Column {
            fn column_def(self) -> lifeguard::ColumnDefinition {
                self.column_def()
            }
        }
        
        impl Column {
            
            /// Get enum type name if this column is an enum (generated by LifeModel macro)
            pub fn column_enum_type_name(self) -> Option<String> {
                match self {
                    #(#enum_type_name_match_arms)*
                }
            }
            
            /// Get all column variants (generated by LifeModel macro)
            ///
            /// Returns a static array of all Column enum variants.
            /// This is useful for iterating through all columns, e.g., when building SELECT queries
            /// that need to check for select_as expressions.
            pub fn all_columns() -> &'static [Column] {
                static COLUMNS: &[Column] = &[
                    #(Column::#column_variant_idents,)*
                ];
                COLUMNS
            }
            
            /// Get save_as expression for this column (generated by LifeModel macro)
            ///
            /// Returns the custom SQL expression to use when saving this column,
            /// or None if no custom expression is defined.
            /// This is a helper method that works around the blanket impl conflict
            /// for ColumnTrait::save_as().
            pub fn column_save_as(self) -> Option<String> {
                self.column_def().save_as
            }
        }

        // Create a type alias to ensure Column is fully resolved before DeriveEntity expands
        // This helps the compiler resolve Column during nested macro expansion
        type _ColumnAlias = Column;

        // STEP 2: Generate Entity struct (unit struct, like SeaORM)
        #[doc = " Generated by lifeguard-derive"]
        #[derive(Copy, Clone, Debug, lifeguard_derive::DeriveEntity)]
        #[table_name = #table_name_lit]
        #[model = #model_name_lit]
        #schema_attr
        pub struct Entity;

        // Table name constant (for convenience, matches Entity::table_name())
        impl Entity {
            pub const TABLE_NAME: &'static str = #table_name_lit;
            
            /// Get table definition metadata (for entity-driven migrations)
            ///
            /// Returns table-level metadata including composite unique constraints,
            /// indexes, CHECK constraints, and table comments.
            pub fn table_definition() -> lifeguard::TableDefinition {
                #table_definition_expr
            }
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
            
            fn get_primary_key_identity(&self) -> lifeguard::Identity {
                #pk_identity_impl
            }
            
            fn get_primary_key_values(&self) -> Vec<sea_query::Value> {
                #pk_values_impl
            }
            
            fn get_by_column_name(&self, column_name: &str) -> Option<sea_query::Value> {
                match column_name {
                    #(#get_by_column_name_match_arms)*
                    _ => None,
                }
            }
            
            fn get_value_type(&self, column: Column) -> Option<&'static str> {
                match column {
                    #(#get_value_type_match_arms)*
                }
            }
        }

        // STEP 8: LifeModelTrait is generated by DeriveEntity (nested expansion)
        // This happens in a separate expansion phase, allowing proper type resolution
        // DeriveEntity sets both type Model and type Column using the identifiers passed via attributes
    };

    TokenStream::from(expanded)
}
