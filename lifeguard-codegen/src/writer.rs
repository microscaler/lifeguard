//! Code generation writer

use crate::entity::EntityDefinition;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{GenericArgument, PathArguments, Type};

pub struct EntityWriter;

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

impl EntityWriter {
    pub fn new() -> Self {
        Self
    }

    /// Generate complete entity code
    pub fn generate_entity_code(
        &self,
        entity: &EntityDefinition,
        expanded: bool,
    ) -> anyhow::Result<String> {
        let code = if expanded {
            self.generate_expanded(entity)
        } else {
            self.generate_compact(entity)
        };

        // Format the code
        let formatted = format_code(&code.to_string())?;
        Ok(formatted)
    }

    /// Generate expanded format (full code with all implementations)
    fn generate_expanded(&self, entity: &EntityDefinition) -> TokenStream {
        let entity_name = &entity.name;
        let model_name = entity.model_name();
        let table_name = &entity.table_name;
        let column_variants = entity.column_variants();
        let primary_key_variants = entity.primary_key_variants();

        // Generate model fields
        let model_fields = entity.fields.iter().map(|f| {
            let field_name = &f.name;
            let field_type = &f.ty;
            quote! {
                pub #field_name: #field_type,
            }
        });

        // Generate column match arms for Iden
        let iden_match_arms: Vec<_> = entity
            .fields
            .iter()
            .zip(column_variants.iter())
            .map(|(f, variant)| {
                let column_name_str = f
                    .column_name
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(|| f.name.to_string());
                let column_name_lit = column_name_str.as_str();
                quote! {
                    Column::#variant => #column_name_lit,
                }
            })
            .collect();

        // Generate from_row fields
        // IMPORTANT: Use row.try_get()? for ALL fields (not row.get()) to match proc-macro behavior
        // This ensures graceful error handling instead of panics on NULL values, missing columns, or type mismatches
        let from_row_fields = entity.fields.iter().map(|f| {
            let field_name = &f.name;
            let field_type = &f.ty;
            let column_name_str = f
                .column_name
                .as_ref()
                .cloned()
                .unwrap_or_else(|| f.name.to_string());
            let column_name_lit = column_name_str.as_str();

            // Generate get expression - use try_get()? for all fields to match proc-macro behavior
            let get_expr = {
                // Handle unsigned integer types (need to convert to signed first, then cast back)
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
                    // For unsigned types, convert to signed equivalent first
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
                            let val: #signed_type = row.try_get::<&str, #signed_type>(#column_name_lit)?;
                            val as #field_type
                        }
                    }
                } else {
                    // For all other types (including Option<T>), use try_get()?
                    quote! {
                        row.try_get::<&str, #field_type>(#column_name_lit)?
                    }
                }
            };

