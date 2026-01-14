//! Entity definition structures

use syn::{Ident, Type};

/// Entity definition parsed from source
#[derive(Debug, Clone)]
pub struct EntityDefinition {
    pub name: Ident,
    pub table_name: String,
    pub fields: Vec<FieldDefinition>,
}

/// Field definition within an entity
#[derive(Debug, Clone)]
pub struct FieldDefinition {
    pub name: Ident,
    pub ty: Type,
    pub is_primary_key: bool,
    pub column_name: Option<String>,
    pub is_nullable: bool,
    pub is_auto_increment: bool,
}

impl EntityDefinition {
    /// Create an example entity for testing
    pub fn example() -> Self {
        use syn::parse_str;

        Self {
            name: parse_str::<Ident>("User").unwrap(),
            table_name: "users".to_string(),
            fields: vec![
                FieldDefinition {
                    name: parse_str::<Ident>("id").unwrap(),
                    ty: parse_str::<Type>("i32").unwrap(),
                    is_primary_key: true,
                    column_name: None,
                    is_nullable: false,
                    is_auto_increment: true,
                },
                FieldDefinition {
                    name: parse_str::<Ident>("email").unwrap(),
                    ty: parse_str::<Type>("String").unwrap(),
                    is_primary_key: false,
                    column_name: None,
                    is_nullable: false,
                    is_auto_increment: false,
                },
                FieldDefinition {
                    name: parse_str::<Ident>("name").unwrap(),
                    ty: parse_str::<Type>("Option<String>").unwrap(),
                    is_primary_key: false,
                    column_name: None,
                    is_nullable: true,
                    is_auto_increment: false,
                },
            ],
        }
    }

    /// Get the model name (e.g., "User" -> "UserModel")
    pub fn model_name(&self) -> Ident {
        syn::Ident::new(&format!("{}Model", self.name), self.name.span())
    }

    /// Get column enum variants
    pub fn column_variants(&self) -> Vec<Ident> {
        self.fields
            .iter()
            .map(|f| {
                // Convert snake_case to PascalCase
                let name = f.name.to_string();
                let pascal = name
                    .split('_')
                    .map(|s| {
                        let mut chars = s.chars();
                        match chars.next() {
                            None => String::new(),
                            Some(first) => {
                                first.to_uppercase().collect::<String>() + chars.as_str()
                            }
                        }
                    })
                    .collect::<String>();
                syn::Ident::new(&pascal, f.name.span())
            })
            .collect()
    }

    /// Get primary key variants
    pub fn primary_key_variants(&self) -> Vec<Ident> {
        self.fields
            .iter()
            .filter(|f| f.is_primary_key)
            .map(|f| {
                let name = f.name.to_string();
                let pascal = name
                    .split('_')
                    .map(|s| {
                        let mut chars = s.chars();
                        match chars.next() {
                            None => String::new(),
                            Some(first) => {
                                first.to_uppercase().collect::<String>() + chars.as_str()
                            }
                        }
                    })
                    .collect::<String>();
                syn::Ident::new(&pascal, f.name.span())
            })
            .collect()
    }
}
