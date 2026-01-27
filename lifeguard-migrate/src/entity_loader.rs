//! Entity loader for generating SQL from Lifeguard entities
//!
//! This module provides functionality to load entities from the examples/entities directory
//! and generate SQL migrations from them.

use std::path::PathBuf;
use std::fs;
use regex;

/// Entity definition with metadata
pub struct EntityInfo {
    pub name: String,
    pub table_name: String,
    pub file_path: PathBuf,
    /// Service path relative to entities directory (e.g., "accounting/general-ledger")
    pub service_path: Option<String>,
}

/// Load entity information from a directory (recursively)
pub fn load_entities(entities_dir: &PathBuf) -> Result<Vec<EntityInfo>, Box<dyn std::error::Error>> {
    let mut entities = Vec::new();
    
    if !entities_dir.exists() {
        return Err(format!("Entities directory does not exist: {}", entities_dir.display()).into());
    }
    
    // Recursively read all .rs files in the entities directory and subdirectories
    load_entities_recursive(entities_dir, entities_dir, &mut entities)?;
    
    Ok(entities)
}

/// Recursively load entities from directory and subdirectories
fn load_entities_recursive(
    entities_dir: &PathBuf,
    current_dir: &PathBuf,
    entities: &mut Vec<EntityInfo>,
) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(current_dir)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.is_dir() {
            // Skip common directories that shouldn't contain entities
            let dir_name = path.file_name().unwrap().to_string_lossy();
            if dir_name == "target" || dir_name == ".git" || dir_name == "node_modules" || dir_name == ".venv" {
                continue;
            }
            // Recursively search subdirectories
            load_entities_recursive(entities_dir, &path, entities)?;
        } else if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("rs") {
            // Skip mod.rs files and other non-entity files
            let file_name = path.file_name().unwrap().to_string_lossy();
            if file_name == "mod.rs" || file_name == "lib.rs" || file_name == "main.rs" {
                continue;
            }
            
            // Check if file contains #[derive(LifeModel)] - only process entity files
            let content = match fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue, // Skip files we can't read
            };
            
            if !contains_lifemodel_derive(&content) {
                continue; // Skip files that don't contain LifeModel derive
            }
            
            // Extract entity name from file (e.g., chart_of_accounts.rs -> ChartOfAccount)
            let entity_name = file_name
                .strip_suffix(".rs")
                .unwrap_or(&file_name)
                .to_string();
            
            // Extract table name from file content (look for #[table_name = "..."] or use entity name)
            let table_name = extract_table_name(&path)?;
            
            // Extract service path relative to entities_dir
            // e.g., if entities_dir is "examples/entities" and path is "examples/entities/src/accounting/general_ledger/chart_of_accounts.rs"
            // then service_path is "src/accounting/general_ledger"
            let service_path = path
                .parent()
                .and_then(|parent| parent.strip_prefix(entities_dir).ok())
                .and_then(|rel_path| {
                    let rel_str = rel_path.to_string_lossy().to_string();
                    if rel_str.is_empty() {
                        None
                    } else {
                        Some(rel_str)
                    }
                });
            
            entities.push(EntityInfo {
                name: entity_name,
                table_name,
                file_path: path,
                service_path,
            });
        }
    }
    
    Ok(())
}

/// Check if content contains #[derive(...LifeModel...)] in any pattern
///
/// This function detects `LifeModel` in any position within a `#[derive(...)]` attribute,
/// not just when it's the first derive. This fixes a bug where entities with patterns like
/// `#[derive(Clone, LifeModel)]` or `#[derive(Debug, Serialize, LifeModel)]` were silently
/// excluded from migration generation.
///
/// Handles cases like:
/// - `#[derive(LifeModel)]`
/// - `#[derive(LifeModel, Clone)]`
/// - `#[derive(Clone, LifeModel)]`
/// - `#[derive(Debug, Serialize, LifeModel)]`
fn contains_lifemodel_derive(content: &str) -> bool {
    // Use regex to match #[derive(...)] attributes and extract the content inside parentheses
    // Pattern: #[derive(...)] where ... can contain LifeModel anywhere
    let derive_pattern = regex::Regex::new(r#"#\[derive\(([^)]*)\)\]"#).unwrap();
    
    for line in content.lines() {
        if let Some(captures) = derive_pattern.captures(line) {
            // Extract just the content inside the parentheses (the derive list)
            if let Some(derive_list) = captures.get(1) {
                let derive_list_str = derive_list.as_str();
                // Check if LifeModel appears in the derive list
                // Look for "LifeModel" as a whole word (not part of another identifier)
                // Pattern: LifeModel must be preceded by start, comma+space, or space
                // and followed by comma, closing paren, or end
                let lifemodel_pattern = regex::Regex::new(r#"(^|,\s*|\s+)LifeModel(\s*,\s*|\)|$)"#).unwrap();
                if lifemodel_pattern.is_match(derive_list_str) {
                    return true;
                }
            }
        }
    }
    false
}

/// Extract table name from entity file
fn extract_table_name(file_path: &PathBuf) -> Result<String, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(file_path)?;
    
    // Look for #[table_name = "..."]
    for line in content.lines() {
        if line.contains("#[table_name") {
            if let Some(start) = line.find("= \"") {
                if let Some(end) = line[start + 3..].find('"') {
                    let table_name = &line[start + 3..start + 3 + end];
                    return Ok(table_name.to_string());
                }
            }
        }
    }
    
    // Fallback: use file name (snake_case)
    let file_name = file_path.file_stem()
        .unwrap()
        .to_string_lossy()
        .to_string();
    Ok(file_name)
}
