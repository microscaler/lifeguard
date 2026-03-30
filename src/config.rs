//! Configuration utilities re-exported at the crate root.
//!
//! [`DatabaseConfig`] is defined in [`crate::pool::config`] (single source of truth) and loaded
//! via [`DatabaseConfig::load`]. Pool timeouts and queue depth apply when building
//! [`crate::pool::LifeguardPool`] with [`crate::pool::LifeguardPool::from_database_config`].

/// Re-export database + pool file/env configuration.
pub mod database {
    pub use crate::pool::config::DatabaseConfig;
}

pub use database::DatabaseConfig;
