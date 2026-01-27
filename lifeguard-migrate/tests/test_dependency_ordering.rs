//! Tests for dependency ordering and foreign key validation

use lifeguard_migrate::dependency_ordering;

#[test]
fn test_extract_foreign_key_table() {
    assert_eq!(
        dependency_ordering::extract_foreign_key_table("banks(id) ON DELETE CASCADE"),
        "banks"
    );
    assert_eq!(
        dependency_ordering::extract_foreign_key_table("bank_accounts(id)"),
        "bank_accounts"
    );
    assert_eq!(
        dependency_ordering::extract_foreign_key_table("users(id) ON DELETE SET NULL"),
        "users"
    );
    assert_eq!(
        dependency_ordering::extract_foreign_key_table("schema.table_name(id)"),
        "table_name"
    );
}

#[test]
fn test_topological_sort_simple_chain() {
    let tables = vec![
        dependency_ordering::TableInfo {
            name: "banks".to_string(),
            sql: "".to_string(),
            dependencies: vec![],
        },
        dependency_ordering::TableInfo {
            name: "bank_accounts".to_string(),
            sql: "".to_string(),
            dependencies: vec!["banks".to_string()],
        },
        dependency_ordering::TableInfo {
            name: "bank_transactions".to_string(),
            sql: "".to_string(),
            dependencies: vec!["bank_accounts".to_string()],
        },
    ];

    let sorted = dependency_ordering::topological_sort(&tables).unwrap();
    assert_eq!(sorted[0], "banks");
    assert_eq!(sorted[1], "bank_accounts");
    assert_eq!(sorted[2], "bank_transactions");
}

#[test]
fn test_topological_sort_multiple_dependencies() {
    let tables = vec![
        dependency_ordering::TableInfo {
            name: "banks".to_string(),
            sql: "".to_string(),
            dependencies: vec![],
        },
        dependency_ordering::TableInfo {
            name: "bank_accounts".to_string(),
            sql: "".to_string(),
            dependencies: vec!["banks".to_string()],
        },
        dependency_ordering::TableInfo {
            name: "bank_statements".to_string(),
            sql: "".to_string(),
            dependencies: vec!["bank_accounts".to_string()],
        },
        dependency_ordering::TableInfo {
            name: "bank_reconciliations".to_string(),
            sql: "".to_string(),
            dependencies: vec!["bank_accounts".to_string(), "bank_statements".to_string()],
        },
    ];

    let sorted = dependency_ordering::topological_sort(&tables).unwrap();
    // banks should be first
    assert_eq!(sorted[0], "banks");
    // bank_accounts should come before bank_statements and bank_reconciliations
    let banks_pos = sorted.iter().position(|s| s == "banks").unwrap();
    let accounts_pos = sorted.iter().position(|s| s == "bank_accounts").unwrap();
    let statements_pos = sorted.iter().position(|s| s == "bank_statements").unwrap();
    let reconciliations_pos = sorted.iter().position(|s| s == "bank_reconciliations").unwrap();

    assert!(accounts_pos > banks_pos);
    assert!(statements_pos > accounts_pos);
    assert!(reconciliations_pos > accounts_pos);
    assert!(reconciliations_pos > statements_pos);
}

#[test]
fn test_validate_foreign_key_references_missing_table() {
    let tables = vec![
        dependency_ordering::TableInfo {
            name: "bank_accounts".to_string(),
            sql: "".to_string(),
            dependencies: vec!["banks".to_string()],
        },
        dependency_ordering::TableInfo {
            name: "bank_transactions".to_string(),
            sql: "".to_string(),
            dependencies: vec!["bank_accounts".to_string()],
        },
    ];

    // Should fail because "banks" is missing
    let result = dependency_ordering::validate_foreign_key_references(&tables);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.contains("bank_accounts"));
    assert!(error.contains("banks"));
}

#[test]
fn test_validate_foreign_key_references_all_exist() {
    let tables = vec![
        dependency_ordering::TableInfo {
            name: "banks".to_string(),
            sql: "".to_string(),
            dependencies: vec![],
        },
        dependency_ordering::TableInfo {
            name: "bank_accounts".to_string(),
            sql: "".to_string(),
            dependencies: vec!["banks".to_string()],
        },
        dependency_ordering::TableInfo {
            name: "bank_transactions".to_string(),
            sql: "".to_string(),
            dependencies: vec!["bank_accounts".to_string()],
        },
    ];

    // Should pass when all dependencies exist
    assert!(dependency_ordering::validate_foreign_key_references(&tables).is_ok());
}

#[test]
fn test_topological_sort_circular_dependency() {
    let tables = vec![
        dependency_ordering::TableInfo {
            name: "table_a".to_string(),
            sql: "".to_string(),
            dependencies: vec!["table_b".to_string()],
        },
        dependency_ordering::TableInfo {
            name: "table_b".to_string(),
            sql: "".to_string(),
            dependencies: vec!["table_a".to_string()],
        },
    ];

    // Should detect circular dependency
    let result = dependency_ordering::topological_sort(&tables);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.contains("Circular dependency"));
}

#[test]
fn test_topological_sort_no_dependencies() {
    let tables = vec![
        dependency_ordering::TableInfo {
            name: "table_a".to_string(),
            sql: "".to_string(),
            dependencies: vec![],
        },
        dependency_ordering::TableInfo {
            name: "table_b".to_string(),
            sql: "".to_string(),
            dependencies: vec![],
        },
    ];

    // Should work fine with no dependencies
    let sorted = dependency_ordering::topological_sort(&tables).unwrap();
    assert_eq!(sorted.len(), 2);
    assert!(sorted.contains(&"table_a".to_string()));
    assert!(sorted.contains(&"table_b".to_string()));
}
