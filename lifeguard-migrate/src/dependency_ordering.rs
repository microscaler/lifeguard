//! Dependency ordering and validation for migration generation
//!
//! This module provides functionality to:
//! - Extract foreign key dependencies from entities
//! - Build dependency graphs
//! - Topologically sort tables by dependencies
//! - Validate that all foreign key references exist

use std::collections::{BTreeSet, HashMap, HashSet};

/// Extract the referenced table name from a foreign key string
/// Format: "table_name(column) ON DELETE action" or "table_name(column)"
/// Also handles "schema.table_name(column)" format - returns just the table name
pub fn extract_foreign_key_table(fk: &str) -> String {
    // Parse format: "table_name(column) ON DELETE action"
    // or "schema.table_name(column) ON DELETE action"
    // or just "table_name(column)"
    if let Some(paren_pos) = fk.find('(') {
        let table_ref = fk[..paren_pos].trim();
        // Handle schema.table format - extract just the table name
        if let Some(dot_pos) = table_ref.rfind('.') {
            table_ref[dot_pos + 1..].to_string()
        } else {
            table_ref.to_string()
        }
    } else {
        // Fallback: if no parentheses, assume the whole string is the table name
        let table_ref = fk.trim();
        if let Some(dot_pos) = table_ref.rfind('.') {
            table_ref[dot_pos + 1..].to_string()
        } else {
            table_ref.to_string()
        }
    }
}

/// Extract all foreign key dependencies from an entity's columns
pub fn extract_foreign_key_dependencies<E>() -> Vec<String>
where
    E: lifeguard::LifeModelTrait + lifeguard::LifeEntityName + Default,
    E::Column: lifeguard::ColumnTrait
        + Copy
        + sea_query::IdenStatic
        + PartialEq
        + lifeguard::query::column::column_trait::ColumnDefHelper,
{
    use lifeguard::query::column::column_trait::ColumnDefHelper;
    let columns = E::all_columns();
    let mut dependencies = Vec::new();

    for col in columns {
        let col_def = col.column_def();
        if let Some(fk) = &col_def.foreign_key {
            let table_name = extract_foreign_key_table(fk);
            dependencies.push(table_name);
        }
    }

    dependencies
}

/// Table metadata for dependency ordering
#[derive(Debug, Clone)]
pub struct TableInfo {
    pub name: String,
    pub sql: String,
    pub dependencies: Vec<String>, // Tables this table depends on
}

/// Build a dependency graph from table information
pub fn build_dependency_graph(tables: &[TableInfo]) -> HashMap<String, Vec<String>> {
    let mut graph: HashMap<String, Vec<String>> = HashMap::new();

    for table in tables {
        graph.insert(table.name.clone(), table.dependencies.clone());
    }

    graph
}

/// Topologically sort tables by their dependencies
/// Returns tables in order: dependencies first, dependents last
/// Returns None if there's a circular dependency
pub fn topological_sort(tables: &[TableInfo]) -> Result<Vec<String>, String> {
    // Deduplicate by table name: keep the last occurrence so that
    // whichever entry appears last in the input wins (merge semantics).
    let mut by_name: HashMap<String, TableInfo> = HashMap::new();
    for t in tables {
        by_name.insert(t.name.clone(), t.clone());
    }
    // Name order, so the result does not depend on HashMap iteration: generated code (the build-script registry) and
    // migrations come out the same on every build, and an unchanged service binary stays byte-identical.
    let mut tables: Vec<TableInfo> = by_name.into_values().collect();
    tables.sort_by(|a, b| a.name.cmp(&b.name));

    // Build reverse graph: for each table, track which tables depend on it
    let mut reverse_graph: HashMap<String, Vec<String>> = HashMap::new();
    let mut in_degree: HashMap<String, usize> = HashMap::new();

    // Initialize
    for table in &tables {
        in_degree.insert(table.name.clone(), table.dependencies.len());
        reverse_graph.insert(table.name.clone(), Vec::new());
    }

    // Build reverse graph: if A depends on B, then B has A as a dependent
    for table in &tables {
        for dep in &table.dependencies {
            if let Some(dependents) = reverse_graph.get_mut(dep) {
                dependents.push(table.name.clone());
            }
        }
    }

    // Find all tables with no dependencies (can be created first)
    // Ready tables in name order (smallest first), for the same reason.
    let mut queue: BTreeSet<String> = tables
        .iter()
        .filter(|t| t.dependencies.is_empty())
        .map(|t| t.name.clone())
        .collect();

    let mut result = Vec::new();

    // Process queue
    while let Some(current) = queue.pop_first() {
        result.push(current.clone());

        // For each table that depends on current, reduce its in-degree
        if let Some(dependents) = reverse_graph.get(&current) {
            for dependent in dependents {
                let degree = in_degree.get_mut(dependent).unwrap();
                *degree -= 1;
                if *degree == 0 {
                    queue.insert(dependent.clone());
                }
            }
        }
    }

    // Check for circular dependencies
    if result.len() != tables.len() {
        return Err("Circular dependency detected in foreign key references".to_string());
    }

    Ok(result)
}

