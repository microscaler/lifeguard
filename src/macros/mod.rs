pub mod execute;
pub mod go;
pub mod insert_many;
pub mod query;
mod seed_test;
mod temp_table;
mod test_data;
pub mod txn;

#[allow(unused_imports)]
use crate::pool::config::DatabaseConfig;
#[allow(unused_imports)]
use crate::DbPoolManager;

/// Build a [`DbPoolManager`] backed by `MockDatabase` for unit tests.
#[macro_export]
macro_rules! test_pool {
    () => {{
        use sea_orm::{DatabaseBackend, MockDatabase};
        $crate::DbPoolManager::from_connection(
            MockDatabase::new(DatabaseBackend::Postgres).into_connection(),
        )
    }};
}
