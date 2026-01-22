//! Build script helper for entity registry generation
//!
//! This module provides functions that can be used in user's build.rs
//! to automatically discover entities and generate a registry module.

use std::fs;
use std::path::{Path, PathBuf};
use std::collections::HashMap;

/// Entity information discovered from source files
#[derive(Debug, Clone)]
pub struct EntityInfo {
    pub table_name: String,
    pub struct_name: String,
    pub file_path: PathBuf,
    pub module_path: String,
}

/// Discover all entities in a source directory
///
/// Recursively scans for Rust files containing `#[derive(LifeModel)]`
/// and extracts entity information.
pub fn discover_entities(source_dir: &Path) -> Result<Vec<EntityInfo>, Box<dyn std::error::Error>> {
    let mut entities = Vec::new();
    discover_entities_recursive(source_dir, source_dir, &mut entities)?;
    Ok(entities)
}

/// Recursively discover entities
fn discover_entities_recursive(
    root_dir: &Path,
    current_dir: &Path,
    entities: &mut Vec<EntityInfo>,
) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(current_dir)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.is_dir() {
            // Skip common directories
            let dir_name = path.file_name().unwrap().to_string_lossy();
            if dir_name == "target" || dir_name == ".git" || dir_name == "node_modules" {
                continue;
            }
            discover_entities_recursive(root_dir, &path, entities)?;
        } else if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("rs") {
            // Check if file contains #[derive(LifeModel)]
            if let Ok(content) = fs::read_to_string(&path) {
                if content.contains("#[derive(LifeModel)]") || content.contains("#[derive(LifeModel,") {
                    // Extract entity information
                    if let Some(entity_info) = extract_entity_info(&path, &content, root_dir)? {
                        entities.push(entity_info);
                    }
                }
            }
        }
    }
    
    Ok(())
}

/// Extract entity information from a Rust source file
///
/// Uses simple string parsing to find #[derive(LifeModel)] and extract basic info.
/// This is NOT full AST parsing - we're just finding entities to include in registry.
/// The actual entity compilation happens when the registry module is compiled.
fn extract_entity_info(
    file_path: &Path,
    content: &str,
    root_dir: &Path,
) -> Result<Option<EntityInfo>, Box<dyn std::error::Error>> {
    // Check if file contains #[derive(LifeModel)]
    if !content.contains("#[derive(LifeModel)]") && !content.contains("#[derive(LifeModel,") {
        return Ok(None);
    }
    
    // Extract struct name - look for "pub struct" after derive
    let struct_name = extract_struct_name(content)?;
    if struct_name.is_none() {
        return Ok(None);
    }
    let struct_name = struct_name.unwrap();
    
    // Extract table name from #[table_name = "..."] attribute
    let table_name = extract_table_name_from_string(content)
        .unwrap_or_else(|| snake_case(&struct_name));
    
    // Calculate module path (relative to root_dir)
    let rel_path = file_path.strip_prefix(root_dir)
        .unwrap_or(file_path)
        .parent()
        .unwrap_or(Path::new(""));
    
    let module_path = rel_path
        .iter()
        .map(|c| c.to_string_lossy().replace("-", "_"))
        .collect::<Vec<_>>()
        .join("::");
    
    Ok(Some(EntityInfo {
        table_name,
        struct_name,
        file_path: file_path.to_path_buf(),
        module_path: if module_path.is_empty() {
            "crate".to_string()
        } else {
            format!("crate::{}", module_path)
        },
    }))
}

