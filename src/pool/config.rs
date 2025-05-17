use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "postgres://postgres:postgres@localhost:5432/postgres".into(),
            max_connections: 5,
        }
    }
}

impl DatabaseConfig {
    /// Load config from:
    /// 1. ./config/config.toml (if it exists)
    /// 2. Environment variables (LIFEGUARD__DATABASE__...)
    /// 3. Sensible defaults
    pub fn load() -> DatabaseConfig {
        let mut settings = config::Config::default();

        // Prefer ./config/config.toml if it exists
        if Path::new("config/config.toml").exists() {
            settings = config::Config::builder()
                .add_source(config::File::with_name("config/config").required(false))
                .build()
                .unwrap_or_else(|_| config::Config::default());
        } else {
            // Fall back to ./config.toml if not using nested folder
            settings = config::Config::builder()
                .add_source(config::File::with_name("config").required(false))
                .build()
                .unwrap_or_else(|_| config::Config::default());
        }

        // Then override with env vars
        settings = config::Config::builder()
            .add_source(config::Environment::with_prefix("LIFEGUARD").separator("__"))
            .build()
            .unwrap_or_else(|_| config::Config::default());

        // Finally default
        settings
            .get::<DatabaseConfig>("database")
            .unwrap_or_else(|_| DatabaseConfig::default())
    }
}
