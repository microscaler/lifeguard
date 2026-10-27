//! The Postgres side: session, advisory lock, LISTEN, and the main loop around [`Engine`].

use crate::config::{Config, TableSpec};
use crate::engine::{Engine, Role, TakeOver};
use crate::metrics::metrics;
use crate::sink::{CacheSink, SinkError};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Why the reflector refused to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartError {
    /// The cache is configured so that invalidation-based coherence is unsafe.
    UnsafeCache(String),
    /// The cache could not be reached at start-up.
    Cache(String),
}

impl std::fmt::Display for StartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsafeCache(m) | Self::Cache(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for StartError {}

/// Refuse a `noeviction` cache.
///
/// The reflector only deletes; readers refill. With `noeviction`, a cache that reaches
/// `maxmemory` rejects those refills (and every other write) instead of evicting cold keys, so a
/// cache sized for the hot set turns into an outage. The policy comes from `CONFIG GET`; when the
/// server refuses that (common on managed Redis), the operator must state it in
/// `LIFEREFLECTOR_REDIS_EVICTION_POLICY`.
///
/// # Errors
///
/// [`StartError::UnsafeCache`] for `noeviction` or an unknown policy.
pub fn check_eviction_policy(sink: &dyn CacheSink, cfg: &Config) -> Result<String, StartError> {
    let policy = sink
        .eviction_policy()
        .or_else(|| cfg.asserted_eviction_policy.clone())
        .ok_or_else(|| {
            StartError::UnsafeCache(
                "cannot read Redis maxmemory-policy (CONFIG GET refused); set \
                 LIFEREFLECTOR_REDIS_EVICTION_POLICY to the server's policy to proceed"
                    .into(),
            )
        })?;
    if policy == "noeviction" {
        return Err(StartError::UnsafeCache(
            "Redis maxmemory-policy is noeviction: cache refills would fail once maxmemory is \
             reached; use allkeys-lru (or allkeys-lfu) for the cache instance"
                .into(),
        ));
    }
    if policy.starts_with("volatile-") {
        log::warn!(
            "lifereflector: maxmemory-policy {policy} only evicts keys with a TTL; cache entries \
             must be written with one"
        );
    }
    Ok(policy)
}

/// Builds a cache connection; called at start and after repeated cache failures.
pub type SinkFactory = dyn Fn() -> Result<Box<dyn CacheSink>, SinkError> + Send + Sync;

/// Run until `stop` is set: connect, wait for the lock as a listening standby, lead, and
/// reconnect on failure.
///
/// # Errors
///
/// Only start-up refusals ([`StartError`]); runtime failures are retried.
pub fn run(cfg: Config, connect_sink: &SinkFactory, stop: &AtomicBool) -> Result<(), StartError> {
    let cfg = Arc::new(cfg);
    let sink = connect_sink().map_err(|e| StartError::Cache(e.0))?;
    let policy = check_eviction_policy(&*sink, &cfg)?;
    log::info!(
        "lifereflector: cache eviction policy {policy}; channels {:?}; lock key {}",
        cfg.channels,
        cfg.lock_key
    );
    let mut engine = Engine::new(cfg.clone(), sink);
    let mut backoff = Duration::from_millis(200);
    while !stop.load(Ordering::Relaxed) {
        let started = Instant::now();
        match session(&cfg, &mut engine, connect_sink, stop) {
            Ok(()) => break,
            Err(e) => {
                log::warn!("lifereflector: session ended: {e}");
                engine.step_down();
                metrics().reconnects.with_label_values(&["postgres"]).inc();
                if started.elapsed() > Duration::from_secs(30) {
                    backoff = Duration::from_millis(200);
                }
                sleep_unless(stop, backoff);
                backoff = (backoff * 2).min(Duration::from_secs(10));
            }
        }
    }
    metrics().leader.set(0);
    Ok(())
}

fn sleep_unless(stop: &AtomicBool, d: Duration) {
    let end = Instant::now() + d;
    while !stop.load(Ordering::Relaxed) && Instant::now() < end {
        may::coroutine::sleep(d.min(Duration::from_millis(50)));
    }
}

fn pg_err(e: &may_postgres::Error) -> String {
    e.as_db_error().map_or_else(
        || e.to_string(),
        |d| format!("{}: {}", d.code().code(), d.message()),
    )
}

fn quote_ident(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

/// One Postgres session: LISTEN first, then contend for the lock, then lead.
fn session(
    cfg: &Config,
    engine: &mut Engine,
    connect_sink: &SinkFactory,
    stop: &AtomicBool,
) -> Result<(), String> {
    let client =
        may_postgres::connect(&cfg.pg_url).map_err(|e| format!("connect: {}", pg_err(&e)))?;
    let listen = cfg.channels.iter().fold(String::new(), |mut s, c| {
        s.push_str("LISTEN ");
        s.push_str(&quote_ident(c));
        s.push(';');
        s
    });
    client
        .batch_execute(&listen)
        .map_err(|e| format!("LISTEN: {}", pg_err(&e)))?;
    let try_lock = client
        .prepare("SELECT pg_try_advisory_lock($1)")
        .map_err(|e| pg_err(&e))?;

    // Standby: keep listening (so a take-over can be gapless), retry the lock every heartbeat.
    let mut next_try = Instant::now();
    loop {
        if stop.load(Ordering::Relaxed) {
            return Ok(());
        }
        drain(&client, engine, usize::MAX);
        if Instant::now() >= next_try {
            let got: bool = client
                .query_one(&try_lock, &[&cfg.lock_key])
                .map_err(|e| format!("advisory lock: {}", pg_err(&e)))?
                .get(0);
            if got {
                break;
            }
            next_try = Instant::now() + cfg.heartbeat;
        }
        if client.is_closed() {
            return Err("connection closed while standby".into());
        }
        may::coroutine::sleep(cfg.poll_interval.max(Duration::from_millis(20)));
    }
    metrics().leader.set(1);
    log::info!("lifereflector: acquired leadership");
    drain(&client, engine, usize::MAX);
    match engine.take_over() {
        Ok(TakeOver::Gapless) => log::info!(
            "lifereflector: standby buffer covers the gap; {} keys to invalidate",
            engine.len()
        ),
        Ok(TakeOver::CatchUp) => log::info!("lifereflector: catch-up required"),
        Err(e) => {
            log::warn!("lifereflector: reading watermark failed ({e}); catch-up scheduled");
        }
    }
    lead(cfg, &client, engine, connect_sink, stop)
}

fn drain(client: &may_postgres::Client, engine: &mut Engine, max: usize) -> usize {
    let mut n = 0;
    while n < max {
        let Some(note) = client.notifications().pop() else {
            break;
        };
        engine.push_payload(note.payload());
        n += 1;
    }
    n
}

fn ranged_query(
    client: &may_postgres::Client,
) -> impl FnMut(&TableSpec, &str, i64, usize) -> Result<Vec<String>, String> + '_ {
    move |spec, column, since_ms, limit| {
        let pk: String = client
            .query_one(
                "SELECT a.attname::text FROM pg_index i \
                 JOIN pg_attribute a ON a.attrelid = i.indrelid AND a.attnum = ANY(i.indkey) \
                 WHERE i.indrelid = $1::text::regclass AND i.indisprimary",
                &[&spec.table],
            )
            .map_err(|e| format!("primary key of {}: {}", spec.table, pg_err(&e)))?
            .get(0);
        let sql = format!(
            "SELECT {pk}::text FROM {table} WHERE {col} >= to_timestamp($1::float8 / 1000.0) LIMIT {lim}",
            pk = quote_ident(&pk),
            table = quote_ident(&spec.table),
            col = quote_ident(column),
            lim = limit.saturating_add(1),
        );
        #[allow(clippy::cast_precision_loss)]
        let since = since_ms as f64;
        let rows = client
            .query(sql.as_str(), &[&since])
            .map_err(|e| pg_err(&e))?;
        Ok(rows.iter().map(|r| r.get::<_, String>(0)).collect())
    }
}

fn lead(
    cfg: &Config,
    client: &may_postgres::Client,
    engine: &mut Engine,
    connect_sink: &SinkFactory,
    stop: &AtomicBool,
) -> Result<(), String> {
    let mut next_beat = Instant::now() + cfg.heartbeat;
    let mut cache_backoff = Duration::ZERO;
    let mut cache_retry_at = Instant::now();
    loop {
        let stopping = stop.load(Ordering::Relaxed);
        let got = drain(client, engine, cfg.batch_max);
        let drained = got < cfg.batch_max;
        let cache_ready = Instant::now() >= cache_retry_at;

        // Catch-up advances one page per pass, between live flushes, so changes arriving while
        // a large cache is scanned are still invalidated promptly.
        if cache_ready && engine.needs_catch_up() && !stopping {
            if let Err(e) = engine.catch_up_step(&mut ranged_query(client)) {
                cache_failed(
                    engine,
                    connect_sink,
                    &e,
                    &mut cache_backoff,
                    &mut cache_retry_at,
                );
            }
        }
        let cache_ready = Instant::now() >= cache_retry_at;
        if cache_ready && (engine.due(drained) || (stopping && !engine.is_empty())) {
            match engine.flush() {
                Ok(_) => cache_backoff = Duration::ZERO,
                Err(e) => {
                    cache_failed(
                        engine,
                        connect_sink,
                        &e,
                        &mut cache_backoff,
                        &mut cache_retry_at,
                    );
                }
            }
        }

        if stopping && (engine.is_empty() || !cache_ready) {
            let _ = engine.persist_watermark(Duration::ZERO);
            return Ok(());
        }

        if Instant::now() >= next_beat {
            next_beat = Instant::now() + cfg.heartbeat;
            client
                .simple_query("SELECT 1")
                .map_err(|e| format!("heartbeat: {}", pg_err(&e)))?;
            if let Err(e) = engine.persist_watermark(Duration::from_secs(1)) {
                log::warn!("lifereflector: watermark write failed: {e}");
            }
        }
        if client.is_closed() {
            return Err("connection closed while leading".into());
        }
        if engine.needs_catch_up() && cache_ready {
            // More catch-up work: no idle sleep.
        } else if got == 0 && (engine.is_empty() || !cache_ready) {
            may::coroutine::sleep(cfg.poll_interval);
        } else if got == 0 && !engine.due(true) {
            // Waiting out batch_delay.
            may::coroutine::sleep(cfg.poll_interval);
        }
        debug_assert_eq!(engine.role(), Role::Leader);
    }
}

fn cache_failed(
    engine: &mut Engine,
    connect_sink: &SinkFactory,
    e: &SinkError,
    backoff: &mut Duration,
    retry_at: &mut Instant,
) {
    log::warn!("lifereflector: cache operation failed: {e}");
    *backoff = (*backoff * 2).clamp(Duration::from_millis(50), Duration::from_secs(5));
    *retry_at = Instant::now() + *backoff;
    if engine.consecutive_failures() >= 3 || engine.needs_catch_up() {
        match connect_sink() {
            Ok(s) => {
                metrics().reconnects.with_label_values(&["redis"]).inc();
                engine.set_sink(s);
            }
            Err(e) => log::warn!("lifereflector: cache reconnect failed: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sink::MemorySink;

    #[test]
    fn noeviction_is_refused() {
        let cfg = Config::new("pg", "r");
        let mut s = MemorySink::new();
        s.policy = Some("noeviction".into());
        assert!(matches!(
            check_eviction_policy(&s, &cfg),
            Err(StartError::UnsafeCache(_))
        ));
        s.policy = Some("allkeys-lru".into());
        assert_eq!(check_eviction_policy(&s, &cfg).unwrap(), "allkeys-lru");
    }

    #[test]
    fn unreadable_policy_needs_operator_assertion() {
        let mut cfg = Config::new("pg", "r");
        let mut s = MemorySink::new();
        s.policy = None;
        assert!(check_eviction_policy(&s, &cfg).is_err());
        cfg.asserted_eviction_policy = Some("allkeys-lfu".into());
        assert_eq!(check_eviction_policy(&s, &cfg).unwrap(), "allkeys-lfu");
        cfg.asserted_eviction_policy = Some("noeviction".into());
        assert!(check_eviction_policy(&s, &cfg).is_err());
    }

    #[test]
    fn server_policy_wins_over_assertion() {
        let mut cfg = Config::new("pg", "r");
        cfg.asserted_eviction_policy = Some("allkeys-lru".into());
        let mut s = MemorySink::new();
        s.policy = Some("noeviction".into());
        assert!(check_eviction_policy(&s, &cfg).is_err());
    }

    #[test]
    fn identifiers_are_quoted() {
        assert_eq!(quote_ident("a\"b"), "\"a\"\"b\"");
    }
}
