//! `LifeRecord` derive macro implementation
#![allow(clippy::too_many_lines, clippy::explicit_iter_loop)] // Complex macro code

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, Data, DataStruct, DeriveInput, Fields, GenericArgument, Ident, LitStr,
    PathArguments, Type,
};

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

/// Derive macro for `LifeRecord` - generates mutable change-set objects
///
/// This macro generates:
/// - `Record` struct (mutable change-set with Option<T> fields)
/// - `from_model()` method (create from `LifeModel` for updates)
/// - `to_model()` → `Result<LifeModel, ActiveModelError>` (required fields must be set)
/// - `dirty_fields()` method (returns list of changed fields)
/// - `is_dirty()` method (checks if any fields changed)
/// - Setter methods for each field
pub fn derive_life_record(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    // Extract struct name
    let struct_name = &input.ident;
    let record_name = Ident::new(&format!("{struct_name}Record"), struct_name.span());
    let model_name = Ident::new(&format!("{struct_name}Model"), struct_name.span());

    // Extract struct fields
    let fields = match &input.data {
        Data::Struct(DataStruct {
            fields: Fields::Named(fields),
            ..
        }) => &fields.named,
        _ => {
            return syn::Error::new(
                struct_name.span(),
                "LifeRecord can only be derived for structs with named fields",
            )
            .to_compile_error()
            .into();
        }
    };

    // Collect valid column names from struct fields for validation
    let mut valid_columns = std::collections::HashSet::new();
    for field in fields {
        if let Some(field_name) = &field.ident {
            if attributes::has_attribute(field, "skip")
                || attributes::has_attribute(field, "ignore")
            {
                continue;
            }
            valid_columns.insert(
                attributes::extract_column_name(field)
                    .unwrap_or_else(|| utils::snake_case(&field_name.to_string())),
            );
        }
    }

    // Parse table-level attributes to get hook metadata
    let table_attrs = match attributes::parse_table_attributes(&input.attrs, &valid_columns) {
        Ok(attrs) => attrs,
        Err(e) => return e.to_compile_error().into(),
    };

    // Generate Entity name (assumes Entity struct exists from LifeModel)
    let entity_name = Ident::new("Entity", struct_name.span());

    // Process fields
    let mut record_fields = Vec::new();
    let mut record_field_names = Vec::new();
    let mut ignored_field_names = Vec::new();
    let mut ignored_field_defaults = Vec::new();
    let mut from_model_fields = Vec::new();
    let mut to_model_lets: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut to_model_struct_fields: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut dirty_fields_check = Vec::new();
    let mut setter_methods = Vec::new();

    // For ActiveModelTrait implementation
    let mut active_model_get_match_arms = Vec::new();
    let mut active_model_set_match_arms = Vec::new();
    let mut active_model_get_col_match_arms = Vec::new();
    let mut active_model_set_col_match_arms = Vec::new();
    let mut active_model_take_match_arms = Vec::new();
    let mut active_model_reset_fields = Vec::new();

    // Track primary keys for CRUD operations
    // We use separate vectors because tuples don't work well in quote! macro
    let mut primary_key_field_names = Vec::new();
    let mut primary_key_column_variants = Vec::new();
    let mut primary_key_auto_increment = Vec::new();

    // Generate CRUD operation code for each field
    let mut insert_column_checks = Vec::new(); // Check if field should be included in INSERT
    let mut update_set_clauses = Vec::new(); // SET clauses for UPDATE (uses self)
    let mut update_set_clauses_from_hooks = Vec::new(); // SET clauses for UPDATE (uses record_for_hooks, includes before_update changes)
    let mut delete_where_clauses = Vec::new(); // WHERE clauses for DELETE
    let mut returning_extractors: Vec<proc_macro2::TokenStream> = Vec::new(); // Code to extract returned PK values
    let mut to_json_field_conversions = Vec::new(); // Code to convert each field to JSON

    for field in fields.iter() {
        let field_name = match utils::field_ident(field) {
            Ok(i) => i,
            Err(e) => return e.to_compile_error().into(),
        };
        let field_type = &field.ty;

        // Check if field type is already Option<T>
        let is_already_option = extract_option_inner_type(field_type).is_some();

        // Extract the inner type from Option<T> if present
        // This is critical: conversion functions need the inner type (e.g., String), not Option<String>
        let inner_type = extract_option_inner_type(field_type).unwrap_or(field_type);

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
            if let Some(attr) = field
                .attrs
                .iter()
                .find(|attr| attr.path().is_ident("skip") || attr.path().is_ident("ignore"))
            {
                return syn::Error::new_spanned(
                    attr,
                    "Field cannot have both `#[primary_key]` and `#[skip]` (or `#[ignore]`) attributes. Primary key fields must be included in database operations.",
                )
                .to_compile_error()
                .into();
            }
            // Fallback to field name if attribute not found (shouldn't happen)
            return syn::Error::new_spanned(
                field_name,
                "Field cannot have both `#[primary_key]` and `#[skip]` (or `#[ignore]`) attributes. Primary key fields must be included in database operations.",
            )
            .to_compile_error()
            .into();
        }

        // Skip ignored fields - they're not included in database operations
        // But we still need to add them to the Record struct and conversion methods
        if is_ignored {
            // Still include in Record struct with original type (not Option<T>)
            record_fields.push(quote! {
                pub #field_name: #field_type,
            });

            // Add to from_model_fields - copy directly from model
            from_model_fields.push(quote! {
                #field_name: model.#field_name.clone(),
            });

            // Add to to_model struct - copy directly to model
            to_model_struct_fields.push(quote! {
                #field_name: self.#field_name.clone(),
            });

            // Track for new() method initialization (use Default::default() instead of None)
            ignored_field_names.push(field_name.clone());
            let default_expr = if extract_option_inner_type(field_type).is_some() {
                quote! { None }
            } else {
                quote! { <#field_type as Default>::default() }
            };
            ignored_field_defaults.push(default_expr);

            // Don't generate Column enum variant, ActiveModel methods, etc. for ignored fields
            continue;
        }

        // Extract column name for database (snake_case) and enum variant (PascalCase)
        let db_column_name = attributes::extract_column_name(field)
            .unwrap_or_else(|| utils::snake_case(&field_name.to_string()));
        let column_variant_name = utils::pascal_case(&field_name.to_string());
        let column_variant = Ident::new(&column_variant_name, field_name.span());

        // Track primary key information
        if is_primary_key {
            primary_key_field_names.push(field_name.clone());
            primary_key_column_variants.push(column_variant.clone());
            primary_key_auto_increment.push(is_auto_increment);
        }

        // Check if field is nullable (has #[nullable] attribute)
        let is_nullable = attributes::has_attribute(field, "nullable");

        // Generate record field type
        // If field is already Option<T>, use it directly (don't wrap in Option<> again)
        // Otherwise, wrap in Option<>
        let record_field_type = if is_already_option {
            // Field is already Option<T>, use it directly
            quote! { #field_type }
        } else {
            // Field is T, wrap in Option<T>
            quote! { Option<#field_type> }
        };

        record_fields.push(quote! {
            pub #field_name: #record_field_type,
        });

        // Store field name for struct initialization
        record_field_names.push(field_name);

        // Generate from_model field assignment
        // If field is already Option<T>, assign directly (don't wrap in Some())
        // Otherwise, wrap in Some()
        if is_already_option {
            from_model_fields.push(quote! {
                #field_name: model.#field_name.clone(),
            });
        } else {
            from_model_fields.push(quote! {
                #field_name: Some(model.#field_name.clone()),
            });
        }

        // Generate to_model bindings + struct fields (Result-based; no runtime expect on required fields)
        if is_already_option {
            to_model_lets.push(quote! {
                let #field_name = self.#field_name.clone();
            });
            to_model_struct_fields.push(quote! { #field_name, });
        } else if is_nullable {
            to_model_lets.push(quote! {
                let #field_name = self.#field_name.clone().unwrap_or_default();
            });
            to_model_struct_fields.push(quote! { #field_name, });
        } else {
            to_model_lets.push(quote! {
                let #field_name = self.#field_name.clone().ok_or_else(|| lifeguard::ActiveModelError::FieldRequired(stringify!(#field_name).to_string()))?;
            });
            to_model_struct_fields.push(quote! { #field_name, });
        }

        // Generate dirty field check
        // For Option<T> fields (both cases), check if Some
        dirty_fields_check.push(quote! {
            if self.#field_name.is_some() {
                dirty.push(stringify!(#field_name).to_string());
            }
        });

        // Generate setter method
        // If field is already Option<T>, setter accepts Option<T> directly
        // Otherwise, setter accepts T and wraps in Some()
        let setter_name = Ident::new(&format!("set_{field_name}"), field_name.span());
        if is_already_option {
            setter_methods.push(quote! {
                /// Set the #field_name field
                pub fn #setter_name(&mut self, value: #field_type) -> &mut Self {
                    self.#field_name = value;
                    self
                }
            });
        } else {
            setter_methods.push(quote! {
                /// Set the #field_name field
                pub fn #setter_name(&mut self, value: #field_type) -> &mut Self {
                    self.#field_name = Some(value);
                    self
                }
            });
        }

        // Generate ActiveModelTrait match arms
        // For get(), convert directly from Option<T> to Option<Value> (optimized, no to_model() needed)
        // Use inner_type for type conversion (e.g., String from Option<String>)
        let field_to_value_conversion =
            type_conversion::generate_option_field_to_value(field_name, inner_type);
        active_model_get_match_arms.push(quote! {
            <#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant => {
                #field_to_value_conversion
            }
        });

        active_model_get_col_match_arms.push(quote! {
            #db_column_name => {
                #field_to_value_conversion
            }
        });

        // For set(), generate type conversion code
        // Use inner_type for type conversion (e.g., String from Option<String>)
        let value_to_field_conversion = type_conversion::generate_value_to_option_field(
            field_name,
            inner_type,
            &column_variant,
        );
        active_model_set_match_arms.push(quote! {
            <#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant => {
                #value_to_field_conversion
            }
        });

        active_model_set_col_match_arms.push(quote! {
            #db_column_name => {
                #value_to_field_conversion
            }
        });

        // For take(), convert directly from Option<T> to Option<Value> and set field to None (optimized)
        // Use inner_type for type conversion (e.g., String from Option<String>)
        let field_to_value_conversion =
            type_conversion::generate_option_field_to_value(field_name, inner_type);
        active_model_take_match_arms.push(quote! {
            <#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant => {
                let value = #field_to_value_conversion;
                self.#field_name = None;
                value
            }
        });

        active_model_reset_fields.push(quote! {
            self.#field_name = None;
        });

        // Generate INSERT column/value collection
        // Skip auto-increment primary keys if not set
        // NOTE: These checks use record_for_hooks.get() to include modifications made by before_insert() hook
        if is_primary_key && is_auto_increment {
            // Auto-increment PK: include only if set
            // NOTE: If save_as is used on an auto-increment PK:
            // - If value is set: save_as expression is used (overrides database auto-increment)
            // - If value is not set: RETURNING clause is used to get database-generated value
            // This means save_as on auto-increment PKs will prevent the database from generating
            // the value when a value is explicitly provided. Users should be aware of this behavior.
            insert_column_checks.push(quote! {
                if let Some(value) = record_for_hooks.get(<#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant) {
                    columns.push(<#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant);
                    // Check for save_as expression
                    if let Some(save_expr) = <#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant.column_save_as() {
                        use lifeguard::query::column::definition::get_static_expr;
                        let static_str = get_static_expr(&save_expr);
                        exprs.push(sea_query::Expr::cust(static_str));
                    } else {
                        exprs.push(sea_query::Expr::val(value));
                    }
                }
            });
            // Track auto-increment PKs that need RETURNING (if not set)
            // Generate code to check if this PK needs RETURNING and extract if so
            // Database returns T (inner type), not Option<T>, so we use inner_type
            // Both Option<T> and T fields need to wrap the returned value in Some()
            // NOTE: Check updated_record (not record_for_hooks) since record_for_hooks is moved to updated_record
            // before this code is expanded. The check happens after the move at line 613.
            // NOTE: If save_as is present and value is set, RETURNING is not needed (expression handles it)
            // If save_as is present but value is not set, RETURNING is still needed to get the generated value
            returning_extractors.push(quote! {
                // Check if this auto-increment PK was not set and needs RETURNING
                // Note: If save_as is used and value is set, the expression is used instead of RETURNING
                if updated_record.get(<#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant).is_none() {
                    // Extract returned value for #field_name (database returns T, wrap in Some())
                    let pk_value: #inner_type = row.get(returning_idx);
                    returning_idx += 1;
                    updated_record.#field_name = Some(pk_value);
                }
            });
        } else if !is_primary_key {
            // Non-PK field: include if set
            insert_column_checks.push(quote! {
                if let Some(value) = record_for_hooks.get(<#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant) {
                    columns.push(<#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant);
                    // Check for save_as expression
                    if let Some(save_expr) = <#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant.column_save_as() {
                        use lifeguard::query::column::definition::get_static_expr;
                        let static_str = get_static_expr(&save_expr);
                        exprs.push(sea_query::Expr::cust(static_str));
                    } else {
                        exprs.push(sea_query::Expr::val(value));
                    }
                }
            });
        } else {
            // Non-auto-increment PK: include if set (required for composite keys)
            insert_column_checks.push(quote! {
                if let Some(value) = record_for_hooks.get(<#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant) {
                    columns.push(<#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant);
                    // Check for save_as expression
                    if let Some(save_expr) = <#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant.column_save_as() {
                        use lifeguard::query::column::definition::get_static_expr;
                        let static_str = get_static_expr(&save_expr);
                        exprs.push(sea_query::Expr::cust(static_str));
                    } else {
                        exprs.push(sea_query::Expr::val(value));
                    }
                }
            });
        }

        // Generate UPDATE SET clause (skip primary keys)
        if !is_primary_key {
            // SET clause using self (for backward compatibility, though not used in update() anymore)
            // Check for save_as expression
            let has_save_as = col_attrs.save_as.is_some();
            if has_save_as {
                update_set_clauses.push(quote! {
                    if self.get(<#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant).is_some() {
                        use lifeguard::query::column::definition::get_static_expr;
                        if let Some(save_expr) = <#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant.column_save_as() {
                            let static_str = get_static_expr(&save_expr);
                            query.value(<#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant, sea_query::Expr::cust(static_str));
                        }
                    }
                });
            } else {
                update_set_clauses.push(quote! {
                    if let Some(value) = self.get(<#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant) {
                        query.value(<#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant, sea_query::Expr::val(value));
                    }
                });
            }

            // SET clause using record_for_hooks (includes before_update() changes)
            // This is the one actually used in update() method
            if has_save_as {
                update_set_clauses_from_hooks.push(quote! {
                    if record_for_hooks.get(<#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant).is_some() {
                        use lifeguard::query::column::definition::get_static_expr;
                        if let Some(save_expr) = <#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant.column_save_as() {
                            let static_str = get_static_expr(&save_expr);
                            query.value(<#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant, sea_query::Expr::cust(static_str));
                        }
                    }
                });
            } else {
                update_set_clauses_from_hooks.push(quote! {
                    if let Some(value) = record_for_hooks.get(<#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant) {
                        query.value(<#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant, sea_query::Expr::val(value));
                    }
                });
            }
        }

        // Generate DELETE WHERE clause (only for primary keys)
        // CRITICAL: Use original PK values from self, NOT hook-modified values
        // This prevents silent data corruption if before_delete() modifies the primary key
        if is_primary_key {
            delete_where_clauses.push(quote! {
                if let Some(pk_value) = original_pk_values.get(&<#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant) {
                    use lifeguard::ColumnTrait;
                    let expr = <#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant.eq(pk_value.clone());
                    query.and_where(expr);
                } else {
                    return Err(lifeguard::ActiveModelError::PrimaryKeyRequired);
                }
            });
        }

        // Generate to_json() field conversion code
        // Only include fields that are set (get() returns Some)
        // Convert sea_query::Value to serde_json::Value
        // Use the database column name (snake_case) for JSON keys
        let json_key = db_column_name.clone();
        let json_key_lit = LitStr::new(&json_key, field_name.span());
        to_json_field_conversions.push(quote! {
            if let Some(value) = self.get(<#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant) {
                // Convert sea_query::Value to serde_json::Value
                let json_value = match value {
                    sea_query::Value::TinyInt(Some(v)) => serde_json::Value::Number(serde_json::Number::from(v as i64)),
                    sea_query::Value::TinyInt(None) => serde_json::Value::Null,
                    sea_query::Value::SmallInt(Some(v)) => serde_json::Value::Number(serde_json::Number::from(v as i64)),
                    sea_query::Value::SmallInt(None) => serde_json::Value::Null,
                    sea_query::Value::Int(Some(v)) => serde_json::Value::Number(serde_json::Number::from(v)),
                    sea_query::Value::Int(None) => serde_json::Value::Null,
                    sea_query::Value::BigInt(Some(v)) => serde_json::Value::Number(serde_json::Number::from(v)),
                    sea_query::Value::BigInt(None) => serde_json::Value::Null,
                    sea_query::Value::BigUnsigned(Some(v)) => serde_json::Value::Number(serde_json::Number::from(v)),
                    sea_query::Value::BigUnsigned(None) => serde_json::Value::Null,
                    sea_query::Value::Float(Some(v)) => {
                        // f32 to JSON number (may lose precision, but JSON only supports f64)
                        let v_f64 = v as f64;
                        // Handle special floating-point values that can't be represented as JSON numbers
                        if v_f64.is_nan() {
                            serde_json::Value::String("NaN".to_string())
                        } else if v_f64.is_infinite() {
                            if v_f64.is_sign_negative() {
                                serde_json::Value::String("-Infinity".to_string())
                            } else {
                                serde_json::Value::String("Infinity".to_string())
                            }
                        } else {
                            // Valid finite number - convert to JSON number
                            serde_json::Value::Number(serde_json::Number::from_f64(v_f64).unwrap_or_else(|| serde_json::Number::from(0)))
                        }
                    },
                    sea_query::Value::Float(None) => serde_json::Value::Null,
                    sea_query::Value::Double(Some(v)) => {
                        // Handle special floating-point values that can't be represented as JSON numbers
                        if v.is_nan() {
                            serde_json::Value::String("NaN".to_string())
                        } else if v.is_infinite() {
                            if v.is_sign_negative() {
                                serde_json::Value::String("-Infinity".to_string())
                            } else {
                                serde_json::Value::String("Infinity".to_string())
                            }
                        } else {
                            // Valid finite number - convert to JSON number
                            serde_json::Value::Number(serde_json::Number::from_f64(v).unwrap_or_else(|| serde_json::Number::from(0)))
                        }
                    },
                    sea_query::Value::Double(None) => serde_json::Value::Null,
                    sea_query::Value::Bool(Some(v)) => serde_json::Value::Bool(v),
                    sea_query::Value::Bool(None) => serde_json::Value::Null,
                    sea_query::Value::String(Some(v)) => serde_json::Value::String(v),
                    sea_query::Value::String(None) => serde_json::Value::Null,
                    sea_query::Value::Bytes(Some(v)) => {
                        // Convert bytes to JSON array of numbers
                        serde_json::Value::Array(v.iter().map(|&b| serde_json::Value::Number(serde_json::Number::from(b))).collect())
                    },
                    sea_query::Value::Bytes(None) => serde_json::Value::Null,
                    sea_query::Value::Json(Some(v)) => (*v).clone(),
                    sea_query::Value::Json(None) => serde_json::Value::Null,
                    _ => {
                        // Unknown value type - convert to string representation
                        // This handles any Value variants we haven't explicitly covered
                        serde_json::Value::String(format!("{:?}", value))
                    },
                };
                map.insert(#json_key_lit.to_string(), json_value);
            }
        });
    }

    // Generate primary key check code for save()
    // If there are no primary keys, save() should always do insert
    let has_primary_keys = !primary_key_field_names.is_empty();
    let mut save_pk_checks = Vec::new();
    for field_name in primary_key_field_names.iter() {
        save_pk_checks.push(quote! {
            record_for_hooks.#field_name.is_some() &&
        });
    }

    // Generate save logic that handles both cases: entities with and without primary keys
    let save_pk_logic = if has_primary_keys {
        quote! {
            {
                // Check if primary key is set (using record_for_hooks)
                let has_primary_key = #(#save_pk_checks)* true;

                if has_primary_key {
                    // Try to update first. If record doesn't exist (RecordNotFound),
                    // fall back to insert. This implements upsert behavior.
                    match record_for_hooks.update(executor) {
                        Ok(model) => Ok(model),
                        Err(lifeguard::ActiveModelError::RecordNotFound) => {
                            // Update affected zero rows - record doesn't exist, try insert
                            record_for_hooks.insert(executor)
                        },
                        Err(e) => Err(e), // Propagate other errors (DatabaseError, etc.)
                    }
                } else {
                    // No primary key set, do insert
                    record_for_hooks.insert(executor)
                }
            }
        }
    } else {
        quote! {
            {
                // Entity has no primary keys - always do insert
                record_for_hooks.insert(executor)
            }
        }
    };

    // Generate conditional code for methods that require primary keys
    let update_pk_check = if has_primary_keys {
        quote! {
            // Check primary key is set
            #(
                if self.#primary_key_field_names.is_none() {
                    return Err(lifeguard::ActiveModelError::PrimaryKeyRequired);
                }
            )*
        }
    } else {
        quote! {
            // Entity has no primary keys - update is not supported
            return Err(lifeguard::ActiveModelError::Other("Cannot update entity without primary key".to_string()));
        }
    };

    let delete_pk_check = if has_primary_keys {
        quote! {}
    } else {
        quote! {
            // Entity has no primary keys - delete is not supported
            return Err(lifeguard::ActiveModelError::Other("Cannot delete entity without primary key".to_string()));
        }
    };

    let parse_hook = |hook: &Option<String>| -> Option<proc_macro2::TokenStream> {
        hook.as_ref().and_then(|h| h.parse().ok())
    };

    let before_insert_impl = if let Some(hook_ts) = parse_hook(&table_attrs.before_insert) {
        if table_attrs.auto_timestamp {
            quote! {
                self.created_at = Some(chrono::Utc::now().naive_utc().to_string());
                self.updated_at = Some(chrono::Utc::now().naive_utc().to_string());
                #hook_ts(self)?;
                Ok(())
            }
        } else {
            quote! {
                #hook_ts(self)?;
                Ok(())
            }
        }
    } else if table_attrs.auto_timestamp {
        quote! {
            self.created_at = Some(chrono::Utc::now().naive_utc().to_string());
            self.updated_at = Some(chrono::Utc::now().naive_utc().to_string());
            Ok(())
        }
    } else {
        quote! { Ok(()) }
    };

    let before_update_impl = if let Some(hook_ts) = parse_hook(&table_attrs.before_update) {
        if table_attrs.auto_timestamp {
            quote! {
                self.updated_at = Some(chrono::Utc::now().naive_utc().to_string());
                #hook_ts(self)?;
                Ok(())
            }
        } else {
            quote! {
                #hook_ts(self)?;
                Ok(())
            }
        }
    } else if table_attrs.auto_timestamp {
        quote! {
            self.updated_at = Some(chrono::Utc::now().naive_utc().to_string());
            Ok(())
        }
    } else {
        quote! { Ok(()) }
    };

    let after_insert_impl = if let Some(hook_ts) = parse_hook(&table_attrs.after_insert) {
        quote! { #hook_ts(self, model)?; Ok(()) }
    } else {
        quote! { Ok(()) }
    };

    let after_update_impl = if let Some(hook_ts) = parse_hook(&table_attrs.after_update) {
        quote! { #hook_ts(self, model)?; Ok(()) }
    } else {
        quote! { Ok(()) }
    };

    let before_delete_impl = if let Some(hook_ts) = parse_hook(&table_attrs.before_delete) {
        quote! { #hook_ts(self)?; Ok(()) }
    } else {
        quote! { Ok(()) }
    };

    let after_delete_impl = if let Some(hook_ts) = parse_hook(&table_attrs.after_delete) {
        quote! { #hook_ts(self)?; Ok(()) }
    } else {
        quote! { Ok(()) }
    };

    let build_delete_query_ts = if table_attrs.soft_delete {
        let set_updated_at = if table_attrs.auto_timestamp {
            quote! {
                query.value(<#entity_name as lifeguard::LifeModelTrait>::Column::UpdatedAt, sea_query::Expr::val(chrono::Utc::now().naive_utc()));
            }
        } else {
            quote! {}
        };

        quote! {
            let mut query = Query::update();
            let entity = #entity_name::default();
            query.table(entity.clone());

            // Soft delete: set deleted_at to current timestamp
            query.value(<#entity_name as lifeguard::LifeModelTrait>::Column::DeletedAt, sea_query::Expr::val(chrono::Utc::now().naive_utc()));
            #set_updated_at

            #(#delete_where_clauses)*
        }
    } else {
        quote! {
            let mut query = Query::delete();
            let entity = #entity_name::default();
            query.from_table(entity.clone());

            #(#delete_where_clauses)*
        }
    };

    // Generate the expanded code
    let expanded = quote! {
        // Record struct (mutable change-set)
        #[derive(Debug, Clone)]
        pub struct #record_name {
            #(#record_fields)*
            pub __graph: lifeguard::active_model::graph::GraphContainer<Self>,
        }

        impl #record_name {
            /// Initialize GraphState if empty and return a mutable reference to it.
            pub fn graph_mut(&mut self) -> &mut lifeguard::active_model::graph::GraphState<Self> {
                self.__graph.0
                    .get_or_insert_with(|| {
                        Box::new(lifeguard::active_model::graph::GraphState::<Self>::new())
                    })
                    .as_mut()
            }

            /// Create a new empty record (all fields None)
            /// Useful for inserts where you set only the fields you need
            pub fn new() -> Self {
                Self {
                    #(
                        #record_field_names: None,
                    )*
                    #(
                        #ignored_field_names: #ignored_field_defaults,
                    )*
                    __graph: lifeguard::active_model::graph::GraphContainer::default(),
                }
            }

            /// Create a record from a Model (for updates)
            /// All fields are set to Some(value) from the model
            pub fn from_model(model: &#model_name) -> Self {
                Self {
                    #(#from_model_fields)*
                    __graph: lifeguard::active_model::graph::GraphContainer::default(),
                }
            }

            /// Convert the record to a model.
            ///
            /// Required (non-nullable) fields must be `Some`; if any are unset, returns
            /// [`ActiveModelError::FieldRequired`](lifeguard::ActiveModelError::FieldRequired).
            /// Nullable columns use [`Default::default()`] when unset.
            pub fn to_model(&self) -> Result<#model_name, lifeguard::ActiveModelError> {
                #(#to_model_lets)*
                Ok(#model_name {
                    #(#to_model_struct_fields)*
                })
            }

            /// Get a list of dirty (changed) field names
            /// Returns a vector of field names that have been set (are Some)
            pub fn dirty_fields(&self) -> Vec<String> {
                let mut dirty = Vec::new();
                #(#dirty_fields_check)*
                dirty
            }

            /// Check if any fields have been changed
            /// Returns true if at least one field is Some
            pub fn is_dirty(&self) -> bool {
                !self.dirty_fields().is_empty()
            }

            #(#setter_methods)*
        }

        impl Default for #record_name {
            fn default() -> Self {
                Self::new()
            }
        }

        // Implement ActiveModelTrait for Record
        impl lifeguard::ActiveModelTrait for #record_name {
            type Entity = #entity_name;
            type Model = #model_name;

            fn get(&self, column: <#entity_name as lifeguard::LifeModelTrait>::Column) -> Option<sea_query::Value> {
                match column {
                    #(#active_model_get_match_arms)*
                }
            }

            fn set(&mut self, column: <#entity_name as lifeguard::LifeModelTrait>::Column, value: sea_query::Value) -> Result<(), lifeguard::ActiveModelError> {
                match column {
                    #(#active_model_set_match_arms)*
                }
            }

            fn take(&mut self, column: <#entity_name as lifeguard::LifeModelTrait>::Column) -> Option<sea_query::Value> {
                match column {
                    #(#active_model_take_match_arms)*
                }
            }

            fn get_col(&self, col_name: &str) -> Option<sea_query::Value> {
                match col_name {
                    #(#active_model_get_col_match_arms)*
                    _ => None,
                }
            }

            fn set_col(&mut self, col_name: &str, value: sea_query::Value) -> Result<(), lifeguard::ActiveModelError> {
                match col_name {
                    #(#active_model_set_col_match_arms)*
                    _ => Err(lifeguard::ActiveModelError::Other(format!("Column string not found on record: {}", col_name)))
                }
            }

            fn reset(&mut self) {
                #(#active_model_reset_fields)*
            }

            fn graph_mut(&mut self) -> &mut lifeguard::active_model::graph::GraphState<Self> {
                self.__graph.0
                    .get_or_insert_with(|| {
                        Box::new(lifeguard::active_model::graph::GraphState::<Self>::new())
                    })
                    .as_mut()
            }

            fn insert(&self, executor: &dyn lifeguard::LifeExecutor) -> Result<Self::Model, lifeguard::ActiveModelError> {
                use sea_query::{Query, PostgresQueryBuilder};
                use lifeguard::{LifeEntityName, ActiveModelBehavior};

                // Call before_insert hook
                let mut record_for_hooks = self.clone();
                record_for_hooks.before_insert()?;
                lifeguard::active_model::validation::run_validators(
                    &record_for_hooks,
                    lifeguard::active_model::validate_op::ValidateOp::Insert,
                )?;

                // Build INSERT statement
                let mut query = Query::insert();
                let entity = #entity_name::default();
                query.into_table(entity);

                // Collect columns and expressions (skip auto-increment PKs if not set)
                // Use Expr instead of Value to support save_as custom expressions
                let mut columns = Vec::new();
                let mut exprs = Vec::new();

                #(#insert_column_checks)*

                if columns.is_empty() {
                    return Err(lifeguard::ActiveModelError::Other("No fields set for insert".to_string()));
                }

                // Add columns and expressions to query
                // SeaQuery API: columns() takes items that implement IntoIden (Column implements Iden, which provides IntoIden via blanket impl)
                // values_panic() takes an iterator of Expr
                query.columns(columns.iter().copied());
                query.values_panic(exprs.iter().cloned());

                // Check if we need RETURNING clause for auto-increment primary keys
                // Track which auto-increment PKs were not set and need RETURNING
                // NOTE: Check record_for_hooks to see if PK is still unset after before_insert() hook
                let mut needs_returning = false;
                let mut returning_cols: Vec<<#entity_name as lifeguard::LifeModelTrait>::Column> = Vec::new();
                #(
                    if record_for_hooks.get(<#entity_name as lifeguard::LifeModelTrait>::Column::#primary_key_column_variants).is_none() && #primary_key_auto_increment {
                        needs_returning = true;
                        returning_cols.push(<#entity_name as lifeguard::LifeModelTrait>::Column::#primary_key_column_variants);
                    }
                )*

                // Add RETURNING clause if needed
                // SeaQuery's returning() expects ReturningClause enum
                // ReturningClause::Columns expects Vec<ColumnRef>
                if needs_returning {
                    use sea_query::{ReturningClause, ColumnRef};
                    // Convert Column enum variants to ColumnRef
                    let returning_vec: Vec<ColumnRef> = returning_cols.iter().copied().map(|c| ColumnRef::from(c)).collect();
                    query.returning(ReturningClause::Columns(returning_vec));
                }

                // Build SQL
                let (sql, sql_values) = query.build(PostgresQueryBuilder);

                // Create a mutable copy of self to update with returned PK values
                // Use the record that went through before_insert hook
                let mut updated_record = record_for_hooks;

                // Execute query and handle RETURNING if needed
                // Use *_values so pooled executors can marshal binds across the pool channel.
                if needs_returning {
                    let row = executor.query_one_values(&sql, &sql_values).map_err(|e| {
                        lifeguard::ActiveModelError::DatabaseError(e.to_string())
                    })?;

                    // Extract returned primary key values and update the record
                    let mut returning_idx = 0usize;
                    #(#returning_extractors)*
                } else {
                    executor.execute_values(&sql, &sql_values).map_err(|e| {
                        lifeguard::ActiveModelError::DatabaseError(e.to_string())
                    })?;
                }

                // Construct the model from the updated record
                let model = updated_record.to_model()?;

                // Call after_insert hook
                updated_record.after_insert(&model)?;

                // Transparent Cache Write-Through
                if let Some(cache) = executor.cache_provider() {
                    let table_name = <#entity_name as lifeguard::LifeEntityName>::table_name(&#entity_name::default());
                    if let Some(pk_value) = lifeguard::ModelTrait::get_primary_key_values(&model).first() {
                        let id_str = match pk_value {
                            sea_query::Value::BigInt(Some(v)) => v.to_string(),
                            sea_query::Value::Int(Some(v)) => v.to_string(),
                            _ => "".to_string(),
                        };
                        if !id_str.is_empty() {
                            let cache_key = format!("lifeguard:model:{}:{}", table_name, id_str);
                            if let Ok(json_str) = serde_json::to_string(&model) {
                                let _ = cache.set(&cache_key, &json_str, Some(3600));
                            }
                        }
                    }
                }

                // Return the model
                Ok(model)
            }

            fn update(&self, executor: &dyn lifeguard::LifeExecutor) -> Result<Self::Model, lifeguard::ActiveModelError> {
                use sea_query::{Query, PostgresQueryBuilder, Expr};
                use lifeguard::{LifeEntityName, ActiveModelBehavior};
                use std::collections::HashMap;

                #update_pk_check

                // CRITICAL: Store original PK values BEFORE calling hooks
                // This prevents silent data corruption if before_update() modifies the primary key
                // The WHERE clause must use the original PK to target the correct record
                let mut original_pk_values: HashMap<<#entity_name as lifeguard::LifeModelTrait>::Column, sea_query::Value> = HashMap::new();
                #(
                    if let Some(pk_value) = self.get(<#entity_name as lifeguard::LifeModelTrait>::Column::#primary_key_column_variants) {
                        original_pk_values.insert(<#entity_name as lifeguard::LifeModelTrait>::Column::#primary_key_column_variants, pk_value);
                    } else {
                        return Err(lifeguard::ActiveModelError::PrimaryKeyRequired);
                    }
                )*

                // Call before_update hook
                let mut record_for_hooks = self.clone();
                record_for_hooks.before_update()?;
                lifeguard::active_model::validation::run_validators(
                    &record_for_hooks,
                    lifeguard::active_model::validate_op::ValidateOp::Update,
                )?;

                // Build UPDATE statement
                let mut query = Query::update();
                let entity = #entity_name::default();
                query.table(entity);

                // Add SET clauses for dirty fields (skip primary keys)
                // Use record_for_hooks to include any changes made in before_update()
                // This ensures before_update() changes are included in the UPDATE query
                #(#update_set_clauses_from_hooks)*

                // Add WHERE clause for primary keys (use ORIGINAL PK values, not hook-modified)
                // CRITICAL: Using original PK ensures we update the correct record even if
                // before_update() hook modifies the primary key
                #(
                    if let Some(pk_value) = original_pk_values.get(&<#entity_name as lifeguard::LifeModelTrait>::Column::#primary_key_column_variants) {
                        use lifeguard::ColumnTrait;
                        let expr = <#entity_name as lifeguard::LifeModelTrait>::Column::#primary_key_column_variants.eq(pk_value.clone());
                        query.and_where(expr);
                    } else {
                        return Err(lifeguard::ActiveModelError::PrimaryKeyRequired);
                    }
                )*

                // Build SQL
                let (sql, sql_values) = query.build(PostgresQueryBuilder);

                let rows_affected = executor.execute_values(&sql, &sql_values).map_err(|e| {
                    lifeguard::ActiveModelError::DatabaseError(e.to_string())
                })?;

                // Check if any rows were affected
                if rows_affected == 0 {
                    return Err(lifeguard::ActiveModelError::RecordNotFound);
                }

                // Construct the model by fetching it from the database to ensure all fields are properly loaded
                let mut find_query = <#entity_name as lifeguard::LifeModelTrait>::find();
                #(
                    if let Some(pk_value) = original_pk_values.get(&<#entity_name as lifeguard::LifeModelTrait>::Column::#primary_key_column_variants) {
                        use lifeguard::ColumnTrait;
                        find_query = find_query.filter(<#entity_name as lifeguard::LifeModelTrait>::Column::#primary_key_column_variants.eq(pk_value.clone()));
                    }
                )*
                let model = find_query.find_one(&executor)
                    .map_err(|e| lifeguard::ActiveModelError::DatabaseError(e.to_string()))?
                    .ok_or(lifeguard::ActiveModelError::RecordNotFound)?;

                // The record used in the after_update hook needs to represent the updated state
                // We recreate it from the fetched model
                let mut record_for_hooks = Self::from_model(&model);

                // Call after_update hook
                record_for_hooks.after_update(&model)?;

                // Transparent Cache Write-Through
                if let Some(cache) = executor.cache_provider() {
                    let table_name = <#entity_name as lifeguard::LifeEntityName>::table_name(&#entity_name::default());
                    if let Some(pk_value) = lifeguard::ModelTrait::get_primary_key_values(&model).first() {
                        let id_str = match pk_value {
                            sea_query::Value::BigInt(Some(v)) => v.to_string(),
                            sea_query::Value::Int(Some(v)) => v.to_string(),
                            _ => "".to_string(),
                        };
                        if !id_str.is_empty() {
                            let cache_key = format!("lifeguard:model:{}:{}", table_name, id_str);
                            if let Ok(json_str) = serde_json::to_string(&model) {
                                let _ = cache.set(&cache_key, &json_str, Some(3600));
                            }
                        }
                    }
                }

                // Return the updated model
                Ok(model)
            }

            fn save(&self, executor: &dyn lifeguard::LifeExecutor) -> Result<Self::Model, lifeguard::ActiveModelError> {
                use lifeguard::ActiveModelBehavior;

                // Call before_save hook
                let mut record_for_hooks = self.clone();
                record_for_hooks.before_save()?;

                // Execute save logic (insert or update) using record_for_hooks
                // This handles both entities with and without primary keys correctly
                // insert()/update() will clone record_for_hooks again and apply their own hooks,
                // then return a model that includes all modifications (including auto-increment PKs from RETURNING)
                let model = #save_pk_logic?;

                // The record used in the after_save hook needs to represent the updated state
                // We recreate it from the returned model to ensure consistency
                let mut record_for_after_save = Self::from_model(&model);

                // Call after_save hook
                record_for_after_save.after_save(&model)?;

                Ok(model)
            }

            fn save_graph(&mut self, executor: &dyn lifeguard::LifeExecutor) -> Result<Self::Model, lifeguard::ActiveModelError> {
                // 1. Drain graph edges safely
                let mut edges = Vec::new();
                if let Some(graph) = &mut self.__graph.0 {
                    edges = std::mem::take(&mut graph.edges);
                }

                // 2. Filter BelongsTo and HasMany
                let mut belongs_to_edges = Vec::new();
                let mut has_many_edges = Vec::new();

                for edge in edges {
                    match edge {
                        lifeguard::active_model::graph::GraphEdge::BelongsTo(action) => {
                            belongs_to_edges.push(action);
                        },
                        lifeguard::active_model::graph::GraphEdge::HasMany(action) => {
                            has_many_edges.push(action);
                        }
                    }
                }

                // 3. Execute BelongsTo hooks (mutates self to update foreign keys with parent's PK)
                for action in belongs_to_edges {
                    action(self, executor)?;
                }

                // 4. Save the root record
                let model = lifeguard::ActiveModelTrait::save(self, executor)?;

                // 5. Update the root record with newly generated database PKs
                *self = Self::from_model(&model);

                // 6. Execute HasMany hooks (reads PKs from self to sync down to children)
                for action in has_many_edges {
                    action(self, executor)?;
                }

                Ok(model)
            }

            fn delete(&self, executor: &dyn lifeguard::LifeExecutor) -> Result<(), lifeguard::ActiveModelError> {
                use sea_query::{Query, PostgresQueryBuilder, Expr};
                use lifeguard::{LifeEntityName, ActiveModelBehavior};
                use std::collections::HashMap;

                #delete_pk_check

                // CRITICAL: Store original PK values BEFORE calling hooks
                // This prevents silent data corruption if before_delete() modifies the primary key
                // The WHERE clause must use the original PK to target the correct record
                let mut original_pk_values: HashMap<<#entity_name as lifeguard::LifeModelTrait>::Column, sea_query::Value> = HashMap::new();
                #(
                    if let Some(pk_value) = self.get(<#entity_name as lifeguard::LifeModelTrait>::Column::#primary_key_column_variants) {
                        original_pk_values.insert(<#entity_name as lifeguard::LifeModelTrait>::Column::#primary_key_column_variants, pk_value);
                    } else {
                        return Err(lifeguard::ActiveModelError::PrimaryKeyRequired);
                    }
                )*

                // Call before_delete hook
                let mut record_for_hooks = self.clone();
                record_for_hooks.before_delete()?;

                // Build DELETE or UPDATE (soft-delete) statement
                // If soft_delete is enabled, this issues an UPDATE instead of DELETE
                // The WHERE clause is also appended inside this block
                #build_delete_query_ts

                // Build SQL
                let (sql, sql_values) = query.build(PostgresQueryBuilder);

                executor.execute_values(&sql, &sql_values).map_err(|e| {
                    lifeguard::ActiveModelError::DatabaseError(e.to_string())
                })?;

                // Call after_delete hook
                record_for_hooks.after_delete()?;

                // Transparent Cache Invalidation
                if let Some(cache) = executor.cache_provider() {
                    let table_name = <#entity_name as lifeguard::LifeEntityName>::table_name(&#entity_name::default());
                    #(
                        if let Some(pk_value) = original_pk_values.get(&<#entity_name as lifeguard::LifeModelTrait>::Column::#primary_key_column_variants) {
                            let id_str = match pk_value {
                                sea_query::Value::BigInt(Some(v)) => v.to_string(),
                                sea_query::Value::Int(Some(v)) => v.to_string(),
                                _ => "".to_string(),
                            };
                            if !id_str.is_empty() {
                                let cache_key = format!("lifeguard:model:{}:{}", table_name, id_str);
                                let _ = cache.invalidate(&cache_key);
                            }
                        }
                    )*
                }

                Ok(())
            }

            fn from_json(json: serde_json::Value) -> Result<Self, lifeguard::ActiveModelError> {
                // Deserialize JSON into Model, then convert to Record using from_model()
                // This requires the Model to implement Deserialize (typically via #[derive(Deserialize)])
                let model: #model_name = serde_json::from_value(json)
                    .map_err(|e| lifeguard::ActiveModelError::Other(
                        format!("Failed to deserialize JSON to Model: {}", e)
                    ))?;

                // Convert Model to Record
                Ok(Self::from_model(&model))
            }

            fn to_json(&self) -> Result<serde_json::Value, lifeguard::ActiveModelError> {
                // Build JSON object from set fields only (doesn't require to_model())
                // This matches the documentation: "Only fields that are set... are included"
                let mut map = serde_json::Map::new();

                #(#to_json_field_conversions)*

                Ok(serde_json::Value::Object(map))
            }
        }

        // Implement ActiveModelBehavior with optionally customized hooks
        impl lifeguard::ActiveModelBehavior for #record_name {
            fn before_insert(&mut self) -> Result<(), lifeguard::ActiveModelError> {
                #before_insert_impl
            }
            fn after_insert(&mut self, model: &Self::Model) -> Result<(), lifeguard::ActiveModelError> {
                #after_insert_impl
            }
            fn before_update(&mut self) -> Result<(), lifeguard::ActiveModelError> {
                #before_update_impl
            }
            fn after_update(&mut self, model: &Self::Model) -> Result<(), lifeguard::ActiveModelError> {
                #after_update_impl
            }
            fn before_delete(&mut self) -> Result<(), lifeguard::ActiveModelError> {
                #before_delete_impl
            }
            fn after_delete(&mut self) -> Result<(), lifeguard::ActiveModelError> {
                #after_delete_impl
            }
        }
    };

    TokenStream::from(expanded)
}
