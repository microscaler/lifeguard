//! LifeRecord derive macro implementation

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput, Data, DataStruct, Fields, Ident};
use quote::quote;

use crate::attributes;
use crate::utils;

/// Derive macro for `LifeRecord` - generates mutable change-set objects
///
/// This macro generates:
/// - `Record` struct (mutable change-set with Option<T> fields)
/// - `from_model()` method (create from LifeModel for updates)
/// - `to_model()` method (convert to LifeModel, None fields use defaults)
/// - `dirty_fields()` method (returns list of changed fields)
/// - `is_dirty()` method (checks if any fields changed)
/// - Setter methods for each field
///
/// # Example
/// ```ignore
/// use lifeguard_derive::{LifeModel, LifeRecord};
///
/// #[derive(LifeModel, LifeRecord)]
/// #[table_name = "users"]
/// pub struct User {
///     #[primary_key]
///     pub id: i32,
///     pub name: String,
///     pub email: String,
/// }
///
/// // Create a record for update
/// let mut record = UserRecord::from_model(&user_model);
/// record.set_name("New Name".to_string());
/// // Only changed fields will be updated
/// ```
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
    
    // Extract table name from attributes
    let table_name = attributes::extract_table_name(&input.attrs)
        .unwrap_or_else(|| utils::snake_case(&struct_name.to_string()));
    
    // Track primary key field for insert/update operations
    let mut primary_key_field: Option<(Ident, syn::Type, String)> = None;
    
    // Process fields
    let mut record_fields = Vec::new();
    let mut record_field_names = Vec::new();
    let mut from_model_fields = Vec::new();
    let mut to_model_fields = Vec::new();
    let mut dirty_fields_check = Vec::new();
    let mut setter_methods = Vec::new();
    let mut insert_field_checks = Vec::new();
    let mut update_sets = Vec::new();
    
    for field in fields.iter() {
        let field_name = field.ident.as_ref().unwrap();
        let field_type = &field.ty;
        let column_name = attributes::extract_column_name(field)
            .unwrap_or_else(|| utils::snake_case(&field_name.to_string()));
        
        // Check if this is a primary key
        let is_primary_key = attributes::has_attribute(field, "primary_key");
        
        // Store primary key info for insert/update operations
        if is_primary_key {
            primary_key_field = Some((
                field_name.clone(),
                field_type.clone(),
                column_name.clone(),
            ));
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
        // For now, we'll use unwrap_or_else with Default::default()
        // This can be enhanced later with #[default_value] attribute support
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
        
        // Generate insert column and value (skip primary key)
        if !is_primary_key {
            let column_name_str = column_name.clone();
            insert_field_checks.push(quote! {
                if let Some(ref val) = self.#field_name {
                    struct ColumnName;
                    impl sea_query::Iden for ColumnName {
                        fn unquoted(&self) -> &str {
                            #column_name_str
                        }
                    }
                    columns.push(ColumnName);
                    values.push(sea_query::Expr::val(val.clone()));
                }
            });
        }
        
        // Generate update set (skip primary key)
        if !is_primary_key {
            let column_name_str_update = column_name.clone();
            update_sets.push(quote! {
                if let Some(ref val) = self.#field_name {
                    struct UpdateColumnName;
                    impl sea_query::Iden for UpdateColumnName {
                        fn unquoted(&self) -> &str {
                            #column_name_str_update
                        }
                    }
                    query.value(UpdateColumnName, sea_query::Expr::val(val.clone()));
                }
            });
        }
    }
    
    // Generate insert/update methods if we have a primary key
    let crud_methods = if let Some((_pk_field_name, pk_field_type, pk_column_name)) = primary_key_field {
        let pk_column_name_str = pk_column_name.clone();
        quote! {
            /// Insert this record into the database
            ///
            /// # Arguments
            ///
            /// * `executor` - The executor to use for the query
            ///
            /// # Returns
            ///
            /// Returns the inserted Model with all fields populated (including generated primary key).
            pub fn insert<E: lifeguard::LifeExecutor>(&self, executor: &E) -> Result<#model_name, lifeguard::LifeError> {
                use sea_query::{InsertStatement, PostgresQueryBuilder};
                use sea_query::Iden;
                
                struct TableName;
                impl Iden for TableName {
                    fn unquoted(&self) -> &str {
                        #struct_name::TABLE_NAME
                    }
                }
                
                let mut query = sea_query::InsertStatement::default();
                query.into_table(TableName);
                
                // Build columns and values from dirty fields
                let mut columns = Vec::new();
                let mut values = Vec::new();
                
                #(#insert_field_checks)*
                
                if columns.is_empty() {
                    return Err(lifeguard::LifeError::Other("No fields to insert".to_string()));
                }
                
                query.columns(columns);
                query.values_panic(values);
                query.returning_col(sea_query::Asterisk);
                
                let (sql, _values) = query.build(PostgresQueryBuilder);
                let row = executor.query_one(&sql, &[])?;
                #model_name::from_row(&row).map_err(|e| lifeguard::LifeError::ParseError(format!("Failed to parse row: {}", e)))
            }
            
            /// Update an existing record in the database
            ///
            /// # Arguments
            ///
            /// * `executor` - The executor to use for the query
            /// * `id` - The primary key value of the record to update
            ///
            /// # Returns
            ///
            /// Returns the updated Model.
            pub fn update<E: lifeguard::LifeExecutor>(&self, executor: &E, id: #pk_field_type) -> Result<#model_name, lifeguard::LifeError> {
                use sea_query::{UpdateStatement, PostgresQueryBuilder, Expr, ExprTrait};
                use sea_query::Iden;
                
                struct TableName;
                impl Iden for TableName {
                    fn unquoted(&self) -> &str {
                        #struct_name::TABLE_NAME
                    }
                }
                
                struct ColumnName;
                impl Iden for ColumnName {
                    fn unquoted(&self) -> &str {
                        #pk_column_name_str
                    }
                }
                
                let mut query = sea_query::UpdateStatement::default();
                query.table(TableName);
                
                // Add SET clauses for dirty fields only (skip primary key)
                #(#update_sets)*
                
                // Add WHERE clause
                query.and_where(Expr::col(ColumnName).eq(id));
                
                // Add RETURNING clause
                query.returning_col(sea_query::Asterisk);
                
                let (sql, _values) = query.build(PostgresQueryBuilder);
                let params: Vec<&dyn may_postgres::types::ToSql> = vec![&id];
                
                let row = executor.query_one(&sql, &params)?;
                #model_name::from_row(&row).map_err(|e| lifeguard::LifeError::ParseError(format!("Failed to parse row: {}", e)))
            }
        }
    } else {
        quote! {}
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
            
            #crud_methods
        }
        
        impl Default for #record_name {
            fn default() -> Self {
                Self::new()
            }
        }
    };
    
    TokenStream::from(expanded)
}
