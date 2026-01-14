//! LifeModel derive macro implementation
//!
//! Based on SeaORM's expand_derive_entity_model pattern (v2.0.0-rc.28)
//! Generates Entity, Column, PrimaryKey, Model, FromRow, and LifeModelTrait

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DataStruct, DeriveInput, Fields, Ident, GenericArgument, PathArguments, Type};

use crate::attributes;
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

/// Derive macro for `LifeModel` - generates Entity, Model, Column, PrimaryKey, and FromRow
///
/// This macro follows SeaORM's pattern exactly:
/// 1. Generates Entity struct with #[derive(DeriveEntity)] (triggers nested expansion)
/// 2. Generates Column enum
/// 3. Generates PrimaryKey enum  
/// 4. Generates Model struct
/// 5. Generates FromRow implementation for Model
/// 6. DeriveEntity (nested) generates LifeModelTrait for Entity
pub fn derive_life_model(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    // Extract struct name and table name
    let struct_name = &input.ident;
    let table_name = attributes::extract_table_name(&input.attrs)
        .unwrap_or_else(|| utils::snake_case(&struct_name.to_string()));
    let table_name_lit = syn::LitStr::new(&table_name, struct_name.span());

    // Extract fields
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

    // Generate Model name
    let model_name = Ident::new(&format!("{}Model", struct_name), struct_name.span());
    let _model_name_lit = syn::LitStr::new(&model_name.to_string(), model_name.span());

    // Process fields to generate:
    // - Column enum variants
    // - PrimaryKey enum variants
    // - Model struct fields
    // - FromRow field extraction
    // - ModelTrait get() match arms
    // - Primary key field tracking
    let mut column_variants = Vec::new();
    let mut primary_key_variants = Vec::new();
    let mut model_fields = Vec::new();
    let mut from_row_fields = Vec::new();
    let mut iden_impls = Vec::new();
    let mut model_get_match_arms = Vec::new();
    let mut primary_key_value_expr: Option<proc_macro2::TokenStream> = None;

    for field in fields.iter() {
        let field_name = field.ident.as_ref().unwrap();
        let field_type = &field.ty;
        let column_name = attributes::extract_column_name(field)
            .unwrap_or_else(|| utils::snake_case(&field_name.to_string()));

        // Check if primary key
        let is_primary_key = attributes::has_attribute(field, "primary_key");

        // Generate Column enum variant (PascalCase)
        let column_variant = Ident::new(
            &utils::pascal_case(&field_name.to_string()),
            field_name.span(),
        );
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
            // Track primary key field for ModelTrait::get_primary_key_value()
            // Generate the value conversion expression now
            if primary_key_value_expr.is_none() {
                let pk_value_expr = match field_type {
                    syn::Type::Path(syn::TypePath {
                        path: syn::Path { segments, .. },
                        ..
                    }) => {
                        if let Some(segment) = segments.first() {
                            let ident_str = segment.ident.to_string();
                            match ident_str.as_str() {
                                "i32" => quote! { sea_query::Value::Int(Some(self.#field_name)) },
                                "i64" => {
                                    quote! { sea_query::Value::BigInt(Some(self.#field_name)) }
                                }
                                "i16" => {
                                    quote! { sea_query::Value::SmallInt(Some(self.#field_name)) }
                                }
                                "u8" => {
                                    quote! { sea_query::Value::SmallInt(Some(self.#field_name as i16)) }
                                }
                                "u16" => {
                                    quote! { sea_query::Value::Int(Some(self.#field_name as i32)) }
                                }
                                "u32" => {
                                    quote! { sea_query::Value::BigInt(Some(self.#field_name as i64)) }
                                }
                                "u64" => {
                                    quote! { sea_query::Value::BigInt(Some(self.#field_name as i64)) }
                                }
                                "String" => {
                                    quote! { sea_query::Value::String(Some(self.#field_name.clone())) }
                                }
                                "Option" => {
                                    // Handle Option<T> for primary key - extract inner type from generic arguments
                                    if let Some(inner_type) = extract_option_inner_type(field_type) {
                                        // Match on the inner type
                                        if let Type::Path(inner_path) = inner_type {
                                            if let Some(inner_segment) = inner_path.path.segments.last() {
                                                let inner_ident = inner_segment.ident.to_string();
                                                match inner_ident.as_str() {
                                                    "i32" => quote! { self.#field_name.map(|v| sea_query::Value::Int(Some(v))).unwrap_or(sea_query::Value::Int(None)) },
                                                    "i64" => quote! { self.#field_name.map(|v| sea_query::Value::BigInt(Some(v))).unwrap_or(sea_query::Value::BigInt(None)) },
                                                    "i16" => quote! { self.#field_name.map(|v| sea_query::Value::SmallInt(Some(v))).unwrap_or(sea_query::Value::SmallInt(None)) },
                                                    "String" => quote! { self.#field_name.as_ref().map(|v| sea_query::Value::String(Some(v.clone()))).unwrap_or(sea_query::Value::String(None)) },
                                                    _ => quote! { sea_query::Value::String(None) },
                                                }
                                            } else {
                                                quote! { sea_query::Value::String(None) }
                                            }
                                        } else {
                                            quote! { sea_query::Value::String(None) }
                                        }
                                    } else {
                                        quote! { sea_query::Value::String(None) }
                                    }
                                }
                                _ => quote! { sea_query::Value::String(None) },
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

        // Generate Model field
        model_fields.push(quote! {
            pub #field_name: #field_type,
        });

        // Generate ModelTrait::get() match arm
        // Convert field value to sea_query::Value
        let field_value_to_value = match field_type {
            syn::Type::Path(syn::TypePath {
                path: syn::Path { segments, .. },
                ..
            }) => {
                if let Some(segment) = segments.first() {
                    let ident_str = segment.ident.to_string();
                    match ident_str.as_str() {
                        "i32" => quote! { sea_query::Value::Int(Some(self.#field_name)) },
                        "i64" => quote! { sea_query::Value::BigInt(Some(self.#field_name)) },
                        "i16" => quote! { sea_query::Value::SmallInt(Some(self.#field_name)) },
                        "u8" => {
                            quote! { sea_query::Value::SmallInt(Some(self.#field_name as i16)) }
                        }
                        "u16" => quote! { sea_query::Value::Int(Some(self.#field_name as i32)) },
                        "u32" => quote! { sea_query::Value::BigInt(Some(self.#field_name as i64)) },
                        "u64" => quote! { sea_query::Value::BigInt(Some(self.#field_name as i64)) },
                        "f32" => quote! { sea_query::Value::Float(Some(self.#field_name)) },
                        "f64" => quote! { sea_query::Value::Double(Some(self.#field_name)) },
                        "bool" => quote! { sea_query::Value::Bool(Some(self.#field_name)) },
                        "String" => {
                            quote! { sea_query::Value::String(Some(self.#field_name.clone())) }
                        }
                        "Option" => {
                            // Handle Option<T> - extract inner type from generic arguments
                            if let Some(inner_type) = extract_option_inner_type(field_type) {
                                // Match on the inner type
                                if let Type::Path(inner_path) = inner_type {
                                    if let Some(inner_segment) = inner_path.path.segments.last() {
                                        let inner_ident = inner_segment.ident.to_string();
                                        match inner_ident.as_str() {
                                            "i32" => quote! { self.#field_name.map(|v| sea_query::Value::Int(Some(v))).unwrap_or(sea_query::Value::Int(None)) },
                                            "i64" => quote! { self.#field_name.map(|v| sea_query::Value::BigInt(Some(v))).unwrap_or(sea_query::Value::BigInt(None)) },
                                            "i16" => quote! { self.#field_name.map(|v| sea_query::Value::SmallInt(Some(v))).unwrap_or(sea_query::Value::SmallInt(None)) },
                                            "f32" => quote! { self.#field_name.map(|v| sea_query::Value::Float(Some(v))).unwrap_or(sea_query::Value::Float(None)) },
                                            "f64" => quote! { self.#field_name.map(|v| sea_query::Value::Double(Some(v))).unwrap_or(sea_query::Value::Double(None)) },
                                            "bool" => quote! { self.#field_name.map(|v| sea_query::Value::Bool(Some(v))).unwrap_or(sea_query::Value::Bool(None)) },
                                            "String" => quote! { self.#field_name.as_ref().map(|v| sea_query::Value::String(Some(v.clone()))).unwrap_or(sea_query::Value::String(None)) },
                                            _ => quote! { sea_query::Value::String(None) },
                                        }
                                    } else {
                                        quote! { sea_query::Value::String(None) }
                                    }
                                } else {
                                    quote! { sea_query::Value::String(None) }
                                }
                            } else {
                                quote! { sea_query::Value::String(None) }
                            }
                        }
                        _ => quote! { sea_query::Value::String(None) }, // Fallback for unknown types
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

        // Generate FromRow field extraction
        let column_name_str = column_name.as_str();
        let get_expr = {
            // Handle unsigned integer types
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
                let signed_type = match field_type {
                    syn::Type::Path(syn::TypePath {
                        path: syn::Path { segments, .. },
                        ..
                    }) => {
                        if let Some(segment) = segments.first() {
                            match segment.ident.to_string().as_str() {
                                "u8" => quote! { i16 },
                                "u16" => quote! { i32 },
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
                        val as #field_type
                    }
                }
            } else {
                quote! {
                    row.try_get::<&str, #field_type>(#column_name_str)?
                }
            }
        };

        from_row_fields.push(quote! {
            #field_name: #get_expr,
        });
    }

    // Generate primary key value expression for ModelTrait
    let pk_value_impl = primary_key_value_expr
        .as_ref()
        .map(|expr| {
            quote! {
                #expr
            }
        })
        .unwrap_or_else(|| {
            quote! {
                sea_query::Value::String(None) // No primary key found
            }
        });

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

        // Create a type alias to ensure Column is fully resolved before DeriveEntity expands
        // This helps the compiler resolve Column during nested macro expansion
        type _ColumnAlias = Column;

        // STEP 2: Generate Entity struct (unit struct, like SeaORM)
        #[doc = " Generated by lifeguard-derive"]
        #[derive(Copy, Clone, Debug, Default)]
        pub struct Entity;

        // Table name constant (for convenience, matches Entity::table_name())
        impl Entity {
            pub const TABLE_NAME: &'static str = #table_name_lit;
        }

        // Implement LifeEntityName for Entity (provides table_name method)
        impl lifeguard::LifeEntityName for Entity {
            fn table_name(&self) -> &'static str {
                #table_name_lit
            }
        }

        // Implement Iden for Entity (for use in sea_query)
        impl sea_query::Iden for Entity {
            fn unquoted(&self) -> &str {
                #table_name_lit
            }
        }

        // Implement IdenStatic for Entity (for use in sea_query)
        impl sea_query::IdenStatic for Entity {
            fn as_str(&self) -> &'static str {
                #table_name_lit
            }
        }

        // CRITICAL: Generate LifeModelTrait implementation directly here
        // This avoids nested macro expansion issues (E0223)
        // Column and Model are now fully defined, so we can reference them directly
        impl lifeguard::LifeModelTrait for Entity {
            type Model = #model_name;
            type Column = Column;  // Direct reference to Column enum defined above
        }

        // STEP 3: Generate PrimaryKey enum
        #[doc = " Generated by lifeguard-derive"]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum PrimaryKey {
            #(#primary_key_variants)*
        }

        // STEP 5: Generate Model struct (like SeaORM's expand_derive_model)
        #[doc = " Generated by lifeguard-derive"]
        #[derive(Debug, Clone)]
        pub struct #model_name {
            #(#model_fields)*
        }

        // STEP 6: Generate FromRow implementation (automatic, no separate derive needed)
        #[automatically_derived]
        impl lifeguard::FromRow for #model_name {
            fn from_row(row: &may_postgres::Row) -> Result<Self, may_postgres::Error> {
                Ok(Self {
                    #(#from_row_fields)*
                })
            }
        }

        // STEP 7: Generate ModelTrait implementation
        #[automatically_derived]
        impl lifeguard::ModelTrait for #model_name {
            type Entity = Entity;

            fn get(&self, column: Entity::Column) -> sea_query::Value {
                match column {
                    #(#model_get_match_arms)*
                }
            }

            fn get_primary_key_value(&self) -> sea_query::Value {
                #pk_value_impl
            }
        }

        // STEP 8: LifeModelTrait is generated by DeriveEntity (nested expansion)
        // This happens in a separate expansion phase, allowing proper type resolution
        // DeriveEntity sets both type Model and type Column using the identifiers passed via attributes
    };

    TokenStream::from(expanded)
}
