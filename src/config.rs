//! Configuration utilities re-exported at the crate root.
//!
//! This exposes [`DatabaseConfig`] so applications can load settings
//! from `config/config.toml` or environment variables using
//! `DatabaseConfig::load()`.

pub use crate::pool::config::*;
