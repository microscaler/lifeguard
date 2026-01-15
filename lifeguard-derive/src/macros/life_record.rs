//! LifeRecord derive macro implementation

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput, Data, DataStruct, Fields, Ident, GenericArgument, PathArguments, Type};
use quote::quote;

use crate::attributes;
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

/// Generate field to Value conversion code for ActiveModelTrait::get() and take()
/// Since LifeRecord fields are always Option<T>, we convert Option<T> to Option<Value>
/// Note: field_type here is the INNER type (e.g., i32), not Option<i32>
/// The actual record field is Option<field_type>
fn generate_field_to_value_conversion(
    field_name: &Ident,
    field_type: &Type,
) -> proc_macro2::TokenStream {
    // In LifeRecord, field_type is the inner type (e.g., i32), and the actual field is Option<i32>
    // So we need to match on field_type directly, not extract from Option
    if let Type::Path(type_path) = field_type {
        let segments = &type_path.path.segments;
        
        // Check for serde_json::Value (multi-segment path)
        let is_json_value = segments.len() == 2
            && segments.first().map(|s| s.ident.to_string()) == Some("serde_json".to_string())
            && segments.last().map(|s| s.ident.to_string()) == Some("Value".to_string());
        
        if is_json_value {
            quote! {
                self.#field_name.as_ref().map(|v| sea_query::Value::Json(Some(Box::new(v.clone()))))
            }
        } else if let Some(segment) = segments.last() {
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
                "String" => quote! {
                    self.#field_name.as_ref().map(|v| sea_query::Value::String(Some(v.clone())))
                },
                "bool" => quote! {
                    self.#field_name.map(|v| sea_query::Value::Bool(Some(v)))
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
                    self.#field_name.map(|v| sea_query::Value::BigInt(Some(v as i64)))
                },
                "f32" => quote! {
                    self.#field_name.map(|v| sea_query::Value::Float(Some(v)))
                },
                "f64" => quote! {
                    self.#field_name.map(|v| sea_query::Value::Double(Some(v)))
                },
                _ => quote! {
                    None // Unknown type
                },
            }
        } else {
            quote! { None }
        }
    } else {
        quote! { None }
    }
}

