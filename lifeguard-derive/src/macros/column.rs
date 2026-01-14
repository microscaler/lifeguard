//! Derive macro for Column enum
//!
//! Generates Column enum with Iden implementation for use in sea_query.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

use crate::attributes;
use crate::utils;

/// Generate Column enum with Iden implementation
pub fn derive_column(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    
    // Extract struct fields (we need the original struct to know the columns)
    let fields = match &input.data {
        Data::Struct(syn::DataStruct {
            fields: Fields::Named(fields),
            ..
        }) => &fields.named,
        _ => {
            return syn::Error::new_spanned(
                &input.ident,
                "DeriveColumn requires struct fields to generate column enum",
            )
            .to_compile_error()
            .into();
        }
    };
    
    // Generate Column enum variants and Iden implementations
    let mut column_variants = Vec::new();
    let mut iden_impls = Vec::new();
    
    for field in fields.iter() {
        let field_name = field.ident.as_ref().unwrap();
        let column_name = attributes::extract_column_name(field)
            .unwrap_or_else(|| utils::snake_case(&field_name.to_string()));
        
        // Generate Column enum variant
        let column_variant = syn::Ident::new(
            &utils::pascal_case(&field_name.to_string()),
            field_name.span()
        );
        
        column_variants.push(quote! {
            #column_variant,
        });
        
        // Generate Iden implementation for this variant
        let column_name_str = column_name.clone();
        iden_impls.push(quote! {
            Column::#column_variant => #column_name_str,
        });
    }
    
    let expanded: TokenStream2 = quote! {
        // Column enum
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Column {
            #(#column_variants)*
        }
        
        // Implement Iden for Column enum (required for Expr::col())
        impl sea_query::Iden for Column {
            fn unquoted(&self) -> &str {
                match self {
                    #(#iden_impls)*
                }
            }
        }
    };
    
    TokenStream::from(expanded)
}
