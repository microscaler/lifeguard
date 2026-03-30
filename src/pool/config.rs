//! Pool and database configuration loaded from `config/config.toml` and `LIFEGUARD__*` env vars.
//!
//! Use [`LifeguardPoolSettings::from_database_config`] with [`DatabaseConfig`] so
//! `pool_timeout_seconds` and queue depth apply to [`super::LifeguardPool`](crate::pool::LifeguardPool).

use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Deserialize, Default)]
pub struct DatabaseConfig {
    #[serde(default = "default_db_url")]
    pub url: String,
    #[serde(default = "default_max_connections")]
    pub max_connections: usize,
    /// Maximum seconds to wait when submitting work to a saturated pool worker queue (Hikari `connectionTimeout` analogue).
    #[serde(default = "default_pool_timeout_seconds")]
    pub pool_timeout_seconds: u64,
    /// Bounded jobs waiting per worker; bounds memory under spike load (PRD R6.x).
    #[serde(default = "default_pool_job_queue_depth_per_worker")]
    pub pool_job_queue_depth_per_worker: usize,
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

fn default_pool_job_queue_depth_per_worker() -> usize {
    8
}

impl DatabaseConfig {
    /// Loads configuration from `config/config.toml` and overlays with environment variables.
    ///
    /// Prefix: `LIFEGUARD_` with nested keys as `LIFEGUARD_URL`, `LIFEGUARD_MAX_CONNECTIONS`,
    /// `LIFEGUARD_POOL_TIMEOUT_SECONDS`, `LIFEGUARD_POOL_JOB_QUEUE_DEPTH_PER_WORKER`, etc.
    pub fn load() -> Result<Self, ConfigError> {
        Config::builder()
            .add_source(File::with_name("config/config").required(false))
            .add_source(Environment::with_prefix("LIFEGUARD").separator("__"))
            .build()?
            .try_deserialize::<DatabaseConfig>()
    }
}

/// Runtime knobs for [`crate::pool::LifeguardPool`] construction (acquire timeout, queue bounds).
#[derive(Debug, Clone)]
pub struct LifeguardPoolSettings {
    /// Wall-clock budget to place a job on a worker queue when that queue is full.
    pub acquire_timeout: Duration,
    /// Capacity of each worker’s inbound job channel (`crossbeam_channel::bounded`).
    pub job_queue_capacity_per_worker: usize,
}

impl Default for LifeguardPoolSettings {
    fn default() -> Self {
        Self {
            acquire_timeout: Duration::from_secs(30),
            job_queue_capacity_per_worker: default_pool_job_queue_depth_per_worker(),
        }
    }
}

impl LifeguardPoolSettings {
    /// Maps file/env [`DatabaseConfig`] into pool settings used by [`crate::pool::LifeguardPool`].
    #[must_use]
    pub fn from_database_config(cfg: &DatabaseConfig) -> Self {
        Self {
            acquire_timeout: Duration::from_secs(cfg.pool_timeout_seconds.max(1)),
            job_queue_capacity_per_worker: cfg.pool_job_queue_depth_per_worker.max(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifeguard_pool_settings_from_database_config_maps_fields() {
        let db = DatabaseConfig {
            pool_timeout_seconds: 42,
            pool_job_queue_depth_per_worker: 3,
            ..Default::default()
        };
        let s = LifeguardPoolSettings::from_database_config(&db);
        assert_eq!(s.acquire_timeout, Duration::from_secs(42));
        assert_eq!(s.job_queue_capacity_per_worker, 3);
    }

    #[test]
    fn lifeguard_pool_settings_clamps_minimums() {
        let db = DatabaseConfig {
            pool_timeout_seconds: 0,
            pool_job_queue_depth_per_worker: 0,
            ..Default::default()
        };
        let s = LifeguardPoolSettings::from_database_config(&db);
        assert_eq!(s.acquire_timeout, Duration::from_secs(1));
        assert_eq!(s.job_queue_capacity_per_worker, 1);
    }
}