/// Extract struct name from file content (simple string parsing)
fn extract_struct_name(content: &str) -> Result<Option<String>, Box<dyn std::error::Error>> {
    // Look for pattern: pub struct StructName {
    // or: struct StructName {
    let lines: Vec<&str> = content.lines().collect();
    
    for (i, line) in lines.iter().enumerate() {
        if line.contains("#[derive(LifeModel)]") || line.contains("#[derive(LifeModel,") {
            // Look ahead for struct definition (within next 10 lines)
            for j in (i + 1)..(i + 10).min(lines.len()) {
                let struct_line = lines[j].trim();
                if struct_line.starts_with("pub struct ") || struct_line.starts_with("struct ") {
                    // Extract struct name
                    let parts: Vec<&str> = struct_line.split_whitespace().collect();
                    // Handle both "pub struct Name" and "struct Name"
                    let name_index = if parts.len() >= 3 && parts[0] == "pub" && parts[1] == "struct" {
                        2  // "pub struct Name" -> use index 2
                    } else if parts.len() >= 2 && parts[0] == "struct" {
                        1  // "struct Name" -> use index 1
                    } else {
                        continue; // Invalid format
                    };
                    
                    if name_index < parts.len() {
                        let name = parts[name_index];
                        // Remove generics if present
                        let name = name.split('<').next().unwrap_or(name);
                        // Remove braces if present
                        let name = name.split('{').next().unwrap_or(name).trim();
                        return Ok(Some(name.to_string()));
                    }
                }
            }
        }
    }
    
    Ok(None)
}

/// Extract table name from #[table_name = "..."] attribute (simple string parsing)
fn extract_table_name_from_string(content: &str) -> Option<String> {
    for line in content.lines() {
        if line.contains("#[table_name") {
            // Look for = "value"
            if let Some(start) = line.find("= \"") {
                if let Some(end) = line[start + 3..].find('"') {
                    return Some(line[start + 3..start + 3 + end].to_string());
                }
            }
        }
    }
    None
}

/// Convert PascalCase to snake_case
fn snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(c.to_lowercase().next().unwrap_or(c));
    }
    result
}

