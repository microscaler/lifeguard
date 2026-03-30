//! Pool and database configuration loaded from `config/config.toml` and `LIFEGUARD__*` env vars.
//!
//! Use [`LifeguardPoolSettings::from_database_config`] with [`DatabaseConfig`] so
//! `pool_timeout_seconds` and queue depth apply to [`super::LifeguardPool`](crate::pool::LifeguardPool).

use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Deserialize)]
pub struct DatabaseConfig {
    #[serde(default = "default_db_url")]
    pub url: String,
    #[serde(default = "default_max_connections")]
    pub max_connections: usize,
    /// Maximum seconds to wait when submitting work to a saturated pool worker queue (Hikari `connectionTimeout` analogue).
    /// Default **30** matches [`LifeguardPoolSettings::default`] (same default as `acquire_timeout`; PRD R1.3).
    #[serde(default = "default_pool_timeout_seconds")]
    pub pool_timeout_seconds: u64,
    /// Bounded jobs waiting per worker; bounds memory under spike load (PRD R6.x).
    #[serde(default = "default_pool_job_queue_depth_per_worker")]
    pub pool_job_queue_depth_per_worker: usize,
    /// How often [`super::wal::WalLagMonitor`] polls the replica for receive/replay lag (milliseconds).
    #[serde(default = "default_wal_lag_poll_interval_ms")]
    pub wal_lag_poll_interval_ms: u64,
    /// Receive/replay WAL byte lag above this marks the replica as lagging (PRD R7.2). **`0`** disables
    /// the byte criterion (use time-only when [`Self::wal_lag_max_apply_lag_seconds`] is set). If **both**
    /// this and [`Self::wal_lag_max_apply_lag_seconds`] are zero/disabled, the effective byte threshold is
    /// **1_000_000** (same as historical hardcoded default).
    #[serde(default = "default_wal_lag_max_bytes")]
    pub wal_lag_max_bytes: u64,
    /// When **> 0**, standby apply lag (`clock_timestamp() - pg_last_xact_replay_timestamp()`) above this
    /// many seconds also marks lagging. **`0`** disables (default). Combined with [`Self::wal_lag_max_bytes`]
    /// using **OR** semantics when both are active.
    #[serde(default)]
    pub wal_lag_max_apply_lag_seconds: u64,
    /// Max failed **initial** connects to the replica for [`super::wal::WalLagMonitor`] before giving up
    /// (PRD R7.3). **`0`** = unlimited retries (default).
    #[serde(default)]
    pub wal_lag_monitor_max_connect_retries: u32,
    /// Optional max age of a pooled `Client` before rotation (PRD R3.1). **`0`** = disabled.
    #[serde(default)]
    pub max_connection_lifetime_seconds: u64,
    /// Extra jitter for max lifetime (milliseconds), spread across slots (PRD R3.1). **`0`** = none.
    #[serde(default)]
    pub max_connection_lifetime_jitter_ms: u64,
    /// When **> 0**, each idle pool worker runs a cheap `SELECT 1` at this interval to detect dead
    /// TCP sessions before the next real query (PRD R4.2). **`0`** disables idle probes (default).
    /// Values are clamped to **1s–1h** when loaded from file/env; use [`LifeguardPoolSettings`] directly
    /// for sub-second intervals in tests.
    #[serde(default = "default_idle_liveness_interval_ms")]
    pub idle_liveness_interval_ms: u64,
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

fn default_wal_lag_poll_interval_ms() -> u64 {
    500
}

fn default_wal_lag_max_bytes() -> u64 {
    1_000_000
}

fn default_idle_liveness_interval_ms() -> u64 {
    0
}

/// Matches `config/config.toml` `[database]` table (single source for file + env merge).
#[derive(Debug, Deserialize)]
struct ConfigRoot {
    #[serde(default)]
    database: DatabaseConfig,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: default_db_url(),
            max_connections: default_max_connections(),
            pool_timeout_seconds: default_pool_timeout_seconds(),
            pool_job_queue_depth_per_worker: default_pool_job_queue_depth_per_worker(),
            wal_lag_poll_interval_ms: default_wal_lag_poll_interval_ms(),
            wal_lag_max_bytes: default_wal_lag_max_bytes(),
            wal_lag_max_apply_lag_seconds: 0,
            wal_lag_monitor_max_connect_retries: 0,
            max_connection_lifetime_seconds: 0,
            max_connection_lifetime_jitter_ms: 0,
            idle_liveness_interval_ms: default_idle_liveness_interval_ms(),
        }
    }
}

