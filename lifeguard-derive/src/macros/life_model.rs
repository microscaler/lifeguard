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
    
    for field in fields.iter() {
        let field_name = field.ident.as_ref().unwrap();
        let field_type = &field.ty;
        let column_name = attributes::extract_column_name(field)
            .unwrap_or_else(|| utils::snake_case(&field_name.to_string()));
        
        // Check if this is a primary key
        let is_primary_key = attributes::has_attribute(field, "primary_key");
        
        // Generate Column enum variant
        let column_variant = Ident::new(
            &utils::pascal_case(&field_name.to_string()),
            field_name.span()
        );
        column_variants.push(quote! {
            #column_variant,
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
        
        // Table name constant
        impl #struct_name {
            pub const TABLE_NAME: &'static str = #table_name;
        }
    };
    
    TokenStream::from(expanded)
}