            quote! {
                #field_name: #get_expr,
            }
        });

        // Generate primary key value expression
        // Match the comprehensive type handling from life_model.rs
        let primary_key_field = entity.fields.iter()
            .find(|f| f.is_primary_key)
            .map(|f| {
                let field_name = &f.name;
                let field_type = &f.ty;

                match field_type {
                    syn::Type::Path(type_path) => {
                        if let Some(first_segment) = type_path.path.segments.first() {
                            let ident_str = first_segment.ident.to_string();
                            match ident_str.as_str() {
                                "i32" => quote! { sea_query::Value::Int(Some(self.#field_name)) },
                                "i64" => quote! { sea_query::Value::BigInt(Some(self.#field_name)) },
                                "i16" => quote! { sea_query::Value::SmallInt(Some(self.#field_name)) },
                                "u8" => quote! { sea_query::Value::SmallInt(Some(self.#field_name as i16)) },
                                "u16" => quote! { sea_query::Value::Int(Some(self.#field_name as i32)) },
                                "u32" => quote! { sea_query::Value::BigInt(Some(self.#field_name as i64)) },
                                "u64" => quote! { sea_query::Value::BigInt(Some(self.#field_name as i64)) },
                                "String" => quote! { sea_query::Value::String(Some(self.#field_name.clone())) },
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
                }
            })
            .unwrap_or_else(|| quote! { sea_query::Value::String(None) });

        // Generate ModelTrait::get() match arms
        // Match the comprehensive type handling from life_model.rs
        let model_get_match_arms = entity.fields.iter().zip(column_variants.iter()).map(|(f, variant)| {
            let field_name = &f.name;
            let field_type = &f.ty;

            let value_expr = match field_type {
                syn::Type::Path(type_path) => {
                    if let Some(first_segment) = type_path.path.segments.first() {
                        let ident_str = first_segment.ident.to_string();
                        match ident_str.as_str() {
                            "i32" => quote! { sea_query::Value::Int(Some(self.#field_name)) },
                            "i64" => quote! { sea_query::Value::BigInt(Some(self.#field_name)) },
                            "i16" => quote! { sea_query::Value::SmallInt(Some(self.#field_name)) },
                            "u8" => quote! { sea_query::Value::SmallInt(Some(self.#field_name as i16)) },
                            "u16" => quote! { sea_query::Value::Int(Some(self.#field_name as i32)) },
                            "u32" => quote! { sea_query::Value::BigInt(Some(self.#field_name as i64)) },
                            "u64" => quote! { sea_query::Value::BigInt(Some(self.#field_name as i64)) },
                            "f32" => quote! { sea_query::Value::Float(Some(self.#field_name)) },
                            "f64" => quote! { sea_query::Value::Double(Some(self.#field_name)) },
                            "bool" => quote! { sea_query::Value::Bool(Some(self.#field_name)) },
                            "String" => quote! { sea_query::Value::String(Some(self.#field_name.clone())) },
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
                            _ => quote! { sea_query::Value::String(None) },
                        }
                    } else {
                        quote! { sea_query::Value::String(None) }
                    }
                }
                _ => quote! { sea_query::Value::String(None) },
            };

            quote! {
                Column::#variant => #value_expr,
            }
        });

        quote! {
            // Generated by lifeguard-codegen
            // This file is generated - do not edit manually

            use lifeguard::{LifeModelTrait, LifeEntityName, ModelTrait, FromRow};

            // Entity unit struct
            #[derive(Copy, Clone, Default, Debug)]
            pub struct #entity_name;

            // LifeEntityName implementation
            impl LifeEntityName for #entity_name {
                fn table_name(&self) -> &'static str {
                    #table_name
                }
            }

            // Iden implementation for Entity
            impl sea_query::Iden for #entity_name {
                fn unquoted(&self) -> &str {
                    #table_name
                }
            }

            // Column enum
            #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
            pub enum Column {
                #(#column_variants,)*
            }

            // Iden implementation for Column
            impl sea_query::Iden for Column {
                fn unquoted(&self) -> &str {
                    match self {
                        #(#iden_match_arms)*
                    }
                }
            }

            // PrimaryKey enum
            #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
            pub enum PrimaryKey {
                #(#primary_key_variants,)*
            }

            // Model struct
            #[derive(Debug, Clone)]
            pub struct #model_name {
                #(#model_fields)*
            }

            // FromRow implementation
            impl FromRow for #model_name {
                fn from_row(row: &may_postgres::Row) -> Result<Self, may_postgres::Error> {
                    Ok(Self {
                        #(#from_row_fields)*
                    })
                }
            }

            // ModelTrait implementation
            impl ModelTrait for #model_name {
                type Entity = #entity_name;

                fn get(&self, column: <Self::Entity as LifeModelTrait>::Column) -> sea_query::Value {
                    match column {
                        #(#model_get_match_arms)*
                    }
                }

                fn get_primary_key_value(&self) -> sea_query::Value {
                    #primary_key_field
                }
            }

            // LifeModelTrait implementation
            impl LifeModelTrait for #entity_name {
                type Model = #model_name;
                type Column = Column;
            }

            // Table name constant
            impl #entity_name {
                pub const TABLE_NAME: &'static str = #table_name;
            }
        }
    }

    /// Generate compact format (minimal code)
    fn generate_compact(&self, entity: &EntityDefinition) -> TokenStream {
        // For now, use expanded format
        self.generate_expanded(entity)
    }
}

/// Format Rust code using rustfmt
fn format_code(code: &str) -> anyhow::Result<String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    // Try to use rustfmt if available
    let mut child = match Command::new("rustfmt")
        .args(&["--edition", "2021", "--emit", "stdout"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => {
            // rustfmt not available, try cargo fmt
            return format_with_cargo_fmt(code);
        }
    };

    // Write code to rustfmt stdin
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(code.as_bytes())?;
        stdin.flush()?;
    }

    let output = child.wait_with_output()?;

    if output.status.success() {
        Ok(String::from_utf8(output.stdout)?)
    } else {
        // rustfmt failed, try cargo fmt or return unformatted
        format_with_cargo_fmt(code)
    }
}

/// Format code using cargo fmt (fallback)
fn format_with_cargo_fmt(code: &str) -> anyhow::Result<String> {
    use std::fs;
    use std::process::Command;

    // Create a temporary file
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join(format!("lifeguard_codegen_{}.rs", std::process::id()));

    // Write code to temp file
    fs::write(&temp_file, code)?;

    // Try cargo fmt
    let output = Command::new("cargo")
        .args(&["fmt", "--", temp_file.to_str().unwrap()])
        .output();

    match output {
        Ok(result) if result.status.success() => {
            // Read formatted code
            let formatted = fs::read_to_string(&temp_file)?;
            fs::remove_file(&temp_file).ok(); // Clean up
            Ok(formatted)
        }
        _ => {
            // Both rustfmt and cargo fmt failed, return unformatted
            fs::remove_file(&temp_file).ok(); // Clean up
            Ok(code.to_string())
        }
    }
}
