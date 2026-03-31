//! Tests for `DeriveMigrationName`.

use lifeguard::migration::MigrationName;
use lifeguard_derive::DeriveMigrationName;

#[derive(DeriveMigrationName)]
pub struct CreateUsersTable;

#[derive(DeriveMigrationName)]
pub struct AddSortOrderColumn;

#[test]
fn migration_name_snake_case_from_struct() {
    assert_eq!(CreateUsersTable::MIGRATION_NAME, "create_users_table");
    assert_eq!(
        MigrationName::migration_name(&CreateUsersTable),
        "create_users_table"
    );
    assert_eq!(AddSortOrderColumn::MIGRATION_NAME, "add_sort_order_column");
    assert_eq!(
        MigrationName::migration_name(&AddSortOrderColumn),
        "add_sort_order_column"
    );
}
