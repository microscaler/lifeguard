//! Runtime configuration, read from `LIFEREFLECTOR_*` environment variables.

use std::time::Duration;

/// How one table is brought back in sync after the reflector missed notifications.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatchUp {
    /// Invalidate every cached key of the table (`SCAN` + `UNLINK` on `{prefix}{table}:*`).
    /// Always correct; costs a cold cache for that table.
    Flush,
    /// Invalidate only rows whose `column` (a `timestamptz` maintained on every write) moved
    /// since the watermark. Cheaper, but blind to hard deletes, so only for tables that never
    /// hard-delete (append/update-only or soft-delete).
    Ranged {
        /// The change-timestamp column.
        column: String,
    },
}

/// A table the reflector knows how to catch up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSpec {
    /// Table name as triggers report it.
    pub table: String,
    /// Catch-up strategy.
    pub catch_up: CatchUp,
}

/// Reflector configuration.
#[derive(Debug, Clone)]
pub struct Config {
    /// Direct Postgres DSN (not through `PgBouncer` in transaction mode: LISTEN and session
    /// advisory locks need a dedicated server session).
    pub pg_url: String,
    /// Redis URL (`redis://[:password@]host:port[/db]`).
    pub redis_url: String,
    /// NOTIFY channels to LISTEN on.
    pub channels: Vec<String>,
    /// Cache key prefix; keys are `{prefix}{table}:{id}`.
    pub key_prefix: String,
    /// Tables covered by catch-up. Notifications for other tables are still invalidated.
    pub tables: Vec<TableSpec>,
    /// Redis key holding the last processed trigger timestamp (ms).
    pub watermark_key: String,
    /// Safety margin subtracted from the watermark for ranged catch-up (clock skew between the
    /// trigger's `clock_timestamp()` and commit; long transactions).
    pub catch_up_slack: Duration,
    /// Session advisory-lock key; one active reflector per key.
    pub lock_key: i64,
    /// Most keys per invalidation batch.
    pub batch_max: usize,
    /// How long to let a non-full batch gather before flushing (0: flush when the queue drains).
    pub batch_delay: Duration,
    /// Sleep between notification-queue polls when idle (`may_postgres` has no wake-up).
    pub poll_interval: Duration,
    /// Postgres/Redis liveness check and standby lock-retry period.
    pub heartbeat: Duration,
    /// Keys held while Redis is failing before the reflector gives up on them and schedules a
    /// catch-up instead (bounds memory during a Redis outage).
    pub max_pending: usize,
    /// Operator-asserted eviction policy, used only when `CONFIG GET` is unavailable (managed
    /// Redis often disables it).
    pub asserted_eviction_policy: Option<String>,
    /// `host:port` for `/metrics` and `/healthz`; `None` disables the endpoint.
    pub metrics_addr: Option<String>,
}

/// A configuration value that could not be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError(pub String);

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ConfigError {}

/// Default advisory-lock key: a fixed 63-bit value derived from the name, so every reflector of
/// a database contends for the same lock unless told otherwise.
pub const DEFAULT_LOCK_KEY: i64 = 0x4c66_5265_666c_6563; // "LfReflec"

impl Config {
    /// Defaults for everything but the two URLs.
    #[must_use]
    pub fn new(pg_url: impl Into<String>, redis_url: impl Into<String>) -> Self {
        Self {
            pg_url: pg_url.into(),
            redis_url: redis_url.into(),
            channels: vec!["lifeguard_changes".into()],
            key_prefix: "lifeguard:model:".into(),
            tables: Vec::new(),
            watermark_key: "lifereflector:watermark".into(),
            catch_up_slack: Duration::from_secs(5),
            lock_key: DEFAULT_LOCK_KEY,
            batch_max: 512,
            batch_delay: Duration::ZERO,
            poll_interval: Duration::from_micros(500),
            heartbeat: Duration::from_secs(2),
            max_pending: 200_000,
            asserted_eviction_policy: None,
            metrics_addr: Some("0.0.0.0:9187".into()),
        }
    }

