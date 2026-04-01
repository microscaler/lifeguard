//! `LifeModel` derive macro implementation
//!
//! Based on `SeaORM`'s `expand_derive_entity_model` pattern (v2.0.0-rc.28)
//! Generates Entity, Column, `PrimaryKey`, Model, `FromRow`, and `LifeModelTrait`
#![allow(clippy::map_unwrap_or, clippy::explicit_iter_loop)] // Allow in macro-generated code

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, Data, DataStruct, DeriveInput, Fields, GenericArgument, Ident,
    PathArguments, Type,
};

use crate::attributes;
use crate::type_conversion;
use crate::utils;

/// Extract the inner type from Option<T>
/// Returns None if the type is not Option<T> or if extraction fails
fn extract_option_inner_type(ty: &Type) -> Option<&Type> {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Option" {
                // Extract inner type from generic arguments
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(inner_type)) = args.args.first() {
                        return Some(inner_type);
                    }
                }
            }
        }
    }
    None
}

/// Infer SQL type from Rust type for automatic column type mapping
fn infer_sql_type_from_rust_type(ty: &Type) -> Option<String> {
    // Extract inner type if it's Option<T>
    let inner_type = extract_option_inner_type(ty).unwrap_or(ty);

    // Check if this is a path type (e.g., uuid::Uuid, chrono::NaiveDateTime)
    if let Type::Path(syn::TypePath {
        path: syn::Path { segments, .. },
        ..
    }) = inner_type
    {
        // Get the last segment identifier (most specific type name)
        if let Some(last_seg) = segments.last() {
            let last_ident = last_seg.ident.to_string();

            // Check for UUID - last segment is "Uuid"
            if last_ident == "Uuid" {
                return Some("UUID".to_string());
            }

            // Check for NaiveDateTime - last segment is "NaiveDateTime"
            if last_ident == "NaiveDateTime" {
                return Some("TIMESTAMP".to_string());
            }

            // Check for NaiveDate - last segment is "NaiveDate"
            if last_ident == "NaiveDate" {
                return Some("DATE".to_string());
            }

            // Check for String - last segment is "String"
            if last_ident == "String" {
                return Some("TEXT".to_string());
            }

            // Check for Value (serde_json::Value) - last segment is "Value"
            // When imported as `use serde_json::Value;`, the path is just "Value"
            // When fully qualified, it's "serde_json::Value" (2 segments)
            if last_ident == "Value" {
                // Check if this is serde_json::Value by looking at the path
                // If it's 2 segments and first is "serde_json", it's definitely JSONB
                // If it's 1 segment (just "Value"), it might be imported, so we'll assume JSONB
                // (entities should use #[column_type = "JSONB"] if this is wrong)
                if segments.len() == 2 {
                    if let Some(first_seg) = segments.first() {
                        if first_seg.ident == "serde_json" {
                            return Some("JSONB".to_string());
                        }
                    }
                } else if segments.len() == 1 {
                    // Single segment "Value" - likely imported serde_json::Value
                    // We'll assume JSONB (can be overridden with #[column_type])
                    return Some("JSONB".to_string());
                }
            }

            // Check for Decimal (rust_decimal::Decimal)
            if last_ident == "Decimal" {
                // Check if this is rust_decimal::Decimal
                if segments.len() == 2 {
                    if let Some(first_seg) = segments.first() {
                        if first_seg.ident == "rust_decimal" {
                            return Some("NUMERIC(19, 4)".to_string());
                        }
                    }
                } else if segments.len() == 1 {
                    // Single segment "Decimal" - likely imported rust_decimal::Decimal
                    return Some("NUMERIC(19, 4)".to_string());
                }
            }

            // Check for Money (rusty_money::Money<Currency>)
            if last_ident == "Money" {
                // Check if this is rusty_money::Money
                if segments.len() == 2 {
                    if let Some(first_seg) = segments.first() {
                        if first_seg.ident == "rusty_money" {
                            return Some("NUMERIC(19, 4)".to_string());
                        }
                    }
                } else if segments.len() == 1 {
                    // Single segment "Money" - likely imported rusty_money::Money
                    // Check if it has generic arguments (Money<Currency>)
                    if let PathArguments::AngleBracketed(_) = &segments[0].arguments {
                        return Some("NUMERIC(19, 4)".to_string());
                    }
                }
            }
        }

        // Check for integer types - primitive types are single-segment
        if segments.len() == 1 {
            if let Some(first_seg) = segments.first() {
                let first_ident = first_seg.ident.to_string();
                match first_ident.as_str() {
                    "i8" | "i16" | "i32" | "u8" | "u16" | "u32" => {
                        return Some("INTEGER".to_string())
                    }
                    "i64" | "u64" => return Some("BIGINT".to_string()),
                    "bool" => return Some("BOOLEAN".to_string()),
                    _ => {}
                }
            }
        }
    }

    None
}

fn relation_attr_on_field<'a>(field: &'a syn::Field, name: &str) -> Option<&'a syn::Attribute> {
    field.attrs.iter().find(|a| a.path().is_ident(name))
}

fn parse_relation_entity_path(
    entity_str: &str,
    relation_attr: Option<&syn::Attribute>,
    fallback_ident: &syn::Ident,
    attr_label: &str,
) -> Result<syn::Path, syn::Error> {
    syn::parse_str::<syn::Path>(entity_str).map_err(|e| {
        let msg = format!("invalid `entity` path in {attr_label}: {e}");
        if let Some(a) = relation_attr {
            syn::Error::new_spanned(a, msg)
        } else {
            syn::Error::new_spanned(fallback_ident, msg)
        }
    })
}