impl DatabaseConfig {
    /// Loads configuration from `config/config.toml` and overlays with environment variables.
    ///
    /// # File layout
    ///
    /// Values live under **`[database]`** in `config/config.toml` (see repository `config/config.toml`).
    ///
    /// # Environment (prefix `LIFEGUARD`, separator `__`)
    ///
    /// Nested keys follow the TOML path: `database.url` → `LIFEGUARD__DATABASE__URL`,
    /// `database.pool_timeout_seconds` → `LIFEGUARD__DATABASE__POOL_TIMEOUT_SECONDS`, etc.
    ///
    /// | Field | Example env var |
    /// |-------|-----------------|
    /// | `url` | `LIFEGUARD__DATABASE__URL` |
    /// | `max_connections` | `LIFEGUARD__DATABASE__MAX_CONNECTIONS` |
    /// | `pool_timeout_seconds` | `LIFEGUARD__DATABASE__POOL_TIMEOUT_SECONDS` |
    /// | `pool_job_queue_depth_per_worker` | `LIFEGUARD__DATABASE__POOL_JOB_QUEUE_DEPTH_PER_WORKER` |
    /// | `wal_lag_poll_interval_ms` | `LIFEGUARD__DATABASE__WAL_LAG_POLL_INTERVAL_MS` |
    /// | `wal_lag_max_bytes` | `LIFEGUARD__DATABASE__WAL_LAG_MAX_BYTES` |
    /// | `wal_lag_max_apply_lag_seconds` | `LIFEGUARD__DATABASE__WAL_LAG_MAX_APPLY_LAG_SECONDS` |
    /// | `wal_lag_monitor_max_connect_retries` | `LIFEGUARD__DATABASE__WAL_LAG_MONITOR_MAX_CONNECT_RETRIES` |
    /// | `max_connection_lifetime_seconds` | `LIFEGUARD__DATABASE__MAX_CONNECTION_LIFETIME_SECONDS` |
    /// | `max_connection_lifetime_jitter_ms` | `LIFEGUARD__DATABASE__MAX_CONNECTION_LIFETIME_JITTER_MS` |
    /// | `idle_liveness_interval_ms` | `LIFEGUARD__DATABASE__IDLE_LIVENESS_INTERVAL_MS` |
    ///
    /// The environment layer is merged **after** the file and overrides matching keys (PRD R2.2).
    pub fn load() -> Result<Self, ConfigError> {
        let root: ConfigRoot = Config::builder()
            .add_source(File::with_name("config/config").required(false))
            .add_source(Environment::with_prefix("LIFEGUARD").separator("__"))
            .build()?
            .try_deserialize()?;
        Ok(root.database)
    }
}

/// Runtime knobs for [`crate::pool::LifeguardPool`] construction (acquire timeout, queue bounds).
#[derive(Debug, Clone)]
pub struct LifeguardPoolSettings {
    /// Wall-clock budget to place a job on a worker queue when that queue is full.
    pub acquire_timeout: Duration,
    /// Capacity of each worker’s inbound job channel (`crossbeam_channel::bounded`).
    pub job_queue_capacity_per_worker: usize,
    /// Interval between [`super::wal::WalLagMonitor`] lag polls on the replica connection.
    pub wal_lag_poll_interval: Duration,
    /// Byte lag threshold for [`super::wal::WalLagMonitor`] (receive vs replay LSN on the standby). **`0`**
    /// disables the byte check when [`Self::wal_lag_max_apply_lag`] is **Some**; if both byte and time
    /// limits are “off”, [`super::wal::WalLagPolicy::from_pool_settings`] restores **1 MiB** bytes.
    pub wal_lag_max_bytes: u64,
    /// Optional maximum standby **apply** lag in wall-clock time (from `pg_last_xact_replay_timestamp()`).
    pub wal_lag_max_apply_lag: Option<Duration>,
    /// Cap failed WAL monitor connect attempts before giving up (PRD R7.3). **`0`** = unlimited.
    pub wal_lag_monitor_max_connect_retries: u32,
    /// Rotate each slot’s client after this age (PRD R3.1). **`None`** = off.
    pub max_connection_lifetime: Option<Duration>,
    /// Jitter added to [`Self::max_connection_lifetime`] to avoid synchronized rotations.
    pub max_connection_lifetime_jitter: Duration,
    /// When **Some**, workers that are **idle** (no queued work) run `SELECT 1` on this interval
    /// so half-open TCP sessions are detected and healed (PRD R4.2). **`None`** disables probes.
    pub idle_liveness_interval: Option<Duration>,
}