    /// Read the configuration from the environment.
    ///
    /// Required: `LIFEREFLECTOR_PG_URL`, `LIFEREFLECTOR_REDIS_URL`. Optional:
    /// `LIFEREFLECTOR_CHANNELS` (comma list), `LIFEREFLECTOR_KEY_PREFIX`,
    /// `LIFEREFLECTOR_TABLES` (`table` for flush catch-up, `table=column` for ranged),
    /// `LIFEREFLECTOR_WATERMARK_KEY`, `LIFEREFLECTOR_CATCHUP_SLACK_MS`, `LIFEREFLECTOR_LOCK_KEY`,
    /// `LIFEREFLECTOR_BATCH_MAX`, `LIFEREFLECTOR_BATCH_DELAY_MS`, `LIFEREFLECTOR_POLL_US`,
    /// `LIFEREFLECTOR_HEARTBEAT_MS`, `LIFEREFLECTOR_MAX_PENDING`,
    /// `LIFEREFLECTOR_REDIS_EVICTION_POLICY`, `LIFEREFLECTOR_METRICS_ADDR` (`off` disables).
    ///
    /// # Errors
    ///
    /// A missing URL or an unparsable value.
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|k| std::env::var(k).ok())
    }

    /// [`Config::from_env`] over an arbitrary lookup (tests).
    ///
    /// # Errors
    ///
    /// See [`Config::from_env`].
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let need = |k: &str| {
            get(k)
                .filter(|v| !v.trim().is_empty())
                .ok_or_else(|| ConfigError(format!("{k} is required")))
        };
        let mut c = Self::new(
            need("LIFEREFLECTOR_PG_URL")?,
            need("LIFEREFLECTOR_REDIS_URL")?,
        );
        if let Some(v) = get("LIFEREFLECTOR_CHANNELS") {
            c.channels = split_list(&v);
            if c.channels.is_empty() {
                return Err(ConfigError("LIFEREFLECTOR_CHANNELS is empty".into()));
            }
            for ch in &c.channels {
                if !is_identifier(ch) {
                    return Err(ConfigError(format!(
                        "channel `{ch}` is not a plain identifier"
                    )));
                }
            }
        }
        if let Some(v) = get("LIFEREFLECTOR_KEY_PREFIX") {
            c.key_prefix = v;
        }
        if let Some(v) = get("LIFEREFLECTOR_TABLES") {
            c.tables = parse_tables(&v)?;
        }
        if let Some(v) = get("LIFEREFLECTOR_WATERMARK_KEY") {
            c.watermark_key = v;
        }
        let num = |k: &str| -> Result<Option<u64>, ConfigError> {
            get(k)
                .map(|v| {
                    v.trim()
                        .parse::<u64>()
                        .map_err(|e| ConfigError(format!("{k}={v}: {e}")))
                })
                .transpose()
        };
        if let Some(v) = num("LIFEREFLECTOR_CATCHUP_SLACK_MS")? {
            c.catch_up_slack = Duration::from_millis(v);
        }
        if let Some(v) = get("LIFEREFLECTOR_LOCK_KEY") {
            c.lock_key = v
                .trim()
                .parse()
                .map_err(|e| ConfigError(format!("LIFEREFLECTOR_LOCK_KEY={v}: {e}")))?;
        }
        if let Some(v) = num("LIFEREFLECTOR_BATCH_MAX")? {
            c.batch_max = usize::try_from(v.max(1)).unwrap_or(usize::MAX);
        }
        if let Some(v) = num("LIFEREFLECTOR_BATCH_DELAY_MS")? {
            c.batch_delay = Duration::from_millis(v);
        }
        if let Some(v) = num("LIFEREFLECTOR_POLL_US")? {
            c.poll_interval = Duration::from_micros(v.max(50));
        }
        if let Some(v) = num("LIFEREFLECTOR_HEARTBEAT_MS")? {
            c.heartbeat = Duration::from_millis(v.max(100));
        }
        if let Some(v) = num("LIFEREFLECTOR_MAX_PENDING")? {
            c.max_pending = usize::try_from(v.max(1)).unwrap_or(usize::MAX);
        }
        c.asserted_eviction_policy = get("LIFEREFLECTOR_REDIS_EVICTION_POLICY")
            .map(|v| v.trim().to_ascii_lowercase())
            .filter(|v| !v.is_empty());
        if let Some(v) = get("LIFEREFLECTOR_METRICS_ADDR") {
            c.metrics_addr = if v.eq_ignore_ascii_case("off") || v.trim().is_empty() {
                None
            } else {
                Some(v)
            };
        }
        Ok(c)
    }

    /// The catch-up spec for `table`, if configured.
    #[must_use]
    pub fn table(&self, table: &str) -> Option<&TableSpec> {
        self.tables.iter().find(|t| t.table == table)
    }
}

