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

/// Parse a column reference string and extract the column path and variant name
/// 
/// Examples:
/// - "Column::UserId" -> (None, "UserId")
/// - "super::users::Column::Id" -> (Some("super::users"), "Id")
/// - "Column::UserId, Column::TenantId" -> (None, ["UserId", "TenantId"]) for composite keys
fn parse_column_reference(column_ref: &str) -> (Option<String>, Vec<String>) {
    // Check if this is a composite key (multiple columns separated by comma)
    let columns: Vec<&str> = column_ref.split(',').map(|s| s.trim()).collect();
    
    let mut column_variants = Vec::new();
    let mut path_prefix: Option<String> = None;
    
    for col_ref in columns {
        let parts: Vec<&str> = col_ref.split("::").collect();
        if let Some(last_segment) = parts.last() {
            column_variants.push(last_segment.to_string());
        }
        
        // Extract path prefix (everything before the last "Column::" part)
        if parts.len() >= 2 {
            // Find the index of "Column" in the path
            if let Some(col_idx) = parts.iter().position(|&p| p == "Column") {
                if col_idx > 0 {
                    let prefix = parts[..col_idx].join("::");
                    if path_prefix.is_none() {
                        path_prefix = Some(prefix);
                    }
                }
            }
        }
    }
    
    (path_prefix, column_variants)
}