/// Generate registry module from discovered entities
///
/// This generates a registry module that includes all discovered entities.
/// The registry uses `#[path = "..."]` to include entity files and provides
/// functions to iterate over entities and generate SQL.
pub fn generate_registry_module(
    entities: &[EntityInfo],
    output_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    if entities.is_empty() {
        return Err("No entities found to generate registry".into());
    }
    
    // Group entities by service path (directory structure)
    let mut entities_by_service: HashMap<String, Vec<&EntityInfo>> = HashMap::new();
    
    for entity in entities {
        // Extract service path from file path
        // e.g., src/accounting/general_ledger/chart_of_accounts.rs -> accounting/general_ledger
        let service_path = entity.file_path
            .parent()
            .and_then(|p| {
                // Find "src" or "entities" directory
                let mut parts: Vec<_> = p.iter().collect();
                if let Some(src_idx) = parts.iter().position(|c| {
                    c.to_string_lossy() == "src" || c.to_string_lossy() == "entities"
                }) {
                    parts.drain(..=src_idx);
                    let path_str = parts.iter()
                        .map(|c| c.to_string_lossy().to_string())
                        .collect::<Vec<_>>()
                        .join("/");
                    if path_str.is_empty() {
                        None
                    } else {
                        Some(path_str)
                    }
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "default".to_string());
        
        entities_by_service
            .entry(service_path)
            .or_insert_with(Vec::new)
            .push(entity);
    }
    
    // Calculate output directory for relative paths
    let output_dir = output_path.parent()
        .ok_or("Output path must have a parent directory")?;
    
    // Generate registry module content
    let mut registry_content = String::from("//! Auto-generated entity registry\n");
    registry_content.push_str("//! DO NOT EDIT - This file is generated by build script\n\n");
    registry_content.push_str("use lifeguard::{LifeModelTrait, LifeEntityName};\n");
    registry_content.push_str("use lifeguard_migrate::sql_generator;\n\n");
    
    registry_content.push_str("pub mod entity_registry {\n");
    registry_content.push_str("    use super::*;\n\n");
    
    // Generate module includes for each entity
    for (service_path, service_entities) in &entities_by_service {
        // Create service module name (sanitize for Rust identifier)
        let service_module = sanitize_module_name(service_path);
        registry_content.push_str(&format!("    pub mod {} {{\n", service_module));
        registry_content.push_str("        use super::*;\n\n");
        
        for entity in service_entities {
            // Generate module name from struct name (snake_case)
            let module_name = snake_case(&entity.struct_name);
            
            // Calculate relative path from output directory to entity file
            // For build scripts, the registry is in OUT_DIR and entities are in src/
            // We need to calculate relative path from OUT_DIR to the entity file
            let relative_path = pathdiff::diff_paths(&entity.file_path, output_dir)
                .ok_or_else(|| format!("Failed to calculate relative path from {:?} to {:?}", 
                    output_dir, entity.file_path))?;
            
            // Convert to string with forward slashes (works on all platforms)
            let path_str = relative_path.to_string_lossy().replace('\\', "/");
            
            registry_content.push_str(&format!(
                "        #[path = r#\"{}\"#]\n",
                path_str
            ));
            registry_content.push_str(&format!(
                "        pub mod {};\n",
                module_name
            ));
        }
        
        registry_content.push_str("    }\n\n");
    }
    
    // Generate entity metadata and iteration functions
    registry_content.push_str("    /// Entity metadata for registry iteration\n");
    registry_content.push_str("    pub struct EntityMetadata {\n");
    registry_content.push_str("        pub table_name: &'static str,\n");
    registry_content.push_str("        pub service_path: &'static str,\n");
    registry_content.push_str("    }\n\n");
    
    registry_content.push_str("    /// Get all entity metadata\n");
    registry_content.push_str("    pub fn all_entity_metadata() -> Vec<EntityMetadata> {\n");
    registry_content.push_str("        vec![\n");
    
    for (service_path, service_entities) in &entities_by_service {
        let service_module = sanitize_module_name(service_path);
        for entity in service_entities {
            let module_name = snake_case(&entity.struct_name);
            registry_content.push_str(&format!(
                "            EntityMetadata {{\n"
            ));
            registry_content.push_str(&format!(
                "                table_name: {}::{}::Entity::TABLE_NAME,\n",
                service_module, module_name
            ));
            registry_content.push_str(&format!(
                "                service_path: r#\"{}\"#,\n",
                service_path
            ));
            registry_content.push_str("            },\n");
        }
    }
    
    registry_content.push_str("        ]\n");
    registry_content.push_str("    }\n\n");
    
    // Generate function to generate SQL for all entities
    registry_content.push_str("    /// Generate SQL for all entities\n");
    registry_content.push_str("    pub fn generate_sql_for_all() -> Result<Vec<(String, String)>, String> {\n");
    registry_content.push_str("        let mut results = Vec::new();\n\n");
    
    for (service_path, service_entities) in &entities_by_service {
        let service_module = sanitize_module_name(service_path);
        for entity in service_entities {
            let module_name = snake_case(&entity.struct_name);
            let struct_name = &entity.struct_name;
            
            registry_content.push_str(&format!(
                "        // Generate SQL for {}::{}::Entity\n",
                service_module, module_name
            ));
            registry_content.push_str(&format!(
                "        {{\n"
            ));
            registry_content.push_str(&format!(
                "            use {}::{}::Entity;\n",
                service_module, module_name
            ));
            registry_content.push_str(&format!(
                "            let table_def = Entity::table_definition();\n"
            ));
            registry_content.push_str(&format!(
                "            match sql_generator::generate_create_table_sql::<Entity>(table_def) {{\n"
            ));
            registry_content.push_str(&format!(
                "                Ok(sql) => results.push((Entity::TABLE_NAME.to_string(), sql)),\n"
            ));
            registry_content.push_str(&format!(
                "                Err(e) => return Err(format!(\"Failed to generate SQL for {}: {{}}\", e)),\n",
                struct_name
            ));
            registry_content.push_str("            }\n");
            registry_content.push_str("        }\n\n");
        }
    }
    
    registry_content.push_str("        Ok(results)\n");
    registry_content.push_str("    }\n");
    registry_content.push_str("}\n");
    
    // Write registry module
    fs::write(output_path, registry_content)?;
    
    Ok(())
}

/// Sanitize a path string to be a valid Rust module name
fn sanitize_module_name(path: &str) -> String {
    path.replace("/", "_")
        .replace("-", "_")
        .replace(".", "_")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .collect::<String>()
}
