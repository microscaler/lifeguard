use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;

#[derive(Debug, Deserialize, Default)]
pub struct DatabaseConfig {
    #[serde(default = "default_db_url")]
    pub url: String,
    #[serde(default = "default_max_connections")]
    pub max_connections: i32,
    #[serde(default = "default_pool_timeout_seconds")]
    pub pool_timeout_seconds: u64,
}

fn default_db_url() -> String {
    "postgres://postgres:postgres@localhost:5432/lifeguard_dev".to_string()
}

fn default_max_connections() -> i32 {
    10
}

fn default_pool_timeout_seconds() -> u64 {
    30 // Default timeout of 30 seconds
}

impl DatabaseConfig {
    /// Load the database configuration from `config/config.toml`, falling back to env vars.
    pub fn load() -> Result<Self, ConfigError> {
        // Build configuration by reading the TOML file (optional) and environment variables
        let builder = Config::builder()
            .add_source(File::with_name("config/config.toml").required(false))
            .add_source(Environment::with_prefix("LIFEGUARD").separator("__"));

        // Try to build the configuration, handling missing or unreadable file
        let settings = match builder.build() {
            Ok(cfg) => cfg,
            Err(err) => {
                // If the file existed but was unreadable (parse error, permission issue, etc.), log a warning and retry with env only
                if std::path::Path::new("config/config.toml").exists() {
                    eprintln!(
                        "Warning: failed to load config file, falling back to env. Error: {}",
                        err
                    );
                }
                // Retry using only environment variables as source
                Config::builder()
                    .add_source(Environment::with_prefix("LIFEGUARD").separator("__"))
                    .build()
                    .map_err(|env_err| {
                        // If even environment loading fails, return a clear combined error
                        ConfigError::Message(format!(
                            "Failed to load configuration from file and env: {}, then env-only error: {}",
                            err, env_err
                        ))
                    })?
            }
        };

        // Deserialize the configuration into our DatabaseConfig struct
        let db_config: DatabaseConfig =
            settings.get::<DatabaseConfig>("database").map_err(|e| {
                // Provide a clear error if the database config section is missing or invalid
                ConfigError::Message(format!(
                    "Database configuration could not be loaded from file or environment: {}",
                    e
                ))
            })?;

        Ok(db_config)
    }
}