impl Default for LifeguardPoolSettings {
    fn default() -> Self {
        Self {
            acquire_timeout: Duration::from_secs(30),
            job_queue_capacity_per_worker: default_pool_job_queue_depth_per_worker(),
            wal_lag_poll_interval: Duration::from_millis(default_wal_lag_poll_interval_ms()),
            wal_lag_max_bytes: default_wal_lag_max_bytes(),
            wal_lag_max_apply_lag: None,
            wal_lag_monitor_max_connect_retries: 0,
            max_connection_lifetime: None,
            max_connection_lifetime_jitter: Duration::ZERO,
            idle_liveness_interval: None,
        }
    }
}

impl LifeguardPoolSettings {
    /// Maps file/env [`DatabaseConfig`] into pool settings used by [`crate::pool::LifeguardPool`].
    #[must_use]
    pub fn from_database_config(cfg: &DatabaseConfig) -> Self {
        let poll_ms = cfg.wal_lag_poll_interval_ms.clamp(10, 60_000);
        let idle_liveness_interval = if cfg.idle_liveness_interval_ms == 0 {
            None
        } else {
            let ms = cfg.idle_liveness_interval_ms.clamp(1_000, 3_600_000);
            Some(Duration::from_millis(ms))
        };
        let wal_lag_max_apply_lag = if cfg.wal_lag_max_apply_lag_seconds == 0 {
            None
        } else {
            let secs = cfg.wal_lag_max_apply_lag_seconds.clamp(1, 86_400);
            Some(Duration::from_secs(secs))
        };
        let wal_lag_max_bytes = cfg.wal_lag_max_bytes.min(1 << 40);
        let wal_lag_monitor_max_connect_retries = cfg.wal_lag_monitor_max_connect_retries.min(100_000);
        let max_connection_lifetime = if cfg.max_connection_lifetime_seconds == 0 {
            None
        } else {
            let secs = cfg.max_connection_lifetime_seconds.clamp(1, 86400 * 30);
            Some(Duration::from_secs(secs))
        };
        let max_connection_lifetime_jitter =
            Duration::from_millis(cfg.max_connection_lifetime_jitter_ms.min(3_600_000));
        Self {
            acquire_timeout: Duration::from_secs(cfg.pool_timeout_seconds.max(1)),
            job_queue_capacity_per_worker: cfg.pool_job_queue_depth_per_worker.max(1),
            wal_lag_poll_interval: Duration::from_millis(poll_ms),
            wal_lag_max_bytes,
            wal_lag_max_apply_lag,
            wal_lag_monitor_max_connect_retries,
            max_connection_lifetime,
            max_connection_lifetime_jitter,
            idle_liveness_interval,
        }
    }
}

#[cfg(test)]
mod tests {
    // `DatabaseConfig::load` and mutex tests use `.expect()` for clear failure messages; the crate
    // denies `expect_used` in library code — allow here only.
    #![allow(clippy::expect_used)]

    use super::*;
    use std::sync::Mutex;

    /// Serialize env mutation for `DatabaseConfig::load` tests (nextest runs tests in parallel).
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn clear_lifeguard_env() {
        let keys: Vec<String> = std::env::vars()
            .map(|(k, _)| k)
            .filter(|k| k.starts_with("LIFEGUARD__"))
            .collect();
        for k in keys {
            std::env::remove_var(&k);
        }
    }

    #[test]
    fn database_config_load_reads_nested_database_section_from_file() {
        let _g = ENV_LOCK.lock().expect("env lock poisoned");
        clear_lifeguard_env();
        let cfg = DatabaseConfig::load().expect("config/config.toml [database] must deserialize");
        assert!(
            cfg.url.contains("postgres://"),
            "url from file: {}",
            cfg.url
        );
        assert_eq!(cfg.pool_timeout_seconds, 5);
    }

