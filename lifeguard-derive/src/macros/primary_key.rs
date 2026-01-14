//! Derive macro for PrimaryKey enum
//!
//! Generates PrimaryKey enum for primary key columns.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

use crate::attributes;
use crate::utils;

/// Generate PrimaryKey enum
pub fn derive_primary_key(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    
    // Extract struct fields
    let fields = match &input.data {
        Data::Struct(syn::DataStruct {
            fields: Fields::Named(fields),
            ..
        }) => &fields.named,
        _ => {
            return syn::Error::new_spanned(
                &input.ident,
                "DerivePrimaryKey requires struct fields to generate primary key enum",
            )
            .to_compile_error()
            .into();
        }
    };
    
    // Generate PrimaryKey enum variants (only for primary key fields)
    let mut primary_key_variants = Vec::new();
    
    for field in fields.iter() {
        let field_name = field.ident.as_ref().unwrap();
        let is_primary_key = attributes::has_attribute(field, "primary_key");
        
        if is_primary_key {
            // Generate PrimaryKey enum variant
            let column_variant = syn::Ident::new(
                &utils::pascal_case(&field_name.to_string()),
                field_name.span()
            );
            
            primary_key_variants.push(quote! {
                #column_variant,
            });
        }
    }
    
    if primary_key_variants.is_empty() {
        return syn::Error::new_spanned(
            &input.ident,
            "DerivePrimaryKey requires at least one field with #[primary_key] attribute",
        )
        .to_compile_error()
        .into();
    }
    
    let expanded: TokenStream2 = quote! {
        // PrimaryKey enum
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum PrimaryKey {
            #(#primary_key_variants)*
        }
    };
    
    TokenStream::from(expanded)
}
