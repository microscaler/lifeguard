//! LifeRecord derive macro implementation

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput, Data, DataStruct, Fields, Ident, GenericArgument, PathArguments, Type, LitStr};
use quote::quote;

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
/// - `from_model()` method (create from LifeModel for updates)
/// - `to_model()` method (convert to LifeModel, None fields use defaults)
/// - `dirty_fields()` method (returns list of changed fields)
/// - `is_dirty()` method (checks if any fields changed)
/// - Setter methods for each field
pub fn derive_life_record(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    
    // Extract struct name
    let struct_name = &input.ident;
    let record_name = Ident::new(&format!("{}Record", struct_name), struct_name.span());
    let model_name = Ident::new(&format!("{}Model", struct_name), struct_name.span());
    
    // Extract struct fields
    let fields = match &input.data {
        Data::Struct(DataStruct { fields: Fields::Named(fields), .. }) => {
            &fields.named
        }
        _ => {
            return syn::Error::new(
                struct_name.span(),
                "LifeRecord can only be derived for structs with named fields"
            )
            .to_compile_error()
            .into();
        }
    };
    
    // Extract table name from attributes (not used in simplified version)
    let _table_name = attributes::extract_table_name(&input.attrs)
        .unwrap_or_else(|| utils::snake_case(&struct_name.to_string()));
    
    // Generate Entity name (assumes Entity struct exists from LifeModel)
    let entity_name = Ident::new("Entity", struct_name.span());
    
    // Process fields
    let mut record_fields = Vec::new();
    let mut record_field_names = Vec::new();
    let mut from_model_fields = Vec::new();
    let mut to_model_fields = Vec::new();
    let mut dirty_fields_check = Vec::new();
    let mut setter_methods = Vec::new();
    
    // For ActiveModelTrait implementation
    let mut active_model_get_match_arms = Vec::new();
    let mut active_model_set_match_arms = Vec::new();
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
        let field_name = field.ident.as_ref().unwrap();
        let field_type = &field.ty;
        
        // Check if field type is already Option<T>
        let is_already_option = extract_option_inner_type(field_type).is_some();
        
        // Extract the inner type from Option<T> if present
        // This is critical: conversion functions need the inner type (e.g., String), not Option<String>
        let inner_type = extract_option_inner_type(field_type).unwrap_or(field_type);
        
        // Extract column name for database (snake_case) and enum variant (PascalCase)
        let db_column_name = attributes::extract_column_name(field)
            .unwrap_or_else(|| utils::snake_case(&field_name.to_string()));
        let column_variant_name = utils::pascal_case(&field_name.to_string());
        let column_variant = Ident::new(&column_variant_name, field_name.span());
        
        // Check if field is primary key
        let is_primary_key = attributes::has_attribute(field, "primary_key");
        let is_auto_increment = attributes::has_attribute(field, "auto_increment");
        
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
        
        // Generate to_model field extraction
        // For Option<T> fields, clone directly (Record field is Option<T>, Model field is Option<T>)
        // For non-Option fields, unwrap (Record field is Option<T>, Model field is T)
        if is_already_option {
            // Field is already Option<T>, clone directly
            to_model_fields.push(quote! {
                #field_name: self.#field_name.clone(),
            });
        } else if is_nullable {
            // Non-Option field, but nullable - use default if None
            to_model_fields.push(quote! {
                #field_name: self.#field_name.clone().unwrap_or_default(),
            });
        } else {
            // Non-Option field, required - panic if None
            to_model_fields.push(quote! {
                #field_name: self.#field_name.clone().expect(&format!("Field {} is required but not set", stringify!(#field_name))),
            });
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
        let setter_name = Ident::new(&format!("set_{}", field_name), field_name.span());
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
        let field_to_value_conversion = type_conversion::generate_option_field_to_value(field_name, inner_type);
        active_model_get_match_arms.push(quote! {
            <#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant => {
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
        
        // For take(), convert directly from Option<T> to Option<Value> and set field to None (optimized)
        // Use inner_type for type conversion (e.g., String from Option<String>)
        let field_to_value_conversion = type_conversion::generate_option_field_to_value(field_name, inner_type);
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
            insert_column_checks.push(quote! {
                if let Some(value) = record_for_hooks.get(<#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant) {
                    columns.push(<#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant);
                    values.push(value);
                }
            });
            // Track auto-increment PKs that need RETURNING (if not set)
            // Generate code to check if this PK needs RETURNING and extract if so
            // Database returns T (inner type), not Option<T>, so we use inner_type
            // Both Option<T> and T fields need to wrap the returned value in Some()
            // NOTE: Check record_for_hooks to see if PK is still unset after before_insert() hook
            returning_extractors.push(quote! {
                // Check if this auto-increment PK was not set and needs RETURNING
                if record_for_hooks.get(<#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant).is_none() {
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
                    values.push(value);
                }
            });
        } else {
            // Non-auto-increment PK: include if set (required for composite keys)
            insert_column_checks.push(quote! {
                if let Some(value) = record_for_hooks.get(<#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant) {
                    columns.push(<#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant);
                    values.push(value);
                }
            });
        }
        
        // Generate UPDATE SET clause (skip primary keys)
        if !is_primary_key {
            // SET clause using self (for backward compatibility, though not used in update() anymore)
            update_set_clauses.push(quote! {
                if let Some(value) = self.get(<#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant) {
                    query.value(<#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant, sea_query::Expr::val(value));
                }
            });
            
            // SET clause using record_for_hooks (includes before_update() changes)
            update_set_clauses_from_hooks.push(quote! {
                if let Some(value) = record_for_hooks.get(<#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant) {
                    query.value(<#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant, sea_query::Expr::val(value));
                }
            });
        }
        
        // Generate DELETE WHERE clause (only for primary keys)
        // Use record_for_hooks to include any changes made in before_delete()
        // This ensures before_delete() changes are included in the DELETE query WHERE clause
        if is_primary_key {
            delete_where_clauses.push(quote! {
                if let Some(pk_value) = record_for_hooks.get(<#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant) {
                    use lifeguard::ColumnTrait;
                    let expr = <#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant.eq(pk_value);
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
                        serde_json::Value::Number(serde_json::Number::from_f64(v as f64).unwrap_or_else(|| serde_json::Number::from(0)))
                    },
                    sea_query::Value::Float(None) => serde_json::Value::Null,
                    sea_query::Value::Double(Some(v)) => {
                        serde_json::Value::Number(serde_json::Number::from_f64(v).unwrap_or_else(|| serde_json::Number::from(0)))
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
    
    // Generate the expanded code
    let expanded = quote! {
        // Record struct (mutable change-set)
        #[derive(Debug, Clone)]
        pub struct #record_name {
            #(#record_fields)*
        }
        
        impl #record_name {
            /// Create a new empty record (all fields None)
            /// Useful for inserts where you set only the fields you need
            pub fn new() -> Self {
                Self {
                    #(
                        #record_field_names: None,
                    )*
                }
            }
            
            /// Create a record from a Model (for updates)
            /// All fields are set to Some(value) from the model
            pub fn from_model(model: &#model_name) -> Self {
                Self {
                    #(#from_model_fields)*
                }
            }
            
            /// Convert the record to a Model
            /// None fields use defaults (Default::default() for nullable, panic for required)
            /// For inserts, ensure all required fields are set before calling this
            pub fn to_model(&self) -> #model_name {
                #model_name {
                    #(#to_model_fields)*
                }
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
            
            fn reset(&mut self) {
                #(#active_model_reset_fields)*
            }
            
            fn insert<E: lifeguard::LifeExecutor>(&self, executor: &E) -> Result<Self::Model, lifeguard::ActiveModelError> {
                use sea_query::{Query, PostgresQueryBuilder};
                use lifeguard::{LifeEntityName, ActiveModelBehavior};
                
                // Call before_insert hook
                let mut record_for_hooks = self.clone();
                record_for_hooks.before_insert()?;
                
                // Build INSERT statement
                let mut query = Query::insert();
                let entity = #entity_name::default();
                query.into_table(entity);
                
                // Collect columns and values (skip auto-increment PKs if not set)
                let mut columns = Vec::new();
                let mut values = Vec::new();
                
                #(#insert_column_checks)*
                
                if columns.is_empty() {
                    return Err(lifeguard::ActiveModelError::Other("No fields set for insert".to_string()));
                }
                
                // Add columns and values to query
                // SeaQuery API: columns() takes items that implement IntoIden (Column implements Iden, which provides IntoIden via blanket impl)
                // values_panic() takes an iterator of Expr (wrapping Values)
                query.columns(columns.iter().copied());
                let exprs: Vec<sea_query::Expr> = values.iter().map(|v| sea_query::Expr::val(v.clone())).collect();
                query.values_panic(exprs.iter().cloned());
                
                // Check if we need RETURNING clause for auto-increment primary keys
                // Track which auto-increment PKs were not set and need RETURNING
                // NOTE: Check record_for_hooks to see if PK is still unset after before_insert() hook
                let mut needs_returning = false;
                let mut returning_cols = Vec::new();
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
                
                // Convert Values to Vec<Value> for parameter binding
                // SeaQuery's Values implements IntoIterator<Item = Value>
                let values_vec: Vec<sea_query::Value> = sql_values.iter().cloned().collect();
                
                // Create a mutable copy of self to update with returned PK values
                // Use the record that went through before_insert hook
                let mut updated_record = record_for_hooks;
                
                // Execute query and handle RETURNING if needed
                if needs_returning {
                    // Use query_one() to get returned values
                    let row = lifeguard::with_converted_params(&values_vec, |params| {
                        executor.query_one(&sql, params).map_err(|e| {
                            lifeguard::ActiveModelError::DatabaseError(e.to_string())
                        })
                    })?;
                    
                    // Extract returned primary key values and update the record
                    let mut returning_idx = 0usize;
                    #(#returning_extractors)*
                } else {
                    // No RETURNING needed, just execute
                    lifeguard::with_converted_params(&values_vec, |params| {
                        executor.execute(&sql, params).map_err(|e| {
                            lifeguard::ActiveModelError::DatabaseError(e.to_string())
                        })?;
                        Ok(())
                    })?;
                }
                
                // Construct the model from the updated record
                let model = updated_record.to_model();
                
                // Call after_insert hook
                updated_record.after_insert(&model)?;
                
                // Return the model
                Ok(model)
            }
            
            fn update<E: lifeguard::LifeExecutor>(&self, executor: &E) -> Result<Self::Model, lifeguard::ActiveModelError> {
                use sea_query::{Query, PostgresQueryBuilder, Expr};
                use lifeguard::{LifeEntityName, ActiveModelBehavior};
                
                #update_pk_check
                
                // Call before_update hook
                let mut record_for_hooks = self.clone();
                record_for_hooks.before_update()?;
                
                // Build UPDATE statement
                let mut query = Query::update();
                let entity = #entity_name::default();
                query.table(entity);
                
                // Add SET clauses for dirty fields (skip primary keys)
                // Use record_for_hooks to include any changes made in before_update()
                // This ensures before_update() changes are included in the UPDATE query
                #(#update_set_clauses_from_hooks)*
                
                // Add WHERE clause for primary keys (use record_for_hooks to get PK values)
                // This ensures PK values from before_update are used
                #(
                    if let Some(pk_value) = record_for_hooks.get(<#entity_name as lifeguard::LifeModelTrait>::Column::#primary_key_column_variants) {
                        use lifeguard::ColumnTrait;
                        let expr = <#entity_name as lifeguard::LifeModelTrait>::Column::#primary_key_column_variants.eq(pk_value);
                        query.and_where(expr);
                    } else {
                        return Err(lifeguard::ActiveModelError::PrimaryKeyRequired);
                    }
                )*
                
                // Build SQL
                let (sql, sql_values) = query.build(PostgresQueryBuilder);
                
                // Convert Values to Vec<Value> for parameter binding
                // SeaQuery's Values implements IntoIterator<Item = Value>
                let values_vec: Vec<sea_query::Value> = sql_values.iter().cloned().collect();
                
                // Convert values to parameters and execute
                let rows_affected = lifeguard::with_converted_params(&values_vec, |params| {
                    executor.execute(&sql, params).map_err(|e| {
                        lifeguard::ActiveModelError::DatabaseError(e.to_string())
                    })
                })?;
                
                // Check if any rows were affected
                if rows_affected == 0 {
                    return Err(lifeguard::ActiveModelError::RecordNotFound);
                }
                
                // Construct the model
                let model = record_for_hooks.to_model();
                
                // Call after_update hook
                record_for_hooks.after_update(&model)?;
                
                // Return the updated model
                Ok(model)
            }
            
            fn save<E: lifeguard::LifeExecutor>(&self, executor: &E) -> Result<Self::Model, lifeguard::ActiveModelError> {
                use lifeguard::ActiveModelBehavior;
                
                // Call before_save hook
                let mut record_for_hooks = self.clone();
                record_for_hooks.before_save()?;
                
                // Execute save logic (insert or update) using record_for_hooks
                // This handles both entities with and without primary keys correctly
                // insert()/update() will clone record_for_hooks again and apply their own hooks,
                // then return a model that includes all modifications (including auto-increment PKs from RETURNING)
                let model = #save_pk_logic?;
                
                // CRITICAL FIX: Convert the returned model back to a record so after_save() receives
                // a record that matches the model. This ensures consistency with after_insert()/after_update()
                // which receive records that match their returned models.
                // Without this, after_save() would receive record_for_hooks which only has before_save()
                // modifications, missing before_insert()/before_update() modifications and auto-increment PKs.
                let record_for_after_save = Self::from_model(&model);
                
                // Call after_save hook with record that matches the returned model
                record_for_after_save.after_save(&model)?;
                
                Ok(model)
            }
            
            fn delete<E: lifeguard::LifeExecutor>(&self, executor: &E) -> Result<(), lifeguard::ActiveModelError> {
                use sea_query::{Query, PostgresQueryBuilder, Expr};
                use lifeguard::{LifeEntityName, ActiveModelBehavior};
                
                #delete_pk_check
                
                // Call before_delete hook
                let mut record_for_hooks = self.clone();
                record_for_hooks.before_delete()?;
                
                // Build DELETE statement
                let mut query = Query::delete();
                let entity = #entity_name::default();
                query.from_table(entity);
                
                // Add WHERE clause for primary keys
                #(#delete_where_clauses)*
                
                // Build SQL
                let (sql, sql_values) = query.build(PostgresQueryBuilder);
                
                // Convert Values to Vec<Value> for parameter binding
                // SeaQuery's Values implements IntoIterator<Item = Value>
                let values_vec: Vec<sea_query::Value> = sql_values.iter().cloned().collect();
                
                // Convert values to parameters and execute
                lifeguard::with_converted_params(&values_vec, |params| {
                    executor.execute(&sql, params).map_err(|e| {
                        lifeguard::ActiveModelError::DatabaseError(e.to_string())
                    })?;
                    Ok(())
                })?;
                
                // Call after_delete hook
                record_for_hooks.after_delete()?;
                
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
        
        // Implement ActiveModelBehavior with default (empty) implementations
        // Users can override specific hooks as needed
        impl lifeguard::ActiveModelBehavior for #record_name {}
    };
    
    TokenStream::from(expanded)
}
