//! Input parsing for entity definitions

use crate::entity::{EntityDefinition, FieldDefinition};
use std::fs;
use std::path::Path;
use syn::{Attribute, Field, Ident, Lit, Type};

#[derive(Debug, serde::Deserialize)]
struct EntityConfig {
    name: String,
    table_name: Option<String>,
    fields: Vec<FieldConfig>,
}

#[derive(Debug, serde::Deserialize)]
struct FieldConfig {
    name: String,
    #[serde(rename = "type")]
    type_str: String,
    primary_key: Option<bool>,
    column_name: Option<String>,
    nullable: Option<bool>,
    auto_increment: Option<bool>,
}

pub fn parse_entity_from_file(path: &Path) -> anyhow::Result<EntityDefinition> {
    let content = fs::read_to_string(path)?;
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");

    match ext {
        "rs" => parse_rust_struct(&content),
        "toml" => parse_toml(&content),
        "json" => parse_json(&content),
        "yaml" | "yml" => parse_yaml(&content),
        _ => {
            // Try to detect format from content
            if content.trim_start().starts_with("struct") || content.contains("#[derive") {
                parse_rust_struct(&content)
            } else if content.trim_start().starts_with('{') {
                parse_json(&content)
            } else if content.trim_start().starts_with('[') || content.contains('=') {
                parse_toml(&content)
            } else {
                anyhow::bail!(
                    "Unknown file format. Supported: .rs (Rust structs), .toml, .json, .yaml"
                )
            }
        }
    }
}

/// Parse Rust struct definition from source code
fn parse_rust_struct(content: &str) -> anyhow::Result<EntityDefinition> {
    let file = syn::parse_file(content)?;

    // Find the first struct in the file
    let struct_item = file
        .items
        .iter()
        .find_map(|item| {
            if let syn::Item::Struct(item) = item {
                Some(item)
            } else {
                None
            }
        })
        .ok_or_else(|| anyhow::anyhow!("No struct found in file"))?;

    let struct_name = &struct_item.ident;

    // Extract table name from attributes
    let table_name = extract_table_name(&struct_item.attrs)
        .unwrap_or_else(|| to_snake_case(&struct_name.to_string()));

    // Parse fields
    let fields = match &struct_item.fields {
        syn::Fields::Named(named) => named
            .named
            .iter()
            .map(|field| parse_field(field))
            .collect::<anyhow::Result<Vec<_>>>()?,
        syn::Fields::Unnamed(_) => {
            anyhow::bail!("Tuple structs are not supported")
        }
        syn::Fields::Unit => {
            anyhow::bail!("Unit structs are not supported")
        }
    };

    Ok(EntityDefinition {
        name: struct_name.clone(),
        table_name,
        fields,
    })
}

/// Parse a field from a Rust struct
fn parse_field(field: &Field) -> anyhow::Result<FieldDefinition> {
    let field_name = field
        .ident
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Unnamed fields not supported"))?;

    let field_type = field.ty.clone();

    // Extract attributes
    let is_primary_key = has_attribute(&field.attrs, "primary_key");
    let is_auto_increment = has_attribute(&field.attrs, "auto_increment");
    let is_nullable = field_type_is_option(&field_type) || has_attribute(&field.attrs, "nullable");
    let column_name = extract_column_name(&field.attrs);

    Ok(FieldDefinition {
        name: field_name.clone(),
        ty: field_type,
        is_primary_key,
        column_name,
        is_nullable,
        is_auto_increment,
    })
}

/// Check if a type is Option<T>
fn field_type_is_option(ty: &Type) -> bool {
    if let syn::Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            segment.ident == "Option"
        } else {
            false
        }
    } else {
        false
    }
}

/// Extract table name from attributes
fn extract_table_name(attrs: &[Attribute]) -> Option<String> {
    for attr in attrs {
        if attr.path().is_ident("table_name") {
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(syn::ExprLit {
                    lit: Lit::Str(s), ..
                }) = &meta.value
                {
                    return Some(s.value());
                }
            }
        }
    }
    None
}

/// Extract column name from attributes
fn extract_column_name(attrs: &[Attribute]) -> Option<String> {
    for attr in attrs {
        if attr.path().is_ident("column_name") {
            if let Ok(meta) = attr.meta.require_name_value() {
                if let syn::Expr::Lit(syn::ExprLit {
                    lit: Lit::Str(s), ..
                }) = &meta.value
                {
                    return Some(s.value());
                }
            }
        }
    }
    None
}

/// Check if field has a specific attribute
fn has_attribute(attrs: &[Attribute], attr_name: &str) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident(attr_name))
}

fn parse_toml(content: &str) -> anyhow::Result<EntityDefinition> {
    let config: EntityConfig = toml::from_str(content)?;
    convert_config_to_entity(config)
}

fn parse_json(content: &str) -> anyhow::Result<EntityDefinition> {
    let config: EntityConfig = serde_json::from_str(content)?;
    convert_config_to_entity(config)
}

fn parse_yaml(_content: &str) -> anyhow::Result<EntityDefinition> {
    // YAML parsing not fully implemented - use Rust structs, TOML, or JSON instead
    anyhow::bail!(
        "YAML parsing not fully implemented. Please use Rust structs (.rs), TOML, or JSON format."
    )
}

fn convert_config_to_entity(config: EntityConfig) -> anyhow::Result<EntityDefinition> {
    let entity_name = syn::parse_str::<Ident>(&config.name)?;
    let table_name = config
        .table_name
        .unwrap_or_else(|| to_snake_case(&config.name));

    let fields = config
        .fields
        .into_iter()
        .map(|f| {
            let field_name = syn::parse_str::<Ident>(&f.name)?;
            let field_type = syn::parse_str::<Type>(&f.type_str)?;

            Ok(FieldDefinition {
                name: field_name,
                ty: field_type,
                is_primary_key: f.primary_key.unwrap_or(false),
                column_name: f.column_name,
                is_nullable: f
                    .nullable
                    .unwrap_or_else(|| f.type_str.starts_with("Option<")),
                is_auto_increment: f.auto_increment.unwrap_or(false),
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(EntityDefinition {
        name: entity_name,
        table_name,
        fields,
    })
}

fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(c.to_lowercase().next().unwrap());
    }
    result
}

/// Parse entity definitions from a directory
pub fn parse_entities_from_dir(dir: &Path) -> anyhow::Result<Vec<EntityDefinition>> {
    let mut entities = Vec::new();

    if !dir.exists() {
        return Ok(entities);
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");

            if matches!(ext, "rs" | "toml" | "json" | "yaml" | "yml") {
                match parse_entity_from_file(&path) {
                    Ok(entity) => entities.push(entity),
                    Err(e) => eprintln!("Warning: Failed to parse {}: {}", path.display(), e),
                }
            }
        }
    }

    Ok(entities)
}
