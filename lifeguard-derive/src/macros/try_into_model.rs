//! Derive macro for `DeriveTryIntoModel` - generates `TryIntoModel` trait implementations
//!
//! This macro generates `TryIntoModel` implementations for converting custom types (DTOs, partial models, etc.)
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
//!
//! # Critical Implementation Details
//!
//! ## Field Attribute Parsing (BUG-2026-01-19-02)
//!
//! **CRITICAL**: Field attributes MUST be extracted in a single pass using `extract_field_attributes()`.
//! 
//! **DO NOT** call `extract_field_attribute()` multiple times (e.g., once for "`map_from`", once for "convert").
//! This causes `parse_nested_meta` to be invoked multiple times on the same attribute, leading to:
//! - Macro expansion failures with "expected `,`" errors
//! - Token consumption issues
//! - Silent error handling problems
//!
//! **Correct Pattern:**
//! ```rust,ignore
//! let (map_from, convert) = extract_field_attributes(field)?;  // ✅ Single pass
//! ```
//!
//! **Anti-Pattern (DO NOT USE):**
//! ```rust,ignore
//! let map_from = extract_field_attribute(field, "map_from")?;  // ❌ First call
//! let convert = extract_field_attribute(field, "convert")?;    // ❌ Second call - BREAKS!
//! ```
//!
//! See `extract_field_attributes()` documentation for detailed explanation.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, Field};

use crate::attributes;

