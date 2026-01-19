//! Derive macro for `DeriveLinked` - generates Linked trait implementations
//!
//! This macro generates:
//! - Linked trait implementations for each relationship variant in the enum
//! - Multi-hop relationship paths using Related trait implementations

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, Data, DataEnum, DeriveInput, Variant};

/// Derive macro for `DeriveLinked` - generates Linked trait implementations
///
/// This macro generates `Linked<I, T>` trait implementations from enum variants
/// with `#[lifeguard(linked = "...")]` attributes, reducing boilerplate for
/// multi-hop relationship queries.
///
/// # Example
///
/// ```ignore
/// use lifeguard_derive::DeriveLinked;
///
/// #[derive(DeriveLinked)]
/// pub enum LinkedRelation {
///     #[lifeguard(linked = "PostEntity -> CommentEntity")]
///     Comments,
/// }
/// ```
///
/// This generates:
/// ```ignore
/// use lifeguard::relation::Linked;
/// use lifeguard::{Related, RelationDef};
///
/// impl Linked<PostEntity, CommentEntity> for Entity {
///     fn via() -> Vec<RelationDef> {
///         vec![
///             <Entity as Related<PostEntity>>::to(),
///             <PostEntity as Related<CommentEntity>>::to(),
///         ]
///     }
/// }
/// ```
pub fn derive_linked(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    
    let enum_name = &input.ident;
    
    // Extract enum variants
    let variants = match &input.data {
        Data::Enum(DataEnum { variants, .. }) => variants,
        _ => {
            return syn::Error::new_spanned(
                &input.ident,
                "DeriveLinked can only be derived for enums",
            )
            .to_compile_error()
            .into();
        }
    };
    
    // Process each variant to extract linked path information
    let mut linked_impls = Vec::new();
    
    for variant in variants {
        match process_linked_variant(variant, enum_name) {
            Ok(Some(linked_impl)) => linked_impls.push(linked_impl),
            Ok(None) => {
                // No linked attribute - skip this variant (not an error)
            }
            Err(err) => {
                // Return error immediately
                return err.to_compile_error().into();
            }
        }
    }
    
    let expanded: TokenStream2 = quote! {
        #(#linked_impls)*
    };
    
    TokenStream::from(expanded)
}

/// Structure representing a parsed linked path
struct LinkedPath {
    /// All entity paths in the chain (excluding Self)
    /// For "PostEntity -> CommentEntity", this is [PostEntity, CommentEntity]
    /// For "A -> B -> C", this is [A, B, C]
    hops: Vec<syn::Path>,
}

/// Process a linked variant and generate Linked trait implementation
///
/// Returns:
/// - `Ok(Some(impl))` if variant has linked attribute and parsing succeeds
/// - `Ok(None)` if variant has no linked attribute (not an error)
/// - `Err(error)` if parsing fails
fn process_linked_variant(
    variant: &Variant,
    _enum_name: &syn::Ident,
) -> Result<Option<TokenStream2>, syn::Error> {
    let mut linked_path: Option<String> = None;
    
    // Parse attributes to find linked path
    for attr in &variant.attrs {
        if attr.path().is_ident("lifeguard") {
            // Parse nested attributes like #[lifeguard(linked = "...")]
            if let Err(err) = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("linked") {
                    // Parse: linked = "PostEntity -> CommentEntity"
                    let value: syn::LitStr = meta.value()?.parse()?;
                    linked_path = Some(value.value());
                    Ok(())
                } else {
                    // Ignore other lifeguard attributes
                    Ok(())
                }
            }) {
                return Err(err);
            }
        }
    }
    
    // Generate impl if path found
    if let Some(path_str) = linked_path {
        let path = parse_linked_path(&path_str, variant.ident.span())?;
        Ok(Some(generate_linked_impl(variant, &path)?))
    } else {
        // No linked attribute - skip this variant
        Ok(None)
    }
}

/// Parse linked path from attribute string
///
/// Examples:
/// - "PostEntity -> CommentEntity" -> [PostEntity, CommentEntity]
/// - "PostEntity -> CommentEntity -> ReactionEntity" -> [PostEntity, CommentEntity, ReactionEntity]
/// - "super::posts::PostEntity -> CommentEntity" -> [super::posts::PostEntity, CommentEntity]
fn parse_linked_path(path_str: &str, error_span: proc_macro2::Span) -> Result<LinkedPath, syn::Error> {
    // Split by "->" to get hops
    let hops: Vec<&str> = path_str
        .split("->")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    
    if hops.is_empty() {
        return Err(syn::Error::new(
            error_span,
            "Linked path cannot be empty. Use format: `Entity1 -> Entity2` or `Entity1 -> Entity2 -> Entity3`",
        ));
    }
    
    if hops.len() < 2 {
        return Err(syn::Error::new(
            error_span,
            format!(
                "Linked path must have at least 2 hops (intermediate and target), found {}. Use format: `Entity1 -> Entity2`",
                hops.len()
            ),
        ));
    }
    
    // Parse each hop as a Rust path
    let mut parsed_hops = Vec::new();
    for (idx, hop) in hops.iter().enumerate() {
        let hop_path: syn::Path = syn::parse_str(hop).map_err(|e| {
            syn::Error::new(
                error_span,
                format!(
                    "Invalid entity path in hop {} '{}': {}. Expected a valid Rust path like `PostEntity` or `super::posts::PostEntity`",
                    idx + 1,
                    hop,
                    e
                ),
            )
        })?;
        parsed_hops.push(hop_path);
    }
    
    Ok(LinkedPath {
        hops: parsed_hops,
    })
}

/// Generate Linked trait implementation
///
/// For a path like "PostEntity -> CommentEntity", generates:
/// ```rust
/// impl Linked<PostEntity, CommentEntity> for Entity {
///     fn via() -> Vec<RelationDef> {
///         vec![
///             <Entity as Related<PostEntity>>::to(),
///             <PostEntity as Related<CommentEntity>>::to(),
///         ]
///     }
/// }
/// ```
fn generate_linked_impl(
    variant: &Variant,
    path: &LinkedPath,
) -> Result<TokenStream2, syn::Error> {
    if path.hops.len() < 2 {
        return Err(syn::Error::new_spanned(
            variant,
            "Linked path must have at least 2 hops",
        ));
    }
    
    // First hop is intermediate, last is target
    let intermediate = &path.hops[0];
    let target = &path.hops[path.hops.len() - 1];
    
    // Build the path segments: Self -> I1 -> I2 -> ... -> T
    let mut path_segments = Vec::new();
    
    // First hop: Self -> Intermediate
    path_segments.push(quote! {
        <Entity as lifeguard::Related<#intermediate>>::to(),
    });
    
    // Additional hops: I1 -> I2, I2 -> I3, etc.
    for i in 0..(path.hops.len() - 1) {
        let from = &path.hops[i];
        let to = &path.hops[i + 1];
        path_segments.push(quote! {
            <#from as lifeguard::Related<#to>>::to(),
        });
    }
    
    // Generate the impl block
    // Note: Linked is in lifeguard::relation, but we use the full path for clarity
    Ok(quote! {
        impl lifeguard::relation::Linked<#intermediate, #target> for Entity {
            fn via() -> Vec<lifeguard::RelationDef> {
                vec![
                    #(#path_segments)*
                ]
            }
        }
    })
}
