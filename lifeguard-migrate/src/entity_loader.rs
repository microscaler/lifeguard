//! Entity loader for generating SQL from Lifeguard entities
//!
//! This module provides functionality to load entities from the examples/entities directory
//! and generate SQL migrations from them.

use std::path::PathBuf;
use std::fs;

/// Entity definition with metadata
pub struct EntityInfo {
    pub name: String,
    pub table_name: String,
    pub file_path: PathBuf,
}

/// Load entity information from a directory
pub fn load_entities(entities_dir: &PathBuf) -> Result<Vec<EntityInfo>, Box<dyn std::error::Error>> {
    let mut entities = Vec::new();
    
    if !entities_dir.exists() {
        return Err(format!("Entities directory does not exist: {}", entities_dir.display()).into());
    }
    
    // Read all .rs files in the entities directory
    for entry in fs::read_dir(entities_dir)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("rs") {
            // Skip README and other non-entity files
            let file_name = path.file_name().unwrap().to_string_lossy();
            if file_name == "README.md" || file_name.starts_with("IMPLEMENTATION") || file_name.starts_with("MIGRATION") {
                continue;
            }
            
            // Extract entity name from file (e.g., chart_of_accounts.rs -> ChartOfAccount)
            let entity_name = file_name
                .strip_suffix(".rs")
                .unwrap_or(&file_name)
                .to_string();
            
            // Extract table name from file content (look for #[table_name = "..."] or use entity name)
            let table_name = extract_table_name(&path)?;
            
            entities.push(EntityInfo {
                name: entity_name,
                table_name,
                file_path: path,
            });
        }
    }
    
    Ok(entities)
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
