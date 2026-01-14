//! Derive macro for `FromRow` trait
//!
//! This macro generates the `FromRow` implementation for converting
//! `may_postgres::Row` into a Model struct. It's separate from `LifeModel`
//! to avoid trait bound resolution issues during macro expansion.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

use crate::utils;

/// Generate FromRow implementation for a Model struct
pub fn derive_from_row(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    
    let struct_name = &input.ident;
    
    // Extract struct fields
    let fields = match &input.data {
        Data::Struct(syn::DataStruct {
            fields: Fields::Named(fields),
            ..
        }) => &fields.named,
        _ => {
            return syn::Error::new_spanned(
                &input.ident,
                "FromRow can only be derived for structs with named fields",
            )
            .to_compile_error()
            .into();
        }
    };
    
    // Generate field extraction code
    let from_row_fields: Vec<TokenStream2> = fields
        .iter()
        .map(|field| {
            let field_name = field.ident.as_ref().unwrap();
            let field_type = &field.ty;
            
            // Get column name from attribute or use snake_case of field name
            let column_name = field
                .attrs
                .iter()
                .find(|attr| attr.path().is_ident("column_name"))
                .and_then(|attr| {
                    attr.parse_args::<syn::LitStr>().ok().map(|lit| lit.value())
                })
                .unwrap_or_else(|| {
                    // Convert field name to snake_case
                    let name = field_name.to_string();
                    utils::snake_case(&name)
                });
            
            let column_name_str = column_name.as_str();
            
            // Handle unsigned integer types by converting to signed first
            let get_expr = {
                // Check if this is an unsigned integer type
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
                    // For unsigned types, convert to signed equivalent first
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
                            let val: #signed_type = row.get(#column_name_str)?;
                            val as #field_type
                        }
                    }
                } else {
                    quote! {
                        row.get(#column_name_str)?
                    }
                }
            };
            
            quote! {
                #field_name: #get_expr,
            }
        })
        .collect();
    
    let expanded: TokenStream2 = quote! {
        // Implement FromRow trait for Model
        impl lifeguard::FromRow for #struct_name {
            fn from_row(row: &may_postgres::Row) -> Result<Self, may_postgres::Error> {
                Ok(Self {
                    #(#from_row_fields)*
                })
            }
        }
    };
    
    TokenStream::from(expanded)
}
