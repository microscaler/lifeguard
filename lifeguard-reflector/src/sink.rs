//! Where invalidations go: Redis in production, memory in tests.

use may_redis::{cmd, RedisClient, RedisError, RedisValue};
use std::collections::HashMap;
use std::sync::Mutex;

/// A failed cache operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SinkError(pub String);

impl std::fmt::Display for SinkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SinkError {}

impl From<RedisError> for SinkError {
    fn from(e: RedisError) -> Self {
        Self(e.to_string())
    }
}

/// The cache operations the reflector needs.
pub trait CacheSink: Send + Sync {
    /// Delete `keys` (missing keys are fine). Returns how many existed.
    ///
    /// # Errors
    ///
    /// The cache is unreachable or refused the command.
    fn invalidate(&self, keys: &[String]) -> Result<u64, SinkError>;

    /// Delete every key matching the glob `pattern`. Returns how many were deleted.
    ///
    /// # Errors
    ///
    /// As [`CacheSink::invalidate`].
    fn invalidate_matching(&self, pattern: &str) -> Result<u64, SinkError> {
        let mut cursor = 0;
        let mut deleted = 0;
        loop {
            let (next, n) = self.invalidate_matching_page(pattern, cursor)?;
            deleted += n;
            if next == 0 {
                return Ok(deleted);
            }
            cursor = next;
        }
    }

    /// One page of [`CacheSink::invalidate_matching`]: scan from `cursor` (0 to start), delete
    /// what matched, and return the next cursor (0 when the scan is complete) and the count.
    ///
    /// # Errors
    ///
    /// As [`CacheSink::invalidate`].
    fn invalidate_matching_page(&self, pattern: &str, cursor: i64)
        -> Result<(i64, u64), SinkError>;

    /// Read an integer value (`None` when absent).
    ///
    /// # Errors
    ///
    /// As [`CacheSink::invalidate`].
    fn get_i64(&self, key: &str) -> Result<Option<i64>, SinkError>;

    /// Store an integer value without expiry.
    ///
    /// # Errors
    ///
    /// As [`CacheSink::invalidate`].
    fn set_i64(&self, key: &str, value: i64) -> Result<(), SinkError>;

    /// Liveness check.
    ///
    /// # Errors
    ///
    /// As [`CacheSink::invalidate`].
    fn ping(&self) -> Result<(), SinkError>;

    /// The server's `maxmemory-policy`, or `None` when it cannot be read (e.g. `CONFIG` disabled).
    fn eviction_policy(&self) -> Option<String>;
}

/// Keys per `UNLINK`: large enough to amortise the round trip, small enough that one command
/// never blocks Redis noticeably (`UNLINK` frees memory off the main thread).
pub const UNLINK_CHUNK: usize = 1024;

/// [`CacheSink`] over a `may_redis` connection (pipelined, coroutine-native).
pub struct RedisSink {
    client: RedisClient,
}

impl RedisSink {
    /// Connect.
    ///
    /// # Errors
    ///
    /// The URL is invalid or the server unreachable.
    pub fn connect(url: &str) -> Result<Self, SinkError> {
        Ok(Self {
            client: RedisClient::connect_url(url)?,
        })
    }
}

fn as_u64(v: &RedisValue) -> u64 {
    match v {
        RedisValue::Integer(n) => u64::try_from(*n).unwrap_or(0),
        _ => 0,
    }
}

impl CacheSink for RedisSink {
    fn invalidate(&self, keys: &[String]) -> Result<u64, SinkError> {
        match keys.len() {
            0 => Ok(0),
            n if n <= UNLINK_CHUNK => {
                let n: i64 = self.client.execute(cmd("UNLINK").args(keys))?;
                Ok(u64::try_from(n).unwrap_or(0))
            }
            _ => {
                // Several chunks in one round trip.
                let mut p = self.client.pipeline();
                for chunk in keys.chunks(UNLINK_CHUNK) {
                    p.add(cmd("UNLINK").args(chunk));
                }
                let replies = p.execute_raw()?;
                let mut n = 0;
                for r in &replies {
                    if let RedisValue::Error(e) = r {
                        return Err(SinkError(e.clone()));
                    }
                    n += as_u64(r);
                }
                Ok(n)
            }
        }
    }

    fn invalidate_matching_page(
        &self,
        pattern: &str,
        cursor: i64,
    ) -> Result<(i64, u64), SinkError> {
        let (next, keys): (i64, Vec<String>) = self.client.execute(
            cmd("SCAN")
                .arg(cursor.to_string())
                .arg("MATCH")
                .arg(pattern)
                .arg("COUNT")
                .arg("1000"),
        )?;
        Ok((next, self.invalidate(&keys)?))
    }

