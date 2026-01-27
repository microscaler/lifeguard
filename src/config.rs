//! Configuration utilities re-exported at the crate root.
//!
//! This exposes [`DatabaseConfig`] so applications can load settings
//! from `config/config.toml` or environment variables using
//! `DatabaseConfig::load()`.

// Pool config will be rebuilt in Epic 04
// For now, we keep the config module structure
pub mod database {
    use config::{Config, ConfigError, Environment, File};
    use serde::Deserialize;

    #[derive(Debug, Deserialize, Default)]
    pub struct DatabaseConfig {
        #[serde(default = "default_db_url")]
        pub url: String,
        #[serde(default = "default_max_connections")]
        pub max_connections: usize,
        #[serde(default = "default_pool_timeout_seconds")]
        pub pool_timeout_seconds: u64,
    }

    fn default_db_url() -> String {
        "postgres://postgres:postgres@localhost:5432/lifeguard_dev".to_string()
    }

    fn default_max_connections() -> usize {
        10
    }

    fn default_pool_timeout_seconds() -> u64 {
        30
    }

    impl DatabaseConfig {
        /// Loads configuration from `config/config.toml` and overlays with environment variables.
        ///
        /// # Errors
        ///
        /// Returns `ConfigError` if configuration loading or parsing fails.
        pub fn load() -> Result<Self, ConfigError> {
            Config::builder()
                .add_source(File::with_name("config/config").required(false))
                .add_source(Environment::with_prefix("LIFEGUARD").separator("__"))
                .build()?
                .try_deserialize::<DatabaseConfig>()
        }
    }
}

pub use database::*;