/// Generate Value to field conversion code for ActiveModelTrait::set()
/// Since LifeRecord fields are always Option<T>, we convert Value to T and wrap in Some()
/// Note: field_type here is the INNER type (e.g., i32), not Option<i32>
/// The actual record field is Option<field_type>
fn generate_value_to_field_conversion(
    field_name: &Ident,
    field_type: &Type,
    column_variant: &Ident,
) -> proc_macro2::TokenStream {
    // In LifeRecord, field_type is the inner type (e.g., i32), and the actual field is Option<i32>
    // So we need to match on field_type directly, not extract from Option
    if let Type::Path(type_path) = field_type {
        let segments = &type_path.path.segments;
        
        // Check for serde_json::Value (multi-segment path)
        let is_json_value = segments.len() == 2
            && segments.first().map(|s| s.ident.to_string()) == Some("serde_json".to_string())
            && segments.last().map(|s| s.ident.to_string()) == Some("Value".to_string());
        
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
                    _ => Err(lifeguard::ActiveModelError::InvalidValueType {
                        column: stringify!(#column_variant).to_string(),
                        expected: "Json".to_string(),
                        actual: format!("{:?}", value),
                    })
                }
            }
        } else if let Some(segment) = segments.last() {
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
                            sea_query::Value::BigInt(Some(v)) => {
                                self.#field_name = Some(v as u64);
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
    let mut update_set_clauses = Vec::new(); // SET clauses for UPDATE
    let mut delete_where_clauses = Vec::new(); // WHERE clauses for DELETE
    
    for field in fields.iter() {
        let field_name = field.ident.as_ref().unwrap();
        let field_type = &field.ty;
        
        // Extract column name (use column_name attribute or convert field name to PascalCase)
        let column_name = attributes::extract_column_name(field)
            .unwrap_or_else(|| utils::pascal_case(&field_name.to_string()));
        let column_variant = Ident::new(&column_name, field_name.span());
        
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
        
        // Generate record field (Option<T>)
        record_fields.push(quote! {
            pub #field_name: Option<#field_type>,
        });
        
        // Store field name for struct initialization
        record_field_names.push(field_name);
        
        // Generate from_model field assignment
        from_model_fields.push(quote! {
            #field_name: Some(model.#field_name.clone()),
        });
        
        // Generate to_model field extraction
        // For inserts, None fields use defaults (or panic if required)
        if is_nullable {
            to_model_fields.push(quote! {
                #field_name: self.#field_name.clone().unwrap_or_default(),
            });
        } else {
            to_model_fields.push(quote! {
                #field_name: self.#field_name.clone().expect(&format!("Field {} is required but not set", stringify!(#field_name))),
            });
        }
        
        // Generate dirty field check
        dirty_fields_check.push(quote! {
            if self.#field_name.is_some() {
                dirty.push(stringify!(#field_name).to_string());
            }
        });
        
        // Generate setter method
        let setter_name = Ident::new(&format!("set_{}", field_name), field_name.span());
        setter_methods.push(quote! {
            /// Set the #field_name field
            pub fn #setter_name(&mut self, value: #field_type) -> &mut Self {
                self.#field_name = Some(value);
                self
            }
        });
        
        // Generate ActiveModelTrait match arms
        // For get(), convert directly from Option<T> to Option<Value> (optimized, no to_model() needed)
        let field_to_value_conversion = generate_field_to_value_conversion(field_name, field_type);
        active_model_get_match_arms.push(quote! {
            <#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant => {
                #field_to_value_conversion
            }
        });
        
        // For set(), generate type conversion code
        let value_to_field_conversion = generate_value_to_field_conversion(
            field_name,
            field_type,
            &column_variant,
        );
        active_model_set_match_arms.push(quote! {
            <#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant => {
                #value_to_field_conversion
            }
        });
        
        // For take(), convert directly from Option<T> to Option<Value> and set field to None (optimized)
        let field_to_value_conversion = generate_field_to_value_conversion(field_name, field_type);
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
        if is_primary_key && is_auto_increment {
            // Auto-increment PK: include only if set
            insert_column_checks.push(quote! {
                if let Some(value) = self.get(<#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant) {
                    columns.push(<#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant);
                    values.push(value);
                }
            });
        } else if !is_primary_key {
            // Non-PK field: include if set
            insert_column_checks.push(quote! {
                if let Some(value) = self.get(<#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant) {
                    columns.push(<#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant);
                    values.push(value);
                }
            });
        } else {
            // Non-auto-increment PK: include if set (required for composite keys)
            insert_column_checks.push(quote! {
                if let Some(value) = self.get(<#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant) {
                    columns.push(<#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant);
                    values.push(value);
                }
            });
        }
        
        // Generate UPDATE SET clause (skip primary keys)
        if !is_primary_key {
            update_set_clauses.push(quote! {
                if let Some(value) = self.get(<#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant) {
                    query.value(<#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant, sea_query::Expr::val(value));
                }
            });
        }
        
        // Generate DELETE WHERE clause (only for primary keys)
        if is_primary_key {
            delete_where_clauses.push(quote! {
                if let Some(pk_value) = self.get(<#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant) {
                    let expr = sea_query::Expr::col(<#entity_name as lifeguard::LifeModelTrait>::Column::#column_variant).eq(pk_value);
                    query.and_where(expr);
                } else {
                    return Err(lifeguard::ActiveModelError::PrimaryKeyRequired);
                }
            });
        }
    }
    
    // Generate primary key check code for save()
    let mut save_pk_checks = Vec::new();
    for (field_name, column_variant) in primary_key_field_names.iter().zip(primary_key_column_variants.iter()) {
        save_pk_checks.push(quote! {
            self.#field_name.is_some() &&
        });
    }
    
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
                use lifeguard::LifeEntityName;
                
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
                // SeaQuery API: columns() takes items that implement IntoIden
                // values_panic() takes an iterator of Expr (wrapping Values)
                query.columns(columns.iter().map(|c| *c));
                let exprs: Vec<sea_query::Expr> = values.iter().map(|v| sea_query::Expr::val(v.clone())).collect();
                query.values_panic(exprs.iter().cloned());
                
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
                
                // Return the model constructed from the record
                // TODO: For auto-increment PKs, fetch the generated value using RETURNING
                Ok(self.to_model())
            }
            
            fn update<E: lifeguard::LifeExecutor>(&self, executor: &E) -> Result<Self::Model, lifeguard::ActiveModelError> {
                use sea_query::{Query, PostgresQueryBuilder, Expr};
                use lifeguard::LifeEntityName;
                
                // Check primary key is set
                #(
                    if self.#primary_key_field_names.is_none() {
                        return Err(lifeguard::ActiveModelError::PrimaryKeyRequired);
                    }
                )*
                
                // Build UPDATE statement
                let mut query = Query::update();
                let entity = #entity_name::default();
                query.table(entity);
                
                // Add SET clauses for dirty fields (skip primary keys)
                #(#update_set_clauses)*
                
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
                
                // Return the updated model
                Ok(self.to_model())
            }
            
            fn save<E: lifeguard::LifeExecutor>(&self, executor: &E) -> Result<Self::Model, lifeguard::ActiveModelError> {
                // Check if primary key is set
                let has_primary_key = #(#save_pk_checks)* true;
                
                if has_primary_key {
                    // Try to find the record to see if it exists
                    // For now, try update first, if it fails with "no rows", do insert
                    // TODO: Use Entity::find() to check if record exists
                    match self.update(executor) {
                        Ok(model) => Ok(model),
                        Err(lifeguard::ActiveModelError::DatabaseError(_)) => {
                            // Update failed, try insert instead
                            self.insert(executor)
                        },
                        Err(e) => Err(e), // Propagate other errors
                    }
                } else {
                    // No primary key set, do insert
                    self.insert(executor)
                }
            }
            
            fn delete<E: lifeguard::LifeExecutor>(&self, executor: &E) -> Result<(), lifeguard::ActiveModelError> {
                use sea_query::{Query, PostgresQueryBuilder, Expr};
                use lifeguard::LifeEntityName;
                
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
                
                Ok(())
            }
        }
    };
    
    TokenStream::from(expanded)
}
