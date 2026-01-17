//! Derive macro for `DeriveRelation` - generates Relation enum and Related trait implementations
//!
//! This macro generates:
//! - Relation enum with variants for each relationship
//! - Related trait implementations for each relationship
//! - Query builders using SelectQuery

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, Data, DataEnum, DeriveInput, Variant};

/// Extract column name from column reference like "Column::UserId" or "super::users::Column::Id"
fn extract_column_name(column_ref: &str) -> Option<String> {
    // Parse "Column::UserId" or "super::users::Column::Id" format
    // Extract the last segment after the last "::"
    if let Some(last_segment) = column_ref.split("::").last() {
        // Convert PascalCase to snake_case for column name
        // Example: "UserId" -> "user_id"
        let snake_case = convert_pascal_to_snake_case(last_segment);
        Some(snake_case)
    } else {
        None
    }
}

/// Convert PascalCase to snake_case
/// Example: "UserId" -> "user_id", "OwnerId" -> "owner_id"
fn convert_pascal_to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(c.to_lowercase().next().unwrap_or(c));
    }
    result
}

/// Derive macro for `DeriveRelation` - generates Relation enum and Related implementations
///
/// This macro generates:
/// - Relation enum (already defined by user, we just process it)
/// - Related trait implementations for each relationship variant
///
/// # Example
///
/// ```no_run
/// use lifeguard_derive::DeriveRelation;
/// use lifeguard::{Related, SelectQuery, LifeModelTrait};
///
/// #[derive(DeriveRelation)]
/// pub enum Relation {
///     #[lifeguard(has_many = "super::posts::Entity")]
///     Posts,
///     #[lifeguard(
///         belongs_to = "super::users::Entity",
///         from = "Column::UserId",
///         to = "super::users::Column::Id"
///     )]
///     User,
/// }
/// ```
pub fn derive_relation(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    
    let enum_name = &input.ident;
    
    // Extract enum variants
    let variants = match &input.data {
        Data::Enum(DataEnum { variants, .. }) => variants,
        _ => {
            return syn::Error::new_spanned(
                &input.ident,
                "DeriveRelation can only be derived for enums",
            )
            .to_compile_error()
            .into();
        }
    };
    
    // Process each variant to extract relationship information
    let mut related_impls = Vec::new();
    
    for variant in variants {
        if let Some(related_impl) = process_relation_variant(variant, enum_name) {
            related_impls.push(related_impl);
        }
    }
    
    let expanded: TokenStream2 = quote! {
        #(#related_impls)*
    };
    
    TokenStream::from(expanded)
}

/// Process a relation variant and generate Related trait implementation
fn process_relation_variant(
    variant: &Variant,
    _enum_name: &syn::Ident,
) -> Option<TokenStream2> {
    let _variant_name = &variant.ident;
    
    // Parse attributes to find relationship type and target entity
    let mut relationship_type: Option<String> = None;
    let mut target_entity: Option<String> = None;
    let mut from_column: Option<String> = None;
    let mut to_column: Option<String> = None;
    
    for attr in &variant.attrs {
        if attr.path().is_ident("lifeguard") {
            // Parse nested attributes like #[lifeguard(has_many = "...")]
            // Use parse_nested_meta for syn 2.0
            let _ = attr.parse_nested_meta(|meta| {
                // Check for key-value pairs like has_many = "..."
                if meta.path.is_ident("has_many") || meta.path.is_ident("has_one") || meta.path.is_ident("belongs_to") {
                    let key = meta.path.get_ident().map(|i| i.to_string()).unwrap_or_default();
                    let value: syn::LitStr = meta.value()?.parse()?;
                    relationship_type = Some(key);
                    target_entity = Some(value.value());
                    Ok(())
                } else if meta.path.is_ident("from") {
                    let value: syn::LitStr = meta.value()?.parse()?;
                    from_column = Some(value.value());
                    Ok(())
                } else if meta.path.is_ident("to") {
                    let value: syn::LitStr = meta.value()?.parse()?;
                    to_column = Some(value.value());
                    Ok(())
                } else {
                    Ok(())
                }
            });
        }
    }
    
    // Generate Related trait implementation if we have the required information
    if let (Some(_rel_type), Some(target)) = (relationship_type, target_entity) {
        // Parse target entity path (e.g., "super::posts::Entity")
        let target_entity_path: syn::Path = syn::parse_str(&target)
            .unwrap_or_else(|_| {
                // If parsing fails, try to construct a path
                let segments: Vec<&str> = target.split("::").collect();
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
            });
        
        // Generate Related trait implementation and RelationMetadata implementation
        // Parse from/to columns if provided
        // Generate RelationMetadata implementation if from/to columns are provided
        // Store foreign key column name as a const for use in find_related()
        let fk_col_impl = if let (Some(from_col), Some(_to_col)) = (from_column.as_ref(), to_column.as_ref()) {
            // Extract column name from "Column::UserId" format
            // The "from" column is the foreign key column in the related entity's table
            if let Some(fk_name) = extract_column_name(from_col) {
                // Convert to snake_case for column name
                let fk_name_lit = syn::LitStr::new(&fk_name, proc_macro2::Span::call_site());
                quote! {
                    impl lifeguard::RelationMetadata<Entity> for #target_entity_path {
                        fn foreign_key_column() -> Option<&'static str> {
                            Some(#fk_name_lit)
                        }
                    }
                }
            } else {
                quote! {}
            }
        } else {
            // No from/to columns specified - use default behavior
            quote! {}
        };
        
        // Generate Related trait implementation
        // Note: Entity is assumed to be in the same module as the Relation enum
        Some(quote! {
            impl lifeguard::Related<#target_entity_path> for Entity {
                fn to() -> lifeguard::SelectQuery<#target_entity_path> {
                    // Return base query - find_related() will add WHERE clause using relationship metadata
                    lifeguard::SelectQuery::new()
                }
            }
            #fk_col_impl
        })
    } else {
        None
    }
}
