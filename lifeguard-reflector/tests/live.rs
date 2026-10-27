//! End-to-end tests against a real Postgres and Redis.
//!
//! Set `LIFEREFLECTOR_TEST_PG_URL` (a direct connection; the role needs CREATE on `public`) and
//! `LIFEREFLECTOR_TEST_REDIS_URL` (a disposable instance: one test changes `maxmemory-policy`
//! and restores it). Without them every test returns early.
//!
//! Each test uses its own table, channel, key prefix, watermark key and lock key, so they can
//! run in parallel.

use lifeguard_reflector::{
    run, CacheSink, CatchUp, Config, RedisSink, SinkError, StartError, TableSpec,
};
use may_redis::{cmd, RedisClient};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

struct Env {
    pg: String,
    redis: String,
}

fn env() -> Option<Env> {
    let pg = std::env::var("LIFEREFLECTOR_TEST_PG_URL").ok()?;
    let redis = std::env::var("LIFEREFLECTOR_TEST_REDIS_URL").ok()?;
    may::config().set_stack_size(0x10_0000);
    // The reflector refuses `noeviction` (Redis's default); the test instance is disposable.
    let r = RedisClient::connect_url(&redis).expect("redis");
    let _: String = r
        .execute(
            cmd("CONFIG")
                .arg("SET")
                .arg("maxmemory-policy")
                .arg("allkeys-lru"),
        )
        .expect("config set");
    Some(Env { pg, redis })
}

/// Table with a trigger shaped exactly like lifeguard-migrate's `#[notify]` output.
fn create_table(pg: &may_postgres::Client, table: &str, channel: &str) {
    pg.batch_execute(&format!(
        "DROP TABLE IF EXISTS {table} CASCADE;
         CREATE TABLE {table} (id BIGINT PRIMARY KEY, body TEXT NOT NULL,
                               changed_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp());
         CREATE OR REPLACE FUNCTION {table}_notify() RETURNS trigger LANGUAGE plpgsql AS $f$
         BEGIN
           PERFORM pg_notify('{channel}', json_build_object(
             'table', TG_TABLE_NAME, 'op', TG_OP,
             'id', (CASE WHEN TG_OP = 'DELETE' THEN OLD.id ELSE NEW.id END),
             'v', (pg_current_wal_insert_lsn() - '0/0'::pg_lsn)::bigint,
             'ts', (extract(epoch FROM clock_timestamp()) * 1000)::bigint)::text);
           RETURN NULL;
         END $f$;
         CREATE TRIGGER {table}_notify AFTER INSERT OR UPDATE OR DELETE ON {table}
           FOR EACH ROW EXECUTE FUNCTION {table}_notify();"
    ))
    .expect("create table");
}

fn config(e: &Env, name: &str, lock_key: i64) -> Config {
    let mut c = Config::new(e.pg.clone(), e.redis.clone());
    c.channels = vec![format!("{name}_ch")];
    c.key_prefix = format!("lrt:{name}:");
    c.watermark_key = format!("lrt:{name}:__wm");
    c.lock_key = lock_key;
    c.heartbeat = Duration::from_millis(200);
    c.metrics_addr = None;
    c.catch_up_slack = Duration::from_millis(500);
    c
}

fn redis(e: &Env) -> RedisClient {
    RedisClient::connect_url(&e.redis).expect("redis")
}

fn exists(r: &RedisClient, key: &str) -> bool {
    r.execute::<i64>(cmd("EXISTS").arg(key)).expect("exists") == 1
}

fn put(r: &RedisClient, key: &str) {
    let _: String = r.execute(cmd("SET").arg(key).arg("cached")).expect("set");
}

fn wait_until(timeout: Duration, mut f: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + timeout;
    while Instant::now() < end {
        if f() {
            return true;
        }
        may::coroutine::sleep(Duration::from_millis(10));
    }
    f()
}

struct Running {
    stop: Arc<AtomicBool>,
    handle: Option<may::coroutine::JoinHandle<Result<(), StartError>>>,
}

