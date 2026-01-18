//! Derive macro for `DerivePartialModel` - generates PartialModelTrait and FromRow implementations
//!
//! This macro generates:
//! - PartialModelTrait implementation with selected_columns() method
//! - FromRow implementation for converting database rows to partial models
//! - Column name extraction from field names or column_name attribute

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

use crate::utils;

/// Generate PartialModelTrait and FromRow implementations for a partial model struct
pub fn derive_partial_model(input: TokenStream) -> TokenStream {
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
                "DerivePartialModel can only be derived for structs with named fields",
            )
            .to_compile_error()
            .into();
        }
    };
    
    // Extract Entity type from attribute
    let entity_type = match extract_entity_type(&input) {
        Ok(Some(ty)) => ty,
        Ok(None) => {
            // Check if there's a lifeguard attribute at all
            let has_lifeguard_attr = input.attrs.iter().any(|attr| attr.path().is_ident("lifeguard"));
            if !has_lifeguard_attr {
                return syn::Error::new_spanned(
                    &input.ident,
                    "DerivePartialModel requires #[lifeguard(entity = \"path::to::Entity\")] attribute. No #[lifeguard] attribute found.",
                )
                .to_compile_error()
                .into();
            } else {
                return syn::Error::new_spanned(
                    &input.ident,
                    "DerivePartialModel requires #[lifeguard(entity = \"path::to::Entity\")] attribute. Found #[lifeguard] but missing entity parameter.",
                )
                .to_compile_error()
                .into();
            }
        }
        Err(err) => {
            // Return the parsing error
            return err.into();
        }
    };
    
    // Generate column names and FromRow field extraction
    let mut column_names = Vec::new();
    let mut from_row_fields: Vec<TokenStream2> = Vec::new();
    
    for field in fields.iter() {
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
        
        column_names.push(column_name.clone());
        
        // Generate FromRow field extraction (similar to from_row.rs)
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
    
    // Generate selected_columns() method returning column names
    let column_name_literals: Vec<TokenStream2> = column_names
        .iter()
        .map(|name| {
            let name_str = name.as_str();
            quote! { #name_str }
        })
        .collect();
    
    let expanded: TokenStream2 = quote! {
        // Implement PartialModelTrait for partial model
        impl lifeguard::PartialModelTrait for #struct_name {
            type Entity = #entity_type;
            
            fn selected_columns() -> Vec<&'static str> {
                vec![
                    #(#column_name_literals),*
                ]
            }
        }
        
        // Implement FromRow trait for partial model
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

/// Extract Entity type from #[lifeguard(entity = "...")] attribute
/// Returns Some(TokenStream2) if found, None if not found, or an error TokenStream if parsing fails
fn extract_entity_type(input: &DeriveInput) -> Result<Option<TokenStream2>, TokenStream2> {
    for attr in &input.attrs {
        if attr.path().is_ident("lifeguard") {
            // Parse nested attributes like #[lifeguard(entity = "...")]
            // Use parse_nested_meta for syn 2.0
            let mut entity_path_str: Option<String> = None;
            
            // Check result and propagate errors instead of silently ignoring them
            if let Err(err) = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("entity") {
                    let value: syn::LitStr = meta.value()?.parse()?;
                    entity_path_str = Some(value.value());
                    Ok(())
                } else {
                    Ok(())
                }
            }) {
                // Return compile error if parsing fails
                return Err(err.to_compile_error());
            }
            
            if let Some(entity_path_str) = entity_path_str {
                // Parse the entity path string
                // Try parsing as a path first, then fall back to manual construction
                let entity_path: syn::Path = if let Ok(path) = syn::parse_str::<syn::Path>(&entity_path_str) {
                    path
                } else {
                    // If parsing fails, construct a path manually
                    // Handle both simple identifiers (e.g., "UserEntity") and paths (e.g., "users::Entity")
                    let segments: Vec<&str> = entity_path_str.split("::").collect();
                    let mut path = syn::Path {
                        leading_colon: None,
                        segments: syn::punctuated::Punctuated::new(),
                    };
                    for segment in segments {
                        path.segments.push(syn::PathSegment {
                            ident: syn::Ident::new(segment, proc_macro2::Span::call_site()),
                            arguments: syn::PathArguments::None,
                        });
                    }
                    path
                };
                return Ok(Some(quote! { #entity_path }));
            }
        }
    }
    Ok(None)
}