/// Derive macro for `LifeModel` - generates Entity, Model, Column, `PrimaryKey`, and `FromRow`
///
/// This macro follows `SeaORM`'s pattern exactly:
/// 1. Generates Entity struct with #[derive(DeriveEntity)] (triggers nested expansion)
/// 2. Generates Column enum
/// 3. Generates `PrimaryKey` enum  
/// 4. Generates Model struct
/// 5. Generates `FromRow` implementation for Model
/// 6. `DeriveEntity` (nested) generates `LifeModelTrait` for Entity
#[allow(clippy::too_many_lines)]
pub fn derive_life_model(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    // Extract struct name, table name, and schema name
    let struct_name = &input.ident;
    let table_name = attributes::extract_table_name(&input.attrs)
        .unwrap_or_else(|| utils::snake_case(&struct_name.to_string()));
    let table_name_lit = syn::LitStr::new(&table_name, struct_name.span());
    let schema_name = attributes::extract_schema_name(&input.attrs);
    let schema_attr = schema_name.as_ref().map(|s| {
        let schema_lit = syn::LitStr::new(s, struct_name.span());
        quote! { #[schema_name = #schema_lit] }
    });

    // Extract fields first to collect column names for validation
    let fields = match &input.data {
        Data::Struct(DataStruct {
            fields: Fields::Named(fields),
            ..
        }) => &fields.named,
        _ => {
            return syn::Error::new_spanned(
                &input.ident,
                "LifeModel can only be derived for structs with named fields",
            )
            .to_compile_error()
            .into();
        }
    };

    // Collect valid column names from struct fields for index validation
    // Exclude fields marked with #[skip] or #[ignore] since they don't exist in the database
    let mut valid_columns = std::collections::HashSet::new();
    for field in fields {
        if let Some(field_name) = &field.ident {
            // Skip fields marked with #[skip] or #[ignore] - they're not database columns
            if attributes::has_attribute(field, "skip")
                || attributes::has_attribute(field, "ignore")
            {
                continue;
            }
            let column_name = attributes::extract_column_name(field)
                .unwrap_or_else(|| utils::snake_case(&field_name.to_string()));
            valid_columns.insert(column_name);
        }
    }

    // Parse table-level attributes (composite_unique, index, check, table_comment)
    // Pass valid_columns for validation
    let table_attrs = match attributes::parse_table_attributes(&input.attrs, &valid_columns) {
        Ok(attrs) => attrs,
        Err(e) => return e.to_compile_error().into(),
    };
    if let Err(e) = attributes::validate_require_index_coverage(
        fields,
        &valid_columns,
        &table_attrs,
        struct_name.span(),
    ) {
        return e.to_compile_error().into();
    }

    // Generate Model name
    let model_name = Ident::new(&format!("{struct_name}Model"), struct_name.span());
    let model_name_lit = syn::LitStr::new(&model_name.to_string(), model_name.span());

    // Process fields to generate:
    // - Column enum variants
    // - PrimaryKey enum variants
    // - Model struct fields
    // - FromRow field extraction
    // - ModelTrait get() match arms
    // - Primary key field tracking
    let mut column_variants = Vec::new();
    let mut column_variant_idents = Vec::new(); // Store identifiers for all_columns() method
    let mut primary_key_variants = Vec::new();
    let mut primary_key_variant_idents = Vec::new(); // Store (variant identifier, auto_increment) tuples for trait implementations
    let mut primary_key_field_names = Vec::new(); // Store field names for value extraction
    let mut model_fields = Vec::new();
    let mut from_row_fields = Vec::new();
    let mut iden_impls = Vec::new();

    // Generate table definition expression
    let table_comment_expr = table_attrs.table_comment.as_ref().map_or_else(
        || quote! { None },
        |tc| {
            let tc_lit = syn::LitStr::new(tc, struct_name.span());
            quote! { Some(#tc_lit.to_string()) }
        },
    );

    // Generate composite unique constraints
    let composite_unique_expr = if table_attrs.composite_unique.is_empty() {
        quote! { Vec::new() }
    } else {
        let unique_vecs: Vec<_> = table_attrs
            .composite_unique
            .iter()
            .map(|cols| {
                let col_lits: Vec<_> = cols
                    .iter()
                    .map(|c| {
                        let c_lit = syn::LitStr::new(c, struct_name.span());
                        quote! { #c_lit.to_string() }
                    })
                    .collect();
                quote! { vec![#(#col_lits),*] }
            })
            .collect();
        quote! { vec![#(#unique_vecs),*] }
    };

    // Generate index definitions
    let indexes_expr = if table_attrs.indexes.is_empty() {
        quote! { Vec::new() }
    } else {
        let index_defs: Vec<_> = table_attrs
            .indexes
            .iter()
            .map(|idx| {
                let name_lit = syn::LitStr::new(&idx.name, struct_name.span());
                let col_lits: Vec<_> = idx
                    .columns
                    .iter()
                    .map(|c| {
                        let c_lit = syn::LitStr::new(c, struct_name.span());
                        quote! { #c_lit.to_string() }
                    })
                    .collect();
                let inc_lits: Vec<_> = idx
                    .include_columns
                    .iter()
                    .map(|c| {
                        let c_lit = syn::LitStr::new(c, struct_name.span());
                        quote! { #c_lit.to_string() }
                    })
                    .collect();
                let unique_lit = syn::LitBool::new(idx.unique, struct_name.span());
                let where_expr = idx.partial_where.as_ref().map_or_else(
                    || quote! { None },
                    |w| {
                        let w_lit = syn::LitStr::new(w, struct_name.span());
                        quote! { Some(#w_lit.to_string()) }
                    },
                );
                let key_list_expr = idx.key_list_sql.as_ref().map_or_else(
                    || quote! { None },
                    |k| {
                        let k_lit = syn::LitStr::new(k, struct_name.span());
                        quote! { Some(#k_lit.to_string()) }
                    },
                );
                let key_parts_expr = if idx.key_parts.is_empty() {
                    quote! { Vec::new() }
                } else {
                    use crate::attributes::{ParsedBtreeNulls, ParsedBtreeSort, ParsedIndexKeyPart};
                    let parts: Vec<_> = idx.key_parts.iter().map(|p| {
                        match p {
                            ParsedIndexKeyPart::Column {
                                name,
                                opclass,
                                collate,
                                sort,
                                nulls,
                            } => {
                                let nl = syn::LitStr::new(name, struct_name.span());
                                let opc = opclass.as_ref().map_or_else(
                                    || quote! { None },
                                    |o| {
                                        let ol = syn::LitStr::new(o, struct_name.span());
                                        quote! { Some(#ol.to_string()) }
                                    },
                                );
                                let col = collate.as_ref().map_or_else(
                                    || quote! { None },
                                    |c| {
                                        let cl = syn::LitStr::new(c, struct_name.span());
                                        quote! { Some(#cl.to_string()) }
                                    },
                                );
                                let sort_ts = match sort {
                                    None => quote! { None },
                                    Some(ParsedBtreeSort::Asc) => {
                                        quote! { Some(lifeguard::IndexBtreeSort::Asc) }
                                    }
                                    Some(ParsedBtreeSort::Desc) => {
                                        quote! { Some(lifeguard::IndexBtreeSort::Desc) }
                                    }
                                };
                                let nulls_ts = match nulls {
                                    None => quote! { None },
                                    Some(ParsedBtreeNulls::First) => {
                                        quote! { Some(lifeguard::IndexBtreeNulls::First) }
                                    }
                                    Some(ParsedBtreeNulls::Last) => {
                                        quote! { Some(lifeguard::IndexBtreeNulls::Last) }
                                    }
                                };
                                quote! {
                                    lifeguard::IndexKeyPart::Column {
                                        name: #nl.to_string(),
                                        opclass: #opc,
                                        collate: #col,
                                        sort: #sort_ts,
                                        nulls: #nulls_ts,
                                    }
                                }
                            }
                            ParsedIndexKeyPart::Expression {
                                sql,
                                coverage_columns,
                                opclass,
                                collate,
                                sort,
                                nulls,
                            } => {
                                let sl = syn::LitStr::new(sql, struct_name.span());
                                let cov_lits: Vec<_> = coverage_columns
                                    .iter()
                                    .map(|c| {
                                        let cl = syn::LitStr::new(c, struct_name.span());
                                        quote! { #cl.to_string() }
                                    })
                                    .collect();
                                let opc = opclass.as_ref().map_or_else(
                                    || quote! { None },
                                    |o| {
                                        let ol = syn::LitStr::new(o, struct_name.span());
                                        quote! { Some(#ol.to_string()) }
                                    },
                                );
                                let col = collate.as_ref().map_or_else(
                                    || quote! { None },
                                    |c| {
                                        let cl = syn::LitStr::new(c, struct_name.span());
                                        quote! { Some(#cl.to_string()) }
                                    },
                                );
                                let sort_ts = match sort {
                                    None => quote! { None },
                                    Some(ParsedBtreeSort::Asc) => {
                                        quote! { Some(lifeguard::IndexBtreeSort::Asc) }
                                    }
                                    Some(ParsedBtreeSort::Desc) => {
                                        quote! { Some(lifeguard::IndexBtreeSort::Desc) }
                                    }
                                };
                                let nulls_ts = match nulls {
                                    None => quote! { None },
                                    Some(ParsedBtreeNulls::First) => {
                                        quote! { Some(lifeguard::IndexBtreeNulls::First) }
                                    }
                                    Some(ParsedBtreeNulls::Last) => {
                                        quote! { Some(lifeguard::IndexBtreeNulls::Last) }
                                    }
                                };
                                quote! {
                                    lifeguard::IndexKeyPart::Expression {
                                        sql: #sl.to_string(),
                                        coverage_columns: vec![#(#cov_lits),*],
                                        opclass: #opc,
                                        collate: #col,
                                        sort: #sort_ts,
                                        nulls: #nulls_ts,
                                    }
                                }
                            }
                        }
                    }).collect();
                    quote! { vec![#(#parts),*] }
                };
                quote! {
                    lifeguard::IndexDefinition {
                        name: #name_lit.to_string(),
                        columns: vec![#(#col_lits),*],
                        key_list_sql: #key_list_expr,
                        key_parts: #key_parts_expr,
                        include_columns: vec![#(#inc_lits),*],
                        unique: #unique_lit,
                        partial_where: #where_expr,
                    }
                }
            })
            .collect();
        quote! { vec![#(#index_defs),*] }
    };

    // Generate CHECK constraints
    let check_constraints_expr = if table_attrs.check_constraints.is_empty() {
        quote! { Vec::new() }
    } else {
        let check_tuples: Vec<_> = table_attrs
            .check_constraints
            .iter()
            .map(|(name, expr)| {
                let expr_lit = syn::LitStr::new(expr, struct_name.span());
                let name_expr = name.as_ref().map_or_else(
                    || quote! { None },
                    |n| {
                        let n_lit = syn::LitStr::new(n, struct_name.span());
                        quote! { Some(#n_lit.to_string()) }
                    },
                );
                quote! { (#name_expr, #expr_lit.to_string()) }
            })
            .collect();
        quote! { vec![#(#check_tuples),*] }
    };

    let table_definition_expr = quote! {
        lifeguard::TableDefinition {
            table_comment: #table_comment_expr,
            composite_unique: #composite_unique_expr,
            indexes: #indexes_expr,
            check_constraints: #check_constraints_expr,
        }
    };
    let mut model_get_match_arms = Vec::new();
    let mut model_set_match_arms = Vec::new();
    let mut get_by_column_name_match_arms: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut get_value_type_match_arms: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut primary_key_value_expr: Option<proc_macro2::TokenStream> = None;
    // Track primary key metadata for PrimaryKeyTrait
    let mut primary_key_type: Option<&Type> = None; // Keep for backward compatibility (first key only)
    let mut primary_key_types: Vec<&Type> = Vec::new(); // Track all primary key types for tuple ValueType
    let mut _primary_key_auto_increment = false; // Reserved for future PrimaryKeyTrait implementation
    let mut primary_key_to_column_mappings = Vec::new();
    // Track column definitions for ColumnTrait::def() implementations
    let mut column_def_match_arms = Vec::new();
    let mut enum_type_name_match_arms = Vec::new();
    let mut relation_impls = Vec::new();

    for field in fields {
        let field_name = match utils::field_ident(field) {
            Ok(i) => i,
            Err(e) => return e.to_compile_error().into(),
        };
        let field_type = &field.ty;
        let column_name = attributes::extract_column_name(field)
            .unwrap_or_else(|| utils::snake_case(&field_name.to_string()));

        // Extract all column attributes
        let col_attrs = match attributes::parse_column_attributes(field) {
            Ok(attrs) => attrs,
            Err(err) => return err.to_compile_error().into(),
        };
        let is_primary_key = col_attrs.is_primary_key;
        let is_auto_increment = col_attrs.is_auto_increment;
        let is_ignored = col_attrs.is_ignored;

        // Pass-through `#[graphql(...)]` only when built with `graphql` (see `graphql_derive` below).
        // Otherwise attributes would be orphaned without `#[derive(SimpleObject)]`.
        #[cfg(feature = "graphql")]
        let graphql_attrs: Vec<_> = field
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident("graphql"))
            .collect();
        #[cfg(not(feature = "graphql"))]
        let graphql_attrs: Vec<&syn::Attribute> = Vec::new();

        // Validate: primary key fields cannot be skipped/ignored
        if is_primary_key && is_ignored {
            // Find the skip/ignore attribute to use its span for better error location
            if let Some(attr) = field
                .attrs
                .iter()
                .find(|attr| attr.path().is_ident("skip") || attr.path().is_ident("ignore"))
            {
                return syn::Error::new_spanned(
                    attr,
                    "Field cannot have both `#[primary_key]` and `#[skip]` (or `#[ignore]`) attributes. Primary key fields must be included in database operations.",
                )
                .to_compile_error()
                .into();
            }
            // Fallback to field name if attribute not found (shouldn't happen)
            return syn::Error::new_spanned(
                field_name,
                "Field cannot have both `#[primary_key]` and `#[skip]` (or `#[ignore]`) attributes. Primary key fields must be included in database operations.",
            )
            .to_compile_error()
            .into();
        }

        // Skip ignored fields - they're not mapped to database columns
        // But we still need to add them to the Model struct and FromRow
        if is_ignored {
            // Still include in Model struct
            // If it is Option<T>, skip serializing if None
            let serde_skip = if extract_option_inner_type(field_type).is_some() {
                quote! { #[serde(skip_serializing_if = "Option::is_none")] }
            } else {
                quote! {}
            };

            model_fields.push(quote! {
                #(#graphql_attrs)*
                #serde_skip
                pub #field_name: #field_type,
            });
            // Add to FromRow with default value (since they're not in database)
            // Use Default::default() if available, otherwise use a placeholder
            // For Option<T>, use None; for other types, try Default::default()
            let default_expr = if extract_option_inner_type(field_type).is_some() {
                quote! { None }
            } else {
                quote! { <#field_type as Default>::default() }
            };
            from_row_fields.push(quote! {
                #field_name: #default_expr,
            });

            // Build auto-generated relation traits
            if let Some(rel) = &col_attrs.has_many {
                let rel_attr = relation_attr_on_field(field, "has_many");
                let entity_path = match parse_relation_entity_path(
                    &rel.entity,
                    rel_attr,
                    field_name,
                    "#[has_many]",
                ) {
                    Ok(p) => p,
                    Err(e) => return e.to_compile_error().into(),
                };
                let from_col = rel.from.as_deref().unwrap_or("id");
                let Some(to_col) = rel.to.as_deref() else {
                    let e = match rel_attr {
                        Some(a) => syn::Error::new_spanned(
                            a,
                            "#[has_many] requires `to = \"column_name\"` (foreign key column on the related entity)",
                        ),
                        None => syn::Error::new_spanned(
                            field_name,
                            "#[has_many] requires `to = \"column_name\"` (foreign key column on the related entity)",
                        ),
                    };
                    return e.to_compile_error().into();
                };
                relation_impls.push(quote! {
                    impl lifeguard::Related<#entity_path> for Entity {
                        fn to() -> lifeguard::relation::RelationDef {
                            lifeguard::relation::RelationDef {
                                rel_type: lifeguard::relation::RelationType::HasMany,
                                from_tbl: sea_query::DynIden::from(<Entity as lifeguard::LifeEntityName>::table_name(&Entity)).into(),
                                to_tbl: sea_query::DynIden::from(<#entity_path as lifeguard::LifeEntityName>::table_name(&<#entity_path as Default>::default())).into(),
                                from_col: lifeguard::relation::identity::Identity::Unary(sea_query::DynIden::from(#from_col)),
                                to_col: lifeguard::relation::identity::Identity::Unary(sea_query::DynIden::from(#to_col)),
                                is_owner: false, skip_fk: false, through_from_col: None, through_to_col: None, through_tbl: None, on_condition: None, condition_type: sea_query::ConditionType::All,
                            }
                        }
                    }
                    impl lifeguard::query::loader::RelationInjector<#entity_path> for #model_name {
                        fn inject(&mut self, items: Vec<<#entity_path as lifeguard::query::traits::LifeModelTrait>::Model>) {
                            self.#field_name = Some(items);
                        }
                    }
                });
            }
            if let Some(rel) = &col_attrs.belongs_to {
                let rel_attr = relation_attr_on_field(field, "belongs_to");
                let entity_path = match parse_relation_entity_path(
                    &rel.entity,
                    rel_attr,
                    field_name,
                    "#[belongs_to]",
                ) {
                    Ok(p) => p,
                    Err(e) => return e.to_compile_error().into(),
                };
                let Some(from_col) = rel.from.as_deref() else {
                    let e = match rel_attr {
                        Some(a) => syn::Error::new_spanned(
                            a,
                            "#[belongs_to] requires `from = \"column_name\"` (foreign key column on this entity)",
                        ),
                        None => syn::Error::new_spanned(
                            field_name,
                            "#[belongs_to] requires `from = \"column_name\"` (foreign key column on this entity)",
                        ),
                    };
                    return e.to_compile_error().into();
                };
                let to_col = rel.to.as_deref().unwrap_or("id");
                relation_impls.push(quote! {
                    impl lifeguard::Related<#entity_path> for Entity {
                        fn to() -> lifeguard::relation::RelationDef {
                            lifeguard::relation::RelationDef {
                                rel_type: lifeguard::relation::RelationType::BelongsTo,
                                from_tbl: sea_query::DynIden::from(<Entity as lifeguard::LifeEntityName>::table_name(&Entity)).into(),
                                to_tbl: sea_query::DynIden::from(<#entity_path as lifeguard::LifeEntityName>::table_name(&<#entity_path as Default>::default())).into(),
                                from_col: lifeguard::relation::identity::Identity::Unary(sea_query::DynIden::from(#from_col)),
                                to_col: lifeguard::relation::identity::Identity::Unary(sea_query::DynIden::from(#to_col)),
                                is_owner: false, skip_fk: false, through_from_col: None, through_to_col: None, through_tbl: None, on_condition: None, condition_type: sea_query::ConditionType::All,
                            }
                        }
                    }
                    impl lifeguard::query::loader::RelationInjector<#entity_path> for #model_name {
                        fn inject(&mut self, mut items: Vec<<#entity_path as lifeguard::query::traits::LifeModelTrait>::Model>) {
                            self.#field_name = items.into_iter().next();
                        }
                    }
                });
            }
            if let Some(rel) = &col_attrs.has_one {
                let rel_attr = relation_attr_on_field(field, "has_one");
                let entity_path = match parse_relation_entity_path(
                    &rel.entity,
                    rel_attr,
                    field_name,
                    "#[has_one]",
                ) {
                    Ok(p) => p,
                    Err(e) => return e.to_compile_error().into(),
                };
                let from_col = rel.from.as_deref().unwrap_or("id");
                let Some(to_col) = rel.to.as_deref() else {
                    let e = match rel_attr {
                        Some(a) => syn::Error::new_spanned(
                            a,
                            "#[has_one] requires `to = \"column_name\"` (foreign key column on the related entity)",
                        ),
                        None => syn::Error::new_spanned(
                            field_name,
                            "#[has_one] requires `to = \"column_name\"` (foreign key column on the related entity)",
                        ),
                    };
                    return e.to_compile_error().into();
                };
                relation_impls.push(quote! {
                    impl lifeguard::Related<#entity_path> for Entity {
                        fn to() -> lifeguard::relation::RelationDef {
                            lifeguard::relation::RelationDef {
                                rel_type: lifeguard::relation::RelationType::HasOne,
                                from_tbl: sea_query::DynIden::from(<Entity as lifeguard::LifeEntityName>::table_name(&Entity)).into(),
                                to_tbl: sea_query::DynIden::from(<#entity_path as lifeguard::LifeEntityName>::table_name(&<#entity_path as Default>::default())).into(),
                                from_col: lifeguard::relation::identity::Identity::Unary(sea_query::DynIden::from(#from_col)),
                                to_col: lifeguard::relation::identity::Identity::Unary(sea_query::DynIden::from(#to_col)),
                                is_owner: false, skip_fk: false, through_from_col: None, through_to_col: None, through_tbl: None, on_condition: None, condition_type: sea_query::ConditionType::All,
                            }
                        }
                    }
                    impl lifeguard::query::loader::RelationInjector<#entity_path> for #model_name {
                        fn inject(&mut self, mut items: Vec<<#entity_path as lifeguard::query::traits::LifeModelTrait>::Model>) {
                            self.#field_name = items.into_iter().next();
                        }
                    }
                });
            }

            // Don't generate Column enum variant, Iden, etc. for ignored fields
            continue;
        }

        // For non-ignored fields, add to Model struct with serde attributes

        // Generate Column enum variant (PascalCase)
        let column_variant = Ident::new(
            &utils::pascal_case(&field_name.to_string()),
            field_name.span(),
        );
        column_variant_idents.push(column_variant.clone()); // Store identifier for all_columns()
        column_variants.push(quote! {
            #column_variant,
        });

        // Generate Iden implementation
        let column_name_str = column_name.as_str();
        iden_impls.push(quote! {
            Column::#column_variant => #column_name_str,
        });

        // Generate PrimaryKey variant if primary key
        if is_primary_key {
            primary_key_variants.push(quote! {
                #column_variant,
            });
            primary_key_variant_idents.push((column_variant.clone(), is_auto_increment)); // Store (identifier, auto_increment) for trait implementations
            primary_key_field_names.push(field_name.clone()); // Store field name for value extraction

            // Track primary key metadata for PrimaryKeyTrait
            if primary_key_type.is_none() {
                primary_key_type = Some(field_type);
                _primary_key_auto_increment = is_auto_increment; // Keep for backward compatibility, but per-variant tracking is used
            }
            // Track all primary key types for tuple ValueType support
            primary_key_types.push(field_type);

            // Track mapping for PrimaryKeyToColumn
            primary_key_to_column_mappings.push(quote! {
                PrimaryKey::#column_variant => Column::#column_variant,
            });

            // Track primary key field for ModelTrait::get_primary_key_value()
            // Generate the value conversion expression now
            if primary_key_value_expr.is_none() {
                #[allow(clippy::single_match_else)]
                let pk_value_expr = match field_type {
                    syn::Type::Path(syn::TypePath {
                        path: syn::Path { segments, .. },
                        ..
                    }) => {
                        // Check if this is Option<T> first (using segments.last() like extract_option_inner_type)
                        // In syn's representation, Option<i32> is a single path segment with generic arguments,
                        // so segments.len() is 1, not 2. We need to check the last segment for "Option".
                        if let Some(last_segment) = segments.last() {
                            if last_segment.ident == "Option" {
                                // Handle Option<T> for primary key - extract inner type from generic arguments
                                if let Some(inner_type) = extract_option_inner_type(field_type) {
                                    type_conversion::generate_option_field_to_value_with_default(
                                        field_name, inner_type,
                                    )
                                } else {
                                    quote! { sea_query::Value::String(None) }
                                }
                            } else {
                                // Not Option, use direct field-to-value conversion
                                type_conversion::generate_field_to_value(field_name, field_type)
                            }
                        } else {
                            quote! { sea_query::Value::String(None) }
                        }
                    }
                    _ => quote! { sea_query::Value::String(None) },
                };
                primary_key_value_expr = Some(pk_value_expr);
            }
        }

        // Generate Model field with serde rename attribute to match to_json() behavior
        // This ensures from_json() and to_json() use the same JSON key names (database column names)
        // Also add custom deserializers for f32/f64 to handle NaN/infinity string representations
        let column_name_lit = syn::LitStr::new(&column_name, field_name.span());

        // Check if this is a float type that needs custom deserialization
        let deserialize_attr = if type_conversion::is_f32_type(field_type) {
            Some(quote! {
                #[serde(deserialize_with = "lifeguard::deserialize_f32")]
            })
        } else if type_conversion::is_f64_type(field_type) {
            Some(quote! {
                #[serde(deserialize_with = "lifeguard::deserialize_f64")]
            })
        } else if type_conversion::is_option_f32_type(field_type) {
            Some(quote! {
                #[serde(deserialize_with = "lifeguard::deserialize_option_f32")]
            })
        } else if type_conversion::is_option_f64_type(field_type) {
            Some(quote! {
                #[serde(deserialize_with = "lifeguard::deserialize_option_f64")]
            })
        } else {
            None
        };

        model_fields.push(quote! {
            #(#graphql_attrs)*
            #[serde(rename = #column_name_lit)]
            #deserialize_attr
            pub #field_name: #field_type,
        });

        // Generate ModelTrait::get() match arm
        // Convert field value to sea_query::Value
        #[allow(clippy::single_match_else)]
        let field_value_to_value = match field_type {
            syn::Type::Path(syn::TypePath {
                path: syn::Path { segments, .. },
                ..
            }) => {
                // Check if this is Option<T> first (using segments.last() like extract_option_inner_type)
                // In syn's representation, Option<i32> is a single path segment with generic arguments,
                // so segments.len() is 1, not 2. We need to check the last segment for "Option".
                if let Some(last_segment) = segments.last() {
                    if last_segment.ident == "Option" {
                        // Handle Option<T> - extract inner type from generic arguments
                        if let Some(inner_type) = extract_option_inner_type(field_type) {
                            type_conversion::generate_option_field_to_value_with_default(
                                field_name, inner_type,
                            )
                        } else {
                            quote! { sea_query::Value::String(None) }
                        }
                    } else {
                        // Not Option, use direct field-to-value conversion
                        type_conversion::generate_field_to_value(field_name, field_type)
                    }
                } else {
                    quote! { sea_query::Value::String(None) }
                }
            }
            _ => quote! { sea_query::Value::String(None) },
        };

        model_get_match_arms.push(quote! {
            Column::#column_variant => #field_value_to_value,
        });

        // Generate get_by_column_name match arm
        // Note: column_name_lit is already defined above (line 180)
        get_by_column_name_match_arms.push(quote! {
            #column_name_lit => Some(self.get(Column::#column_variant)),
        });

        // Generate get_value_type match arm
        let type_string = type_conversion::type_to_string(field_type);
        let type_string_lit = syn::LitStr::new(&type_string, field_name.span());
        get_value_type_match_arms.push(quote! {
            Column::#column_variant => Some(#type_string_lit),
        });

        // Generate ModelTrait::set() match arm
        // Convert sea_query::Value to field value
        #[allow(clippy::single_match_else)]
        let value_to_field_value = match field_type {
            syn::Type::Path(syn::TypePath {
                path: syn::Path { segments, .. },
                ..
            }) => {
                // Check if this is Option<T> first
                if let Some(last_segment) = segments.last() {
                    if last_segment.ident == "Option" {
                        // Handle Option<T> - extract inner type
                        #[allow(clippy::collapsible_match)]
                        if let Some(inner_type) = extract_option_inner_type(field_type) {
                            if let Type::Path(inner_path) = inner_type {
                                // Check for serde_json::Value
                                let is_json_value = inner_path.path.segments.len() == 2
                                    && inner_path
                                        .path
                                        .segments
                                        .first()
                                        .map(|s| s.ident.to_string())
                                        == Some("serde_json".to_string())
                                    && inner_path.path.segments.last().map(|s| s.ident.to_string())
                                        == Some("Value".to_string());

                                if is_json_value {
                                    quote! {
                                        match value {
                                            sea_query::Value::Json(Some(v)) => {
                                                self.#field_name = Some(*v);
                                                Ok(())
                                            }
                                            sea_query::Value::Json(None) => {
                                                self.#field_name = None;
                                                Ok(())
                                            }
                                            _ => Err(lifeguard::ModelError::InvalidValueType {
                                                column: stringify!(#column_variant).to_string(),
                                                expected: "Json".to_string(),
                                                actual: format!("{:?}", value),
                                            })
                                        }
                                    }
                                } else if let Some(inner_segment) = inner_path.path.segments.last()
                                {
                                    let inner_ident = inner_segment.ident.to_string();
                                    match inner_ident.as_str() {
                                        "i32" => quote! {
                                            match value {
                                                sea_query::Value::Int(Some(v)) => {
                                                    self.#field_name = Some(v);
                                                    Ok(())
                                                }
                                                sea_query::Value::Int(None) => {
                                                    self.#field_name = None;
                                                    Ok(())
                                                }
                                                _ => Err(lifeguard::ModelError::InvalidValueType {
                                                    column: stringify!(#column_variant).to_string(),
                                                    expected: "Int".to_string(),
                                                    actual: format!("{:?}", value),
                                                })
                                            }
                                        },
                                        "i64" => quote! {
                                            match value {
                                                sea_query::Value::BigInt(Some(v)) => {
                                                    self.#field_name = Some(v);
                                                    Ok(())
                                                }
                                                sea_query::Value::BigInt(None) => {
                                                    self.#field_name = None;
                                                    Ok(())
                                                }
                                                _ => Err(lifeguard::ModelError::InvalidValueType {
                                                    column: stringify!(#column_variant).to_string(),
                                                    expected: "BigInt".to_string(),
                                                    actual: format!("{:?}", value),
                                                })
                                            }
                                        },
                                        "i16" => quote! {
                                            match value {
                                                sea_query::Value::SmallInt(Some(v)) => {
                                                    self.#field_name = Some(v);
                                                    Ok(())
                                                }
                                                sea_query::Value::SmallInt(None) => {
                                                    self.#field_name = None;
                                                    Ok(())
                                                }
                                                _ => Err(lifeguard::ModelError::InvalidValueType {
                                                    column: stringify!(#column_variant).to_string(),
                                                    expected: "SmallInt".to_string(),
                                                    actual: format!("{:?}", value),
                                                })
                                            }
                                        },
                                        "String" => quote! {
                                            match value {
                                                sea_query::Value::String(Some(v)) => {
                                                    self.#field_name = Some(v);
                                                    Ok(())
                                                }
                                                sea_query::Value::String(None) => {
                                                    self.#field_name = None;
                                                    Ok(())
                                                }
                                                _ => Err(lifeguard::ModelError::InvalidValueType {
                                                    column: stringify!(#column_variant).to_string(),
                                                    expected: "String".to_string(),
                                                    actual: format!("{:?}", value),
                                                })
                                            }
                                        },
                                        "bool" => quote! {
                                            match value {
                                                sea_query::Value::Bool(Some(v)) => {
                                                    self.#field_name = Some(v);
                                                    Ok(())
                                                }
                                                sea_query::Value::Bool(None) => {
                                                    self.#field_name = None;
                                                    Ok(())
                                                }
                                                _ => Err(lifeguard::ModelError::InvalidValueType {
                                                    column: stringify!(#column_variant).to_string(),
                                                    expected: "Bool".to_string(),
                                                    actual: format!("{:?}", value),
                                                })
                                            }
                                        },
                                        "u8" => quote! {
                                            match value {
                                                sea_query::Value::SmallInt(Some(v)) => {
                                                    self.#field_name = Some(v as u8);
                                                    Ok(())
                                                }
                                                sea_query::Value::SmallInt(None) => {
                                                    self.#field_name = None;
                                                    Ok(())
                                                }
                                                _ => Err(lifeguard::ModelError::InvalidValueType {
                                                    column: stringify!(#column_variant).to_string(),
                                                    expected: "SmallInt".to_string(),
                                                    actual: format!("{:?}", value),
                                                })
                                            }
                                        },
                                        "u16" => quote! {
                                            match value {
                                                sea_query::Value::Int(Some(v)) => {
                                                    self.#field_name = Some(v as u16);
                                                    Ok(())
                                                }
                                                sea_query::Value::Int(None) => {
                                                    self.#field_name = None;
                                                    Ok(())
                                                }
                                                _ => Err(lifeguard::ModelError::InvalidValueType {
                                                    column: stringify!(#column_variant).to_string(),
                                                    expected: "Int".to_string(),
                                                    actual: format!("{:?}", value),
                                                })
                                            }
                                        },
                                        "u32" => quote! {
                                            match value {
                                                sea_query::Value::BigInt(Some(v)) => {
                                                    self.#field_name = Some(v as u32);
                                                    Ok(())
                                                }
                                                sea_query::Value::BigInt(None) => {
                                                    self.#field_name = None;
                                                    Ok(())
                                                }
                                                _ => Err(lifeguard::ModelError::InvalidValueType {
                                                    column: stringify!(#column_variant).to_string(),
                                                    expected: "BigInt".to_string(),
                                                    actual: format!("{:?}", value),
                                                })
                                            }
                                        },
                                        "u64" => quote! {
                                            match value {
                                                sea_query::Value::BigInt(Some(v)) => {
                                                    self.#field_name = Some(v as u64);
                                                    Ok(())
                                                }
                                                sea_query::Value::BigInt(None) => {
                                                    self.#field_name = None;
                                                    Ok(())
                                                }
                                                _ => Err(lifeguard::ModelError::InvalidValueType {
                                                    column: stringify!(#column_variant).to_string(),
                                                    expected: "BigInt".to_string(),
                                                    actual: format!("{:?}", value),
                                                })
                                            }
                                        },
                                        "f32" => quote! {
                                            match value {
                                                sea_query::Value::Float(Some(v)) => {
                                                    self.#field_name = Some(v);
                                                    Ok(())
                                                }
                                                sea_query::Value::Float(None) => {
                                                    self.#field_name = None;
                                                    Ok(())
                                                }
                                                _ => Err(lifeguard::ModelError::InvalidValueType {
                                                    column: stringify!(#column_variant).to_string(),
                                                    expected: "Float".to_string(),
                                                    actual: format!("{:?}", value),
                                                })
                                            }
                                        },
                                        "f64" => quote! {
                                            match value {
                                                sea_query::Value::Double(Some(v)) => {
                                                    self.#field_name = Some(v);
                                                    Ok(())
                                                }
                                                sea_query::Value::Double(None) => {
                                                    self.#field_name = None;
                                                    Ok(())
                                                }
                                                _ => Err(lifeguard::ModelError::InvalidValueType {
                                                    column: stringify!(#column_variant).to_string(),
                                                    expected: "Double".to_string(),
                                                    actual: format!("{:?}", value),
                                                })
                                            }
                                        },
                                        _ => quote! {
                                            Err(lifeguard::ModelError::InvalidValueType {
                                                column: stringify!(#column_variant).to_string(),
                                                expected: "supported type".to_string(),
                                                actual: format!("{:?}", value),
                                            })
                                        },
                                    }
                                } else {
                                    quote! {
                                        Err(lifeguard::ModelError::InvalidValueType {
                                            column: stringify!(#column_variant).to_string(),
                                            expected: "supported type".to_string(),
                                            actual: format!("{:?}", value),
                                        })
                                    }
                                }
                            } else {
                                quote! {
                                    Err(lifeguard::ModelError::InvalidValueType {
                                        column: stringify!(#column_variant).to_string(),
                                        expected: "supported type".to_string(),
                                        actual: format!("{:?}", value),
                                    })
                                }
                            }
                        } else {
                            quote! {
                                Err(lifeguard::ModelError::InvalidValueType {
                                    column: stringify!(#column_variant).to_string(),
                                    expected: "supported type".to_string(),
                                    actual: format!("{:?}", value),
                                })
                            }
                        }
                    } else {
                        // Not Option, check for serde_json::Value or primitive types
                        let is_json_value = segments.len() == 2
                            && segments.first().map(|s| s.ident.to_string())
                                == Some("serde_json".to_string())
                            && segments.last().map(|s| s.ident.to_string())
                                == Some("Value".to_string());

                        if is_json_value {
                            quote! {
                                match value {
                                    sea_query::Value::Json(Some(v)) => {
                                        self.#field_name = *v;
                                        Ok(())
                                    }
                                    sea_query::Value::Json(None) => {
                                        Err(lifeguard::ModelError::InvalidValueType {
                                            column: stringify!(#column_variant).to_string(),
                                            expected: "Json(Some(_))".to_string(),
                                            actual: "Json(None)".to_string(),
                                        })
                                    }
                                    _ => Err(lifeguard::ModelError::InvalidValueType {
                                        column: stringify!(#column_variant).to_string(),
                                        expected: "Json".to_string(),
                                        actual: format!("{:?}", value),
                                    })
                                }
                            }
                        } else if let Some(segment) = segments.first() {
                            let ident_str = segment.ident.to_string();
                            match ident_str.as_str() {
                                "i32" => quote! {
                                    match value {
                                        sea_query::Value::Int(Some(v)) => {
                                            self.#field_name = v;
                                            Ok(())
                                        }
                                        sea_query::Value::Int(None) => {
                                            Err(lifeguard::ModelError::InvalidValueType {
                                                column: stringify!(#column_variant).to_string(),
                                                expected: "Int(Some(_))".to_string(),
                                                actual: "Int(None)".to_string(),
                                            })
                                        }
                                        _ => Err(lifeguard::ModelError::InvalidValueType {
                                            column: stringify!(#column_variant).to_string(),
                                            expected: "Int".to_string(),
                                            actual: format!("{:?}", value),
                                        })
                                    }
                                },
                                "i64" => quote! {
                                    match value {
                                        sea_query::Value::BigInt(Some(v)) => {
                                            self.#field_name = v;
                                            Ok(())
                                        }
                                        sea_query::Value::BigInt(None) => {
                                            Err(lifeguard::ModelError::InvalidValueType {
                                                column: stringify!(#column_variant).to_string(),
                                                expected: "BigInt(Some(_))".to_string(),
                                                actual: "BigInt(None)".to_string(),
                                            })
                                        }
                                        _ => Err(lifeguard::ModelError::InvalidValueType {
                                            column: stringify!(#column_variant).to_string(),
                                            expected: "BigInt".to_string(),
                                            actual: format!("{:?}", value),
                                        })
                                    }
                                },
                                "i16" => quote! {
                                    match value {
                                        sea_query::Value::SmallInt(Some(v)) => {
                                            self.#field_name = v;
                                            Ok(())
                                        }
                                        sea_query::Value::SmallInt(None) => {
                                            Err(lifeguard::ModelError::InvalidValueType {
                                                column: stringify!(#column_variant).to_string(),
                                                expected: "SmallInt(Some(_))".to_string(),
                                                actual: "SmallInt(None)".to_string(),
                                            })
                                        }
                                        _ => Err(lifeguard::ModelError::InvalidValueType {
                                            column: stringify!(#column_variant).to_string(),
                                            expected: "SmallInt".to_string(),
                                            actual: format!("{:?}", value),
                                        })
                                    }
                                },
                                "String" => quote! {
                                    match value {
                                        sea_query::Value::String(Some(v)) => {
                                            self.#field_name = v;
                                            Ok(())
                                        }
                                        sea_query::Value::String(None) => {
                                            Err(lifeguard::ModelError::InvalidValueType {
                                                column: stringify!(#column_variant).to_string(),
                                                expected: "String(Some(_))".to_string(),
                                                actual: "String(None)".to_string(),
                                            })
                                        }
                                        _ => Err(lifeguard::ModelError::InvalidValueType {
                                            column: stringify!(#column_variant).to_string(),
                                            expected: "String".to_string(),
                                            actual: format!("{:?}", value),
                                        })
                                    }
                                },
                                "bool" => quote! {
                                    match value {
                                        sea_query::Value::Bool(Some(v)) => {
                                            self.#field_name = v;
                                            Ok(())
                                        }
                                        sea_query::Value::Bool(None) => {
                                            Err(lifeguard::ModelError::InvalidValueType {
                                                column: stringify!(#column_variant).to_string(),
                                                expected: "Bool(Some(_))".to_string(),
                                                actual: "Bool(None)".to_string(),
                                            })
                                        }
                                        _ => Err(lifeguard::ModelError::InvalidValueType {
                                            column: stringify!(#column_variant).to_string(),
                                            expected: "Bool".to_string(),
                                            actual: format!("{:?}", value),
                                        })
                                    }
                                },
                                "u8" => quote! {
                                    match value {
                                        sea_query::Value::SmallInt(Some(v)) => {
                                            self.#field_name = v as u8;
                                            Ok(())
                                        }
                                        sea_query::Value::SmallInt(None) => {
                                            Err(lifeguard::ModelError::InvalidValueType {
                                                column: stringify!(#column_variant).to_string(),
                                                expected: "SmallInt(Some(_))".to_string(),
                                                actual: "SmallInt(None)".to_string(),
                                            })
                                        }
                                        _ => Err(lifeguard::ModelError::InvalidValueType {
                                            column: stringify!(#column_variant).to_string(),
                                            expected: "SmallInt".to_string(),
                                            actual: format!("{:?}", value),
                                        })
                                    }
                                },
                                "u16" => quote! {
                                    match value {
                                        sea_query::Value::Int(Some(v)) => {
                                            self.#field_name = v as u16;
                                            Ok(())
                                        }
                                        sea_query::Value::Int(None) => {
                                            Err(lifeguard::ModelError::InvalidValueType {
                                                column: stringify!(#column_variant).to_string(),
                                                expected: "Int(Some(_))".to_string(),
                                                actual: "Int(None)".to_string(),
                                            })
                                        }
                                        _ => Err(lifeguard::ModelError::InvalidValueType {
                                            column: stringify!(#column_variant).to_string(),
                                            expected: "Int".to_string(),
                                            actual: format!("{:?}", value),
                                        })
                                    }
                                },
                                "u32" => quote! {
                                    match value {
                                        sea_query::Value::BigInt(Some(v)) => {
                                            self.#field_name = v as u32;
                                            Ok(())
                                        }
                                        sea_query::Value::BigInt(None) => {
                                            Err(lifeguard::ModelError::InvalidValueType {
                                                column: stringify!(#column_variant).to_string(),
                                                expected: "BigInt(Some(_))".to_string(),
                                                actual: "BigInt(None)".to_string(),
                                            })
                                        }
                                        _ => Err(lifeguard::ModelError::InvalidValueType {
                                            column: stringify!(#column_variant).to_string(),
                                            expected: "BigInt".to_string(),
                                            actual: format!("{:?}", value),
                                        })
                                    }
                                },
                                "u64" => quote! {
                                    match value {
                                        sea_query::Value::BigInt(Some(v)) => {
                                            self.#field_name = v as u64;
                                            Ok(())
                                        }
                                        sea_query::Value::BigInt(None) => {
                                            Err(lifeguard::ModelError::InvalidValueType {
                                                column: stringify!(#column_variant).to_string(),
                                                expected: "BigInt(Some(_))".to_string(),
                                                actual: "BigInt(None)".to_string(),
                                            })
                                        }
                                        _ => Err(lifeguard::ModelError::InvalidValueType {
                                            column: stringify!(#column_variant).to_string(),
                                            expected: "BigInt".to_string(),
                                            actual: format!("{:?}", value),
                                        })
                                    }
                                },
                                "f32" => quote! {
                                    match value {
                                        sea_query::Value::Float(Some(v)) => {
                                            self.#field_name = v;
                                            Ok(())
                                        }
                                        sea_query::Value::Float(None) => {
                                            Err(lifeguard::ModelError::InvalidValueType {
                                                column: stringify!(#column_variant).to_string(),
                                                expected: "Float(Some(_))".to_string(),
                                                actual: "Float(None)".to_string(),
                                            })
                                        }
                                        _ => Err(lifeguard::ModelError::InvalidValueType {
                                            column: stringify!(#column_variant).to_string(),
                                            expected: "Float".to_string(),
                                            actual: format!("{:?}", value),
                                        })
                                    }
                                },
                                "f64" => quote! {
                                    match value {
                                        sea_query::Value::Double(Some(v)) => {
                                            self.#field_name = v;
                                            Ok(())
                                        }
                                        sea_query::Value::Double(None) => {
                                            Err(lifeguard::ModelError::InvalidValueType {
                                                column: stringify!(#column_variant).to_string(),
                                                expected: "Double(Some(_))".to_string(),
                                                actual: "Double(None)".to_string(),
                                            })
                                        }
                                        _ => Err(lifeguard::ModelError::InvalidValueType {
                                            column: stringify!(#column_variant).to_string(),
                                            expected: "Double".to_string(),
                                            actual: format!("{:?}", value),
                                        })
                                    }
                                },
                                _ => quote! {
                                    Err(lifeguard::ModelError::InvalidValueType {
                                        column: stringify!(#column_variant).to_string(),
                                        expected: "supported type".to_string(),
                                        actual: format!("{:?}", value),
                                    })
                                },
                            }
                        } else {
                            quote! {
                                Err(lifeguard::ModelError::InvalidValueType {
                                    column: stringify!(#column_variant).to_string(),
                                    expected: "supported type".to_string(),
                                    actual: format!("{:?}", value),
                                })
                            }
                        }
                    }
                } else {
                    quote! {
                        Err(lifeguard::ModelError::InvalidValueType {
                            column: stringify!(#column_variant).to_string(),
                            expected: "supported type".to_string(),
                            actual: format!("{:?}", value),
                        })
                    }
                }
            }
            _ => quote! {
                Err(lifeguard::ModelError::InvalidValueType {
                    column: stringify!(#column_variant).to_string(),
                    expected: "supported type".to_string(),
                    actual: format!("{:?}", value),
                })
            },
        };

        model_set_match_arms.push(quote! {
            Column::#column_variant => #value_to_field_value,
        });

        // Generate FromRow field extraction
        let column_name_str = column_name.as_str();

        // Determine nullability from Option<T> or #[nullable] attribute
        let is_nullable = col_attrs.is_nullable || extract_option_inner_type(field_type).is_some();

        let get_expr = {
            // Check for special types that need custom handling
            // First, extract the inner type if it's Option<T>
            let inner_type = extract_option_inner_type(field_type).unwrap_or(field_type);

            // Keep detection in sync with `type_conversion` (Record/ActiveModel → `sea_query::Value`).
            let is_uuid = type_conversion::is_uuid_type(inner_type);
            let is_naive_datetime = type_conversion::is_naive_datetime_type(inner_type);
            let is_decimal = type_conversion::is_decimal_type(inner_type);
            let is_money = type_conversion::is_money_type(inner_type);

            // Handle uuid::Uuid — bind Postgres `uuid` via `FromSql` (not text parsing).
            if is_uuid {
                if is_nullable {
                    quote! {
                        row.try_get::<&str, Option<uuid::Uuid>>(#column_name_str)?
                    }
                } else {
                    quote! {
                        row.try_get::<&str, uuid::Uuid>(#column_name_str)?
                    }
                }
            }
            // Handle rust_decimal::Decimal - uses FromSql directly (NUMERIC type)
            else if is_decimal {
                // Decimal implements FromSql for NUMERIC, so we can use try_get directly
                quote! {
                    row.try_get::<&str, #field_type>(#column_name_str)?
                }
            }
            // Handle rusty_money::Money - needs special construction from amount + currency_code
            // Note: This requires currency_code field to exist in the same struct
            // For now, we'll use a placeholder - Money support will need entity-level coordination
            else if is_money {
                // TODO: Implement Money construction from NUMERIC amount + currency_code
                // This requires finding the currency_code field in the same struct
                // For now, fall back to direct try_get (may not work if Money doesn't implement FromSql)
                quote! {
                    // Money type - requires amount (NUMERIC) + currency_code (VARCHAR)
                    // TODO: Implement proper Money construction
                    // For now, try direct FromSql (may fail if Money doesn't implement it)
                    row.try_get::<&str, #field_type>(#column_name_str)?
                }
            }
            // Handle chrono::NaiveDateTime - get as SystemTime then convert
            else if is_naive_datetime {
                if is_nullable {
                    quote! {
                        row.try_get::<&str, Option<std::time::SystemTime>>(#column_name_str)?
                            .map(|st| chrono::DateTime::<chrono::Utc>::from(st).naive_utc())
                    }
                } else {
                    quote! {
                        {
                            let st = row.try_get::<&str, std::time::SystemTime>(#column_name_str)?;
                            chrono::DateTime::<chrono::Utc>::from(st).naive_utc()
                        }
                    }
                }
            }
            // Handle unsigned integer types
            else {
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
                    #[allow(clippy::single_match_else)]
                    let signed_type = match field_type {
                        syn::Type::Path(syn::TypePath {
                            path: syn::Path { segments, .. },
                            ..
                        }) => {
                            if let Some(segment) = segments.first() {
                                match segment.ident.to_string().as_str() {
                                    "u8" => quote! { i16 },
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
                            #field_type::try_from(val).map_err(|_| {
                                lifeguard::from_row_unsigned_try_from_failed(row, #column_name_str)
                            })?
                        }
                    }
                } else {
                    quote! {
                        row.try_get::<&str, #field_type>(#column_name_str)?
                    }
                }
            }
        };

        from_row_fields.push(quote! {
            #field_name: #get_expr,
        });

        // Generate ColumnTrait::def() match arm
        // Determine nullability from Option<T> or #[nullable] attribute
        // Use extract_option_inner_type to properly detect Option<T> types
        let is_nullable = col_attrs.is_nullable || extract_option_inner_type(field_type).is_some();

        // Build ColumnDefinition struct literal
        // If column_type is not explicitly provided, infer it from Rust type
        let column_type_expr = if let Some(ct) = col_attrs.column_type.as_ref() {
            let ct_lit = syn::LitStr::new(ct, field_name.span());
            quote! { Some(#ct_lit.to_string()) }
        } else {
            // Infer SQL type from Rust type
            let inferred_type = infer_sql_type_from_rust_type(field_type);
            if let Some(sql_type) = inferred_type {
                let sql_type_lit = syn::LitStr::new(&sql_type, field_name.span());
                quote! { Some(#sql_type_lit.to_string()) }
            } else {
                quote! { None }
            }
        };

        let default_value_expr = col_attrs.default_value.as_ref().map_or_else(
            || quote! { None },
            |dv| {
                let dv_lit = syn::LitStr::new(dv, field_name.span());
                quote! { Some(#dv_lit.to_string()) }
            },
        );

        let default_expr_expr = col_attrs.default_expr.as_ref().map_or_else(
            || quote! { None },
            |de| {
                let de_lit = syn::LitStr::new(de, field_name.span());
                quote! { Some(#de_lit.to_string()) }
            },
        );

        let renamed_from_expr = col_attrs.renamed_from.as_ref().map_or_else(
            || quote! { None },
            |rf| {
                let rf_lit = syn::LitStr::new(rf, field_name.span());
                quote! { Some(#rf_lit.to_string()) }
            },
        );

        let select_as_expr = col_attrs.select_as.as_ref().map_or_else(
            || quote! { None },
            |sa| {
                let sa_lit = syn::LitStr::new(sa, field_name.span());
                quote! { Some(#sa_lit.to_string()) }
            },
        );

        let save_as_expr = col_attrs.save_as.as_ref().map_or_else(
            || quote! { None },
            |sa| {
                let sa_lit = syn::LitStr::new(sa, field_name.span());
                quote! { Some(#sa_lit.to_string()) }
            },
        );

        let comment_expr = col_attrs.comment.as_ref().map_or_else(
            || quote! { None },
            |c| {
                let c_lit = syn::LitStr::new(c, field_name.span());
                quote! { Some(#c_lit.to_string()) }
            },
        );

        let foreign_key_expr = col_attrs.foreign_key.as_ref().map_or_else(
            || quote! { None },
            |fk| {
                let fk_lit = syn::LitStr::new(fk, field_name.span());
                quote! { Some(#fk_lit.to_string()) }
            },
        );

        let check_expr = col_attrs.check.as_ref().map_or_else(
            || quote! { None },
            |c| {
                let c_lit = syn::LitStr::new(c, field_name.span());
                quote! { Some(#c_lit.to_string()) }
            },
        );

        // Extract boolean attributes for use in quote! macro
        let is_primary_key_attr = col_attrs.is_primary_key;
        let is_unique_attr = col_attrs.is_unique;
        let is_indexed_attr = col_attrs.is_indexed;
        let is_auto_increment_attr = col_attrs.is_auto_increment;

        column_def_match_arms.push(quote! {
            Column::#column_variant => lifeguard::ColumnDefinition {
                column_type: #column_type_expr,
                nullable: #is_nullable,
                default_value: #default_value_expr,
                default_expr: #default_expr_expr,
                renamed_from: #renamed_from_expr,
                select_as: #select_as_expr,
                save_as: #save_as_expr,
                comment: #comment_expr,
                primary_key: #is_primary_key_attr,
                unique: #is_unique_attr,
                indexed: #is_indexed_attr,
                auto_increment: #is_auto_increment_attr,
                foreign_key: #foreign_key_expr,
                check: #check_expr,
            },
        });

        // Generate ColumnTrait::enum_type_name() match arm if enum_name is present
        if let Some(ref enum_name) = col_attrs.enum_name {
            let enum_name_lit = syn::LitStr::new(enum_name, field_name.span());
            enum_type_name_match_arms.push(quote! {
                Column::#column_variant => Some(#enum_name_lit.to_string()),
            });
        } else {
            enum_type_name_match_arms.push(quote! {
                Column::#column_variant => None,
            });
        }
    }

    // Generate primary key value expression for ModelTrait
    let single_pk_value_impl = primary_key_value_expr.as_ref().map_or_else(
        || {
            quote! {
                // WARNING: No primary key found for this entity.
                // get_primary_key_value() returns String(None) when no primary key is defined.
                // Consider adding a #[primary_key] attribute to one of the fields.
                sea_query::Value::String(None)
            }
        },
        |expr| {
            quote! {
                #expr
            }
        },
    );

    // Generate get_primary_key_identity() implementation
    let pk_identity_impl = if primary_key_variant_idents.is_empty() {
        // No primary key - return empty Identity with arity 0 to match get_primary_key_values()
        // Using Many(vec![]) ensures arity() returns 0, matching the empty vec![] from get_primary_key_values()
        quote! {
            lifeguard::Identity::Many(vec![])
        }
    } else {
        // Generate Identity based on number of primary keys
        // Convert Column enum variants to DynIden using column name strings
        match primary_key_variant_idents.len() {
            1 => {
                let col = &primary_key_variant_idents[0].0;
                // Get column name from IdenStatic::as_str()
                quote! {
                    {
                        use sea_query::IdenStatic;
                        lifeguard::Identity::Unary(sea_query::DynIden::from(Column::#col.as_str()))
                    }
                }
            }
            2 => {
                let col1 = &primary_key_variant_idents[0].0;
                let col2 = &primary_key_variant_idents[1].0;
                quote! {
                    {
                        use sea_query::IdenStatic;
                        lifeguard::Identity::Binary(
                            sea_query::DynIden::from(Column::#col1.as_str()),
                            sea_query::DynIden::from(Column::#col2.as_str())
                        )
                    }
                }
            }
            3 => {
                let col1 = &primary_key_variant_idents[0].0;
                let col2 = &primary_key_variant_idents[1].0;
                let col3 = &primary_key_variant_idents[2].0;
                quote! {
                    {
                        use sea_query::IdenStatic;
                        lifeguard::Identity::Ternary(
                            sea_query::DynIden::from(Column::#col1.as_str()),
                            sea_query::DynIden::from(Column::#col2.as_str()),
                            sea_query::DynIden::from(Column::#col3.as_str())
                        )
                    }
                }
            }
            _n => {
                // 4 or more keys - use Many variant
                let cols: Vec<_> = primary_key_variant_idents
                    .iter()
                    .map(|(col, _)| {
                        quote! { sea_query::DynIden::from(Column::#col.as_str()) }
                    })
                    .collect();
                quote! {
                    {
                        use sea_query::IdenStatic;
                        lifeguard::Identity::Many(vec![#(#cols),*])
                    }
                }
            }
        }
    };

    // Generate get_primary_key_values() implementation
    // Reuse the same conversion logic as get_primary_key_value() for consistency
    let pk_values_impl = if primary_key_field_names.is_empty() {
        // No primary key - return empty vector
        quote! {
            vec![]
        }
    } else {
        // Generate code to extract all primary key values
        // We need to match the field types and use the same conversion as get_primary_key_value()
        // For now, use a simpler approach: collect all primary key values
        let mut value_exprs = Vec::new();
        for (idx, field_name) in primary_key_field_names.iter().enumerate() {
            // Get the field type for this primary key
            if idx < primary_key_types.len() {
                let field_type = primary_key_types[idx];
                // Use the same conversion logic as get_primary_key_value()
                // Check if it's Option<T> and handle accordingly
                if let Some(inner_type) = extract_option_inner_type(field_type) {
                    // Option<T> - use the same conversion as get() method
                    value_exprs.push(
                        type_conversion::generate_option_field_to_value_with_default(
                            field_name, inner_type,
                        ),
                    );
                } else {
                    // Non-Option - use direct conversion
                    value_exprs.push(type_conversion::generate_field_to_value(
                        field_name, field_type,
                    ));
                }
            } else {
                // Fallback if types don't match (shouldn't happen)
                value_exprs.push(quote! { sea_query::Value::String(None) });
            }
        }
        quote! {
            vec![#(#value_exprs),*]
        }
    };

    // Generate PrimaryKeyTrait and PrimaryKeyToColumn implementations (if primary key exists)
    let primary_key_trait_impls = if !primary_key_variant_idents.is_empty()
        && primary_key_type.is_some()
    {
        // Generate ValueType - tuple for composite keys, single type for single keys
        let value_type = if primary_key_types.len() == 1 {
            // Single primary key - extract inner type if Option<T>
            let pk_type = primary_key_types[0];
            if let Some(inner_type) = extract_option_inner_type(pk_type) {
                // Option<T> -> use inner type T
                quote! { #inner_type }
            } else {
                // Non-Option type -> use as-is
                quote! { #pk_type }
            }
        } else {
            // Composite primary key - generate tuple type
            let tuple_types: Vec<proc_macro2::TokenStream> = primary_key_types
                .iter()
                .map(|pk_type| {
                    if let Some(inner_type) = extract_option_inner_type(pk_type) {
                        // Option<T> -> use inner type T
                        quote! { #inner_type }
                    } else {
                        // Non-Option type -> use as-is
                        quote! { #pk_type }
                    }
                })
                .collect();
            quote! { (#(#tuple_types),*) }
        };

        // Generate auto_increment match arms
        // Each variant uses its own auto_increment value, supporting composite primary keys
        // with mixed auto_increment settings
        let auto_increment_arms = primary_key_variant_idents
            .iter()
            .map(|(variant, auto_inc)| {
                if *auto_inc {
                    quote! {
                        PrimaryKey::#variant => true,
                    }
                } else {
                    quote! {
                        PrimaryKey::#variant => false,
                    }
                }
            });

        // Generate PrimaryKeyArity implementation
        // Determine arity at macro expansion time based on number of primary key variants
        // Lifeguard enhancement: granular arity variants for better type safety
        let primary_key_arity_impl = match primary_key_variant_idents.len() {
            1 => quote! {
                lifeguard::PrimaryKeyArity::Single
            },
            2 => quote! {
                lifeguard::PrimaryKeyArity::Tuple2
            },
            3 => quote! {
                lifeguard::PrimaryKeyArity::Tuple3
            },
            4 => quote! {
                lifeguard::PrimaryKeyArity::Tuple4
            },
            5 => quote! {
                lifeguard::PrimaryKeyArity::Tuple5
            },
            _ => quote! {
                lifeguard::PrimaryKeyArity::Tuple6Plus
            },
        };

        quote! {
            // Implement PrimaryKeyTrait
            impl lifeguard::PrimaryKeyTrait for PrimaryKey {
                type ValueType = #value_type;

                fn auto_increment(self) -> bool {
                    match self {
                        #(#auto_increment_arms)*
                    }
                }
            }

            // Implement PrimaryKeyToColumn
            impl lifeguard::PrimaryKeyToColumn for PrimaryKey {
                type Column = Column;

                fn to_column(self) -> Self::Column {
                    match self {
                        #(#primary_key_to_column_mappings)*
                    }
                }
            }

            // Implement PrimaryKeyArityTrait
            impl lifeguard::PrimaryKeyArityTrait for PrimaryKey {
                fn arity() -> lifeguard::PrimaryKeyArity {
                    #primary_key_arity_impl
                }
            }
        }
    } else {
        quote! {
            // No primary key defined - PrimaryKeyTrait, PrimaryKeyToColumn, and PrimaryKeyArityTrait not implemented
        }
    };

    // Generate FromRow implementation conditionally
    let from_row_impl = if table_attrs.skip_from_row {
        quote! {
            // FromRow generation skipped (skip_from_row attribute set)
            // This is useful for SQL generation when types don't implement FromSql
        }
    } else {
        quote! {
            #[automatically_derived]
            impl lifeguard::FromRow for #model_name {
                fn from_row(row: &may_postgres::Row) -> Result<Self, may_postgres::Error> {
                    Ok(Self {
                        #(#from_row_fields)*
                    })
                }
            }
        }
    };

    // Pass soft_delete down to DeriveEntity by declaring it before quote!
    let soft_delete_attr = if table_attrs.soft_delete {
        quote! { #[soft_delete] }
    } else {
        quote! {}
    };

    let cursor_tiebreak_user = attributes::extract_cursor_tiebreak(&input.attrs);
    if let Some(ref tie) = cursor_tiebreak_user {
        if primary_key_variant_idents.len() != 1 {
            return syn::Error::new_spanned(
                tie,
                "#[cursor_tiebreak] is only valid for entities with a single-column primary key",
            )
            .to_compile_error()
            .into();
        }
    }
    let cursor_tiebreak_attr = if let Some(variant) = cursor_tiebreak_user {
        let lit = syn::LitStr::new(&variant.to_string(), variant.span());
        quote! { #[cursor_tiebreak = #lit] }
    } else {
        quote! {}
    };

    #[cfg(feature = "graphql")]
    let graphql_derive = quote! {
        #[derive(lifeguard::async_graphql::SimpleObject)]
        #[graphql(name = #model_name_lit)]
    };
    #[cfg(not(feature = "graphql"))]
    let graphql_derive = quote! {};

    // Generate Entity with nested DeriveEntity (like SeaORM)
    // This triggers nested expansion where DeriveEntity generates LifeModelTrait
    let expanded = quote! {
        // STEP 1: Generate Column enum FIRST (before Entity, so DeriveEntity can reference it)
        // Make it pub so it's visible to DeriveEntity during nested expansion
        #[doc = " Generated by lifeguard-derive"]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Column {
            #(#column_variants)*
        }

        // Implement Iden for Column
        impl sea_query::Iden for Column {
            fn unquoted(&self) -> &str {
                match self {
                    #(#iden_impls)*
                }
            }
        }

        // Implement IdenStatic for Column (required by LifeModelTrait::Column)
        impl sea_query::IdenStatic for Column {
            fn as_str(&self) -> &'static str {
                match self {
                    #(#iden_impls)*
                }
            }
        }

        // NOTE: We can't generate `impl ColumnTrait for Column` because it conflicts
        // with the blanket impl `impl<T: IntoColumnRef> ColumnTrait for T {}`.
        // Rust doesn't allow overriding blanket impls with specific impls.
        //
        // For now, we'll generate helper functions that can be used to get column metadata.
        // Users can call these functions directly, or we can work on a better solution later.
        //
        // TODO: Consider using specialization (when stable) or a different trait design
        // to allow macro-generated impls to override default trait methods.
        //
        // Alternative: Generate a separate trait or use associated constants/functions
        // that the default ColumnTrait implementations can call.

        // Generate helper functions for column definitions (workaround for blanket impl conflict)
        impl Column {
            /// Get column definition metadata (generated by LifeModel macro)
            pub fn column_def(self) -> lifeguard::ColumnDefinition {
                match self {
                    #(#column_def_match_arms)*
                }
            }
        }

        // Implement ColumnDefHelper trait to allow generic code to call column_def()
        impl lifeguard::query::column::column_trait::ColumnDefHelper for Column {
            fn column_def(self) -> lifeguard::ColumnDefinition {
                self.column_def()
            }
        }

        impl Column {

            /// Get enum type name if this column is an enum (generated by LifeModel macro)
            pub fn column_enum_type_name(self) -> Option<String> {
                match self {
                    #(#enum_type_name_match_arms)*
                }
            }

            /// Get all column variants (generated by LifeModel macro)
            ///
            /// Returns a static array of all Column enum variants.
            /// This is useful for iterating through all columns, e.g., when building SELECT queries
            /// that need to check for select_as expressions.
            pub fn all_columns() -> &'static [Column] {
                static COLUMNS: &[Column] = &[
                    #(Column::#column_variant_idents,)*
                ];
                COLUMNS
            }

            /// Get save_as expression for this column (generated by LifeModel macro)
            ///
            /// Returns the custom SQL expression to use when saving this column,
            /// or None if no custom expression is defined.
            /// This is a helper method that works around the blanket impl conflict
            /// for ColumnTrait::save_as().
            pub fn column_save_as(self) -> Option<String> {
                self.column_def().save_as
            }
        }

        // Create a type alias to ensure Column is fully resolved before DeriveEntity expands
        // This helps the compiler resolve Column during nested macro expansion
        type _ColumnAlias = Column;

        // STEP 2: Generate Entity struct (unit struct, like SeaORM)
        #[doc = " Generated by lifeguard-derive"]
        #[derive(Copy, Clone, Debug, lifeguard_derive::DeriveEntity)]
        #[table_name = #table_name_lit]
        #[model = #model_name_lit]
        #schema_attr
        #soft_delete_attr
        #cursor_tiebreak_attr
        pub struct Entity;

        // Table name constant (for convenience, matches Entity::table_name())
        impl Entity {
            pub const TABLE_NAME: &'static str = #table_name_lit;

            /// Get table definition metadata (for entity-driven migrations)
            ///
            /// Returns table-level metadata including composite unique constraints,
            /// indexes, CHECK constraints, and table comments.
            pub fn table_definition() -> lifeguard::TableDefinition {
                #table_definition_expr
            }
        }

        // NOTE: LifeEntityName, Iden, IdenStatic, Default, and LifeModelTrait are all
        // generated by DeriveEntity (nested expansion via #[derive(DeriveEntity)] above)
        // Do NOT generate them here to avoid conflicts

        // STEP 3: Generate PrimaryKey enum
        #[doc = " Generated by lifeguard-derive"]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum PrimaryKey {
            #(#primary_key_variants)*
        }

        // STEP 4: Generate PrimaryKeyTrait and PrimaryKeyToColumn implementations
        #primary_key_trait_impls

        // STEP 5: Generate Model struct (like SeaORM's expand_derive_model)
        // Note: Serialize/Deserialize are added for JSON support (core feature)
        #[doc = " Generated by lifeguard-derive"]
        #graphql_derive
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
        pub struct #model_name {
            #(#model_fields)*
        }

        // STEP 6: Generate FromRow implementation (automatic, no separate derive needed)
        // Skip if skip_from_row attribute is set (useful for SQL generation)
        #from_row_impl

        // STEP 7: Generate ModelTrait implementation
        // NOTE: We use Column directly instead of Entity::Column to avoid E0223 errors
        // during macro expansion. Entity::Column will be available after DeriveEntity expands.
        #[automatically_derived]
        impl lifeguard::ModelTrait for #model_name {
            type Entity = Entity;

            fn get(&self, column: Column) -> sea_query::Value {
                match column {
                    #(#model_get_match_arms)*
                    // Note: Match is exhaustive - all Column variants must have corresponding fields
                    // This is enforced at compile time by Rust
                }
            }

            fn set(
                &mut self,
                column: Column,
                value: sea_query::Value,
            ) -> Result<(), lifeguard::ModelError> {
                match column {
                    #(#model_set_match_arms)*
                    // Note: Match is exhaustive - all Column variants must have corresponding fields
                    // This is enforced at compile time by Rust
                }
            }

            fn get_primary_key_value(&self) -> sea_query::Value {
                #single_pk_value_impl
            }

            fn get_primary_key_identity(&self) -> lifeguard::Identity {
                #pk_identity_impl
            }

            fn get_primary_key_values(&self) -> Vec<sea_query::Value> {
                #pk_values_impl
            }

            fn get_by_column_name(&self, column_name: &str) -> Option<sea_query::Value> {
                match column_name {
                    #(#get_by_column_name_match_arms)*
                    _ => None,
                }
            }

            fn get_value_type(&self, column: Column) -> Option<&'static str> {
                match column {
                    #(#get_value_type_match_arms)*
                }
            }
        }

        // STEP 8: Setup dynamic relationship links generated from #[has_many/one/belongs_to] attributes
        #(#relation_impls)*

        // STEP 9: LifeModelTrait is generated by DeriveEntity (nested expansion)
        // This happens in a separate expansion phase, allowing proper type resolution
        // DeriveEntity sets both type Model and type Column using the identifiers passed via attributes
    };

    TokenStream::from(expanded)
}

#[cfg(test)]
mod relation_parse_error_tests {
    #![allow(clippy::unwrap_used)] // Test fixtures use `parse_quote!` named fields only

    use super::{parse_relation_entity_path, relation_attr_on_field};
    use syn::parse_quote;

    #[test]
    fn invalid_entity_path_returns_syn_error_with_message() {
        let field: syn::Field = parse_quote! {
            #[has_many(entity = "crate::bad::!!!", to = "x")]
            rel: Option<Vec<()>>
        };
        let field_name = field.ident.as_ref().unwrap();
        let rel_attr = relation_attr_on_field(&field, "has_many");
        let err =
            parse_relation_entity_path("crate::bad::!!!", rel_attr, field_name, "#[has_many]")
                .expect_err("invalid path should error");
        let msg = err.to_string();
        assert!(
            msg.contains("invalid `entity` path") || msg.contains("expected identifier"),
            "unexpected message: {msg}"
        );
    }

    #[test]
    fn invalid_belongs_to_entity_path_returns_syn_error_with_message() {
        let field: syn::Field = parse_quote! {
            #[belongs_to(entity = "crate::bad::!!!", from = "x")]
            parent: Option<()>
        };
        let field_name = field.ident.as_ref().unwrap();
        let rel_attr = relation_attr_on_field(&field, "belongs_to");
        let err =
            parse_relation_entity_path("crate::bad::!!!", rel_attr, field_name, "#[belongs_to]")
                .expect_err("invalid path should error");
        let msg = err.to_string();
        assert!(
            msg.contains("invalid `entity` path") || msg.contains("expected identifier"),
            "unexpected message: {msg}"
        );
    }
}
