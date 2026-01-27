//! Derive macro for `DerivePartialModel` - generates `PartialModelTrait` and `FromRow` implementations
//!
//! This macro generates:
//! - `PartialModelTrait` implementation with `selected_columns()` method
//! - `FromRow` implementation for converting database rows to partial models
//! - Column name extraction from field names or `column_name` attribute
#![allow(clippy::too_many_lines, clippy::single_match_else, clippy::match_same_arms, clippy::explicit_iter_loop)] // Complex macro code

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

use crate::utils;
use crate::attributes;

/// Generate `PartialModelTrait` and `FromRow` implementations for a partial model struct
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
            }
            return syn::Error::new_spanned(
                &input.ident,
                "DerivePartialModel requires #[lifeguard(entity = \"path::to::Entity\")] attribute. Found #[lifeguard] but missing entity parameter.",
            )
            .to_compile_error()
            .into();
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
        // Use the same extract_column_name() function as LifeModel macro for consistency
        let column_name = attributes::extract_column_name(field)
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
/// Returns Some(TokenStream2) if found, None if not found, or an error `TokenStream` if parsing fails
fn extract_entity_type(input: &DeriveInput) -> Result<Option<TokenStream2>, TokenStream2> {
    for attr in &input.attrs {
        if attr.path().is_ident("lifeguard") {
            // Parse nested attributes like #[lifeguard(entity = "...")]
            // Use parse_nested_meta for syn 2.0
            let mut entity_path_str: Option<String> = None;
            let mut entity_lit_span: Option<proc_macro2::Span> = None;
            
            // Check result and propagate errors instead of silently ignoring them
            if let Err(err) = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("entity") {
                    let value: syn::LitStr = meta.value()?.parse()?;
                    entity_lit_span = Some(value.span());
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
                // Use the struct ident span for error reporting (attribute span is not easily accessible)
                // The error will appear on the struct, but the message will be clear
                let error_span = &input.ident;
                
                // Validate that the entity path is not empty
                if entity_path_str.trim().is_empty() {
                    return Err(syn::Error::new_spanned(
                        error_span,
                        "Entity path cannot be empty. Use #[lifeguard(entity = \"path::to::Entity\")] with a valid path.",
                    )
                    .to_compile_error());
                }
                
                // Check for leading colons (absolute paths starting with ::)
                // These are valid Rust syntax but we want to catch them as errors for clarity
                if entity_path_str.starts_with("::") {
                    return Err(syn::Error::new_spanned(
                        error_span,
                        format!("Entity path has leading colons. Found absolute path in #[lifeguard(entity = \"{entity_path_str}\")]. Use a valid path like \"foo::Entity\" or \"Entity\"."),
                    )
                    .to_compile_error());
                }
                
                // Parse the entity path string
                // Try parsing as a path first, then fall back to manual construction
                let entity_path: syn::Path = if let Ok(path) = syn::parse_str::<syn::Path>(&entity_path_str) {
                    // Even if parsing succeeds, check for leading colons in the parsed path
                    if path.leading_colon.is_some() {
                        return Err(syn::Error::new_spanned(
                            error_span,
                            format!("Entity path has leading colons. Found absolute path in #[lifeguard(entity = \"{entity_path_str}\")]. Use a valid path like \"foo::Entity\" or \"Entity\"."),
                        )
                        .to_compile_error());
                    }
                    path
                } else {
                    // If parsing fails, construct a path manually
                    // Handle both simple identifiers (e.g., "UserEntity") and paths (e.g., "users::Entity")
                    let segments: Vec<&str> = entity_path_str.split("::").collect();
                    
                    // Validate segments: check for empty segments and invalid identifiers
                    // Empty segments can occur with:
                    // - Empty string: ""
                    // - Trailing colons: "foo::"
                    // - Consecutive colons: "foo::::bar"
                    // Invalid identifiers can occur with:
                    // - Single colon: ":foo" or "foo:bar"
                    // - Starting with number: "123abc"
                    // - Containing hyphens: "foo-bar"
                    // - Other invalid Rust identifier characters
                    for (idx, segment) in segments.iter().enumerate() {
                        if segment.is_empty() {
                            let error_msg = if segments.len() == 1 {
                                format!("Entity path cannot be empty. Found empty string in #[lifeguard(entity = \"{entity_path_str}\")].")
                            } else if idx == segments.len() - 1 {
                                format!("Entity path has trailing colons. Found empty segment at end in #[lifeguard(entity = \"{entity_path_str}\")]. Use a valid path like \"foo::Entity\" or \"Entity\".")
                            } else {
                                format!("Entity path has consecutive colons. Found empty segment at position {} in #[lifeguard(entity = \"{entity_path_str}\")]. Use a valid path like \"foo::Entity\" or \"Entity\".", idx + 1)
                            };
                            
                            return Err(syn::Error::new_spanned(
                                error_span,
                                error_msg,
                            )
                            .to_compile_error());
                        }
                        
                        // Validate that the segment is a valid Rust identifier
                        // Use syn::parse_str to safely check if the segment is a valid identifier
                        if syn::parse_str::<syn::Ident>(segment).is_err() {
                            return Err(syn::Error::new_spanned(
                                error_span,
                                format!("Entity path contains invalid identifier \"{segment}\" at position {} in #[lifeguard(entity = \"{entity_path_str}\")]. Identifiers must be valid Rust identifiers (e.g., start with a letter or underscore, contain only alphanumeric characters and underscores).", idx + 1),
                            )
                            .to_compile_error());
                        }
                    }
                    
                    let mut path = syn::Path {
                        leading_colon: None,
                        segments: syn::punctuated::Punctuated::new(),
                    };
                    for segment in segments {
                        // At this point, we've validated that segment is not empty and is a valid identifier
                        // Parse the segment as an identifier to get proper span handling
                        // This is safe because we've already validated it above
                        let ident = syn::parse_str::<syn::Ident>(segment)
                            .expect("Segment should be valid identifier after validation");
                        path.segments.push(syn::PathSegment {
                            ident,
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
