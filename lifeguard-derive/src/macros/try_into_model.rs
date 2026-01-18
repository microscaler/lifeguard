//! Derive macro for `DeriveTryIntoModel` - generates TryIntoModel trait implementations
//!
//! This macro generates TryIntoModel implementations for converting custom types (DTOs, partial models, etc.)
//! into Model instances with proper error handling.
//!
//! # Example
//!
//! ```ignore
//! use lifeguard_derive::DeriveTryIntoModel;
//!
//! #[derive(DeriveTryIntoModel)]
//! #[lifeguard(model = "UserModel")]
//! struct CreateUserRequest {
//!     name: String,
//!     email: String,
//!     // Missing fields (id, etc.) will use Default::default()
//! }
//!
//! // The macro generates:
//! // impl TryIntoModel<UserModel> for CreateUserRequest {
//! //     type Error = LifeError;
//! //     fn try_into_model(self) -> Result<UserModel, LifeError> { ... }
//! // }
//! ```

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, Field};

use crate::attributes;

/// Derive macro for `DeriveTryIntoModel` - generates TryIntoModel trait implementations
pub fn derive_try_into_model(input: TokenStream) -> TokenStream {
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
                "DeriveTryIntoModel can only be derived for structs with named fields",
            )
            .to_compile_error()
            .into();
        }
    };
    
    // Extract target Model type from attribute
    let (model_type, error_type) = match extract_model_type(&input) {
        Ok(Some((model, error))) => (model, error),
        Ok(None) => {
            // Check if there's a lifeguard attribute at all
            let has_lifeguard_attr = input.attrs.iter().any(|attr| attr.path().is_ident("lifeguard"));
            if !has_lifeguard_attr {
                return syn::Error::new_spanned(
                    &input.ident,
                    "DeriveTryIntoModel requires #[lifeguard(model = \"path::to::Model\")] attribute. No #[lifeguard] attribute found.",
                )
                .to_compile_error()
                .into();
            } else {
                return syn::Error::new_spanned(
                    &input.ident,
                    "DeriveTryIntoModel requires #[lifeguard(model = \"path::to::Model\")] attribute. Found #[lifeguard] but missing model parameter.",
                )
                .to_compile_error()
                .into();
            }
        }
        Err(err) => {
            return err.to_compile_error().into();
        }
    };
    
    // Check if error type is LifeError (default) or a custom error type
    // We need to compare the string representation to determine if we should wrap in LifeError::Other
    let error_type_str = error_type.to_string();
    let is_life_error = error_type_str == "lifeguard::LifeError" 
        || error_type_str == "LifeError"
        || error_type_str.ends_with("::LifeError");
    
    // Generate field mapping code
    let mut field_mappings: Vec<TokenStream2> = Vec::new();
    
    for field in fields.iter() {
        let field_name = field.ident.as_ref().unwrap();
        let _field_type = &field.ty;
        
        // Extract field attributes
        let map_from = extract_field_attribute(field, "map_from");
        let convert_fn = extract_field_attribute(field, "convert");
        let _is_optional = attributes::has_attribute(field, "optional");
        
        // Determine target field name
        let target_field_name = if let Some(map_from) = map_from {
            // Custom mapping: use the specified field name
            match syn::parse_str::<syn::Ident>(&map_from) {
                Ok(ident) => ident,
                Err(e) => {
                    return syn::Error::new_spanned(
                        field,
                        format!("Invalid field name in map_from attribute: {}", e)
                    )
                    .to_compile_error()
                    .into();
                }
            }
        } else {
            // Direct mapping: use the same field name
            field_name.clone()
        };
        
        // Generate field assignment
        let field_assignment = if let Some(convert_fn) = convert_fn {
            // Custom conversion function
            let convert_fn_ident = match syn::parse_str::<syn::Path>(&convert_fn) {
                Ok(path) => path,
                Err(e) => {
                    return syn::Error::new_spanned(
                        field,
                        format!("Invalid conversion function path: {}", e)
                    )
                    .to_compile_error()
                    .into();
                }
            };
            
            // If error type is LifeError, wrap in LifeError::Other
            // Otherwise, use ? directly which requires CustomError: From<ConversionError>
            if is_life_error {
                quote! {
                    #target_field_name: #convert_fn_ident(self.#field_name)
                        .map_err(|e| lifeguard::LifeError::Other(format!(
                            "Failed to convert field '{}': {}",
                            stringify!(#field_name),
                            e
                        )))?,
                }
            } else {
                // For custom error types, use ? directly
                // This requires CustomError: From<ConversionError>
                quote! {
                    #target_field_name: #convert_fn_ident(self.#field_name)?,
                }
            }
        } else {
            // Direct field mapping
            quote! {
                #target_field_name: self.#field_name,
            }
        };
        
        field_mappings.push(field_assignment);
    }
    
    // Generate the TryIntoModel implementation
    // Use ..Default::default() to handle missing fields
    // This requires the Model type to implement Default
    let expanded = quote! {
        impl lifeguard::TryIntoModel<#model_type> for #struct_name {
            type Error = #error_type;
            
            fn try_into_model(self) -> Result<#model_type, Self::Error> {
                Ok(#model_type {
                    #(#field_mappings)*
                    ..Default::default()
                })
            }
        }
    };
    
    TokenStream::from(expanded)
}