impl Running {
    fn stop(mut self) -> Result<(), StartError> {
        self.stop.store(true, Ordering::Relaxed);
        self.handle.take().expect("handle").join().expect("join")
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn start(cfg: Config) -> Running {
    let stop = Arc::new(AtomicBool::new(false));
    let s = stop.clone();
    let url = cfg.redis_url.clone();
    let handle = may::go!(move || {
        let connect = move || -> Result<Box<dyn CacheSink>, SinkError> {
            Ok(Box::new(RedisSink::connect(&url)?))
        };
        run(cfg, &connect, &s)
    });
    Running {
        stop,
        handle: Some(handle),
    }
}

fn lock_holders(pg: &may_postgres::Client, key: i64) -> i64 {
    // A bigint advisory key is stored as classid = high 32 bits, objid = low 32 bits.
    let (hi, lo) = (key >> 32, key & 0xffff_ffff);
    pg.query_one(
        "SELECT count(*) FROM pg_locks WHERE locktype = 'advisory' AND granted
           AND classid::bigint = $1 AND objid::bigint = $2 AND objsubid = 1",
        &[&hi, &lo],
    )
    .expect("pg_locks")
    .get(0)
}

#[test]
fn invalidates_changed_rows_in_batches() {
    let Some(e) = env() else { return };
    let pg = may_postgres::connect(&e.pg).expect("pg");
    create_table(&pg, "lrt_basic", "lrt_basic_ch");
    pg.batch_execute(
        "INSERT INTO lrt_basic (id, body) SELECT g, 'x' FROM generate_series(1, 200) g",
    )
    .expect("seed");
    let r = redis(&e);
    let cfg = config(&e, "lrt_basic", 0x7001);
    let prefix = cfg.key_prefix.clone();
    let run = start(cfg);
    assert!(wait_until(Duration::from_secs(5), || lock_holders(
        &pg, 0x7001
    ) == 1));
    may::coroutine::sleep(Duration::from_millis(300)); // initial catch-up
    for id in 1..=200 {
        put(&r, &format!("{prefix}lrt_basic:{id}"));
    }
    pg.batch_execute(
        "UPDATE lrt_basic SET body = 'y' WHERE id <= 150; DELETE FROM lrt_basic WHERE id = 200",
    )
    .expect("write");
    assert!(
        wait_until(Duration::from_secs(5), || {
            (1..=150).all(|id| !exists(&r, &format!("{prefix}lrt_basic:{id}")))
                && !exists(&r, &format!("{prefix}lrt_basic:200"))
        }),
        "changed rows must be invalidated"
    );
    assert!(
        exists(&r, &format!("{prefix}lrt_basic:151")),
        "untouched rows stay cached"
    );
    run.stop().expect("clean stop");
}

#[test]
fn single_leader_and_gapless_failover() {
    let Some(e) = env() else { return };
    let pg = may_postgres::connect(&e.pg).expect("pg");
    create_table(&pg, "lrt_failover", "lrt_failover_ch");
    pg.batch_execute(
        "INSERT INTO lrt_failover (id, body) SELECT g, 'x' FROM generate_series(1, 10) g",
    )
    .expect("seed");
    let r = redis(&e);
    let key = 0x7002;
    let cfg = config(&e, "lrt_failover", key);
    let prefix = cfg.key_prefix.clone();
    let a = start(cfg.clone());
    assert!(wait_until(Duration::from_secs(5), || lock_holders(
        &pg, key
    ) == 1));
    let b = start(cfg);
    may::coroutine::sleep(Duration::from_millis(800));
    assert_eq!(lock_holders(&pg, key), 1, "exactly one active reflector");

    // Let A persist a watermark, then stop it; B must take over without a flush.
    put(&r, &format!("{prefix}lrt_failover:1"));
    pg.batch_execute("UPDATE lrt_failover SET body = 'y' WHERE id = 1")
        .expect("w1");
    assert!(wait_until(Duration::from_secs(3), || !exists(
        &r,
        &format!("{prefix}lrt_failover:1")
    )));
    may::coroutine::sleep(Duration::from_millis(1200));
    a.stop().expect("stop a");
    for id in 2..=10 {
        put(&r, &format!("{prefix}lrt_failover:{id}"));
    }
    // Written while nobody leads: only B's standby LISTEN sees it.
    pg.batch_execute("UPDATE lrt_failover SET body = 'z' WHERE id = 2")
        .expect("w2");
    assert!(wait_until(Duration::from_secs(5), || lock_holders(
        &pg, key
    ) == 1));
    assert!(
        wait_until(Duration::from_secs(5), || !exists(
            &r,
            &format!("{prefix}lrt_failover:2")
        )),
        "change during the hand-over is applied by the new leader"
    );
    assert!(
        exists(&r, &format!("{prefix}lrt_failover:3")),
        "gapless take-over does not flush the table"
    );
    b.stop().expect("stop b");
}

#[test]
fn catches_up_after_downtime() {
    let Some(e) = env() else { return };
    let pg = may_postgres::connect(&e.pg).expect("pg");
    create_table(&pg, "lrt_ranged", "lrt_catch_ch");
    create_table(&pg, "lrt_flushed", "lrt_catch_ch");
    pg.batch_execute(
        "INSERT INTO lrt_ranged (id, body) SELECT g, 'x' FROM generate_series(1, 10) g;
         INSERT INTO lrt_flushed (id, body) SELECT g, 'x' FROM generate_series(1, 10) g;
         ALTER TABLE lrt_ranged DISABLE TRIGGER USER;
         UPDATE lrt_ranged SET changed_at = clock_timestamp() - interval '1 hour';
         ALTER TABLE lrt_ranged ENABLE TRIGGER USER;",
    )
    .expect("seed");
    let r = redis(&e);
    let mut cfg = config(&e, "lrt_catch", 0x7003);
    cfg.tables = vec![
        TableSpec {
            table: "lrt_ranged".into(),
            catch_up: CatchUp::Ranged {
                column: "changed_at".into(),
            },
        },
        TableSpec {
            table: "lrt_flushed".into(),
            catch_up: CatchUp::Flush,
        },
    ];
    let prefix = cfg.key_prefix.clone();
    // First run establishes a watermark.
    let first = start(cfg.clone());
    assert!(wait_until(Duration::from_secs(5), || lock_holders(
        &pg, 0x7003
    ) == 1));
    pg.batch_execute("UPDATE lrt_ranged SET body = 'w' WHERE id = 10")
        .expect("w");
    may::coroutine::sleep(Duration::from_millis(1500));
    first.stop().expect("stop");
    let wm: Option<String> = r.execute(cmd("GET").arg(&cfg.watermark_key)).expect("wm");
    assert!(wm.is_some(), "watermark persisted on stop");

    // Downtime: cache filled, rows changed, no reflector running. Wait past the slack so the
    // unchanged rows' timestamps fall before `watermark - slack`.
    may::coroutine::sleep(Duration::from_millis(700));
    for id in 1..=10 {
        put(&r, &format!("{prefix}lrt_ranged:{id}"));
        put(&r, &format!("{prefix}lrt_flushed:{id}"));
    }
    pg.batch_execute(
        "UPDATE lrt_ranged SET body = 'y', changed_at = clock_timestamp() WHERE id IN (2, 3);
         UPDATE lrt_flushed SET body = 'y' WHERE id = 2;",
    )
    .expect("offline writes");

    let second = start(cfg);
    assert!(wait_until(Duration::from_secs(5), || {
        !exists(&r, &format!("{prefix}lrt_ranged:2"))
            && !exists(&r, &format!("{prefix}lrt_ranged:3"))
    }));
    assert!(
        exists(&r, &format!("{prefix}lrt_ranged:1")),
        "ranged catch-up keeps unchanged rows"
    );
    assert!(
        wait_until(Duration::from_secs(5), || (1..=10)
            .all(|id| !exists(&r, &format!("{prefix}lrt_flushed:{id}")))),
        "flush catch-up empties the table's keys"
    );
    second.stop().expect("stop");
}

#[test]
fn refuses_noeviction() {
    // Through `run` with a cache reporting `noeviction` (changing the shared test server's policy
    // would race the other tests).
    let Some(e) = env() else { return };
    let stop = AtomicBool::new(false);
    let factory = || -> Result<Box<dyn CacheSink>, SinkError> {
        let mut s = lifeguard_reflector::MemorySink::new();
        s.policy = Some("noeviction".into());
        Ok(Box::new(s))
    };
    let result = run(config(&e, "lrt_noevict", 0x7004), &factory, &stop);
    assert!(
        matches!(result, Err(StartError::UnsafeCache(_))),
        "{result:?}"
    );
}
