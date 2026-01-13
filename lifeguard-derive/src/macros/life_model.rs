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
    
    // Track primary key field for CRUD operations
    let mut primary_key_field: Option<(Ident, syn::Type, String)> = None;
    
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
            #field_name: row.try_get::<&str, #field_type>(#column_name_str)?,
        });
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
                lifeguard::SelectQuery::new(#struct_name::TABLE_NAME)
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
        
        // Implement IntoColumnRef for Column enum (enables type-safe query building)
        impl sea_query::IntoColumnRef for Column {
            fn into_column_ref(self) -> sea_query::ColumnRef {
                match self {
                    #(#column_impls)*
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