    fn get_i64(&self, key: &str) -> Result<Option<i64>, SinkError> {
        let v: Option<String> = self.client.execute(cmd("GET").arg(key))?;
        Ok(v.and_then(|s| s.parse().ok()))
    }

    fn set_i64(&self, key: &str, value: i64) -> Result<(), SinkError> {
        let _: String = self
            .client
            .execute(cmd("SET").arg(key).arg(value.to_string()))?;
        Ok(())
    }

    fn ping(&self) -> Result<(), SinkError> {
        self.client.ping().map(|_| ()).map_err(Into::into)
    }

    fn eviction_policy(&self) -> Option<String> {
        let v: Vec<String> = self
            .client
            .execute(cmd("CONFIG").arg("GET").arg("maxmemory-policy"))
            .ok()?;
        v.get(1).map(|s| s.to_ascii_lowercase())
    }
}

/// In-memory [`CacheSink`] for tests and benchmarks of the reflector's own overhead.
#[derive(Default)]
pub struct MemorySink {
    /// Current keys and values.
    pub data: Mutex<HashMap<String, String>>,
    /// Eviction policy to report.
    pub policy: Option<String>,
    /// When set, every operation fails with this message.
    pub fail: Mutex<Option<String>>,
    /// Number of `invalidate` calls (round trips).
    pub invalidate_calls: std::sync::atomic::AtomicU64,
}

impl MemorySink {
    /// Empty sink reporting `allkeys-lru`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            policy: Some("allkeys-lru".into()),
            ..Self::default()
        }
    }

    /// Insert a key.
    pub fn put(&self, key: &str, value: &str) {
        self.lock().insert(key.to_owned(), value.to_owned());
    }

    /// `true` when `key` is present.
    #[must_use]
    pub fn contains(&self, key: &str) -> bool {
        self.lock().contains_key(key)
    }

    /// Make every operation fail (`Some`) or succeed again (`None`).
    pub fn set_failing(&self, msg: Option<&str>) {
        *self
            .fail
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = msg.map(str::to_owned);
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, String>> {
        self.data
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn check(&self) -> Result<(), SinkError> {
        match &*self
            .fail
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
        {
            Some(m) => Err(SinkError(m.clone())),
            None => Ok(()),
        }
    }
}

/// Minimal glob for tests: only a trailing `*` is supported.
fn glob_match(pattern: &str, key: &str) -> bool {
    pattern
        .strip_suffix('*')
        .map_or(pattern == key, |prefix| key.starts_with(prefix))
}

impl CacheSink for MemorySink {
    fn invalidate(&self, keys: &[String]) -> Result<u64, SinkError> {
        self.check()?;
        self.invalidate_calls
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut d = self.lock();
        Ok(keys
            .iter()
            .filter(|k| d.remove(k.as_str()).is_some())
            .count() as u64)
    }

    fn invalidate_matching_page(
        &self,
        pattern: &str,
        _cursor: i64,
    ) -> Result<(i64, u64), SinkError> {
        self.check()?;
        let mut d = self.lock();
        let before = d.len();
        d.retain(|k, _| !glob_match(pattern, k));
        Ok((0, (before - d.len()) as u64))
    }

    fn get_i64(&self, key: &str) -> Result<Option<i64>, SinkError> {
        self.check()?;
        Ok(self.lock().get(key).and_then(|v| v.parse().ok()))
    }

    fn set_i64(&self, key: &str, value: i64) -> Result<(), SinkError> {
        self.check()?;
        self.lock().insert(key.to_owned(), value.to_string());
        Ok(())
    }

    fn ping(&self) -> Result<(), SinkError> {
        self.check()
    }

    fn eviction_policy(&self) -> Option<String> {
        self.policy.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_sink_semantics() {
        let s = MemorySink::new();
        s.put("p:t:1", "a");
        s.put("p:t:2", "b");
        s.put("p:u:1", "c");
        assert_eq!(s.invalidate(&["p:t:1".into(), "p:t:9".into()]).unwrap(), 1);
        assert_eq!(s.invalidate_matching("p:t:*").unwrap(), 1);
        assert!(s.contains("p:u:1"));
        s.set_i64("wm", 5).unwrap();
        assert_eq!(s.get_i64("wm").unwrap(), Some(5));
        s.set_failing(Some("down"));
        assert!(s.ping().is_err());
        s.set_failing(None);
        assert!(s.ping().is_ok());
    }
}
