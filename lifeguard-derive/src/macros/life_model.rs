//! LifeModel derive macro implementation
//!
//! Based on SeaORM's expand_derive_entity_model pattern (v2.0.0-rc.28)
//! Generates Entity, Column, PrimaryKey, Model, FromRow, and LifeModelTrait

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput, Data, DataStruct, Fields, Ident};
use quote::quote;

use crate::attributes;
use crate::utils;

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
    let model_name = Ident::new(
        &format!("{}Model", struct_name),
        struct_name.span()
    );
    let model_name_lit = syn::LitStr::new(&model_name.to_string(), model_name.span());
    
    // Process fields to generate:
    // - Column enum variants
    // - PrimaryKey enum variants
    // - Model struct fields
    // - FromRow field extraction
    let mut column_variants = Vec::new();
    let mut primary_key_variants = Vec::new();
    let mut model_fields = Vec::new();
    let mut from_row_fields = Vec::new();
    let mut iden_impls = Vec::new();
    
    for field in fields.iter() {
        let field_name = field.ident.as_ref().unwrap();
        let field_type = &field.ty;
        let column_name = attributes::extract_column_name(field)
            .unwrap_or_else(|| utils::snake_case(&field_name.to_string()));
        
        // Check if primary key
        let is_primary_key = attributes::has_attribute(field, "primary_key");
        
        // Generate Column enum variant (PascalCase)
        let column_variant = Ident::new(
            &utils::pascal_case(&field_name.to_string()),
            field_name.span()
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
        }
        
        // Generate Model field
        model_fields.push(quote! {
            pub #field_name: #field_type,
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
    
    // Generate Entity with nested DeriveEntity (like SeaORM)
    // This triggers nested expansion where DeriveEntity generates LifeModelTrait
    let expanded = quote! {
        // STEP 1: Generate Entity with nested DeriveEntity (like SeaORM's pattern)
        // DeriveEntity will generate LifeModelTrait in a separate expansion phase
        // Note: DeriveEntity generates Default, so we don't derive it here to avoid conflict
        #[doc = " Generated by lifeguard-derive"]
        #[derive(Copy, Clone, Debug, lifeguard_derive::DeriveEntity)]
        #[table_name = #table_name_lit]
        #[model = #model_name_lit]
        pub struct Entity;
        
        // Table name constant (for convenience, matches Entity::table_name())
        impl Entity {
            pub const TABLE_NAME: &'static str = #table_name_lit;
        }
        
        // STEP 2: Generate Column enum (like SeaORM's expand_derive_entity_model)
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
        
        // STEP 3: Generate PrimaryKey enum
        #[doc = " Generated by lifeguard-derive"]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum PrimaryKey {
            #(#primary_key_variants)*
        }
        
        // STEP 4: Generate Model struct (like SeaORM's expand_derive_model)
        #[doc = " Generated by lifeguard-derive"]
        #[derive(Debug, Clone)]
        pub struct #model_name {
            #(#model_fields)*
        }
        
        // STEP 5: Generate FromRow implementation (automatic, no separate derive needed)
        #[automatically_derived]
        impl lifeguard::FromRow for #model_name {
            fn from_row(row: &may_postgres::Row) -> Result<Self, may_postgres::Error> {
                Ok(Self {
                    #(#from_row_fields)*
                })
            }
        }
        
        // STEP 6: LifeModelTrait is generated by DeriveEntity (nested expansion)
        // This happens in a separate expansion phase, allowing proper type resolution
    };
    
    TokenStream::from(expanded)
}
