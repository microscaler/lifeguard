//! LifeRecord derive macro implementation

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DataStruct, DeriveInput, Fields, Ident};

use crate::attributes;
use crate::utils;

/// Derive macro for `LifeRecord` - generates mutable change-set objects
///
/// This macro generates:
/// - `Record` struct (mutable change-set with Option<T> fields)
/// - `from_model()` method (create from LifeModel for updates)
/// - `to_model()` method (convert to LifeModel, None fields use defaults)
/// - `dirty_fields()` method (returns list of changed fields)
/// - `is_dirty()` method (checks if any fields changed)
/// - Setter methods for each field
pub fn derive_life_record(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    // Extract struct name
    let struct_name = &input.ident;
    let record_name = Ident::new(&format!("{}Record", struct_name), struct_name.span());
    let model_name = Ident::new(&format!("{}Model", struct_name), struct_name.span());

    // Extract struct fields
    let fields = match &input.data {
        Data::Struct(DataStruct {
            fields: Fields::Named(fields),
            ..
        }) => &fields.named,
        _ => {
            return syn::Error::new(
                struct_name.span(),
                "LifeRecord can only be derived for structs with named fields",
            )
            .to_compile_error()
            .into();
        }
    };

    // Extract table name from attributes (not used in simplified version)
    let _table_name = attributes::extract_table_name(&input.attrs)
        .unwrap_or_else(|| utils::snake_case(&struct_name.to_string()));

    // Process fields
    let mut record_fields = Vec::new();
    let mut record_field_names = Vec::new();
    let mut from_model_fields = Vec::new();
    let mut to_model_fields = Vec::new();
    let mut dirty_fields_check = Vec::new();
    let mut setter_methods = Vec::new();

    for field in fields.iter() {
        let field_name = field.ident.as_ref().unwrap();
        let field_type = &field.ty;

        // Check if field is nullable (has #[nullable] attribute)
        let is_nullable = attributes::has_attribute(field, "nullable");

        // Generate record field (Option<T>)
        record_fields.push(quote! {
            pub #field_name: Option<#field_type>,
        });

        // Store field name for struct initialization
        record_field_names.push(field_name);

        // Generate from_model field assignment
        from_model_fields.push(quote! {
            #field_name: Some(model.#field_name.clone()),
        });

        // Generate to_model field extraction
        // For inserts, None fields use defaults (or panic if required)
        if is_nullable {
            to_model_fields.push(quote! {
                #field_name: self.#field_name.clone().unwrap_or_default(),
            });
        } else {
            to_model_fields.push(quote! {
                #field_name: self.#field_name.clone().expect(&format!("Field {} is required but not set", stringify!(#field_name))),
            });
        }

        // Generate dirty field check
        dirty_fields_check.push(quote! {
            if self.#field_name.is_some() {
                dirty.push(stringify!(#field_name).to_string());
            }
        });

        // Generate setter method
        let setter_name = Ident::new(&format!("set_{}", field_name), field_name.span());
        setter_methods.push(quote! {
            /// Set the #field_name field
            pub fn #setter_name(&mut self, value: #field_type) -> &mut Self {
                self.#field_name = Some(value);
                self
            }
        });
    }

    // Generate the expanded code
    let expanded = quote! {
        // Record struct (mutable change-set)
        #[derive(Debug, Clone)]
        pub struct #record_name {
            #(#record_fields)*
        }

        impl #record_name {
            /// Create a new empty record (all fields None)
            /// Useful for inserts where you set only the fields you need
            pub fn new() -> Self {
                Self {
                    #(
                        #record_field_names: None,
                    )*
                }
            }

            /// Create a record from a Model (for updates)
            /// All fields are set to Some(value) from the model
            pub fn from_model(model: &#model_name) -> Self {
                Self {
                    #(#from_model_fields)*
                }
            }

            /// Convert the record to a Model
            /// None fields use defaults (Default::default() for nullable, panic for required)
            /// For inserts, ensure all required fields are set before calling this
            pub fn to_model(&self) -> #model_name {
                #model_name {
                    #(#to_model_fields)*
                }
            }

            /// Get a list of dirty (changed) field names
            /// Returns a vector of field names that have been set (are Some)
            pub fn dirty_fields(&self) -> Vec<String> {
                let mut dirty = Vec::new();
                #(#dirty_fields_check)*
                dirty
            }

            /// Check if any fields have been changed
            /// Returns true if at least one field is Some
            pub fn is_dirty(&self) -> bool {
                !self.dirty_fields().is_empty()
            }

            #(#setter_methods)*
        }

        impl Default for #record_name {
            fn default() -> Self {
                Self::new()
            }
        }
    };

    TokenStream::from(expanded)
}