/// Build Identity from column reference string(s)
/// 
/// Supports:
/// - Single column: "Column::UserId" -> Identity::Unary
/// - Composite keys: "Column::UserId, Column::TenantId" -> Identity::Binary
/// - Path-qualified: "super::users::Column::Id" -> Identity::Unary
/// 
/// Note: When the path is just "Column", it refers to the current entity's Column type.
/// The macro uses `<Entity as LifeModelTrait>::Column` to access it.
fn build_identity_from_column_ref(column_ref: &str, error_span: proc_macro2::Span) -> Result<TokenStream2, TokenStream2> {
    let (path_prefix, column_variants) = parse_column_reference(column_ref);
    
    // Build the column path (e.g., "Column" or "super::users::Column")
    // If no prefix, assume it's the current entity's Column type
    
    let column_path_expr = if let Some(prefix) = path_prefix {
        // Full path like "super::users::Column"
        let prefix_segments: Vec<&str> = prefix.split("::").collect();
        
        // Validate path segments before creating identifiers
        for (idx, segment) in prefix_segments.iter().enumerate() {
            if segment.is_empty() {
                let error_msg = if prefix_segments.len() == 1 {
                    format!("Column reference path cannot be empty. Found empty string in column reference \"{}\".", column_ref)
                } else if idx == prefix_segments.len() - 1 {
                    format!("Column reference path has trailing colons. Found empty segment at end in \"{}\". Use a valid path like \"super::users::Column::Id\".", column_ref)
                } else {
                    format!("Column reference path has consecutive colons. Found empty segment at position {} in \"{}\". Use a valid path like \"super::users::Column::Id\".", idx + 1, column_ref)
                };
                return Err(syn::Error::new(
                    error_span,
                    error_msg,
                )
                .to_compile_error());
            }
            
            // Validate that the segment is a valid Rust identifier
            if syn::parse_str::<syn::Ident>(segment).is_err() {
                return Err(syn::Error::new(
                    error_span,
                    format!("Column reference path contains invalid identifier \"{}\" at position {} in \"{}\". Identifiers must be valid Rust identifiers (e.g., start with a letter or underscore, contain only alphanumeric characters and underscores).", segment, idx + 1, column_ref),
                )
                .to_compile_error());
            }
        }
        
        let mut path_tokens = quote! {};
        for segment in prefix_segments {
            // At this point, we've validated that segment is not empty and is a valid identifier
            let ident = syn::parse_str::<syn::Ident>(segment)
                .expect("Segment should be valid identifier after validation");
            path_tokens = quote! { #path_tokens::#ident };
        }
        quote! { #path_tokens::Column }
    } else {
        // No prefix - use Entity's Column type
        quote! { <Entity as lifeguard::LifeModelTrait>::Column }
    };
    
    // Validate column variant identifiers
    for (idx, col_variant) in column_variants.iter().enumerate() {
        if col_variant.is_empty() {
            return Err(syn::Error::new(
                error_span,
                format!("Column variant cannot be empty at position {} in column reference \"{}\".", idx + 1, column_ref),
            )
            .to_compile_error());
        }
        
        // Validate that the column variant is a valid Rust identifier
        if syn::parse_str::<syn::Ident>(col_variant).is_err() {
            return Err(syn::Error::new(
                error_span,
                format!("Column reference contains invalid identifier \"{}\" at position {} in \"{}\". Identifiers must be valid Rust identifiers (e.g., start with a letter or underscore, contain only alphanumeric characters and underscores).", col_variant, idx + 1, column_ref),
            )
            .to_compile_error());
        }
    }
    
    match column_variants.len() {
        1 => {
            let col_variant = &column_variants[0];
            // At this point, we've validated that col_variant is a valid identifier
            let col_ident = syn::parse_str::<syn::Ident>(col_variant)
                .expect("Column variant should be valid identifier after validation");
            Ok(quote! {
                {
                    use lifeguard::LifeModelTrait;
                    use sea_query::IdenStatic;
                    let col = #column_path_expr::#col_ident;
                    lifeguard::Identity::Unary(sea_query::DynIden::from(col.as_str()))
                }
            })
        }
        2 => {
            let col1_variant = &column_variants[0];
            let col2_variant = &column_variants[1];
            // At this point, we've validated that both variants are valid identifiers
            let col1_ident = syn::parse_str::<syn::Ident>(col1_variant)
                .expect("Column variant should be valid identifier after validation");
            let col2_ident = syn::parse_str::<syn::Ident>(col2_variant)
                .expect("Column variant should be valid identifier after validation");
            Ok(quote! {
                {
                    use lifeguard::LifeModelTrait;
                    use sea_query::IdenStatic;
                    let col1 = #column_path_expr::#col1_ident;
                    let col2 = #column_path_expr::#col2_ident;
                    lifeguard::Identity::Binary(
                        sea_query::DynIden::from(col1.as_str()),
                        sea_query::DynIden::from(col2.as_str())
                    )
                }
            })
        }
        3 => {
            let col1_variant = &column_variants[0];
            let col2_variant = &column_variants[1];
            let col3_variant = &column_variants[2];
            // At this point, we've validated that all variants are valid identifiers
            let col1_ident = syn::parse_str::<syn::Ident>(col1_variant)
                .expect("Column variant should be valid identifier after validation");
            let col2_ident = syn::parse_str::<syn::Ident>(col2_variant)
                .expect("Column variant should be valid identifier after validation");
            let col3_ident = syn::parse_str::<syn::Ident>(col3_variant)
                .expect("Column variant should be valid identifier after validation");
            Ok(quote! {
                {
                    use lifeguard::LifeModelTrait;
                    use sea_query::IdenStatic;
                    let col1 = #column_path_expr::#col1_ident;
                    let col2 = #column_path_expr::#col2_ident;
                    let col3 = #column_path_expr::#col3_ident;
                    lifeguard::Identity::Ternary(
                        sea_query::DynIden::from(col1.as_str()),
                        sea_query::DynIden::from(col2.as_str()),
                        sea_query::DynIden::from(col3.as_str())
                    )
                }
            })
        }
        _n => {
            // 4 or more columns - use Many variant
            // At this point, we've validated that all variants are valid identifiers
            let cols: Vec<_> = column_variants.iter().map(|col_variant| {
                let col_ident = syn::parse_str::<syn::Ident>(col_variant)
                    .expect("Column variant should be valid identifier after validation");
                quote! {
                    {
                        let col = #column_path_expr::#col_ident;
                        sea_query::DynIden::from(col.as_str())
                    }
                }
            }).collect();
            Ok(quote! {
                {
                    use lifeguard::LifeModelTrait;
                    use sea_query::IdenStatic;
                    lifeguard::Identity::Many(vec![#(#cols),*])
                }
            })
        }
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

/// Extract entity name from entity path and infer foreign key column name
/// 
/// Examples:
/// - "super::users::Entity" -> "users" -> "user_id"
/// - "CommentEntity" -> "Comment" -> "comment_id"
/// - "UserEntity" -> "User" -> "user_id"
fn infer_foreign_key_column_name(entity_path: &str) -> String {
    // Extract the entity name from the path
    // Path format: "super::users::Entity" or "CommentEntity" or "UserEntity"
    let segments: Vec<&str> = entity_path.split("::").collect();
    let entity_name = if let Some(&last_segment) = segments.last() {
        // Special case: if the last segment is exactly "Entity" and there are multiple segments,
        // use the second-to-last segment (e.g., "users" from "super::users::Entity")
        if last_segment == "Entity" && segments.len() > 1 {
            segments[segments.len() - 2]
        } else if last_segment.ends_with("Entity") && last_segment != "Entity" {
            // Remove "Entity" suffix if present (e.g., "CommentEntity" -> "Comment")
            &last_segment[..last_segment.len() - 6]
        } else {
            last_segment
        }
    } else {
        entity_path
    };
    
    // Convert to snake_case and handle plural to singular
    // If the entity_name is already in snake_case (e.g., "users"), convert plural to singular
    let snake_case = if entity_name.contains('_') || entity_name.chars().all(|c| c.is_lowercase()) {
        // Already snake_case - handle plural to singular conversion
        // Simple heuristic: remove trailing "s" if present (e.g., "users" -> "user")
        if entity_name.ends_with('s') && entity_name.len() > 1 {
            entity_name[..entity_name.len() - 1].to_string()
        } else {
            entity_name.to_string()
        }
    } else {
        // PascalCase - convert to snake_case (returns String)
        convert_pascal_to_snake_case(entity_name)
    };
    format!("{}_id", snake_case)
}

/// Derive macro for `DeriveRelation` - generates Relation enum and Related implementations
///
/// This macro generates:
/// - Relation enum (already defined by user, we just process it)
/// - Related trait implementations for each relationship variant
///
/// # Example
///
/// ```ignore
/// use lifeguard_derive::DeriveRelation;
///
/// // In your entity module, define the Relation enum:
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
/// 
/// // The macro generates:
/// // - Related trait implementations returning RelationDef
/// // - RelationMetadata implementations when from/to are specified
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
    let mut related_entity_variants = Vec::new();
    let mut related_entity_impls = Vec::new();
    // Track which target entity paths we've already generated From impls for
    // This prevents duplicate From impls when multiple relations target the same entity
    // (e.g., CreatedPosts and EditedPosts both pointing to PostEntity)
    // We use a helper function to create a unique key from the path
    let mut seen_target_entity_paths: std::collections::HashSet<String> = std::collections::HashSet::new();
    // Track which target entity paths we've already generated Related impls for
    // This prevents duplicate Related impls when multiple relations target the same entity
    // (e.g., CreatedPosts and EditedPosts both pointing to PostEntity)
    // Key: target entity path, Value: (from_col, to_col, variant_name) for error reporting
    let mut seen_related_impls: std::collections::HashMap<String, (Option<String>, Option<String>, syn::Ident)> = std::collections::HashMap::new();
    
    // Helper function to create a unique string key from a syn::Path
    // This allows us to compare paths for equality
    let path_to_key = |path: &syn::Path| -> String {
        let mut key = String::new();
        if path.leading_colon.is_some() {
            key.push_str("::");
        }
        for (i, segment) in path.segments.iter().enumerate() {
            if i > 0 {
                key.push_str("::");
            }
            key.push_str(&segment.ident.to_string());
        }
        key
    };
    
    for variant in variants {
        if let Some((related_impl, target_entity_path, variant_name, from_col, to_col)) = process_relation_variant(variant, enum_name) {
            // Check if this is a dummy path (used for error cases)
            // Error cases are identified by:
            // 1. Path is just "Entity" without any module prefix
            // 2. Both from_col and to_col are None (error cases always return None for both)
            // This distinguishes error cases from valid self-referential relationships,
            // which would have at least one column specified (or use defaults, but the
            // tuple values would still be None if not user-specified, so we need to be careful)
            // However, if the path is "Entity" AND both are None, it's almost certainly an error case
            // because valid self-referential relationships would typically specify columns explicitly
            let is_dummy_path = target_entity_path.segments.len() == 1 
                && target_entity_path.segments[0].ident == "Entity"
                && from_col.is_none()
                && to_col.is_none();
            
            // Only deduplicate Related impls if this is not a dummy path (error case)
            // Error cases should always be emitted to show the compile error
            if !is_dummy_path {
                // Only generate one Related impl per unique target entity path to avoid conflicts
                // when multiple relations target the same entity (e.g., CreatedPosts and EditedPosts both pointing to PostEntity)
                let target_path_key = path_to_key(&target_entity_path);
                
                // Check if we've already seen this target entity path
                if let Some((existing_from, existing_to, existing_variant)) = seen_related_impls.get(&target_path_key) {
                    // Check if the column configuration is different
                    let from_col_str = from_col.as_ref().map(|s| s.as_str()).unwrap_or("default");
                    let to_col_str = to_col.as_ref().map(|s| s.as_str()).unwrap_or("default");
                    let existing_from_str = existing_from.as_ref().map(|s| s.as_str()).unwrap_or("default");
                    let existing_to_str = existing_to.as_ref().map(|s| s.as_str()).unwrap_or("default");
                    
                    if from_col_str != existing_from_str || to_col_str != existing_to_str {
                        // Different column configuration - emit compile error
                        let error_msg = format!(
                            "Multiple relations target the same entity `{}` with different column configurations.\n\
                            \n\
                            First relation `{}` uses:\n\
                            - from: {}\n\
                            - to: {}\n\
                            \n\
                            Second relation `{}` uses:\n\
                            - from: {}\n\
                            - to: {}\n\
                            \n\
                            Rust doesn't allow multiple `impl Related<{}> for Entity` implementations.\n\
                            The macro would silently discard the second configuration, leading to incorrect queries.\n\
                            \n\
                            Solution: Use different target entities, or ensure all relations to the same entity use identical column configurations.",
                            target_path_key,
                            existing_variant,
                            existing_from_str,
                            existing_to_str,
                            variant_name,
                            from_col_str,
                            to_col_str,
                            target_path_key
                        );
                        let error = syn::Error::new_spanned(
                            &variant.ident,
                            error_msg,
                        );
                        related_impls.push(error.to_compile_error());
                        continue;
                    }
                    // Same column configuration - skip this Related impl (already generated)
                } else {
                    // First time seeing this target entity path - record it and add the impl
                    seen_related_impls.insert(target_path_key.clone(), (from_col.clone(), to_col.clone(), variant_name.clone()));
                    related_impls.push(related_impl);
                }
            } else {
                // Always emit error cases (dummy paths)
                related_impls.push(related_impl);
            }
            
            // Only generate RelatedEntity if this is not a dummy path (error case)
            if !is_dummy_path {
                // Collect information for RelatedEntity enum generation
                // RelatedEntity contains Model types, not Entity types
                related_entity_variants.push(quote! {
                    #variant_name(<#target_entity_path as lifeguard::LifeModelTrait>::Model),
                });
                
                // Generate From implementation for RelatedEntity variant
                // Only generate one From impl per unique target entity path to avoid conflicts
                // when multiple relations target the same entity (e.g., CreatedPosts and EditedPosts both pointing to PostEntity)
                let target_path_key = path_to_key(&target_entity_path);
                if !seen_target_entity_paths.contains(&target_path_key) {
                    seen_target_entity_paths.insert(target_path_key);
                    related_entity_impls.push(quote! {
                        impl From<<#target_entity_path as lifeguard::LifeModelTrait>::Model> for RelatedEntity {
                            fn from(model: <#target_entity_path as lifeguard::LifeModelTrait>::Model) -> Self {
                                RelatedEntity::#variant_name(model)
                            }
                        }
                    });
                }
            }
        } else {
            // If process_relation_variant returns None, it means there was no relationship info
            // This is not an error case, just a variant without relationship attributes
        }
    }
    
    // Generate RelatedEntity enum if we have variants
    // Note: If there are parse errors, they will be in related_impls and will stop compilation
    // We don't need to check for errors here since the error token streams will be emitted
    let related_entity_enum = if !related_entity_variants.is_empty() {
        quote! {
            /// RelatedEntity enum for type-safe relationship access
            ///
            /// This enum contains variants for each related entity type,
            /// allowing type-safe access to related entity models.
            ///
            /// # Example
            ///
            /// ```no_run
            /// use lifeguard::RelatedEntity;
            ///
            /// // Match on related entity type
            /// match related_entity {
            ///     RelatedEntity::Posts(post_model) => { /* handle post model */ }
            ///     RelatedEntity::User(user_model) => { /* handle user model */ }
            /// }
            /// ```
            #[derive(Debug, Clone)]
            pub enum RelatedEntity {
                #(#related_entity_variants)*
            }
            
            #(#related_entity_impls)*
        }
    } else {
        quote! {}
    };
    
    let expanded: TokenStream2 = quote! {
        #(#related_impls)*
        #related_entity_enum
    };
    
    TokenStream::from(expanded)
}

/// Process a relation variant and generate Related trait implementation
///
/// Returns a tuple of:
/// - Related trait implementation TokenStream
/// - Target entity path for RelatedEntity enum generation
/// - Variant name for RelatedEntity enum generation
/// - From column configuration (for conflict detection)
/// - To column configuration (for conflict detection)
fn process_relation_variant(
    variant: &Variant,
    _enum_name: &syn::Ident,
) -> Option<(TokenStream2, syn::Path, syn::Ident, Option<String>, Option<String>)> {
    let _variant_name = &variant.ident;
    
    // Parse attributes to find relationship type and target entity
    let mut relationship_type: Option<String> = None;
    let mut target_entity: Option<String> = None;
    let mut through_entity: Option<String> = None;
    let mut from_column: Option<String> = None;
    let mut to_column: Option<String> = None;
    
    for attr in &variant.attrs {
        if attr.path().is_ident("lifeguard") {
            // Parse nested attributes like #[lifeguard(has_many = "...")]
            // Use parse_nested_meta for syn 2.0
            // Check result and propagate errors instead of silently ignoring them
            if let Err(err) = attr.parse_nested_meta(|meta| {
                // Check for key-value pairs like has_many = "..."
                if meta.path.is_ident("has_many") || meta.path.is_ident("has_one") || meta.path.is_ident("belongs_to") || meta.path.is_ident("has_many_through") {
                    let key = meta.path.get_ident().map(|i| i.to_string()).unwrap_or_default();
                    let value: syn::LitStr = meta.value()?.parse()?;
                    relationship_type = Some(key);
                    target_entity = Some(value.value());
                    Ok(())
                } else if meta.path.is_ident("through") {
                    // Parse through attribute for has_many_through relationships
                    let value: syn::LitStr = meta.value()?.parse()?;
                    through_entity = Some(value.value());
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
            }) {
                // Return compile error if parsing fails
                return Some((
                    err.to_compile_error(),
                    syn::parse_str("Entity").unwrap(), // Dummy path - won't be used for RelatedEntity
                    variant.ident.clone(),
                    None, // from_col
                    None, // to_col
                ));
            }
        }
    }
    
    // Generate Related trait implementation if we have the required information
    if let (Some(rel_type_str), Some(target)) = (relationship_type.as_ref(), target_entity.as_ref()) {
        // Validate has_many_through requires through attribute
        if rel_type_str == "has_many_through" && through_entity.is_none() {
            return Some((
                syn::Error::new_spanned(
                    &variant.ident,
                    "has_many_through relationship requires a 'through' attribute specifying the join table entity. Use #[lifeguard(has_many_through = \"target::Entity\", through = \"join_table::Entity\")]",
                )
                .to_compile_error(),
                syn::parse_str("Entity").unwrap(), // Dummy path for error case
                variant.ident.clone(),
                None, // from_col
                None, // to_col
            ));
        }
        
        // Capture relationship type before move
        let rel_type = match rel_type_str.as_str() {
            "has_many" => quote! { lifeguard::RelationType::HasMany },
            "has_one" => quote! { lifeguard::RelationType::HasOne },
            "belongs_to" => quote! { lifeguard::RelationType::BelongsTo },
            "has_many_through" => quote! { lifeguard::RelationType::HasManyThrough },
            _ => quote! { lifeguard::RelationType::HasMany }, // Default
        };
        
        // Helper function to parse and validate entity path
        let parse_entity_path = |entity_str: &str, error_context: &str| -> Result<syn::Path, TokenStream2> {
            match syn::parse_str(entity_str) {
                Ok(path) => Ok(path),
                Err(_) => {
                    // If parsing fails, try to construct a path manually
                    let segments: Vec<&str> = entity_str.split("::").collect();
                    
                    // Validate segments before creating identifiers
                    for (idx, segment) in segments.iter().enumerate() {
                        if segment.is_empty() {
                            let error_msg = if segments.len() == 1 {
                                format!("Entity path cannot be empty. Found empty string in {} \"{}\".", error_context, entity_str)
                            } else if idx == segments.len() - 1 {
                                format!("Entity path has trailing colons. Found empty segment at end in {} \"{}\". Use a valid path like \"super::users::Entity\".", error_context, entity_str)
                            } else {
                                format!("Entity path has consecutive colons. Found empty segment at position {} in {} \"{}\". Use a valid path like \"super::users::Entity\".", idx + 1, error_context, entity_str)
                            };
                            return Err(syn::Error::new(
                                variant.ident.span(),
                                error_msg,
                            )
                            .to_compile_error());
                        }
                        
                        // Validate that the segment is a valid Rust identifier
                        if syn::parse_str::<syn::Ident>(segment).is_err() {
                            return Err(syn::Error::new(
                                variant.ident.span(),
                                format!("Entity path contains invalid identifier \"{}\" at position {} in {} \"{}\". Identifiers must be valid Rust identifiers (e.g., start with a letter or underscore, contain only alphanumeric characters and underscores).", segment, idx + 1, error_context, entity_str),
                            )
                            .to_compile_error());
                        }
                    }
                    
                    // Build the path after validation
                    let mut path = syn::Path {
                        leading_colon: None,
                        segments: syn::punctuated::Punctuated::new(),
                    };
                    for segment in segments {
                        // At this point, we've validated that segment is not empty and is a valid identifier
                        let ident = syn::parse_str::<syn::Ident>(segment)
                            .expect("Segment should be valid identifier after validation");
                        path.segments.push(syn::PathSegment {
                            ident,
                            arguments: syn::PathArguments::None,
                        });
                    }
                    Ok(path)
                }
            }
        };
        
        // Parse target entity path (e.g., "super::posts::Entity")
        let target_entity_path: syn::Path = match parse_entity_path(target, "entity path") {
            Ok(path) => path,
            Err(err) => return Some((err, syn::parse_str("Entity").unwrap(), variant.ident.clone(), None, None)),
        };
        
        // Parse through entity path for has_many_through relationships
        let (through_entity_path, through_table_name) = if let Some(through) = through_entity.as_ref() {
            let through_path: syn::Path = match parse_entity_path(through, "through entity path") {
                Ok(path) => path,
                Err(err) => return Some((err, syn::parse_str("Entity").unwrap(), variant.ident.clone(), None, None)),
            };
            let through_table = quote! {
                {
                    use lifeguard::LifeEntityName;
                    let entity = #through_path::default();
                    entity.table_name()
                }
            };
            (Some(through_path), Some(through_table))
        } else {
            (None, None)
        };
        
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
                    impl lifeguard::relation::RelationMetadata<Entity> for #target_entity_path {
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
        
        // Generate Related trait implementation with RelationDef
        // Note: Entity is assumed to be in the same module as the Relation enum
        // This is a simplified implementation - full Phase 6 will add proper Identity handling
        
        // Get table names using entity's table_name() method
        // Note: Entity is assumed to be in the same module as the Relation enum (documented assumption)
        let from_table_name = quote! {
            {
                use lifeguard::LifeEntityName;
                let entity = Entity::default();
                entity.table_name()
            }
        };
        let to_table_name = quote! {
            {
                use lifeguard::LifeEntityName;
                let entity = #target_entity_path::default();
                entity.table_name()
            }
        };
        
        // Build Identity for from_col and to_col
        // Phase 6: Enhanced to support composite keys and proper column references
        let from_col_identity = if let Some(from_col) = from_column.as_ref() {
            // Parse the column reference and build Identity
            match build_identity_from_column_ref(from_col, variant.ident.span()) {
                Ok(identity) => identity,
                Err(err) => return Some((err, target_entity_path.clone(), variant.ident.clone(), from_column.clone(), to_column.clone())),
            }
        } else {
            // Default: infer from relationship type
            match rel_type_str.as_str() {
                "belongs_to" => {
                    // For belongs_to: from_col is the foreign key in the current table
                    // Infer FK column name from target entity: <target_entity_name>_id
                    let fk_col_name = infer_foreign_key_column_name(target);
                    let fk_col_name_lit = syn::LitStr::new(&fk_col_name, proc_macro2::Span::call_site());
                    quote! {
                        {
                            use sea_query::IdenStatic;
                            // Foreign key column in current table (e.g., "user_id" for Post belongs_to User)
                            lifeguard::Identity::Unary(sea_query::DynIden::from(#fk_col_name_lit))
                        }
                    }
                }
                "has_many" | "has_one" | "has_many_through" => {
                    // For has_many/has_one/has_many_through: from_col is the primary key in the current table
                    quote! {
                        {
                            use lifeguard::LifeModelTrait;
                            use sea_query::IdenStatic;
                            // Primary key column in current table
                            let col = <Entity as lifeguard::LifeModelTrait>::Column::Id;
                            lifeguard::Identity::Unary(sea_query::DynIden::from(col.as_str()))
                        }
                    }
                }
                _ => {
                    // Fallback to primary key
                    quote! {
                        {
                            use lifeguard::LifeModelTrait;
                            use sea_query::IdenStatic;
                            let col = <Entity as lifeguard::LifeModelTrait>::Column::Id;
                            lifeguard::Identity::Unary(sea_query::DynIden::from(col.as_str()))
                        }
                    }
                }
            }
        };
        
        let to_col_identity = if let Some(to_col) = to_column.as_ref() {
            // Parse the column reference and build Identity
            // The "to" column might be in a different module (e.g., "super::users::Column::Id")
            match build_identity_from_column_ref(to_col, variant.ident.span()) {
                Ok(identity) => identity,
                Err(err) => return Some((err, target_entity_path.clone(), variant.ident.clone(), from_column.clone(), to_column.clone())),
            }
        } else {
            // Default: infer from relationship type
            match rel_type_str.as_str() {
                "belongs_to" => {
                    // For belongs_to: to_col is the primary key in the target table
                    quote! {
                        {
                            use lifeguard::LifeModelTrait;
                            use sea_query::IdenStatic;
                            // Primary key column in target table
                            let col = <#target_entity_path as lifeguard::LifeModelTrait>::Column::Id;
                            lifeguard::Identity::Unary(sea_query::DynIden::from(col.as_str()))
                        }
                    }
                }
                "has_many" | "has_one" => {
                    // For has_many/has_one: to_col is the foreign key in the target table
                    // Infer FK column name from current entity: <current_entity_name>_id
                    // We need to get the current entity name - assume it's "Entity" in the same module
                    // For now, we'll use a runtime approach to get the table name and infer the FK
                    // Since we can't easily get the entity name at compile time, we'll use the table name
                    // The table name is available at runtime via LifeEntityName::table_name()
                    // We'll construct the FK name from the table name: <table_name>_id
                    // But we need to convert plural to singular (e.g., "posts" -> "post_id")
                    // For simplicity, we'll use a helper that gets the table name and constructs the FK
                    quote! {
                        {
                            use lifeguard::LifeEntityName;
                            // Get current entity's table name and infer foreign key column
                            // Foreign key in target table (e.g., "post_id" in comments for Post has_many Comments)
                            let from_table = Entity::default().table_name();
                            // Convert table name to singular and append "_id"
                            // Simple heuristic: remove trailing 's' if present, then append "_id"
                            let fk_name = if from_table.ends_with('s') && from_table.len() > 1 {
                                format!("{}_id", &from_table[..from_table.len() - 1])
                            } else {
                                format!("{}_id", from_table)
                            };
                            // Use String directly - DynIden::from() accepts String
                            lifeguard::Identity::Unary(sea_query::DynIden::from(fk_name))
                        }
                    }
                }
                "has_many_through" => {
                    // For has_many_through: to_col is the primary key in the target table
                    quote! {
                        {
                            use lifeguard::LifeModelTrait;
                            use sea_query::IdenStatic;
                            // Primary key column in target table
                            let col = <#target_entity_path as lifeguard::LifeModelTrait>::Column::Id;
                            lifeguard::Identity::Unary(sea_query::DynIden::from(col.as_str()))
                        }
                    }
                }
                _ => {
                    // Fallback to primary key
                    quote! {
                        {
                            use sea_query::IdenStatic;
                            lifeguard::Identity::Unary(sea_query::DynIden::from("id"))
                        }
                    }
                }
            }
        };
        
        // Generate through_tbl field for has_many_through relationships
        let through_tbl_expr = if let (Some(_through_path), Some(through_table)) = (through_entity_path.as_ref(), through_table_name.as_ref()) {
            // through_path is used inside through_table quote! macro above
            quote! {
                Some(TableRef::Table(TableName(None, #through_table.into_iden()), None))
            }
        } else {
            quote! { None }
        };
        
        // Generate through_from_col and through_to_col for has_many_through relationships
        // These are the foreign key columns in the join table pointing to source and target entities
        let (through_from_col_expr, through_to_col_expr) = if rel_type_str == "has_many_through" {
            // For has_many_through: infer FK column names from source and target entity names
            // through_from_col: FK in join table pointing to source (e.g., "post_id" in PostTags for Post -> PostTags -> Tags)
            // through_to_col: FK in join table pointing to target (e.g., "tag_id" in PostTags for Post -> PostTags -> Tags)
            
            // Infer FK column name from source entity (Entity)
            // We need to get the entity name - assume it's "Entity" in the same module
            // Use the table name to infer the FK column name
            let source_fk_col = quote! {
                {
                    use lifeguard::LifeEntityName;
                    // Get source entity's table name and infer foreign key column
                    // Foreign key in join table pointing to source (e.g., "post_id" in post_tags for Post -> PostTags -> Tags)
                    let from_table = Entity::default().table_name();
                    // Convert table name to singular and append "_id"
                    // Simple heuristic: remove trailing 's' if present, then append "_id"
                    let fk_name = if from_table.ends_with('s') && from_table.len() > 1 {
                        format!("{}_id", &from_table[..from_table.len() - 1])
                    } else {
                        format!("{}_id", from_table)
                    };
                    // Use String directly - DynIden::from() accepts String
                    Some(lifeguard::Identity::Unary(sea_query::DynIden::from(fk_name)))
                }
            };
            
            // Infer FK column name from target entity
            let target_fk_col_name = infer_foreign_key_column_name(target);
            let target_fk_col_name_lit = syn::LitStr::new(&target_fk_col_name, proc_macro2::Span::call_site());
            let target_fk_col = quote! {
                Some(lifeguard::Identity::Unary(sea_query::DynIden::from(#target_fk_col_name_lit)))
            };
            
            (source_fk_col, target_fk_col)
        } else {
            // For non-has_many_through relationships, these fields are None
            (quote! { None }, quote! { None })
        };
        
        let variant_name = variant.ident.clone();
        let related_impl = quote! {
            impl lifeguard::Related<#target_entity_path> for Entity {
                fn to() -> lifeguard::RelationDef {
                    use sea_query::{TableRef, TableName, ConditionType, IntoIden};
                    lifeguard::RelationDef {
                        rel_type: #rel_type,
                        from_tbl: TableRef::Table(TableName(None, #from_table_name.into_iden()), None),
                        to_tbl: TableRef::Table(TableName(None, #to_table_name.into_iden()), None),
                        from_col: #from_col_identity,
                        to_col: #to_col_identity,
                        through_tbl: #through_tbl_expr,
                        through_from_col: #through_from_col_expr,
                        through_to_col: #through_to_col_expr,
                        is_owner: true,
                        skip_fk: false,
                        on_condition: None,
                        condition_type: ConditionType::All,
                    }
                }
            }
            #fk_col_impl
        };
        
        Some((related_impl, target_entity_path.clone(), variant_name, from_column.clone(), to_column.clone()))
    } else {
        None
    }
}