fn split_list(v: &str) -> Vec<String> {
    v.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Lower-case-safe SQL identifier check (letters, digits, `_`, not starting with a digit). Table
/// and column names from configuration end up in SQL, so anything else is refused.
#[must_use]
pub fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        && s.len() <= 63
}

fn parse_tables(v: &str) -> Result<Vec<TableSpec>, ConfigError> {
    split_list(v)
        .into_iter()
        .map(|item| {
            let (table, catch_up) = match item.split_once('=') {
                Some((t, col)) => (
                    t.trim().to_owned(),
                    CatchUp::Ranged {
                        column: col.trim().to_owned(),
                    },
                ),
                None => (item.clone(), CatchUp::Flush),
            };
            if !is_identifier(&table) {
                return Err(ConfigError(format!(
                    "table `{table}` is not a plain identifier"
                )));
            }
            if let CatchUp::Ranged { column } = &catch_up {
                if !is_identifier(column) {
                    return Err(ConfigError(format!(
                        "catch-up column `{column}` of `{table}` is not a plain identifier"
                    )));
                }
            }
            Ok(TableSpec { table, catch_up })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn cfg(pairs: &[(&str, &str)]) -> Result<Config, ConfigError> {
        let m: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        Config::from_lookup(|k| m.get(k).cloned())
    }

    #[test]
    fn urls_are_required() {
        assert!(cfg(&[]).is_err());
        assert!(cfg(&[("LIFEREFLECTOR_PG_URL", "x")]).is_err());
        let c = cfg(&[
            ("LIFEREFLECTOR_PG_URL", "pg"),
            ("LIFEREFLECTOR_REDIS_URL", "r"),
        ])
        .unwrap();
        assert_eq!(c.channels, vec!["lifeguard_changes"]);
        assert_eq!(c.batch_max, 512);
        assert_eq!(c.lock_key, DEFAULT_LOCK_KEY);
    }

    #[test]
    fn tables_and_numbers() {
        let c = cfg(&[
            ("LIFEREFLECTOR_PG_URL", "pg"),
            ("LIFEREFLECTOR_REDIS_URL", "r"),
            ("LIFEREFLECTOR_CHANNELS", "a, b"),
            ("LIFEREFLECTOR_TABLES", "items, ledger=changed_at"),
            ("LIFEREFLECTOR_BATCH_MAX", "64"),
            ("LIFEREFLECTOR_METRICS_ADDR", "off"),
            ("LIFEREFLECTOR_REDIS_EVICTION_POLICY", "AllKeys-LRU"),
        ])
        .unwrap();
        assert_eq!(c.channels, vec!["a", "b"]);
        assert_eq!(c.table("items").unwrap().catch_up, CatchUp::Flush);
        assert_eq!(
            c.table("ledger").unwrap().catch_up,
            CatchUp::Ranged {
                column: "changed_at".into()
            }
        );
        assert_eq!(c.batch_max, 64);
        assert_eq!(c.metrics_addr, None);
        assert_eq!(c.asserted_eviction_policy.as_deref(), Some("allkeys-lru"));
    }

    #[test]
    fn identifiers_are_validated() {
        let base = [
            ("LIFEREFLECTOR_PG_URL", "pg"),
            ("LIFEREFLECTOR_REDIS_URL", "r"),
        ];
        let mut bad = base.to_vec();
        bad.push(("LIFEREFLECTOR_TABLES", "items; DROP TABLE x"));
        assert!(cfg(&bad).is_err());
        let mut bad = base.to_vec();
        bad.push(("LIFEREFLECTOR_TABLES", "items=col umn"));
        assert!(cfg(&bad).is_err());
        let mut bad = base.to_vec();
        bad.push(("LIFEREFLECTOR_CHANNELS", "a-b"));
        assert!(cfg(&bad).is_err());
        let mut bad = base.to_vec();
        bad.push(("LIFEREFLECTOR_BATCH_MAX", "many"));
        assert!(cfg(&bad).is_err());
    }
}
