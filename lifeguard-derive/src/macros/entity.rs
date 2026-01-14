//! Derive macro for Entity
//!
//! Generates Entity unit struct, EntityName, Iden, and IdenStatic implementations.
//! This is separate from other derives to match SeaORM's architecture.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{parse_macro_input, DeriveInput};

use crate::attributes;
use crate::utils;

/// Generate Entity, EntityName, Iden, and IdenStatic implementations
pub fn derive_entity(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    // Extract struct name (Entity should be a unit struct)
    let struct_name = &input.ident;

    // Extract table name from attributes
    let table_name = attributes::extract_table_name(&input.attrs)
        .unwrap_or_else(|| utils::snake_case(&struct_name.to_string()));

    // Following SeaORM's EXACT pattern: DeriveEntity generates EntityName, Iden, IdenStatic, and EntityTrait
    // This is called via NESTED macro expansion from DeriveEntityModel (or our LifeModel)
    // The key insight: EntityTrait is generated in a SEPARATE expansion phase, not the same one
    // This allows the compiler to resolve types properly across expansion phases

    // Extract Model name from attributes
    // In SeaORM, Model is always named "Model", but we use "{Struct}Model"
    // We pass it via #[model = "ModelName"] attribute from LifeModel
    let model_name = match attributes::extract_model_name(&input.attrs) {
        Some(ident) => ident,
        None => {
            // Default: assume Model is named "Model" (SeaORM convention)
            // or "{Entity}Model" if Entity is not "Entity"
            if struct_name.to_string() == "Entity" {
                syn::Ident::new("Model", struct_name.span())
            } else {
                // Remove "Entity" suffix if present, add "Model"
                let base = struct_name.to_string();
                let base = if base.ends_with("Entity") {
                    &base[..base.len() - 6]
                } else {
                    &base
                };
                syn::Ident::new(&format!("{}Model", base), struct_name.span())
            }
        }
    };

    // Extract Column enum name from attributes
    // We pass it via #[column = "Column"] attribute from LifeModel
    // Use format_ident! to create the Ident (like SeaORM does)
    let column_name = match attributes::extract_column_name_ident(&input.attrs) {
        Some(ident) => ident,
        None => {
            // Default: assume Column is named "Column"
            // Use format_ident! like SeaORM does
            format_ident!("Column")
        }
    };

    // Following SeaORM's EXACT pattern: DeriveEntity generates trait implementations
    // for an already-declared Entity struct. The struct itself is NOT generated here.
    // This is called via NESTED macro expansion from DeriveEntityModel (or our LifeModel).
    //
    // KEY INSIGHT: EntityTrait is generated in a SEPARATE expansion phase via DeriveEntity,
    // not in the same expansion as Entity and Model. This allows the compiler to resolve
    // types properly across expansion phases.

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

        // CRITICAL: Generate LifeModelTrait implementation here (in the nested expansion)
        // This is the key difference - EntityTrait is generated in DeriveEntity, not DeriveEntityModel
        // This allows the compiler to resolve Entity::Model in a separate expansion phase
        // By the time this expands, Model and Column should already exist from the parent expansion
        // We use #column_name (an Ident) which should resolve to the Column enum generated earlier
        impl lifeguard::LifeModelTrait for #struct_name {
            type Model = #model_name;
            type Column = #column_name;
        }
    };

    TokenStream::from(expanded)
}