/// Derive macro for `DeriveTryIntoModel` - generates `TryIntoModel` trait implementations
/// 
/// ## Field Attribute Parsing
/// 
/// This macro extracts field attributes (`map_from`, convert) using `extract_field_attributes()`,
/// which MUST be called in a single pass. Do NOT call `extract_field_attribute()` multiple times
/// as this causes `parse_nested_meta` to be invoked multiple times on the same attribute, leading
/// to macro expansion failures. See `extract_field_attributes()` documentation for details.
/// 
/// ## Error Handling
/// 
/// All attribute parsing errors are propagated immediately and converted to compile errors.
/// This ensures users get clear error messages for malformed attributes instead of silent failures.
/// See BUG-2026-01-19-02 for historical context.
#[allow(clippy::too_many_lines)]
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
            }
            return syn::Error::new_spanned(
                &input.ident,
                "DeriveTryIntoModel requires #[lifeguard(model = \"path::to::Model\")] attribute. Found #[lifeguard] but missing model parameter.",
            )
            .to_compile_error()
            .into();
        }
        Err(err) => {
            return err.to_compile_error().into();
        }
    };
    
    // Check if error type is lifeguard::LifeError (default) or a custom error type
    // We need to check the path structure to determine if we should wrap in LifeError::Other
    // CRITICAL: Only match lifeguard::LifeError, not any type ending with ::LifeError
    // (e.g., mymod::LifeError should NOT match)
    let error_type_parsed: syn::Type = match syn::parse2(error_type.clone()) {
        Ok(ty) => ty,
        Err(e) => {
            return syn::Error::new_spanned(
                &input.ident,
                format!("Failed to parse error type: {e}")
            )
            .to_compile_error()
            .into();
        }
    };
    let is_life_error = is_lifeguard_life_error(&error_type_parsed);
    
    // Generate field mapping code
    let mut field_mappings: Vec<TokenStream2> = Vec::new();
    
    for field in fields {
        let field_name = field.ident.as_ref().unwrap();
        
        // CRITICAL: Extract all field attributes in a SINGLE pass.
        // 
        // We MUST use extract_field_attributes() which calls parse_nested_meta ONCE per attribute,
        // not extract_field_attribute() multiple times. Calling parse_nested_meta multiple times
        // on the same attribute causes macro expansion failures (see BUG-2026-01-19-02).
        //
        // This extracts both "map_from" and "convert" in a single parse_nested_meta call,
        // ensuring proper error propagation and avoiding token consumption issues.
        let (map_from, convert_fn) = match extract_field_attributes(field) {
            Ok(attrs) => attrs,
            Err(err) => {
                // Propagate parse errors immediately - malformed attributes should cause compile errors
                return err.to_compile_error().into();
            }
        };
        let _is_optional = attributes::has_attribute(field, "optional");
        
        // Determine target field name
        let target_field_name = if let Some(map_from) = map_from {
            // Custom mapping: use the specified field name
            match syn::parse_str::<syn::Ident>(&map_from) {
                Ok(ident) => ident,
                Err(e) => {
                    return syn::Error::new_spanned(
                        field,
                        format!("Invalid field name in map_from attribute: {e}")
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
                        format!("Invalid conversion function path: {e}")
                    )
                    .to_compile_error()
                    .into();
                }
            };
            
            // If error type is LifeError, wrap in LifeError::Other
            // For custom error types, use map_err with Into::into to convert the error
            // This will work if From<ConversionError> is implemented for CustomError,
            // otherwise it will provide a clearer compile error
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
                // For custom error types, use map_err with Into::into to convert the error
                // This will work if Into<CustomError> is implemented for ConversionError,
                // which is automatically the case if From<ConversionError> is implemented for CustomError.
                // Using map_err with Into::into is more explicit than direct ? and provides
                // clearer error messages if the conversion is not available.
                quote! {
                    #target_field_name: #convert_fn_ident(self.#field_name)
                        .map_err(std::convert::Into::<#error_type>::into)?,
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

/// Check if the error type is specifically `lifeguard::LifeError`
/// 
/// This function examines the path structure to determine if the error type
/// is `lifeguard::LifeError`, not just any type ending with `::LifeError`.
/// 
/// # Returns
/// 
/// - `true` if the error type is `lifeguard::LifeError` or unqualified `LifeError`
/// - `false` for any other error type, including custom error types from other modules
///   (e.g., `mymod::LifeError` should return `false`)
fn is_lifeguard_life_error(error_type: &syn::Type) -> bool {
    match error_type {
        syn::Type::Path(type_path) => {
            let path = &type_path.path;
            
            // Check if path has exactly 2 segments: "lifeguard" and "LifeError"
            if path.segments.len() == 2 {
                let seg1 = &path.segments[0];
                let seg2 = &path.segments[1];
                
                // Check if first segment is "lifeguard" and second is "LifeError"
                if seg1.ident == "lifeguard" && seg2.ident == "LifeError" {
                    // Verify there are no arguments (e.g., LifeError<T>)
                    if let syn::PathArguments::None = &seg2.arguments {
                        return true;
                    }
                }
            }
            
            // Check if path has exactly 1 segment: "LifeError" (unqualified)
            if path.segments.len() == 1 {
                let seg = &path.segments[0];
                if seg.ident == "LifeError" {
                    // Verify there are no arguments (e.g., LifeError<T>)
                    if let syn::PathArguments::None = &seg.arguments {
                        return true;
                    }
                }
            }
            
            false
        }
        _ => false,
    }
}

/// Extract the target Model type from #[lifeguard(model = "...")] attribute
/// Also extracts optional error type from #[lifeguard(error = "...")] attribute
fn extract_model_type(input: &DeriveInput) -> Result<Option<(TokenStream2, TokenStream2)>, syn::Error> {
    let mut model_path_str: Option<String> = None;
    let mut error_path_str: Option<String> = None;
    
    for attr in &input.attrs {
        if attr.path().is_ident("lifeguard") {
            attr.parse_nested_meta(|meta| {
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
            })?;
        }
    }
    
    if let Some(model_path_str) = model_path_str {
        // Parse the model type path
        let model_type: syn::Type = syn::parse_str(&model_path_str)
            .map_err(|e| {
                syn::Error::new_spanned(
                    &input.ident,
                    format!("Invalid model type path '{model_path_str}': {e}")
                )
            })?;
        
        // Parse error type (default to LifeError if not specified)
        let error_type: syn::Type = if let Some(error_path_str) = error_path_str {
            syn::parse_str(&error_path_str)
                .map_err(|e| {
                    syn::Error::new_spanned(
                        &input.ident,
                        format!("Invalid error type path '{error_path_str}': {e}")
                    )
                })?
        } else {
            // Default to LifeError
            syn::parse_str("lifeguard::LifeError")
                .map_err(|e| {
                    syn::Error::new_spanned(
                        &input.ident,
                        format!("Failed to parse default error type: {e}")
                    )
                })?
        };
        
        Ok(Some((quote! { #model_type }, quote! { #error_type })))
    } else {
        Ok(None)
    }
}

/// Extract all field attributes (`map_from`, convert) in a single pass
/// 
/// **CRITICAL: This function MUST extract all attributes in a single `parse_nested_meta` call.**
/// 
/// ## Why Single Pass?
/// 
/// Calling `parse_nested_meta` multiple times on the same attribute can cause:
/// 1. **Macro expansion failures**: The macro may fail to expand, causing "expected `,`" errors
/// 2. **Token consumption issues**: Multiple calls may consume tokens incorrectly
/// 3. **Error handling problems**: Errors may not propagate correctly
/// 
/// ## Historical Context
/// 
/// Previously, we had separate `extract_field_attribute` calls for "`map_from`" and "convert",
/// which called `parse_nested_meta` twice on the same attribute. This caused a regression where
/// valid field attributes like `#[lifeguard(convert = "function")]` would fail with "expected `,`"
/// errors, preventing macro expansion. See BUG-2026-01-19-02 for details.
/// 
/// ## Usage Pattern
/// 
/// This function checks ALL `#[lifeguard(...)]` attributes on the field in a single
/// `parse_nested_meta` call. This allows users to split attributes across multiple
/// `#[lifeguard]` blocks (e.g., `#[lifeguard(map_from = "foo")]` and
/// `#[lifeguard(convert = "bar")]` on separate lines).
/// 
/// ## Error Propagation
/// 
/// This function propagates `parse_nested_meta` errors immediately. If a malformed attribute
/// is detected (e.g., `convert = 123` instead of `convert = "function"`), it returns an error
/// that will be converted to a compile error. This ensures users get clear error messages
/// instead of silent failures.
/// 
/// ## Returns
/// 
/// - `Ok((map_from, convert))` where each is `Some(String)` if found, `None` if not found
/// - `Err(syn::Error)` if a parsing error occurs (e.g., malformed attribute value)
/// 
/// ## Example
/// 
/// ```rust,ignore
/// struct MyStruct {
///     #[lifeguard(convert = "my_function")]  // ✅ Works - single pass extracts this
///     name: String,
///     
///     #[lifeguard(map_from = "foo")]        // ✅ Works - single pass extracts this
///     #[lifeguard(convert = "bar")]          // ✅ Works - single pass extracts both
///     value: String,
/// }
/// ```
/// 
/// ## Anti-Pattern (DO NOT DO THIS)
/// 
/// ```rust,ignore
/// // ❌ WRONG: Don't call parse_nested_meta multiple times
/// let map_from = extract_field_attribute(field, "map_from")?;  // First call
/// let convert = extract_field_attribute(field, "convert")?;    // Second call - BAD!
/// ```
/// 
/// This anti-pattern causes the regression described above.
fn extract_field_attributes(field: &Field) -> Result<(Option<String>, Option<String>), syn::Error> {
    let mut map_from: Option<String> = None;
    let mut convert: Option<String> = None;
    
    // Check all attributes, not just the first one
    // This allows users to split attributes across multiple #[lifeguard] blocks
    for attr in &field.attrs {
        if attr.path().is_ident("lifeguard") {
            // CRITICAL: Parse ALL nested attributes in a SINGLE parse_nested_meta call.
            // 
            // This closure is called once per nested item in the attribute. We handle
            // both "map_from" and "convert" in the same closure, ensuring we only
            // call parse_nested_meta once per #[lifeguard] attribute block.
            //
            // If we need to extract more attributes in the future, add them here
            // rather than creating separate extraction functions.
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("map_from") {
                    // Extract map_from attribute value
                    let lit: syn::LitStr = meta.value()?.parse()?;
                    map_from = Some(lit.value());
                    Ok(())
                } else if meta.path.is_ident("convert") {
                    // Extract convert attribute value
                    let lit: syn::LitStr = meta.value()?.parse()?;
                    convert = Some(lit.value());
                    Ok(())
                } else {
                    // Unknown attribute - skip it (don't error on unknown attributes)
                    // This allows for future extensibility without breaking existing code
                    Ok(())
                }
            })?;
        }
    }
    
    Ok((map_from, convert))
}

/// Extract a field attribute value (e.g., `map_from`, convert)
/// 
/// This function checks ALL #[lifeguard(...)] attributes on the field,
/// not just the first one. This allows users to split attributes across
/// multiple #[lifeguard] blocks (e.g., #[`lifeguard(map_from` = "foo")] and
/// #[lifeguard(convert = "bar")] on separate lines).
/// 
/// Returns `Ok(Some(String))` if the attribute is found, `Ok(None)` if not found,
/// or `Err(syn::Error)` if a parsing error occurs (e.g., malformed attribute value).
#[allow(dead_code)]
fn extract_field_attribute(field: &Field, attr_name: &str) -> Result<Option<String>, syn::Error> {
    // Check all attributes, not just the first one
    for attr in &field.attrs {
        if attr.path().is_ident("lifeguard") {
            let mut value: Option<String> = None;
            // Check result and propagate errors instead of silently ignoring them
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident(attr_name) {
                    let lit: syn::LitStr = meta.value()?.parse()?;
                    value = Some(lit.value());
                    Ok(())
                } else {
                    Ok(())
                }
            })?;
            // If we found the requested attribute, return it
            // Otherwise, continue checking other attributes
            if value.is_some() {
                return Ok(value);
            }
        }
    }
    Ok(None)
}
