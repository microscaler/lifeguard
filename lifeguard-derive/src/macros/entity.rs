//! Derive macro for Entity
//!
//! Generates Entity unit struct, `EntityName`, Iden, and `IdenStatic` implementations.
//! This is separate from other derives to match `SeaORM`'s architecture.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, Attribute, DeriveInput};

use crate::attributes;
use crate::utils;

fn default_stripped_suffix_ident(
    struct_name: &syn::Ident,
    strip_suffix: &str,
    append: &str,
) -> syn::Ident {
    let s = struct_name.to_string();
    let base = if let Some(prefix) = s.strip_suffix(strip_suffix) {
        prefix.to_string()
    } else {
        s
    };
    syn::Ident::new(&format!("{base}{append}"), struct_name.span())
}

fn resolve_entity_model_name(struct_name: &syn::Ident, attrs: &[Attribute]) -> syn::Ident {
    match attributes::extract_model_name(attrs) {
        Some(ident) => ident,
        None => {
            if *struct_name == "Entity" {
                syn::Ident::new("Model", struct_name.span())
            } else {
                default_stripped_suffix_ident(struct_name, "Entity", "Model")
            }
        }
    }
}

fn resolve_entity_column_enum_name(struct_name: &syn::Ident, attrs: &[Attribute]) -> syn::Ident {
    match attributes::extract_column_enum_name(attrs) {
        Some(ident) => ident,
        None => {
            if *struct_name == "Entity" {
                syn::Ident::new("Column", struct_name.span())
            } else {
                default_stripped_suffix_ident(struct_name, "Entity", "Column")
            }
        }
    }
}

fn generate_life_model_trait_impl(
    struct_name: &syn::Ident,
    model_name: &syn::Ident,
    column_name: &syn::Ident,
    cursor_tiebreak_impl: &TokenStream2,
    soft_delete_column_impl: &TokenStream2,
    find_impl: &TokenStream2,
) -> TokenStream2 {
    quote! {
        impl lifeguard::LifeModelTrait for #struct_name {
            type Model = #model_name;
            type Column = #column_name;

            fn all_columns() -> &'static [Self::Column] {
                #column_name::all_columns()
            }

            #cursor_tiebreak_impl

            #soft_delete_column_impl

            #find_impl
        }
    }
}

/// Generate Entity, `EntityName`, Iden, and `IdenStatic` implementations
pub fn derive_entity(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    // Extract struct name (Entity should be a unit struct)
    let struct_name = &input.ident;

    // Extract table name and schema name from attributes
    let table_name = attributes::extract_table_name(&input.attrs)
        .unwrap_or_else(|| utils::snake_case(&struct_name.to_string()));
    let schema_name = attributes::extract_schema_name(&input.attrs);

    // Following SeaORM's EXACT pattern: DeriveEntity generates EntityName, Iden, IdenStatic, and EntityTrait
    // via NESTED expansion from LifeModel so Entity::Model resolves in a later phase.
    let model_name = resolve_entity_model_name(struct_name, &input.attrs);
    let column_name = resolve_entity_column_enum_name(struct_name, &input.attrs);

    // Following SeaORM's EXACT pattern: DeriveEntity generates trait implementations
    // for an already-declared Entity struct. The struct itself is NOT generated here.
    // This is called via NESTED macro expansion from DeriveEntityModel (or our LifeModel).
    //
    // KEY INSIGHT: EntityTrait is generated in a SEPARATE expansion phase via DeriveEntity,
    // not in the same expansion as Entity and Model. This allows the compiler to resolve
    // types properly across expansion phases.

    let schema_name_impl = if let Some(ref schema) = schema_name {
        let schema_lit = syn::LitStr::new(schema, struct_name.span());
        quote! {
            fn schema_name(&self) -> Option<&'static str> {
                Some(#schema_lit)
            }
        }
    } else {
        quote! {
            fn schema_name(&self) -> Option<&'static str> {
                None
            }
        }
    };

    // Check if soft_delete is enabled
    // The parent expansion (LifeModel) passes #[soft_delete] down if it's enabled
    let soft_delete = input
        .attrs
        .iter()
        .any(|attr| attr.path().is_ident("soft_delete"));

    let find_impl = if soft_delete {
        quote! {
            fn find() -> lifeguard::SelectQuery<Self> {
                // Return a query that automatically filters out deleted records
                lifeguard::SelectQuery::new().filter(lifeguard::query::column::column_trait::ColumnTrait::is_null(<Self as lifeguard::LifeModelTrait>::Column::DeletedAt))
            }
        }
    } else {
        quote! {} // Use default implementation from trait
    };

    // So `SelectQuery::all` / `apply_soft_delete` / loaders match `find()` when using `SelectQuery::new()`.
    let soft_delete_column_impl = if soft_delete {
        quote! {
            fn soft_delete_column() -> Option<Self::Column> {
                Some(<Self as lifeguard::LifeModelTrait>::Column::DeletedAt)
            }
        }
    } else {
        quote! {}
    };

    let cursor_tiebreak_impl = match attributes::extract_cursor_tiebreak(&input.attrs) {
        Some(variant) => quote! {
            fn cursor_tiebreak_column() -> Option<Self::Column> {
                Some(#column_name::#variant)
            }
        },
        None => quote! {},
    };

    let life_model_trait_impl = generate_life_model_trait_impl(
        struct_name,
        &model_name,
        &column_name,
        &cursor_tiebreak_impl,
        &soft_delete_column_impl,
        &find_impl,
    );

    let expanded: TokenStream2 = quote! {
        // Implement Default for Entity (required by LifeEntityName)
        impl Default for #struct_name {
            fn default() -> Self {
                #struct_name
            }
        }

        // Implement LifeEntityName for Entity (provides table_name method)
        impl lifeguard::LifeEntityName for #struct_name {
            fn table_name(&self) -> &'static str {
                #table_name
            }

            #schema_name_impl
        }

        // Implement Iden for Entity (for use in sea_query)
        impl sea_query::Iden for #struct_name {
            fn unquoted(&self) -> &str {
                #table_name
            }
        }

        // Implement IdenStatic for Entity (for use in sea_query)
        impl sea_query::IdenStatic for #struct_name {
            fn as_str(&self) -> &'static str {
                #table_name
            }
        }

        // CRITICAL: LifeModelTrait is generated in DeriveEntity (nested expansion) so Entity::Model resolves.
        #life_model_trait_impl
    };

    TokenStream::from(expanded)
}
