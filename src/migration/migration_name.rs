//! Stable names for [`super::Migration`] unit structs.

/// Stable **snake_case** migration identifier, usually implemented via **`DeriveMigrationName`**
/// (`lifeguard_derive` / `lifeguard::DeriveMigrationName`).
///
/// Use [`Migration::name`](super::Migration::name) in [`super::Migration`] implementations:
///
/// ```ignore
/// use lifeguard::migration::{DeriveMigrationName, Migration, MigrationName, SchemaManager};
///
/// #[derive(DeriveMigrationName)]
/// pub struct CreateUsersTable;
///
/// impl Migration for CreateUsersTable {
///     fn name(&self) -> &str {
///         MigrationName::migration_name(self)
///     }
///     // ...
/// }
/// ```
pub trait MigrationName: Send + Sync {
    fn migration_name(&self) -> &'static str;
}
