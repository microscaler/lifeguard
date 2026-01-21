//! SQL migration generator for entity-driven migrations.
//!
//! This module generates SQL CREATE TABLE statements from Lifeguard entity metadata.
//! It reads ColumnDefinition and TableDefinition to produce PostgreSQL-compatible SQL.

use lifeguard::{
    LifeModelTrait, LifeEntityName, ColumnTrait,
    TableDefinition,
    query::column::column_trait::ColumnDefHelper,
};
use sea_query::IdenStatic;
use std::fmt::Write;


/// Generate SQL CREATE TABLE statement from entity metadata
///
/// This function generates a complete SQL CREATE TABLE statement including:
/// - Column definitions with types, nullability, defaults
/// - Primary key constraints
/// - Foreign key constraints
/// - CHECK constraints (column and table level)
/// - Unique constraints (single and composite)
/// - Indexes
/// - Table comments
///
/// Foreign keys are generated as separate ALTER TABLE statements to allow
/// proper dependency ordering.
///
/// # Parameters
///
/// * `table_def` - Table-level metadata (composite unique, indexes, CHECK constraints, comments)
///   This should be obtained by calling `Entity::table_definition()` for the entity.
pub fn generate_create_table_sql<E>(
    table_def: TableDefinition,
) -> Result<String, String>
where
    E: LifeModelTrait + LifeEntityName + Default,
    E::Column: ColumnTrait + Copy + sea_query::IdenStatic + PartialEq,
{
    let entity = E::default();
    let table_name = entity.table_name();
    let schema_name = entity.schema_name();
    
    // Build full table name with schema if present
    let full_table_name = if let Some(schema) = schema_name {
        format!("{}.{}", schema, table_name)
    } else {
        table_name.to_string()
    };
    
    let mut sql = String::new();
    
    // Generate CREATE TABLE statement
    writeln!(sql, "CREATE TABLE IF NOT EXISTS {} (", full_table_name).map_err(|e| format!("Failed to write SQL: {}", e))?;
    
    // Get all columns
    let columns = E::all_columns();
    let mut column_defs = Vec::new();
    let mut primary_key_cols = Vec::new();
    let mut foreign_keys = Vec::new();
    let mut check_constraints = Vec::new();
    
    // Process each column
    for col in columns {
        // Use column_def() (inherent method) instead of def() (trait method)
        // The macro generates column_def() with all the attribute metadata
        // The trait method def() has a default implementation that returns empty metadata
        let col_def = col.column_def();
        // Get column name - Column implements IdenStatic which has as_str()
        let col_name = col.as_str();
        
        // Build column definition
        let mut col_sql = format!("    {}", col_name);
        
        // Add column type
        if let Some(ref col_type) = col_def.column_type {
            col_sql.push_str(&format!(" {}", col_type));
        } else {
            // Infer type from Rust type (basic mapping)
            col_sql.push_str(" TEXT"); // Default fallback
        }
        
        // Add nullability
        if col_def.nullable {
            col_sql.push_str(" NULL");
        } else {
            col_sql.push_str(" NOT NULL");
        }
        
        // Check if this column is a primary key
        // Heuristic: if it's auto_increment or named "id", it's likely a primary key
        let is_primary_key = col_def.auto_increment || col_name == "id";
        if is_primary_key {
            primary_key_cols.push(col_name.to_string());
        }
        
        // Add default value or expression
        // Priority: explicit default_expr > explicit default_value > UUID primary key default
        if let Some(ref default_expr) = col_def.default_expr {
            col_sql.push_str(&format!(" DEFAULT {}", default_expr));
        } else if let Some(ref default_val) = col_def.default_value {
            col_sql.push_str(&format!(" DEFAULT {}", default_val));
        } else if is_primary_key {
            // For UUID primary keys, add gen_random_uuid() default if no explicit default is set
            if col_def.column_type.as_ref().map(|s| s.contains("UUID")).unwrap_or(false) {
                col_sql.push_str(" DEFAULT gen_random_uuid()");
            }
        }
        
        // Add PRIMARY KEY constraint
        if is_primary_key {
            col_sql.push_str(" PRIMARY KEY");
        }
        
        // Track unique constraints (single column)
        if col_def.unique {
            col_sql.push_str(" UNIQUE");
        }
        
        // Track foreign keys (will be added as ALTER TABLE)
        if let Some(ref fk) = col_def.foreign_key {
            foreign_keys.push((col_name.to_string(), fk.to_string()));
        }
        
        // Track CHECK constraints (column-level)
        if let Some(ref check) = col_def.check {
            check_constraints.push((col_name.to_string(), check.to_string()));
        }
        
        column_defs.push(col_sql);
    }
    
    // Write column definitions
    for (i, col_def) in column_defs.iter().enumerate() {
        let is_last_column = i == column_defs.len() - 1;
        let has_table_constraints = !table_def.check_constraints.is_empty() || !table_def.composite_unique.is_empty();
        
        if is_last_column && !has_table_constraints {
            // Last column and no table constraints - no comma
            writeln!(sql, "{}", col_def).map_err(|e| format!("Failed to write SQL: {}", e))?;
        } else {
            // Not last or has table constraints - add comma
            writeln!(sql, "{},", col_def).map_err(|e| format!("Failed to write SQL: {}", e))?;
        }
    }
    
    // Add table-level CHECK constraints
    for (i, check_expr) in table_def.check_constraints.iter().enumerate() {
        let is_last = i == table_def.check_constraints.len() - 1 && table_def.composite_unique.is_empty();
        if is_last {
            writeln!(sql, "    CONSTRAINT check_{} CHECK ({})", 
                sanitize_constraint_name(&table_name), check_expr)
                .map_err(|e| format!("Failed to write SQL: {}", e))?;
        } else {
            writeln!(sql, "    CONSTRAINT check_{} CHECK ({}),", 
                sanitize_constraint_name(&table_name), check_expr)
                .map_err(|e| format!("Failed to write SQL: {}", e))?;
        }
    }
    
    // Add composite unique constraints
    for (i, unique_cols) in table_def.composite_unique.iter().enumerate() {
        let is_last = i == table_def.composite_unique.len() - 1;
        let cols_str = unique_cols.join(", ");
        if is_last {
            writeln!(sql, "    UNIQUE({})", cols_str)
                .map_err(|e| format!("Failed to write SQL: {}", e))?;
        } else {
            writeln!(sql, "    UNIQUE({}),", cols_str)
                .map_err(|e| format!("Failed to write SQL: {}", e))?;
        }
    }
    
    // Close CREATE TABLE
    writeln!(sql, ");").map_err(|e| format!("Failed to write SQL: {}", e))?;
    writeln!(sql).map_err(|e| format!("Failed to write SQL: {}", e))?;
    
    // Generate indexes
    for index in &table_def.indexes {
        let mut index_sql = String::new();
        
        if index.unique {
            index_sql.push_str("CREATE UNIQUE INDEX ");
        } else {
            index_sql.push_str("CREATE INDEX ");
        }
        
        index_sql.push_str(&index.name);
        index_sql.push_str(" ON ");
        index_sql.push_str(&full_table_name);
        index_sql.push_str("(");
        index_sql.push_str(&index.columns.join(", "));
        index_sql.push_str(")");
        
        if let Some(ref where_clause) = index.partial_where {
            index_sql.push_str(" WHERE ");
            index_sql.push_str(where_clause);
        }
        
        index_sql.push_str(";");
        writeln!(sql, "{}", index_sql).map_err(|e| format!("Failed to write SQL: {}", e))?;
    }
    
    // Generate foreign key constraints as ALTER TABLE
    for (col_name, fk_def) in &foreign_keys {
        writeln!(sql, "ALTER TABLE {} ADD CONSTRAINT fk_{}_{} FOREIGN KEY ({}) REFERENCES {};",
            full_table_name,
            sanitize_constraint_name(table_name),
            sanitize_constraint_name(col_name),
            col_name,
            fk_def
        ).map_err(|e| format!("Failed to write SQL: {}", e))?;
    }
    
    // Generate column-level CHECK constraints as ALTER TABLE
    for (col_name, check_expr) in &check_constraints {
        writeln!(sql, "ALTER TABLE {} ADD CONSTRAINT check_{}_{} CHECK ({});",
            full_table_name,
            sanitize_constraint_name(table_name),
            sanitize_constraint_name(col_name),
            check_expr
        ).map_err(|e| format!("Failed to write SQL: {}", e))?;
    }
    
    // Generate table comment
    if let Some(ref comment) = table_def.table_comment {
        let escaped_comment = comment.replace("'", "''");
        writeln!(sql, "COMMENT ON TABLE {} IS '{}';",
            full_table_name, escaped_comment
        ).map_err(|e| format!("Failed to write SQL: {}", e))?;
    }
    
    Ok(sql)
}

/// Sanitize a name for use in constraint names
fn sanitize_constraint_name(name: &str) -> String {
    name.replace("-", "_").replace(".", "_").to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // Note: These tests would require actual entity definitions
    // For now, we'll test the sanitize function
    #[test]
    fn test_sanitize_constraint_name() {
        assert_eq!(sanitize_constraint_name("chart-of-accounts"), "chart_of_accounts");
        assert_eq!(sanitize_constraint_name("journal.entries"), "journal_entries");
        assert_eq!(sanitize_constraint_name("UPPERCASE"), "uppercase");
    }
}
