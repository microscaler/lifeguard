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
fn build_identity_from_column_ref(column_ref: &str) -> TokenStream2 {
    let (path_prefix, column_variants) = parse_column_reference(column_ref);
    
    // Build the column path (e.g., "Column" or "super::users::Column")
    // If no prefix, assume it's the current entity's Column type
    
    let column_path_expr = if let Some(prefix) = path_prefix {
        // Full path like "super::users::Column"
        let prefix_segments: Vec<&str> = prefix.split("::").collect();
        let mut path_tokens = quote! {};
        for segment in prefix_segments {
            let seg_ident = syn::Ident::new(segment, proc_macro2::Span::call_site());
            path_tokens = quote! { #path_tokens::#seg_ident };
        }
        quote! { #path_tokens::Column }
    } else {
        // No prefix - use Entity's Column type
        quote! { <Entity as lifeguard::LifeModelTrait>::Column }
    };
    
    match column_variants.len() {
        1 => {
            let col_variant = &column_variants[0];
            let col_ident = syn::Ident::new(col_variant, proc_macro2::Span::call_site());
            quote! {
                {
                    use lifeguard::LifeModelTrait;
                    use sea_query::IdenStatic;
                    let col = #column_path_expr::#col_ident;
                    lifeguard::Identity::Unary(sea_query::DynIden::from(col.as_str()))
                }
            }
        }
        2 => {
            let col1_variant = &column_variants[0];
            let col2_variant = &column_variants[1];
            let col1_ident = syn::Ident::new(col1_variant, proc_macro2::Span::call_site());
            let col2_ident = syn::Ident::new(col2_variant, proc_macro2::Span::call_site());
            quote! {
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
            }
        }
        3 => {
            let col1_variant = &column_variants[0];
            let col2_variant = &column_variants[1];
            let col3_variant = &column_variants[2];
            let col1_ident = syn::Ident::new(col1_variant, proc_macro2::Span::call_site());
            let col2_ident = syn::Ident::new(col2_variant, proc_macro2::Span::call_site());
            let col3_ident = syn::Ident::new(col3_variant, proc_macro2::Span::call_site());
            quote! {
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
            }
        }
        _n => {
            // 4 or more columns - use Many variant
            let cols: Vec<_> = column_variants.iter().map(|col_variant| {
                let col_ident = syn::Ident::new(col_variant, proc_macro2::Span::call_site());
                quote! {
                    {
                        let col = #column_path_expr::#col_ident;
                        sea_query::DynIden::from(col.as_str())
                    }
                }
            }).collect();
            quote! {
                {
                    use lifeguard::LifeModelTrait;
                    use sea_query::IdenStatic;
                    lifeguard::Identity::Many(vec![#(#cols),*])
                }
            }
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
    if let (Some(rel_type_str), Some(target)) = (relationship_type.as_ref(), target_entity.as_ref()) {
        // Capture relationship type before move
        let rel_type = match rel_type_str.as_str() {
            "has_many" => quote! { lifeguard::RelationType::HasMany },
            "has_one" => quote! { lifeguard::RelationType::HasOne },
            "belongs_to" => quote! { lifeguard::RelationType::BelongsTo },
            _ => quote! { lifeguard::RelationType::HasMany }, // Default
        };
        
        // Parse target entity path (e.g., "super::posts::Entity")
        let target_entity_path: syn::Path = syn::parse_str(target)
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
            build_identity_from_column_ref(from_col)
        } else {
            // Default: infer from relationship type
            // For has_many/has_one: foreign key is in the target table (to_col)
            // For belongs_to: foreign key is in the current table (from_col)
            // Default to "id" for now - could be enhanced to infer from entity name
            // Use Entity::Column to get the Column type from the entity
            quote! {
                {
                    use lifeguard::LifeModelTrait;
                    use sea_query::IdenStatic;
                    // Entity is assumed to be in the same module as the Relation enum
                    // Use Entity::Column to access the Column enum
                    let col = <Entity as lifeguard::LifeModelTrait>::Column::Id;
                    lifeguard::Identity::Unary(sea_query::DynIden::from(col.as_str()))
                }
            }
        };
        
        let to_col_identity = if let Some(to_col) = to_column.as_ref() {
            // Parse the column reference and build Identity
            // The "to" column might be in a different module (e.g., "super::users::Column::Id")
            build_identity_from_column_ref(to_col)
        } else {
            // Default: infer from target entity's primary key
            // For now, default to "id" - could be enhanced to query the target entity's primary key
            // This would require access to the target entity's metadata at compile time
            quote! {
                {
                    use sea_query::IdenStatic;
                    // Try to use the target entity's Column enum
                    // If it's in a different module, we'll need the full path
                    // For now, use a string-based approach that will work at runtime
                    lifeguard::Identity::Unary(sea_query::DynIden::from("id"))
                }
            }
        };
        
        Some(quote! {
            impl lifeguard::Related<#target_entity_path> for Entity {
                fn to() -> lifeguard::RelationDef {
                    use sea_query::{TableRef, TableName, ConditionType, IntoIden};
                    lifeguard::RelationDef {
                        rel_type: #rel_type,
                        from_tbl: TableRef::Table(TableName(None, #from_table_name.into_iden()), None),
                        to_tbl: TableRef::Table(TableName(None, #to_table_name.into_iden()), None),
                        from_col: #from_col_identity,
                        to_col: #to_col_identity,
                        is_owner: true,
                        skip_fk: false,
                        on_condition: None,
                        condition_type: ConditionType::All,
                    }
                }
            }
            #fk_col_impl
        })
    } else {
        None
    }
}
