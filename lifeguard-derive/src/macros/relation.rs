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
            // Check result and propagate errors instead of silently ignoring them
            if let Err(err) = attr.parse_nested_meta(|meta| {
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
            }) {
                // Return compile error if parsing fails
                return Some(err.to_compile_error());
            }
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
        let target_entity_path: syn::Path = match syn::parse_str(target) {
            Ok(path) => path,
            Err(_) => {
                // If parsing fails, try to construct a path manually
                let segments: Vec<&str> = target.split("::").collect();
                
                // Validate segments before creating identifiers
                for (idx, segment) in segments.iter().enumerate() {
                    if segment.is_empty() {
                        let error_msg = if segments.len() == 1 {
                            format!("Entity path cannot be empty. Found empty string in entity path \"{}\".", target)
                        } else if idx == segments.len() - 1 {
                            format!("Entity path has trailing colons. Found empty segment at end in \"{}\". Use a valid path like \"super::users::Entity\".", target)
                        } else {
                            format!("Entity path has consecutive colons. Found empty segment at position {} in \"{}\". Use a valid path like \"super::users::Entity\".", idx + 1, target)
                        };
                        return Some(syn::Error::new(
                            variant.ident.span(),
                            error_msg,
                        )
                        .to_compile_error());
                    }
                    
                    // Validate that the segment is a valid Rust identifier
                    if syn::parse_str::<syn::Ident>(segment).is_err() {
                        return Some(syn::Error::new(
                            variant.ident.span(),
                            format!("Entity path contains invalid identifier \"{}\" at position {} in \"{}\". Identifiers must be valid Rust identifiers (e.g., start with a letter or underscore, contain only alphanumeric characters and underscores).", segment, idx + 1, target),
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
                path
            }
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
                Err(err) => return Some(err),
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
                "has_many" | "has_one" => {
                    // For has_many/has_one: from_col is the primary key in the current table
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
                Err(err) => return Some(err),
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