/// Extract the target Model type from #[lifeguard(model = "...")] attribute
/// Also extracts optional error type from #[lifeguard(error = "...")] attribute
fn extract_model_type(input: &DeriveInput) -> Result<Option<(TokenStream2, TokenStream2)>, syn::Error> {
    let mut model_path_str: Option<String> = None;
    let mut error_path_str: Option<String> = None;
    
    for attr in &input.attrs {
        if attr.path().is_ident("lifeguard") {
            if let Err(err) = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("model") {
                    let value: syn::LitStr = meta.value()?.parse()?;
                    model_path_str = Some(value.value());
                    Ok(())
                } else if meta.path.is_ident("error") {
                    let value: syn::LitStr = meta.value()?.parse()?;
                    error_path_str = Some(value.value());
                    Ok(())
                } else {
                    Ok(())
                }
            }) {
                return Err(err);
            }
        }
    }
    
    if let Some(model_path_str) = model_path_str {
        // Parse the model type path
        let model_type: syn::Type = syn::parse_str(&model_path_str)
            .map_err(|e| {
                syn::Error::new_spanned(
                    &input.ident,
                    format!("Invalid model type path '{}': {}", model_path_str, e)
                )
            })?;
        
        // Parse error type (default to LifeError if not specified)
        let error_type: syn::Type = if let Some(error_path_str) = error_path_str {
            syn::parse_str(&error_path_str)
                .map_err(|e| {
                    syn::Error::new_spanned(
                        &input.ident,
                        format!("Invalid error type path '{}': {}", error_path_str, e)
                    )
                })?
        } else {
            // Default to LifeError
            syn::parse_str("lifeguard::LifeError")
                .map_err(|e| {
                    syn::Error::new_spanned(
                        &input.ident,
                        format!("Failed to parse default error type: {}", e)
                    )
                })?
        };
        
        Ok(Some((quote! { #model_type }, quote! { #error_type })))
    } else {
        Ok(None)
    }
}

/// Extract a field attribute value (e.g., map_from, convert)
fn extract_field_attribute(field: &Field, attr_name: &str) -> Option<String> {
    for attr in &field.attrs {
        if attr.path().is_ident("lifeguard") {
            let mut value: Option<String> = None;
            if attr.parse_nested_meta(|meta| {
                if meta.path.is_ident(attr_name) {
                    let lit: syn::LitStr = meta.value()?.parse()?;
                    value = Some(lit.value());
                    Ok(())
                } else {
                    Ok(())
                }
            }).is_ok() {
                return value;
            }
        }
    }
    None
}
