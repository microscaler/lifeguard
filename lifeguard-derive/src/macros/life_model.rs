//! LifeModel derive macro implementation

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput, Data, DataStruct, Fields, Ident};
use quote::quote;

use crate::attributes;
use crate::utils;

/// Derive macro for `LifeModel` - generates immutable database row representation
///
/// This macro generates:
/// - `Model` struct (immutable row representation)
/// - `Column` enum (all columns)
/// - `PrimaryKey` enum (primary key columns)
/// - `Entity` type (entity itself)
/// - `FromRow` implementation for deserializing database rows
/// - Field getters (immutable access)
/// - Table name and column metadata
/// - Primary key identification
///
/// # Example
/// ```ignore
/// use lifeguard_derive::LifeModel;
///
/// #[derive(LifeModel)]
/// #[table_name = "users"]
/// pub struct User {
///     #[primary_key]
///     pub id: i32,
///     pub name: String,
///     pub email: String,
/// }
/// ```
pub fn derive_life_model(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    
    // Extract struct name
    let struct_name = &input.ident;
    let model_name = Ident::new(&format!("{}Model", struct_name), struct_name.span());
    
    // Extract struct fields
    let fields = match &input.data {
        Data::Struct(DataStruct { fields: Fields::Named(fields), .. }) => {
            &fields.named
        }
        _ => {
            return syn::Error::new(
                struct_name.span(),
                "LifeModel can only be derived for structs with named fields"
            )
            .to_compile_error()
            .into();
        }
    };
    
    // Extract table name from attributes
    let table_name = attributes::extract_table_name(&input.attrs)
        .unwrap_or_else(|| utils::snake_case(&struct_name.to_string()));
    
    // Process fields
    let mut column_variants = Vec::new();
    let mut primary_key_variants = Vec::new();
    let mut model_fields = Vec::new();
    let mut from_row_fields = Vec::new();
    let mut column_impls = Vec::new();
    let mut iden_impls = Vec::new();
    
    // Track primary key field for CRUD operations
    let mut primary_key_field: Option<(Ident, syn::Type, String)> = None;
    
    // Track all fields for batch operations
    let mut all_fields_info: Vec<(Ident, String, Ident, bool)> = Vec::new(); // (field_name, column_name, column_variant, is_primary_key)
    
    for field in fields.iter() {
        let field_name = field.ident.as_ref().unwrap();
        let field_type = &field.ty;
        let column_name = attributes::extract_column_name(field)
            .unwrap_or_else(|| utils::snake_case(&field_name.to_string()));
        
        // Check if this is a primary key
        let is_primary_key = attributes::has_attribute(field, "primary_key");
        
        // Store primary key info for CRUD operations
        if is_primary_key {
            primary_key_field = Some((
                field_name.clone(),
                field_type.clone(),
                column_name.clone(),
            ));
        }
        
        // Generate Column enum variant
        let column_variant = Ident::new(
            &utils::pascal_case(&field_name.to_string()),
            field_name.span()
        );
        column_variants.push(quote! {
            #column_variant,
        });
        
        // Store field info for batch operations
        all_fields_info.push((
            field_name.clone(),
            column_name.clone(),
            column_variant.clone(),
            is_primary_key,
        ));
        
        // Generate IntoColumnRef implementation for this variant
        let column_name_str = column_name.clone();
        column_impls.push(quote! {
            Column::#column_variant => {
                use sea_query::Iden;
                struct ColumnName;
                impl Iden for ColumnName {
                    fn unquoted(&self) -> &str {
                        #column_name_str
                    }
                }
                sea_query::ColumnRef::Column(ColumnName.into())
            }
        });
        
        // Generate Iden implementation for this variant
        let column_name_str_iden = column_name.clone();
        iden_impls.push(quote! {
            Column::#column_variant => #column_name_str_iden,
        });
        
        // Generate PrimaryKey enum variant if primary key
        if is_primary_key {
            primary_key_variants.push(quote! {
                #column_variant,
            });
        }
        
        // Generate model field
        model_fields.push(quote! {
            pub #field_name: #field_type,
        });
        
        // Generate FromRow field extraction
        // Use column name if custom, otherwise use snake_case of field name
        let column_name_str = column_name.clone();
        from_row_fields.push(quote! {
            #field_name: row.get(#column_name_str),
        });
    }
    
    // Generate field extraction code for batch operations
    let mut insert_many_column_builders = Vec::new();
    let mut insert_many_value_extractors = Vec::new();
    let mut update_many_setters = Vec::new();
    
    for (field_name, column_name, column_variant, is_primary_key) in &all_fields_info {
        let field_name_ident = field_name.clone();
        let column_name_str = column_name.clone();
        
        // For insert_many: build column reference only if field is Some (skip primary key if auto-increment)
        // This matches the behavior of single insert - only include columns that are set
        // Use &'static str directly since IntoIden is implemented for it, avoiding type conflicts
        if !is_primary_key {
            insert_many_column_builders.push(quote! {
                if let Some(_) = first_record.#field_name_ident {
                    columns.push(#column_name_str);
                }
            });
            
            // For insert_many: extract value from record only if field was included in column list
            // This matches single insert behavior - skip None fields to let DB apply defaults
            // Extract values in the same order as columns were built (only for fields that were Some in first record)
            // Note: All records should have the same fields set, so if a field was included in columns,
            // it should be Some in all records. The validation happens by checking value count matches column count.
            insert_many_value_extractors.push(quote! {
                // Only extract if this field was included in the column list (was Some in first record)
                // We check the first record to determine if this field should be extracted
                if first_record.#field_name_ident.is_some() {
                    if let Some(ref val) = record.#field_name_ident {
                        values.push(sea_query::Expr::val(val.clone()));
                    } else {
                        // If field was in column list but is None in this record, push NULL
                        values.push(sea_query::Expr::null());
                    }
                }
            });
        }
        
        // For update_many: build SET clause (skip primary key)
        if !is_primary_key {
            let column_name_str = column_name.clone();
            let column_struct_name = Ident::new(&format!("ColumnName{}", column_variant), column_variant.span());
            update_many_setters.push(quote! {
                if let Some(ref val) = values.#field_name_ident {
                    struct #column_struct_name;
                    impl sea_query::Iden for #column_struct_name {
                        fn unquoted(&self) -> &str {
                            #column_name_str
                        }
                    }
                    query.value(#column_struct_name, val.clone());
                    set_count += 1;
                }
            });
        }
    }
    
    // Generate CRUD methods if we have a primary key
    let crud_methods = if let Some((_pk_field_name, pk_field_type, pk_column_name)) = primary_key_field {
        let pk_column_name_str = pk_column_name.clone();
        quote! {
            /// Find a record by primary key
            ///
            /// # Arguments
            ///
            /// * `executor` - The executor to use for the query
            /// * `id` - The primary key value
            ///
            /// # Returns
            ///
            /// Returns the Model if found, or an error if not found or on database error.
            pub fn find_by_id<E: lifeguard::LifeExecutor>(executor: &E, id: #pk_field_type) -> Result<Self, lifeguard::LifeError> {
                use sea_query::{SelectStatement, PostgresQueryBuilder, Expr, ExprTrait};
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
                
                let mut query = sea_query::SelectStatement::default();
                query
                    .column(sea_query::Asterisk)
                    .from(TableName)
                    .and_where(Expr::col(ColumnName).eq(id));
                
                let (sql, _values) = query.build(PostgresQueryBuilder);
                let params: Vec<&dyn may_postgres::types::ToSql> = vec![&id];
                
                let row = executor.query_one(&sql, &params)?;
                Self::from_row(&row).map_err(|e| lifeguard::LifeError::ParseError(format!("Failed to parse row: {}", e)))
            }
            
            /// Start a query builder for finding records
            ///
            /// # Returns
            ///
            /// Returns a query builder that can be chained with filters.
            pub fn find() -> lifeguard::SelectQuery<#model_name> {
                lifeguard::SelectQuery::<#model_name>::new(#struct_name::TABLE_NAME)
            }
            
            /// Delete a record by primary key
            ///
            /// # Arguments
            ///
            /// * `executor` - The executor to use for the query
            /// * `id` - The primary key value
            ///
            /// # Returns
            ///
            /// Returns the number of rows deleted (should be 1 if successful).
            pub fn delete<E: lifeguard::LifeExecutor>(executor: &E, id: #pk_field_type) -> Result<u64, lifeguard::LifeError> {
                use sea_query::{DeleteStatement, PostgresQueryBuilder, Expr, ExprTrait};
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
                
                let mut query = sea_query::DeleteStatement::default();
                query
                    .from_table(TableName)
                    .and_where(Expr::col(ColumnName).eq(id));
                
                let (sql, _values) = query.build(PostgresQueryBuilder);
                let params: Vec<&dyn may_postgres::types::ToSql> = vec![&id];
                
                executor.execute(&sql, &params)
            }
            
            /// Batch insert multiple records
            ///
            /// # Arguments
            ///
            /// * `records` - Slice of Records to insert
            /// * `executor` - The executor to use for the query
            ///
            /// # Returns
            ///
            /// Returns a vector of inserted Models with all fields populated (including generated primary keys).
            pub fn insert_many<E: lifeguard::LifeExecutor>(records: &[#struct_name::Record], executor: &E) -> Result<Vec<#model_name>, lifeguard::LifeError> {
                use sea_query::{InsertStatement, PostgresQueryBuilder, Iden};
                
                if records.is_empty() {
                    return Ok(Vec::new());
                }
                
                struct TableName;
                impl Iden for TableName {
                    fn unquoted(&self) -> &str {
                        #struct_name::TABLE_NAME
                    }
                }
                
                let mut query = sea_query::InsertStatement::default();
                query.into_table(TableName);
                
                // Build columns from the first record (all records should have same columns)
                let first_record = &records[0];
                let dirty_fields = first_record.dirty_fields();
                
                if dirty_fields.is_empty() {
                    return Err(lifeguard::LifeError::Other("No fields to insert".to_string()));
                }
                
                // Build column list from dirty fields (only include columns that are Some)
                // This matches single insert behavior - only include columns that are set
                // Use Vec<&'static str> since IntoIden is implemented for &'static str
                let mut columns: Vec<&'static str> = Vec::new();
                #(#insert_many_column_builders)*
                
                if columns.is_empty() {
                    return Err(lifeguard::LifeError::Other("No fields to insert".to_string()));
                }
                
                query.columns(columns);
                
                // For each record, extract values in the same order as columns
                // Only extract values for fields that were included in the column list
                for record in records {
                    let mut values = Vec::new();
                    #(#insert_many_value_extractors)*
                    // Validate that we extracted the correct number of values
                    if values.len() != columns.len() {
                        return Err(lifeguard::LifeError::Other(
                            format!("Record has inconsistent fields: expected {} values, got {}. All records must have the same fields set as the first record.", columns.len(), values.len())
                        ));
                    }
                    query.values_panic(values);
                }
                
                query.returning_col(sea_query::Asterisk);
                
                let (sql, sea_values) = query.build(PostgresQueryBuilder);
                
                // Convert SeaQuery values to ToSql parameters (same pattern as Record::insert)
                let mut bools: Vec<bool> = Vec::new();
                let mut ints: Vec<i32> = Vec::new();
                let mut big_ints: Vec<i64> = Vec::new();
                let mut strings: Vec<String> = Vec::new();
                let mut bytes: Vec<Vec<u8>> = Vec::new();
                let mut nulls: Vec<Option<i32>> = Vec::new();
                let mut floats: Vec<f32> = Vec::new();
                let mut doubles: Vec<f64> = Vec::new();
                
                for value in sea_values.iter() {
                    match value {
                        sea_query::Value::Bool(Some(b)) => bools.push(*b),
                        sea_query::Value::Int(Some(i)) => ints.push(*i),
                        sea_query::Value::BigInt(Some(i)) => big_ints.push(*i),
                        sea_query::Value::String(Some(s)) => strings.push(s.clone()),
                        sea_query::Value::Bytes(Some(b)) => bytes.push(b.clone()),
                        sea_query::Value::TinyInt(Some(i)) => ints.push(*i as i32),
                        sea_query::Value::SmallInt(Some(i)) => ints.push(*i as i32),
                        sea_query::Value::TinyUnsigned(Some(u)) => ints.push(*u as i32),
                        sea_query::Value::SmallUnsigned(Some(u)) => ints.push(*u as i32),
                        sea_query::Value::Unsigned(Some(u)) => big_ints.push(*u as i64),
                        sea_query::Value::BigUnsigned(Some(u)) => {
                            if *u > i64::MAX as u64 {
                                return Err(lifeguard::LifeError::Other(format!("Value too large: {}", u)));
                            }
                            big_ints.push(*u as i64);
                        }
                        sea_query::Value::Float(Some(f)) => floats.push(*f),
                        sea_query::Value::Double(Some(d)) => doubles.push(*d),
                        sea_query::Value::Bool(None) | sea_query::Value::Int(None) | 
                        sea_query::Value::BigInt(None) | sea_query::Value::String(None) | 
                        sea_query::Value::Bytes(None) | sea_query::Value::TinyInt(None) |
                        sea_query::Value::SmallInt(None) | sea_query::Value::TinyUnsigned(None) |
                        sea_query::Value::SmallUnsigned(None) | sea_query::Value::Unsigned(None) |
                        sea_query::Value::BigUnsigned(None) | sea_query::Value::Float(None) | 
                        sea_query::Value::Double(None) => nulls.push(None),
                        #[cfg(feature = "with-json")]
                        sea_query::Value::Json(Some(j)) => strings.push(j.clone()),
                        #[cfg(feature = "with-json")]
                        sea_query::Value::Json(None) => nulls.push(None),
                        _ => return Err(lifeguard::LifeError::Other(format!("Unsupported value type in insert_many"))),
                    }
                }
                
                let mut params: Vec<&dyn may_postgres::types::ToSql> = Vec::new();
                let mut bool_idx = 0;
                let mut int_idx = 0;
                let mut big_int_idx = 0;
                let mut string_idx = 0;
                let mut bytes_idx = 0;
                let mut null_idx = 0;
                let mut float_idx = 0;
                let mut double_idx = 0;
                
                for value in sea_values.iter() {
                    match value {
                        sea_query::Value::Bool(Some(_)) => {
                            params.push(&bools[bool_idx] as &dyn may_postgres::types::ToSql);
                            bool_idx += 1;
                        }
                        sea_query::Value::Int(Some(_)) | sea_query::Value::TinyInt(Some(_)) | 
                        sea_query::Value::SmallInt(Some(_)) | sea_query::Value::TinyUnsigned(Some(_)) | 
                        sea_query::Value::SmallUnsigned(Some(_)) => {
                            params.push(&ints[int_idx] as &dyn may_postgres::types::ToSql);
                            int_idx += 1;
                        }
                        sea_query::Value::BigInt(Some(_)) | sea_query::Value::Unsigned(Some(_)) | 
                        sea_query::Value::BigUnsigned(Some(_)) => {
                            params.push(&big_ints[big_int_idx] as &dyn may_postgres::types::ToSql);
                            big_int_idx += 1;
                        }
                        sea_query::Value::String(Some(_)) => {
                            params.push(&strings[string_idx] as &dyn may_postgres::types::ToSql);
                            string_idx += 1;
                        }
                        sea_query::Value::Bytes(Some(_)) => {
                            params.push(&bytes[bytes_idx] as &dyn may_postgres::types::ToSql);
                            bytes_idx += 1;
                        }
                        sea_query::Value::Bool(None) | sea_query::Value::Int(None) | 
                        sea_query::Value::BigInt(None) | sea_query::Value::String(None) | 
                        sea_query::Value::Bytes(None) | sea_query::Value::TinyInt(None) | 
                        sea_query::Value::SmallInt(None) | sea_query::Value::TinyUnsigned(None) |
                        sea_query::Value::SmallUnsigned(None) | sea_query::Value::Unsigned(None) | 
                        sea_query::Value::BigUnsigned(None) => {
                            params.push(&nulls[null_idx] as &dyn may_postgres::types::ToSql);
                            null_idx += 1;
                        }
                        sea_query::Value::Float(Some(_)) => {
                            params.push(&floats[float_idx] as &dyn may_postgres::types::ToSql);
                            float_idx += 1;
                        }
                        sea_query::Value::Double(Some(_)) => {
                            params.push(&doubles[double_idx] as &dyn may_postgres::types::ToSql);
                            double_idx += 1;
                        }
                        sea_query::Value::Float(None) | sea_query::Value::Double(None) => {
                            params.push(&nulls[null_idx] as &dyn may_postgres::types::ToSql);
                            null_idx += 1;
                        }
                        #[cfg(feature = "with-json")]
                        sea_query::Value::Json(Some(_)) => {
                            params.push(&strings[string_idx] as &dyn may_postgres::types::ToSql);
                            string_idx += 1;
                        }
                        #[cfg(feature = "with-json")]
                        sea_query::Value::Json(None) => {
                            params.push(&nulls[null_idx] as &dyn may_postgres::types::ToSql);
                            null_idx += 1;
                        }
                        _ => return Err(lifeguard::LifeError::Other(format!("Unsupported value type in insert_many"))),
                    }
                }
                
                let rows = executor.query_all(&sql, &params)?;
                let mut models = Vec::new();
                for row in rows {
                    models.push(#model_name::from_row(&row).map_err(|e| lifeguard::LifeError::ParseError(format!("Failed to parse row: {}", e)))?);
                }
                Ok(models)
            }
            
            /// Batch update matching records
            ///
            /// # Arguments
            ///
            /// * `filter` - Filter expression to match records to update
            /// * `values` - Record containing values to update
            /// * `executor` - The executor to use for the query
            ///
            /// # Returns
            ///
            /// Returns the number of rows updated.
            pub fn update_many<E: lifeguard::LifeExecutor>(filter: sea_query::Expr, values: &#struct_name::Record, executor: &E) -> Result<u64, lifeguard::LifeError> {
                use sea_query::{UpdateStatement, PostgresQueryBuilder, ExprTrait};
                use sea_query::Iden;
                
                struct TableName;
                impl Iden for TableName {
                    fn unquoted(&self) -> &str {
                        #struct_name::TABLE_NAME
                    }
                }
                
                let mut query = sea_query::UpdateStatement::default();
                query.table(TableName);
                
                // Add SET clauses for dirty fields (skip primary key)
                let mut set_count = 0;
                #(#update_many_setters)*
                
                if set_count == 0 {
                    return Err(lifeguard::LifeError::Other("No fields to update".to_string()));
                }
                
                // Add WHERE clause
                query.and_where(filter);
                
                let (sql, sea_values) = query.build(PostgresQueryBuilder);
                
                // Convert SeaQuery values to ToSql parameters (same pattern as insert_many)
                let mut bools: Vec<bool> = Vec::new();
                let mut ints: Vec<i32> = Vec::new();
                let mut big_ints: Vec<i64> = Vec::new();
                let mut strings: Vec<String> = Vec::new();
                let mut bytes: Vec<Vec<u8>> = Vec::new();
                let mut nulls: Vec<Option<i32>> = Vec::new();
                let mut floats: Vec<f32> = Vec::new();
                let mut doubles: Vec<f64> = Vec::new();
                
                for value in sea_values.iter() {
                    match value {
                        sea_query::Value::Bool(Some(b)) => bools.push(*b),
                        sea_query::Value::Int(Some(i)) => ints.push(*i),
                        sea_query::Value::BigInt(Some(i)) => big_ints.push(*i),
                        sea_query::Value::String(Some(s)) => strings.push(s.clone()),
                        sea_query::Value::Bytes(Some(b)) => bytes.push(b.clone()),
                        sea_query::Value::TinyInt(Some(i)) => ints.push(*i as i32),
                        sea_query::Value::SmallInt(Some(i)) => ints.push(*i as i32),
                        sea_query::Value::TinyUnsigned(Some(u)) => ints.push(*u as i32),
                        sea_query::Value::SmallUnsigned(Some(u)) => ints.push(*u as i32),
                        sea_query::Value::Unsigned(Some(u)) => big_ints.push(*u as i64),
                        sea_query::Value::BigUnsigned(Some(u)) => {
                            if *u > i64::MAX as u64 {
                                return Err(lifeguard::LifeError::Other(format!("Value too large: {}", u)));
                            }
                            big_ints.push(*u as i64);
                        }
                        sea_query::Value::Float(Some(f)) => floats.push(*f),
                        sea_query::Value::Double(Some(d)) => doubles.push(*d),
                        sea_query::Value::Bool(None) | sea_query::Value::Int(None) | 
                        sea_query::Value::BigInt(None) | sea_query::Value::String(None) | 
                        sea_query::Value::Bytes(None) | sea_query::Value::TinyInt(None) |
                        sea_query::Value::SmallInt(None) | sea_query::Value::TinyUnsigned(None) |
                        sea_query::Value::SmallUnsigned(None) | sea_query::Value::Unsigned(None) |
                        sea_query::Value::BigUnsigned(None) | sea_query::Value::Float(None) | 
                        sea_query::Value::Double(None) => nulls.push(None),
                        #[cfg(feature = "with-json")]
                        sea_query::Value::Json(Some(j)) => strings.push(j.clone()),
                        #[cfg(feature = "with-json")]
                        sea_query::Value::Json(None) => nulls.push(None),
                        _ => return Err(lifeguard::LifeError::Other(format!("Unsupported value type in update_many"))),
                    }
                }
                
                let mut params: Vec<&dyn may_postgres::types::ToSql> = Vec::new();
                let mut bool_idx = 0;
                let mut int_idx = 0;
                let mut big_int_idx = 0;
                let mut string_idx = 0;
                let mut bytes_idx = 0;
                let mut null_idx = 0;
                let mut float_idx = 0;
                let mut double_idx = 0;
                
                for value in sea_values.iter() {
                    match value {
                        sea_query::Value::Bool(Some(_)) => {
                            params.push(&bools[bool_idx] as &dyn may_postgres::types::ToSql);
                            bool_idx += 1;
                        }
                        sea_query::Value::Int(Some(_)) | sea_query::Value::TinyInt(Some(_)) | 
                        sea_query::Value::SmallInt(Some(_)) | sea_query::Value::TinyUnsigned(Some(_)) | 
                        sea_query::Value::SmallUnsigned(Some(_)) => {
                            params.push(&ints[int_idx] as &dyn may_postgres::types::ToSql);
                            int_idx += 1;
                        }
                        sea_query::Value::BigInt(Some(_)) | sea_query::Value::Unsigned(Some(_)) | 
                        sea_query::Value::BigUnsigned(Some(_)) => {
                            params.push(&big_ints[big_int_idx] as &dyn may_postgres::types::ToSql);
                            big_int_idx += 1;
                        }
                        sea_query::Value::String(Some(_)) => {
                            params.push(&strings[string_idx] as &dyn may_postgres::types::ToSql);
                            string_idx += 1;
                        }
                        sea_query::Value::Bytes(Some(_)) => {
                            params.push(&bytes[bytes_idx] as &dyn may_postgres::types::ToSql);
                            bytes_idx += 1;
                        }
                        sea_query::Value::Bool(None) | sea_query::Value::Int(None) | 
                        sea_query::Value::BigInt(None) | sea_query::Value::String(None) | 
                        sea_query::Value::Bytes(None) | sea_query::Value::TinyInt(None) | 
                        sea_query::Value::SmallInt(None) | sea_query::Value::TinyUnsigned(None) |
                        sea_query::Value::SmallUnsigned(None) | sea_query::Value::Unsigned(None) | 
                        sea_query::Value::BigUnsigned(None) => {
                            params.push(&nulls[null_idx] as &dyn may_postgres::types::ToSql);
                            null_idx += 1;
                        }
                        sea_query::Value::Float(Some(_)) => {
                            params.push(&floats[float_idx] as &dyn may_postgres::types::ToSql);
                            float_idx += 1;
                        }
                        sea_query::Value::Double(Some(_)) => {
                            params.push(&doubles[double_idx] as &dyn may_postgres::types::ToSql);
                            double_idx += 1;
                        }
                        sea_query::Value::Float(None) | sea_query::Value::Double(None) => {
                            params.push(&nulls[null_idx] as &dyn may_postgres::types::ToSql);
                            null_idx += 1;
                        }
                        _ => return Err(lifeguard::LifeError::Other(format!("Unsupported value type in update_many"))),
                    }
                }
                
                executor.execute(&sql, &params)
            }
            
            /// Batch delete matching records
            ///
            /// # Arguments
            ///
            /// * `filter` - Filter expression to match records to delete
            /// * `executor` - The executor to use for the query
            ///
            /// # Returns
            ///
            /// Returns the number of rows deleted.
            pub fn delete_many<E: lifeguard::LifeExecutor>(filter: sea_query::Expr, executor: &E) -> Result<u64, lifeguard::LifeError> {
                use sea_query::{DeleteStatement, PostgresQueryBuilder};
                use sea_query::Iden;
                
                struct TableName;
                impl Iden for TableName {
                    fn unquoted(&self) -> &str {
                        #struct_name::TABLE_NAME
                    }
                }
                
                let mut query = sea_query::DeleteStatement::default();
                query
                    .from_table(TableName)
                    .and_where(filter);
                
                let (sql, sea_values) = query.build(PostgresQueryBuilder);
                
                // Convert SeaQuery values to ToSql parameters (same pattern as update_many)
                let mut bools: Vec<bool> = Vec::new();
                let mut ints: Vec<i32> = Vec::new();
                let mut big_ints: Vec<i64> = Vec::new();
                let mut strings: Vec<String> = Vec::new();
                let mut bytes: Vec<Vec<u8>> = Vec::new();
                let mut nulls: Vec<Option<i32>> = Vec::new();
                let mut floats: Vec<f32> = Vec::new();
                let mut doubles: Vec<f64> = Vec::new();
                
                for value in sea_values.iter() {
                    match value {
                        sea_query::Value::Bool(Some(b)) => bools.push(*b),
                        sea_query::Value::Int(Some(i)) => ints.push(*i),
                        sea_query::Value::BigInt(Some(i)) => big_ints.push(*i),
                        sea_query::Value::String(Some(s)) => strings.push(s.clone()),
                        sea_query::Value::Bytes(Some(b)) => bytes.push(b.clone()),
                        sea_query::Value::TinyInt(Some(i)) => ints.push(*i as i32),
                        sea_query::Value::SmallInt(Some(i)) => ints.push(*i as i32),
                        sea_query::Value::TinyUnsigned(Some(u)) => ints.push(*u as i32),
                        sea_query::Value::SmallUnsigned(Some(u)) => ints.push(*u as i32),
                        sea_query::Value::Unsigned(Some(u)) => big_ints.push(*u as i64),
                        sea_query::Value::BigUnsigned(Some(u)) => {
                            if *u > i64::MAX as u64 {
                                return Err(lifeguard::LifeError::Other(format!("Value too large: {}", u)));
                            }
                            big_ints.push(*u as i64);
                        }
                        sea_query::Value::Float(Some(f)) => floats.push(*f),
                        sea_query::Value::Double(Some(d)) => doubles.push(*d),
                        sea_query::Value::Bool(None) | sea_query::Value::Int(None) | 
                        sea_query::Value::BigInt(None) | sea_query::Value::String(None) | 
                        sea_query::Value::Bytes(None) | sea_query::Value::TinyInt(None) |
                        sea_query::Value::SmallInt(None) | sea_query::Value::TinyUnsigned(None) |
                        sea_query::Value::SmallUnsigned(None) | sea_query::Value::Unsigned(None) |
                        sea_query::Value::BigUnsigned(None) | sea_query::Value::Float(None) | 
                        sea_query::Value::Double(None) => nulls.push(None),
                        #[cfg(feature = "with-json")]
                        sea_query::Value::Json(Some(j)) => strings.push(j.clone()),
                        #[cfg(feature = "with-json")]
                        sea_query::Value::Json(None) => nulls.push(None),
                        _ => return Err(lifeguard::LifeError::Other(format!("Unsupported value type in delete_many"))),
                    }
                }
                
                let mut params: Vec<&dyn may_postgres::types::ToSql> = Vec::new();
                let mut bool_idx = 0;
                let mut int_idx = 0;
                let mut big_int_idx = 0;
                let mut string_idx = 0;
                let mut bytes_idx = 0;
                let mut null_idx = 0;
                let mut float_idx = 0;
                let mut double_idx = 0;
                
                for value in sea_values.iter() {
                    match value {
                        sea_query::Value::Bool(Some(_)) => {
                            params.push(&bools[bool_idx] as &dyn may_postgres::types::ToSql);
                            bool_idx += 1;
                        }
                        sea_query::Value::Int(Some(_)) | sea_query::Value::TinyInt(Some(_)) | 
                        sea_query::Value::SmallInt(Some(_)) | sea_query::Value::TinyUnsigned(Some(_)) | 
                        sea_query::Value::SmallUnsigned(Some(_)) => {
                            params.push(&ints[int_idx] as &dyn may_postgres::types::ToSql);
                            int_idx += 1;
                        }
                        sea_query::Value::BigInt(Some(_)) | sea_query::Value::Unsigned(Some(_)) | 
                        sea_query::Value::BigUnsigned(Some(_)) => {
                            params.push(&big_ints[big_int_idx] as &dyn may_postgres::types::ToSql);
                            big_int_idx += 1;
                        }
                        sea_query::Value::String(Some(_)) => {
                            params.push(&strings[string_idx] as &dyn may_postgres::types::ToSql);
                            string_idx += 1;
                        }
                        sea_query::Value::Bytes(Some(_)) => {
                            params.push(&bytes[bytes_idx] as &dyn may_postgres::types::ToSql);
                            bytes_idx += 1;
                        }
                        sea_query::Value::Bool(None) | sea_query::Value::Int(None) | 
                        sea_query::Value::BigInt(None) | sea_query::Value::String(None) | 
                        sea_query::Value::Bytes(None) | sea_query::Value::TinyInt(None) | 
                        sea_query::Value::SmallInt(None) | sea_query::Value::TinyUnsigned(None) |
                        sea_query::Value::SmallUnsigned(None) | sea_query::Value::Unsigned(None) | 
                        sea_query::Value::BigUnsigned(None) => {
                            params.push(&nulls[null_idx] as &dyn may_postgres::types::ToSql);
                            null_idx += 1;
                        }
                        sea_query::Value::Float(Some(_)) => {
                            params.push(&floats[float_idx] as &dyn may_postgres::types::ToSql);
                            float_idx += 1;
                        }
                        sea_query::Value::Double(Some(_)) => {
                            params.push(&doubles[double_idx] as &dyn may_postgres::types::ToSql);
                            double_idx += 1;
                        }
                        sea_query::Value::Float(None) | sea_query::Value::Double(None) => {
                            params.push(&nulls[null_idx] as &dyn may_postgres::types::ToSql);
                            null_idx += 1;
                        }
                        _ => return Err(lifeguard::LifeError::Other(format!("Unsupported value type in delete_many"))),
                    }
                }
                
                executor.execute(&sql, &params)
            }
        }
    } else {
        quote! {}
    };
    
    // Generate the expanded code
    let expanded = quote! {
        // Model struct (immutable row representation)
        #[derive(Debug, Clone)]
        pub struct #model_name {
            #(#model_fields)*
        }
        
        // Column enum
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Column {
            #(#column_variants)*
        }
        
        // Implement Iden for Column enum (required for Expr::col())
        // This allows Column to be used directly with sea_query expressions
        // sea_query provides a blanket impl: impl<T> IntoColumnRef for T where T: Into<ColumnRef>
        // Since Iden implements Into<ColumnRef>, implementing Iden is sufficient
        impl sea_query::Iden for Column {
            fn unquoted(&self) -> &str {
                match self {
                    #(#iden_impls)*
                }
            }
        }
        
        // PrimaryKey enum
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum PrimaryKey {
            #(#primary_key_variants)*
        }
        
        // Entity type alias
        pub type Entity = #struct_name;
        
        // FromRow implementation for converting may_postgres::Row to Model
        impl #model_name {
            /// Convert a `may_postgres::Row` into a `#model_name`
            ///
            /// This extracts values from the row using column names (snake_case by default,
            /// or custom column names if specified via `#[column_name]` attribute).
            pub fn from_row(row: &may_postgres::Row) -> Result<Self, may_postgres::Error> {
                Ok(Self {
                    #(#from_row_fields)*
                })
            }
        }
        
        // Implement FromRow trait for Model
        impl lifeguard::FromRow for #model_name {
            fn from_row(row: &may_postgres::Row) -> Result<Self, may_postgres::Error> {
                #model_name::from_row(row)
            }
        }
        
        // Table name constant
        impl #struct_name {
            pub const TABLE_NAME: &'static str = #table_name;
        }
        
        // CRUD methods on Model
        impl #model_name {
            #crud_methods
        }
    };
    
    TokenStream::from(expanded)
}