    #[test]
    fn lifeguard_env_overrides_database_nested_keys() {
        let _g = ENV_LOCK.lock().expect("env lock poisoned");
        clear_lifeguard_env();
        std::env::set_var("LIFEGUARD__DATABASE__POOL_TIMEOUT_SECONDS", "88");
        std::env::set_var("LIFEGUARD__DATABASE__MAX_CONNECTIONS", "7");
        std::env::set_var(
            "LIFEGUARD__DATABASE__POOL_JOB_QUEUE_DEPTH_PER_WORKER",
            "4",
        );
        std::env::set_var("LIFEGUARD__DATABASE__WAL_LAG_POLL_INTERVAL_MS", "333");
        let cfg = DatabaseConfig::load().expect("load with env overlay");
        assert_eq!(cfg.pool_timeout_seconds, 88);
        assert_eq!(cfg.max_connections, 7);
        assert_eq!(cfg.pool_job_queue_depth_per_worker, 4);
        assert_eq!(cfg.wal_lag_poll_interval_ms, 333);
        clear_lifeguard_env();
    }

    #[test]
    fn lifeguard_pool_settings_from_database_config_maps_fields() {
        let db = DatabaseConfig {
            pool_timeout_seconds: 42,
            pool_job_queue_depth_per_worker: 3,
            wal_lag_poll_interval_ms: 250,
            wal_lag_max_bytes: 2_000_000,
            wal_lag_max_apply_lag_seconds: 30,
            ..Default::default()
        };
        let s = LifeguardPoolSettings::from_database_config(&db);
        assert_eq!(s.acquire_timeout, Duration::from_secs(42));
        assert_eq!(s.job_queue_capacity_per_worker, 3);
        assert_eq!(s.wal_lag_poll_interval, Duration::from_millis(250));
        assert_eq!(s.wal_lag_max_bytes, 2_000_000);
        assert_eq!(s.wal_lag_max_apply_lag, Some(Duration::from_secs(30)));
        assert!(s.idle_liveness_interval.is_none());
    }

    #[test]
    fn lifeguard_pool_settings_wal_apply_lag_none_when_zero() {
        let db = DatabaseConfig {
            wal_lag_max_apply_lag_seconds: 0,
            ..Default::default()
        };
        let s = LifeguardPoolSettings::from_database_config(&db);
        assert!(s.wal_lag_max_apply_lag.is_none());
    }

    #[test]
    fn lifeguard_pool_settings_max_connection_lifetime_and_retries() {
        let db = DatabaseConfig {
            wal_lag_monitor_max_connect_retries: 12,
            max_connection_lifetime_seconds: 3600,
            max_connection_lifetime_jitter_ms: 500,
            ..Default::default()
        };
        let s = LifeguardPoolSettings::from_database_config(&db);
        assert_eq!(s.wal_lag_monitor_max_connect_retries, 12);
        assert_eq!(s.max_connection_lifetime, Some(Duration::from_secs(3600)));
        assert_eq!(s.max_connection_lifetime_jitter, Duration::from_millis(500));
    }

    #[test]
    fn lifeguard_pool_settings_idle_liveness_none_when_zero() {
        let db = DatabaseConfig {
            idle_liveness_interval_ms: 0,
            ..Default::default()
        };
        let s = LifeguardPoolSettings::from_database_config(&db);
        assert!(s.idle_liveness_interval.is_none());
    }

    #[test]
    fn lifeguard_pool_settings_idle_liveness_clamped_from_file_config() {
        let db = DatabaseConfig {
            idle_liveness_interval_ms: 500,
            ..Default::default()
        };
        let s = LifeguardPoolSettings::from_database_config(&db);
        assert_eq!(s.idle_liveness_interval, Some(Duration::from_secs(1)));

        let db2 = DatabaseConfig {
            idle_liveness_interval_ms: 90_000,
            ..Default::default()
        };
        let s2 = LifeguardPoolSettings::from_database_config(&db2);
        assert_eq!(s2.idle_liveness_interval, Some(Duration::from_secs(90)));
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

    #[test]
    fn lifeguard_pool_settings_clamps_wal_poll_interval_ms() {
        let db = DatabaseConfig {
            wal_lag_poll_interval_ms: 3,
            ..Default::default()
        };
        let s = LifeguardPoolSettings::from_database_config(&db);
        assert_eq!(s.wal_lag_poll_interval, Duration::from_millis(10));

        let db2 = DatabaseConfig {
            wal_lag_poll_interval_ms: 120_000,
            ..Default::default()
        };
        let s2 = LifeguardPoolSettings::from_database_config(&db2);
        assert_eq!(s2.wal_lag_poll_interval, Duration::from_secs(60));
    }
}