/// Validate that all foreign key references point to tables that exist
pub fn validate_foreign_key_references(tables: &[TableInfo]) -> Result<(), String> {
    let table_names: HashSet<String> = tables.iter().map(|t| t.name.clone()).collect();
    let mut errors = Vec::new();

    for table in tables {
        for dep in &table.dependencies {
            if !table_names.contains(dep) {
                errors.push(format!(
                    "Table '{}' has foreign key reference to '{}' which does not exist in this migration",
                    table.name, dep
                ));
            }
        }
    }

    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_foreign_key_table() {
        assert_eq!(
            extract_foreign_key_table("banks(id) ON DELETE CASCADE"),
            "banks"
        );
        assert_eq!(
            extract_foreign_key_table("bank_accounts(id)"),
            "bank_accounts"
        );
        assert_eq!(
            extract_foreign_key_table("users(id) ON DELETE SET NULL"),
            "users"
        );
    }

    #[test]
    fn test_topological_sort_simple() {
        let tables = vec![
            TableInfo {
                name: "banks".to_string(),
                sql: "".to_string(),
                dependencies: vec![],
            },
            TableInfo {
                name: "bank_accounts".to_string(),
                sql: "".to_string(),
                dependencies: vec!["banks".to_string()],
            },
            TableInfo {
                name: "bank_transactions".to_string(),
                sql: "".to_string(),
                dependencies: vec!["bank_accounts".to_string()],
            },
        ];

        let sorted = topological_sort(&tables).unwrap();
        assert_eq!(sorted[0], "banks");
        assert_eq!(sorted[1], "bank_accounts");
        assert_eq!(sorted[2], "bank_transactions");
    }

    #[test]
    fn test_topological_sort_deduplicates_table_names_using_last_definition() {
        let tables = vec![
            TableInfo {
                name: "accounts".to_string(),
                sql: "old".to_string(),
                dependencies: vec!["missing_old_dependency".to_string()],
            },
            TableInfo {
                name: "accounts".to_string(),
                sql: "new".to_string(),
                dependencies: vec![],
            },
        ];

        assert_eq!(topological_sort(&tables).unwrap(), vec!["accounts"]);
    }

    #[test]
    fn test_validate_foreign_key_references() {
        let tables = vec![
            TableInfo {
                name: "bank_accounts".to_string(),
                sql: "".to_string(),
                dependencies: vec!["banks".to_string()],
            },
            TableInfo {
                name: "bank_transactions".to_string(),
                sql: "".to_string(),
                dependencies: vec!["bank_accounts".to_string()],
            },
        ];

        // Should fail because "banks" is missing
        assert!(validate_foreign_key_references(&tables).is_err());

        // Should pass when all dependencies exist
        let tables_with_banks = vec![
            TableInfo {
                name: "banks".to_string(),
                sql: "".to_string(),
                dependencies: vec![],
            },
            TableInfo {
                name: "bank_accounts".to_string(),
                sql: "".to_string(),
                dependencies: vec!["banks".to_string()],
            },
            TableInfo {
                name: "bank_transactions".to_string(),
                sql: "".to_string(),
                dependencies: vec!["bank_accounts".to_string()],
            },
        ];

        assert!(validate_foreign_key_references(&tables_with_banks).is_ok());
    }

    /// Ties are broken by name, not by HashMap iteration: the same tables give the same order every time.
    #[test]
    fn test_topological_sort_is_deterministic() {
        let t = |name: &str, deps: &[&str]| TableInfo {
            name: name.to_string(),
            sql: String::new(),
            dependencies: deps.iter().map(|d| d.to_string()).collect(),
        };
        let tables = vec![
            t("role_assignments", &["roles"]),
            t("roles", &[]),
            t("app_role_permissions", &["roles", "permissions"]),
            t("permissions", &[]),
            t("audit_log", &[]),
            t("zones", &[]),
        ];
        let first = topological_sort(&tables).unwrap();
        for _ in 0..50 {
            assert_eq!(topological_sort(&tables).unwrap(), first);
        }
        assert_eq!(
            first,
            vec![
                "audit_log",
                "permissions",
                "roles",
                "app_role_permissions",
                "role_assignments",
                "zones"
            ]
        );
    }
}
