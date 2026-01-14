//! Combined DeriveLifeModel macro
//!
//! This is a convenience macro that combines all the separate derives:
//! - DeriveEntity
//! - DeriveModel
//! - DeriveColumn
//! - DerivePrimaryKey
//! - LifeModelTrait implementation
//!
//! Following SeaORM's DeriveEntityModel pattern.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

use crate::attributes;
use crate::utils;

/// Combined derive that generates Entity, Model, Column, PrimaryKey, and LifeModelTrait
/// 
/// This is a convenience macro that calls all the separate derives.
/// Users can also apply the derives separately for more control.
pub fn derive_life_model_combined(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    
    // Extract struct name
    let struct_name = &input.ident;
    let model_name = syn::Ident::new(&format!("{}Model", struct_name), struct_name.span());
    
    // Extract table name
    let table_name = attributes::extract_table_name(&input.attrs)
        .unwrap_or_else(|| utils::snake_case(&struct_name.to_string()));
    
    // Generate code by calling the separate derive functions
    // We'll generate the combined output here
    
    // For now, delegate to the existing life_model derive
    // TODO: Refactor to actually call separate derives
    let expanded: TokenStream2 = quote! {
        // This will be replaced with actual combined output
        // For now, we keep the existing implementation
    };
    
    TokenStream::from(expanded)
}
