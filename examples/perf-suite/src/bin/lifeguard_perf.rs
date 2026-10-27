//! Lifeguard perf suite: ORM vs raw-driver baselines, pool contention, row mapping, and a
//! LISTEN/NOTIFY -> Redis reflection prototype. Runbook: `docs/perf/lifeguard-baseline.md`.
//!
//! Every number here is measured against a disposable local database (`PERF_RESET=1`).

use lifeguard::{
    delete_many, find_by_ids, insert_many, update_many, upsert_many, ActiveModelTrait,
    BatchOptions, BatchWriter, BatchWriterConfig, ColumnTrait, FromRow, LifeModelTrait,
    LifeguardPool, LifeguardPoolSettings, MayPostgresExecutor, OnConflict, PooledLifeExecutor,
    SessionContext, WriteMode,
};
use lifeguard_perf::entities::{item, ledger, wide};
use lifeguard_perf::stats::{markdown, summarize, Row};
use may_postgres::types::ToSql;
use std::env;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

const SCHEMA_SQL: &str = include_str!("../../sql/schema.sql");
const WIDE_SQL: &str = include_str!("../../sql/wide.sql");

const PK_SQL: &str = "SELECT id, account_id, name, qty, price, active, updated_at FROM perf_items WHERE id = $1";
const LIST_SQL: &str = "SELECT id, account_id, name, qty, price, active, updated_at FROM perf_items WHERE account_id = $1";
const INSERT_SQL: &str = "INSERT INTO perf_items (id, account_id, name, qty, price, active, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7)";
const UPDATE_SQL: &str = "UPDATE perf_items SET qty = $2 WHERE id = $1 RETURNING id, account_id, name, qty, price, active, updated_at";
const UPSERT_SQL: &str = "INSERT INTO perf_items (id, account_id, name, qty, price, active, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7) \
     ON CONFLICT (id) DO UPDATE SET qty = EXCLUDED.qty, updated_at = EXCLUDED.updated_at RETURNING id, account_id, name, qty, price, active, updated_at";
const RLS_SET_SQL: &str = "SELECT public.rls_set_session($1::text, $2::uuid, $3::uuid, $4::text, $5::jsonb, $6::jsonb, $7::text, $8::text)";
const LEDGER_SQL: &str = "SELECT id, tenant_id, amount, memo FROM perf_ledger WHERE id = $1";

// ---------------------------------------------------------------------------------------------
// configuration

struct Cfg {
    admin_url: String,
    app_url: String,
    redis: Option<(String, u16)>,
    iters: usize,
    warmup: usize,
    pool_size: usize,
    concurrency: Vec<usize>,
    contention_ms: u64,
    scenarios: Vec<String>,
    may_workers: usize,
    label: String,
}

fn env_usize(k: &str, d: usize) -> usize {
    env::var(k).ok().and_then(|v| v.trim().parse().ok()).unwrap_or(d)
}

fn cfg() -> Result<Cfg, String> {
    let admin_url = env::var("PERF_DATABASE_URL")
        .map_err(|_| "set PERF_DATABASE_URL (a disposable database; DATABASE_URL is ignored)")?;
    if !matches!(
        env::var("PERF_RESET").unwrap_or_default().trim(),
        "1" | "true" | "yes" | "on"
    ) {
        return Err("refusing to run without PERF_RESET=1: the suite drops and recreates perf_* objects".into());
    }
    // The RLS scenario must run as a non-superuser (superusers bypass row-level security).
    let app_url = env::var("PERF_APP_DATABASE_URL").unwrap_or_else(|_| swap_credentials(&admin_url, "perf_app", "perf_app"));
    let redis = env::var("PERF_REDIS").ok().and_then(|s| {
        let (h, p) = s.rsplit_once(':')?;
        Some((h.to_string(), p.parse().ok()?))
    });
    let scenarios = env::var("PERF_SCENARIOS")
        .unwrap_or_else(|_| "all".into())
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    Ok(Cfg {
        admin_url,
        app_url,
        redis,
        iters: env_usize("PERF_ITERATIONS", 3000).max(1),
        warmup: env_usize("PERF_WARMUP", 300),
        pool_size: env_usize("PERF_POOL_SIZE", 8).max(1),
        concurrency: env::var("PERF_CONCURRENCY")
            .unwrap_or_else(|_| "1,16,128,1024".into())
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect(),
        contention_ms: env_usize("PERF_CONTENTION_MS", 3000) as u64,
        scenarios,
        may_workers: env_usize("PERF_MAY_WORKERS", 8).max(1),
        label: env::var("PERF_LABEL").unwrap_or_else(|_| "run".into()),
    })
}

fn wants(c: &Cfg, s: &str) -> bool {
    c.scenarios.iter().any(|x| x == "all" || x == s)
}

/// `postgres://u:p@host...` -> `postgres://user:pass@host...` (host part kept verbatim).
fn swap_credentials(url: &str, user: &str, pass: &str) -> String {
    let (scheme, rest) = url.split_once("://").unwrap_or(("postgresql", url));
    let host = rest.rsplit_once('@').map_or(rest, |(_, h)| h);
    format!("{scheme}://{user}:{pass}@{host}")
}

fn epoch() -> Instant {
    static E: OnceLock<Instant> = OnceLock::new();
    *E.get_or_init(Instant::now)
}

fn now_ns() -> u64 {
    epoch().elapsed().as_nanos() as u64
}

fn us(d: Duration) -> f64 {
    d.as_secs_f64() * 1e6
}

fn ts() -> chrono::NaiveDateTime {
    chrono::DateTime::from_timestamp(1_767_225_600, 0)
        .expect("fixed timestamp")
        .naive_utc()
}

fn ctx(tenant: &str) -> SessionContext {
    SessionContext {
        tenant_id: tenant.to_string(),
        subject_id: uuid::Uuid::from_u128(0x1111),
        organization_id: uuid::Uuid::from_u128(0x2222),
        session_id: "perf-session".to_string(),
        roles: vec!["member".to_string()],
        permissions: vec!["read".to_string(), "write".to_string()],
        user_type: Some("user".to_string()),
        org_type: Some("tenant".to_string()),
    }
}

/// Keys spread over the 100k bulk rows (ids 10_001..=110_000), deterministic per index.
fn bulk_key(i: usize) -> i64 {
    10_001 + ((i.wrapping_mul(2_654_435_761)) % 100_000) as i64
}

/// Ledger ids whose tenant is `tenant-3` (the context the RLS scenario uses).
fn ledger_key(i: usize) -> i64 {
    (((i.wrapping_mul(2_654_435_761)) % 4_999) * 10 + 3) as i64
}

fn item_pos(r: &may_postgres::Row) -> item::ItemModel {
    item::ItemModel {
        id: r.get(0),
        account_id: r.get(1),
        name: r.get(2),
        qty: r.get(3),
        price: r.get(4),
        active: r.get(5),
        updated_at: r.get(6),
    }
}

fn seq<F: FnMut(usize)>(scn: &str, var: &str, c: &Cfg, note: &str, mut f: F) -> Row {
    for i in 0..c.warmup {
        f(i);
    }
    let mut s = Vec::with_capacity(c.iters);
    let wall = Instant::now();
    for i in 0..c.iters {
        let t = Instant::now();
        f(c.warmup + i);
        s.push(us(t.elapsed()));
    }
    let row = summarize(scn, var, 1, s, wall.elapsed(), note);
    eprintln!("  {:<14} {:<26} p50 {:>8.1}µs p99 {:>8.1}µs", row.scenario, row.variant, row.p50_us, row.p99_us);
    row
}

/// Pool with default settings, adjusted by `PERF_REPLY_TIMEOUT_OFF=1` (no reply deadline, so no
/// watchdog coroutine per call) and `PERF_QUEUE_DEPTH` (statements in flight per slot).
fn open_pool(url: &str, size: usize) -> Result<LifeguardPool, lifeguard::LifeError> {
    let mut s = LifeguardPoolSettings::default();
    if matches!(env::var("PERF_REPLY_TIMEOUT_OFF").as_deref(), Ok("1" | "true")) {
        s.reply_timeout = None;
    }
    if let Some(d) = env::var("PERF_QUEUE_DEPTH").ok().and_then(|v| v.parse().ok()) {
        s.job_queue_capacity_per_worker = d;
    }
    LifeguardPool::new_with_settings(url, size, vec![], 0, &s)
}

// ---------------------------------------------------------------------------------------------
// batching: single-row ORM calls vs lifeguard::batch vs raw COPY

const BATCH_BASE: i64 = 50_000_000;

fn batch_rec(id: i64, qty: i32) -> item::ItemRecord {
    let mut r = item::ItemRecord::new();
    r.set_id(id)
        .set_account_id(9)
        .set_name(format!("b{id}"))
        .set_qty(qty)
        .set_price(2.5)
        .set_active(true)
        .set_updated_at(ts());
    r
}

/// Time `reps` runs of `f(n)` (after `prep` each time, untimed). One row per variant; ops/s is
/// rows per second.
fn batch_row(
    var: &str,
    n: usize,
    reps: usize,
    note: &str,
    mut prep: impl FnMut(),
    mut f: impl FnMut(usize),
) -> Row {
    prep();
    f(n); // warm-up (prepares statements)
    let mut s = Vec::with_capacity(reps);
    let mut total = Duration::ZERO;
    for _ in 0..reps {
        prep();
        let t = Instant::now();
        f(n);
        let e = t.elapsed();
        total += e;
        s.push(us(e));
    }
    let mut row = summarize(&format!("batch_{n}"), var, 1, s, total, note);
    #[allow(clippy::cast_precision_loss)]
    {
        row.ops_per_s = (n * reps) as f64 / total.as_secs_f64();
    }
    eprintln!("  batch_{n:<8} {var:<26} p50 {:>10.0}µs  {:>10.0} rows/s", row.p50_us, row.ops_per_s);
    row
}

#[allow(clippy::too_many_lines)]
fn run_batch(
    c: &Cfg,
    admin: &may_postgres::Client,
    orm: &PooledLifeExecutor,
    pool: &Arc<LifeguardPool>,
    rows: &mut Vec<Row>,
) -> Result<(), Box<dyn std::error::Error>> {
    let sizes: Vec<usize> = env::var("PERF_BATCH_SIZES")
        .unwrap_or_else(|_| "100,1000,10000".into())
        .split(',')
        .filter_map(|v| v.trim().parse().ok())
        .collect();
    let reps = env_usize("PERF_BATCH_REPS", 5);
    let single_max = env_usize("PERF_BATCH_SINGLE_MAX", 1000);
    let clear = || {
        admin
            .batch_execute(&format!("DELETE FROM perf_items WHERE id >= {BATCH_BASE}"))
            .expect("clear");
    };
    let seed = |n: usize| {
        admin
            .batch_execute(&format!(
                "DELETE FROM perf_items WHERE id >= {BATCH_BASE};
                 INSERT INTO perf_items SELECT {BATCH_BASE} + g, 9, 'b' || g, 1, 2.5, true, TIMESTAMP '2026-01-01'
                 FROM generate_series(0, {}) g",
                n - 1
            ))
            .expect("seed");
    };
    let key = |i: usize| BATCH_BASE + i as i64;
    let raw = may_postgres::connect(&c.admin_url)?;
    for &n in &sizes {
        eprintln!("batch n={n}");
        let recs: Vec<item::ItemRecord> = (0..n).map(|i| batch_rec(key(i), 1)).collect();
        let ids: Vec<sea_query::Value> = (0..n).map(|i| sea_query::Value::BigInt(Some(key(i)))).collect();
        if n <= single_max {
            rows.push(batch_row("insert_single_orm", n, reps, "n x ActiveModel::insert", clear, |n| {
                for r in &recs[..n] {
                    r.insert(orm).expect("insert");
                }
            }));
        }
        let values = BatchOptions { unnest_min_rows: usize::MAX, ..BatchOptions::default() };
        rows.push(batch_row("insert_many_values", n, reps, "multi-row VALUES, 1000 rows/stmt", clear, |_| {
            insert_many(orm, &recs, &values).expect("insert_many values");
        }));
        let unnest = BatchOptions { unnest_min_rows: 0, max_rows: 10_000, ..BatchOptions::default() };
        rows.push(batch_row("insert_many_unnest", n, reps, "UNNEST arrays, 10000 rows/stmt", clear, |_| {
            insert_many(orm, &recs, &unnest).expect("insert_many unnest");
        }));
        rows.push(batch_row("insert_many_default", n, reps, "BatchOptions::default()", clear, |_| {
            insert_many(orm, &recs, &BatchOptions::default()).expect("insert_many");
        }));
        rows.push(batch_row("insert_raw_copy", n, reps, "may_postgres COPY FROM STDIN (text)", clear, |n| {
            let mut sink = raw
                .copy_in::<_, bytes::Bytes>("COPY perf_items (id, account_id, name, qty, price, active, updated_at) FROM STDIN")
                .expect("copy");
            let mut buf = String::with_capacity(n * 48);
            for i in 0..n {
                use std::fmt::Write as _;
                let _ = writeln!(buf, "{}\t9\tb{}\t1\t2.5\tt\t2026-01-01 00:00:00", key(i), i);
            }
            sink.send(bytes::Bytes::from(buf)).expect("copy send");
            sink.finish().expect("copy finish");
        }));
        let oc = || {
            OnConflict::column(item::Column::Id)
                .update_columns([item::Column::Qty, item::Column::UpdatedAt])
                .to_owned()
        };
        let upd_recs: Vec<item::ItemRecord> = (0..n).map(|i| batch_rec(key(i), 2)).collect();
        if n <= single_max {
            rows.push(batch_row("upsert_single_orm", n, reps, "n x insert_on_conflict", || seed(n), |n| {
                for r in &upd_recs[..n] {
                    r.insert_on_conflict(orm, oc()).expect("upsert");
                }
            }));
        }
        rows.push(batch_row("upsert_many", n, reps, "existing keys, DO UPDATE", || seed(n), |_| {
            upsert_many(orm, &upd_recs, oc(), &BatchOptions::default()).expect("upsert_many");
        }));
        let qty_only: Vec<item::ItemRecord> = (0..n)
            .map(|i| {
                let mut r = item::ItemRecord::new();
                r.set_id(key(i)).set_qty(3);
                r
            })
            .collect();
        if n <= single_max {
            rows.push(batch_row("update_single_orm", n, reps, "n x ActiveModel::update", || seed(n), |n| {
                for r in &qty_only[..n] {
                    r.update(orm).expect("update");
                }
            }));
        }
        rows.push(batch_row("update_many", n, reps, "UPDATE .. FROM unnest", || seed(n), |_| {
            update_many(orm, &qty_only, &[item::Column::Qty], &BatchOptions::default()).expect("update_many");
        }));
        seed(n);
        if n <= single_max {
            rows.push(batch_row("get_single_orm", n, reps, "n x find_one by pk (the N+1)", || {}, |n| {
                for i in 0..n {
                    item::Entity::find().filter(item::Column::Id.eq(key(i))).find_one(orm).expect("get");
                }
            }));
        }
        rows.push(batch_row("find_by_ids", n, reps, "pk = ANY($1), 1000 keys/stmt", || {}, |_| {
            let got = find_by_ids::<item::Entity>(orm, &ids, &BatchOptions::default()).expect("find_by_ids");
            assert_eq!(got.len(), ids.len());
        }));
        rows.push(batch_row("delete_many", n, reps, "pk = ANY($1)", || seed(n), |_| {
            delete_many::<item::Entity>(orm, &ids, &BatchOptions::default()).expect("delete_many");
        }));
        // Ingest-style: 16 coroutines push concurrently into one BatchWriter; time until closed.
        let writers = 16usize;
        rows.push(batch_row("batch_writer_16_producers", n, reps, "500 rows / 50 ms, upsert, close()", clear, |n| {
            let w: Arc<BatchWriter<item::ItemRecord>> = Arc::new(BatchWriter::new(
                PooledLifeExecutor::new(pool.clone()),
                WriteMode::Upsert { on_conflict: oc(), key: vec![item::Column::Id] },
                BatchWriterConfig::default(),
                |e, rows| eprintln!("batch writer failure: {e} ({} rows)", rows.len()),
            ));
            let hs: Vec<_> = (0..writers)
                .map(|p| {
                    let w = w.clone();
                    may::go!(move || {
                        let mut i = p;
                        while i < n {
                            let _ = w.push(batch_rec(BATCH_BASE + i as i64, 1));
                            i += writers;
                        }
                    })
                })
                .collect();
            for h in hs {
                let _ = h.join();
            }
            let w = Arc::try_unwrap(w).map_err(|_| "writer still shared").expect("unwrap");
            let st = w.close();
            assert_eq!(st.failed, 0);
        }));
        clear();
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// may side (ORM + raw may_postgres + reflector)

fn run_may(c: &Cfg, rows: &mut Vec<Row>) -> Result<(), Box<dyn std::error::Error>> {
    let admin = may_postgres::connect(&c.admin_url)?;
    eprintln!("schema: applying synthetic schema");
    admin.batch_execute(SCHEMA_SQL)?;
    admin.batch_execute(WIDE_SQL)?;

    let pool = Arc::new(open_pool(&c.admin_url, c.pool_size)?);
    let orm = PooledLifeExecutor::new(pool.clone());
    let direct = MayPostgresExecutor::new(may_postgres::connect(&c.admin_url)?);
    let raw = may_postgres::connect(&c.admin_url)?;
    let pk = raw.prepare(PK_SQL)?;
    let list = raw.prepare(LIST_SQL)?;
    let ins = raw.prepare(INSERT_SQL)?;
    let upd = raw.prepare(UPDATE_SQL)?;
    let ups = raw.prepare(UPSERT_SQL)?;

    if wants(c, "pk_select") {
        eprintln!("pk_select");
        rows.push(seq("pk_select", "orm_pool", c, "1 row", |i| {
            let m = item::Entity::find()
                .filter(item::Column::Id.eq(bulk_key(i)))
                .find_one(&orm)
                .expect("orm pk");
            assert!(m.is_some());
        }));
        rows.push(seq("pk_select", "orm_direct_client", c, "MayPostgresExecutor, no pool", |i| {
            let m = item::Entity::find()
                .filter(item::Column::Id.eq(bulk_key(i)))
                .find_one(&direct)
                .expect("orm direct pk");
            assert!(m.is_some());
        }));
        rows.push(seq("pk_select", "raw_may_unprepared", c, "query_one(&str): Parse+Describe every call", |i| {
            let r = raw.query_one(PK_SQL, &[&bulk_key(i)]).expect("raw");
            let _ = item_pos(&r);
        }));
        rows.push(seq("pk_select", "raw_may_prepared", c, "cached Statement", |i| {
            let r = raw.query_one(&pk, &[&bulk_key(i)]).expect("raw");
            let _ = item_pos(&r);
        }));
    }

    if wants(c, "list") {
        eprintln!("list");
        for n in [10i64, 100, 1000] {
            let scn = format!("list_{n}");
            rows.push(seq(&scn, "orm_pool", c, &format!("{n} rows"), |_| {
                let v = item::Entity::find()
                    .filter(item::Column::AccountId.eq(n))
                    .all(&orm)
                    .expect("orm list");
                assert_eq!(v.len() as i64, n);
            }));
            rows.push(seq(&scn, "raw_may_prepared", c, &format!("{n} rows, positional map"), |_| {
                let v: Vec<item::ItemModel> = raw.query(&list, &[&n]).expect("raw").iter().map(item_pos).collect();
                assert_eq!(v.len() as i64, n);
            }));
        }
    }

    if wants(c, "write") {
        eprintln!("write");
        let next = AtomicU64::new(20_000_000);
        rows.push(seq("insert", "orm_pool", c, "LifeRecord::insert", |i| {
            let mut r = item::ItemRecord::new();
            r.set_id(next.fetch_add(1, Ordering::Relaxed) as i64)
                .set_account_id(7)
                .set_name(format!("n{i}"))
                .set_qty(1)
                .set_price(1.5)
                .set_active(true)
                .set_updated_at(ts());
            r.insert(&orm).expect("orm insert");
        }));
        rows.push(seq("insert", "raw_may_prepared", c, "", |i| {
            let id = next.fetch_add(1, Ordering::Relaxed) as i64;
            let name = format!("n{i}");
            raw.execute(&ins, &[&id, &7i64, &name, &1i32, &1.5f64, &true, &ts()]).expect("raw insert");
        }));
        rows.push(seq("update", "orm_pool", c, "LifeRecord::update (returns model)", |i| {
            let mut r = item::ItemRecord::new();
            r.set_id(bulk_key(i)).set_qty((i % 1000) as i32);
            r.update(&orm).expect("orm update");
        }));
        rows.push(seq("update", "raw_may_prepared", c, "UPDATE .. RETURNING (same result)", |i| {
            let r = raw.query_one(&upd, &[&bulk_key(i), &((i % 1000) as i32)]).expect("raw update");
            let _ = item_pos(&r);
        }));
        rows.push(seq("upsert", "orm_pool", c, "insert_on_conflict DO UPDATE", |i| {
            let mut r = item::ItemRecord::new();
            r.set_id(bulk_key(i))
                .set_account_id(7)
                .set_name(format!("n{i}"))
                .set_qty((i % 1000) as i32)
                .set_price(1.5)
                .set_active(true)
                .set_updated_at(ts());
            let oc = OnConflict::column(item::Column::Id)
                .update_column(item::Column::Qty)
                .update_column(item::Column::UpdatedAt)
                .to_owned();
            r.insert_on_conflict(&orm, oc).expect("orm upsert");
        }));
        rows.push(seq("upsert", "raw_may_prepared", c, "", |i| {
            let name = format!("n{i}");
            let r = raw
                .query_one(&ups, &[&bulk_key(i), &7i64, &name, &((i % 1000) as i32), &1.5f64, &true, &ts()])
                .expect("raw upsert");
            let _ = item_pos(&r);
        }));
    }

    if wants(c, "batch") {
        run_batch(c, &admin, &orm, &pool, rows)?;
    }

    if wants(c, "rls") {
        eprintln!("rls");
        let app_pool = Arc::new(open_pool(&c.app_url, c.pool_size)?);
        let app = PooledLifeExecutor::new(app_pool.clone()).with_session_context(ctx("tenant-3"));
        rows.push(seq("rls_pk_select", "orm_pool_session_ctx", c, "BEGIN; rls_set_session; SELECT; COMMIT", |i| {
            let m = ledger::Entity::find()
                .filter(ledger::Column::Id.eq(ledger_key(i)))
                .find_one(&app)
                .expect("rls orm");
            assert!(m.is_some(), "RLS row must be visible to its tenant");
        }));
        let sc = ctx("tenant-3");
        rows.push(seq("rls_pk_select", "orm_pool_session_tx", c, "with_session_transaction, 1 statement", |i| {
            let m = app_pool
                .with_session_transaction(&sc, |ex| {
                    ledger::Entity::find()
                        .filter(ledger::Column::Id.eq(ledger_key(i)))
                        .find_one(ex)
                })
                .expect("rls tx");
            assert!(m.is_some());
        }));
        let araw = may_postgres::connect(&c.app_url)?;
        let set = araw.prepare(RLS_SET_SQL)?;
        let sel = araw.prepare(LEDGER_SQL)?;
        let roles = serde_json::json!(["member"]);
        let perms = serde_json::json!(["read", "write"]);
        let (sid, oid) = (uuid::Uuid::from_u128(0x1111), uuid::Uuid::from_u128(0x2222));
        rows.push(seq("rls_pk_select", "raw_may_prepared", c, "same 4 statements, prepared, simple BEGIN/COMMIT", |i| {
            araw.batch_execute("BEGIN").expect("begin");
            let a: [&dyn ToSql; 8] = [&"tenant-3", &sid, &oid, &"perf-session", &roles, &perms, &"user", &"tenant"];
            araw.execute(&set, &a).expect("set");
            let r = araw.query_one(&sel, &[&ledger_key(i)]).expect("sel");
            let _: i64 = r.get(0);
            araw.batch_execute("COMMIT").expect("commit");
        }));
    }

    if wants(c, "wide") {
        eprintln!("wide");
        let all = raw.query("SELECT * FROM perf_wide ORDER BY id", &[])?;
        let reps = 20usize;
        for (var, positional) in [("derive_from_row", false), ("manual_positional", true)] {
            let mut s = Vec::with_capacity(reps);
            let wall = Instant::now();
            for _ in 0..reps {
                let t = Instant::now();
                for r in &all {
                    if positional {
                        std::hint::black_box(wide::from_row_positional(r));
                    } else {
                        std::hint::black_box(<wide::WideModel as FromRow>::from_row(r).expect("map"));
                    }
                }
                s.push(us(t.elapsed()) * 1000.0 / all.len() as f64);
            }
            let mut row = summarize("wide_map_cpu", var, 1, s, wall.elapsed(), "ns per 40-col row (CPU only, rows preloaded)");
            row.ops_per_s *= all.len() as f64;
            eprintln!("  wide_map_cpu {var}: p50 {:.0} ns/row", row.p50_us);
            rows.push(row);
        }
        rows.push(seq("wide_list_1000", "orm_pool", c, "1000 x 40 cols", |_| {
            let v = wide::Entity::find()
                .filter(wide::Column::Id.lte(1000i64))
                .all(&orm)
                .expect("orm wide");
            assert_eq!(v.len(), 1000);
        }));
        let wl = raw.prepare("SELECT * FROM perf_wide WHERE id <= $1")?;
        rows.push(seq("wide_list_1000", "raw_may_prepared", c, "1000 x 40 cols, positional", |_| {
            let v: Vec<wide::WideModel> = raw.query(&wl, &[&1000i64]).expect("raw wide").iter().map(wide::from_row_positional).collect();
            assert_eq!(v.len(), 1000);
        }));
    }

    if wants(c, "obs_overhead") {
        // CPU cost of the per-operation instrumentation a pooled ORM select pays (entity timer,
        // query histogram, checkout histogram, statement-cache counter, pool gauges), measured
        // in isolation. With `--no-default-features` the same calls compile to no-ops.
        let n = 1_000_000usize;
        let mut s = Vec::with_capacity(5);
        let wall = Instant::now();
        for _ in 0..5 {
            let t = Instant::now();
            for _ in 0..n {
                let timer = lifeguard::metrics::entity::OpTimer::start(
                    <item::Entity as LifeModelTrait>::__lg_entity_metrics(),
                    lifeguard::metrics::entity::Op::Select,
                );
                let w = lifeguard::metrics::pool_gauges::Track::waiting("primary");
                drop(w);
                let f = lifeguard::metrics::pool_gauges::Track::in_flight("primary");
                lifeguard::metrics::METRICS.record_pool_checkout(Duration::from_micros(3), "primary");
                lifeguard::metrics::METRICS.record_statement_cache(true, "primary");
                lifeguard::metrics::METRICS.record_query_duration(Duration::from_micros(40), Some("primary"));
                lifeguard::metrics::METRICS.record_connection_wait(Duration::from_micros(2), Some("primary"));
                drop(f);
                timer.finish(1, true);
            }
            s.push(us(t.elapsed()) * 1000.0 / n as f64);
        }
        let row = summarize("obs_overhead_cpu", if cfg!(feature = "observability") { "metrics_on" } else { "metrics_off" }, 1, s, wall.elapsed(), "ns of instrumentation per pooled select (CPU only)");
        eprintln!("  obs_overhead_cpu: p50 {:.0} ns/op", row.p50_us);
        rows.push(row);
    }

    if wants(c, "contention") {
        eprintln!("contention (pool_size {})", c.pool_size);
        let shared: Vec<(may_postgres::Client, may_postgres::Statement)> = (0..c.pool_size)
            .map(|_| {
                let cl = may_postgres::connect(&c.admin_url).expect("connect");
                let st = cl.prepare(PK_SQL).expect("prepare");
                (cl, st)
            })
            .collect();
        for &k in &c.concurrency {
            rows.push(contend(k, c, "orm_pool", |_| {
                let orm = orm.clone();
                move |i| {
                    item::Entity::find()
                        .filter(item::Column::Id.eq(bulk_key(i)))
                        .find_one(&orm)
                        .expect("orm")
                        .expect("row");
                }
            }));
            // may_postgres pipelines concurrent requests from Client clones over one connection.
            rows.push(contend(k, c, "raw_may_shared_conns", |j| {
                let (cl, st) = &shared[j % shared.len()];
                let (cl, st) = (cl.clone(), st.clone());
                move |i| {
                    let r = cl.query_one(&st, &[&bulk_key(i)]).expect("raw");
                    let _ = item_pos(&r);
                }
            }));
        }
    }

    if wants(c, "reflector") {
        match &c.redis {
            None => eprintln!("reflector: PERF_REDIS not set, skipped"),
            Some((h, p)) => reflector(c, h, *p, &admin, &orm, rows)?,
        }
    }
    if wants(c, "reflector_crate") {
        match &c.redis {
            None => eprintln!("reflector_crate: PERF_REDIS not set, skipped"),
            Some((h, p)) => reflector_crate(c, h, *p, &admin, rows)?,
        }
    }
    Ok(())
}

/// `k` coroutines hammer `op` for `contention_ms`; per-call latency includes pool queueing.
fn contend<M, G>(k: usize, c: &Cfg, var: &str, make: M) -> Row
where
    M: Fn(usize) -> G,
    G: FnMut(usize) + Send + 'static,
{
    let stop = Arc::new(AtomicBool::new(false));
    let counter = Arc::new(AtomicUsize::new(0));
    let (tx, rx) = may::sync::mpsc::channel::<Vec<f64>>();
    let start = Instant::now();
    for j in 0..k {
        let (stop, counter, tx) = (stop.clone(), counter.clone(), tx.clone());
        let mut op = make(j);
        may::go!(move || {
            let mut s = Vec::with_capacity(4096);
            while !stop.load(Ordering::Relaxed) {
                let i = counter.fetch_add(1, Ordering::Relaxed);
                let t = Instant::now();
                op(i);
                s.push(us(t.elapsed()));
            }
            let _ = tx.send(s);
        });
    }
    drop(tx);
    may::coroutine::sleep(Duration::from_millis(c.contention_ms));
    stop.store(true, Ordering::Relaxed);
    let mut all = Vec::new();
    while let Ok(mut s) = rx.recv() {
        all.append(&mut s);
    }
    let row = summarize("pk_contention", var, k, all, start.elapsed(), format!("pool/clients={}", c.pool_size));
    eprintln!("  k={k:<5} {var:<24} {:>9.0} ops/s p50 {:>8.1}µs p99 {:>9.1}µs", row.ops_per_s, row.p50_us, row.p99_us);
    row
}

// ---------------------------------------------------------------------------------------------
// reflector prototype: LISTEN -> (refresh from PG) -> SET in Redis

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    /// One SELECT and one SET per notification (the README's design).
    PerKey,
    /// Drain what is queued, one `= ANY($1)` SELECT, one pipelined batch of SETs.
    Batched,
}

fn reflector(
    c: &Cfg,
    host: &str,
    port: u16,
    admin: &may_postgres::Client,
    orm: &PooledLifeExecutor,
    rows: &mut Vec<Row>,
) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("reflector");
    let redis = may_redis::RedisClient::connect(host, port)?;
    let set = |k: &str, v: &str| -> Result<(), may_redis::RedisError> {
        redis
            .execute::<String>(may_redis::cmd("SET").arg(k).arg(v).arg("EX").arg(300u32))
            .map(|_| ())
    };

    // Redis primitives on their own.
    rows.push(seq("redis", "set_ex_single", c, "may_redis, one SET per round trip", |i| {
        set(&format!("perf:k:{}", i % 1000), "v").expect("set");
    }));
    rows.push(seq("redis", "set_ex_pipeline_64", c, "64 SETs per pipeline (latency per batch)", |i| {
        let mut p = redis.pipeline();
        for j in 0..64 {
            p.add(may_redis::cmd("SET").arg(format!("perf:p:{}", (i * 64 + j) % 4096)).arg("v").arg("EX").arg(300u32));
        }
        p.execute_raw().expect("pipeline");
    }));
    rows.push(seq("cache_read", "redis_get_hit", c, "GET of a warm key", |i| {
        let _ = redis.execute::<Option<String>>(may_redis::cmd("GET").arg(format!("perf:k:{}", i % 1000))).expect("get");
    }));
    rows.push(seq("cache_read", "orm_pool_pk_select", c, "same read served by Postgres", |i| {
        item::Entity::find().filter(item::Column::Id.eq(bulk_key(i))).find_one(orm).expect("orm");
    }));

    // NOTIFY payload limit (8000 bytes by default build).
    let ok = admin.batch_execute(&format!("SELECT pg_notify('perf_limit', repeat('x', {}))", 7999)).is_ok();
    let err = admin
        .batch_execute(&format!("SELECT pg_notify('perf_limit', repeat('x', {}))", 8000))
        .err()
        .map(|e| e.as_db_error().map_or_else(|| e.to_string(), |d| d.message().to_string()))
        .unwrap_or_else(|| "accepted".into());
    admin.batch_execute(
        "DROP TABLE IF EXISTS perf_fat; CREATE TABLE perf_fat (id BIGINT PRIMARY KEY, body TEXT NOT NULL);
         CREATE OR REPLACE FUNCTION perf_fat_notify() RETURNS trigger LANGUAGE plpgsql AS $f$
         BEGIN PERFORM pg_notify('perf_fat', row_to_json(NEW)::text); RETURN NULL; END $f$;
         CREATE TRIGGER perf_fat_notify AFTER INSERT ON perf_fat FOR EACH ROW EXECUTE FUNCTION perf_fat_notify();",
    )?;
    let fat = admin
        .batch_execute("INSERT INTO perf_fat VALUES (1, repeat('y', 9000))")
        .err()
        .map(|e| e.as_db_error().map_or_else(|| e.to_string(), |d| d.message().to_string()))
        .unwrap_or_else(|| "accepted".into());
    eprintln!("  payload 7999B ok={ok}; 8000B -> {err}; full-row trigger on a 9 KB row -> {fat}");
    rows.push(summarize("notify_payload_limit", "7999_bytes", 1, vec![], Duration::ZERO, if ok { "accepted" } else { "rejected" }));
    rows.push(summarize("notify_payload_limit", "8000_bytes", 1, vec![], Duration::ZERO, err));
    rows.push(summarize("notify_payload_limit", "row_to_json_9KB_row_insert", 1, vec![], Duration::ZERO, format!("INSERT fails: {fat}")));

    let paced_n = env_usize("PERF_REFLECT_PACED", 2000);
    let burst_ms = env_usize("PERF_REFLECT_BURST_MS", 3000) as u64;
    let writers = env_usize("PERF_REFLECT_WRITERS", 16);
    let mut base: i64 = 1;
    for mode in [Mode::PerKey, Mode::Batched] {
        let name = if mode == Mode::PerKey { "per_key" } else { "batched" };
        for (phase, paced) in [("paced_500_per_s", true), ("burst", false)] {
            admin.batch_execute("TRUNCATE perf_reflect")?;
            let cap = 2_000_000usize;
            let start_ns: Arc<Vec<AtomicU64>> = Arc::new((0..cap).map(|_| AtomicU64::new(0)).collect());
            let done_ns: Arc<Vec<AtomicU64>> = Arc::new((0..cap).map(|_| AtomicU64::new(0)).collect());
            let reflected = Arc::new(AtomicUsize::new(0));
            let stop = Arc::new(AtomicBool::new(false));
            let listener = spawn_listener(c, host, port, mode, base, start_ns.clone(), done_ns.clone(), reflected.clone(), stop.clone())?;
            may::coroutine::sleep(Duration::from_millis(200));

            let written = Arc::new(AtomicUsize::new(0));
            let write_lat = Arc::new(Mutex::new(Vec::<f64>::new()));
            let t0 = Instant::now();
            let n_writers = if paced { 1 } else { writers };
            let deadline = Instant::now() + Duration::from_millis(burst_ms);
            let mut hs = Vec::new();
            for w in 0..n_writers {
                let (url, start_ns, written, write_lat) = (c.admin_url.clone(), start_ns.clone(), written.clone(), write_lat.clone());
                hs.push(may::go!(move || {
                    let cl = may_postgres::connect(&url).expect("writer connect");
                    let st = cl.prepare("INSERT INTO perf_reflect (id, version, body) VALUES ($1, 1, $2)").expect("prep");
                    let body = "x".repeat(200);
                    let mut lat = Vec::new();
                    let mut j = 0usize;
                    loop {
                        let seq = if paced { j } else { j * n_writers + w };
                        if paced && seq >= paced_n || !paced && Instant::now() >= deadline || seq >= cap {
                            break;
                        }
                        let id = base + seq as i64;
                        start_ns[seq].store(now_ns(), Ordering::Release);
                        let t = Instant::now();
                        cl.execute(&st, &[&id, &body]).expect("insert");
                        lat.push(us(t.elapsed()));
                        written.fetch_add(1, Ordering::Relaxed);
                        if paced {
                            may::coroutine::sleep(Duration::from_millis(2));
                        }
                        j += 1;
                    }
                    write_lat.lock().expect("lat").extend(lat);
                }));
            }
            for h in hs {
                let _ = h.join();
            }
            let write_wall = t0.elapsed();
            let total = written.load(Ordering::Relaxed);
            // Drain: wait until everything written has been reflected (or 30 s).
            let drain_t = Instant::now();
            while reflected.load(Ordering::Relaxed) < total && drain_t.elapsed() < Duration::from_secs(30) {
                may::coroutine::sleep(Duration::from_millis(5));
            }
            let all_wall = t0.elapsed();
            stop.store(true, Ordering::Relaxed);
            let _ = listener.join();
            let mut e2e = Vec::with_capacity(total);
            let mut missing = 0usize;
            for s in 0..total.min(cap) {
                let (a, b) = (start_ns[s].load(Ordering::Acquire), done_ns[s].load(Ordering::Acquire));
                if a == 0 {
                    continue;
                }
                if b == 0 {
                    missing += 1;
                } else {
                    e2e.push((b.saturating_sub(a)) as f64 / 1000.0);
                }
            }
            let wl = std::mem::take(&mut *write_lat.lock().expect("lat"));
            let wrow = summarize("reflector_write", &format!("{name}_{phase}"), n_writers, wl, write_wall, "INSERT with NOTIFY trigger (commit)");
            let mut erow = summarize(
                "reflector_e2e",
                &format!("{name}_{phase}"),
                n_writers,
                e2e,
                all_wall,
                format!("write start -> Redis SET done; written {total}, missing {missing}, drain {:.0} ms", (all_wall - write_wall).as_secs_f64() * 1e3),
            );
            erow.ops_per_s = reflected.load(Ordering::Relaxed) as f64 / all_wall.as_secs_f64();
            eprintln!(
                "  {name}/{phase}: wrote {total} ({:.0}/s), e2e p50 {:.0}µs p99 {:.0}µs, reflected {:.0}/s, missing {missing}",
                total as f64 / write_wall.as_secs_f64(),
                erow.p50_us,
                erow.p99_us,
                erow.ops_per_s
            );
            rows.push(wrow);
            rows.push(erow);
            base += cap as i64;
        }
    }
    Ok(())
}

/// Wraps the real Redis sink and stamps when each key's invalidation completed.
struct TimingSink {
    inner: lifeguard_reflector::RedisSink,
    base: i64,
    done_ns: Arc<Vec<AtomicU64>>,
    reflected: Arc<AtomicUsize>,
}

impl lifeguard_reflector::CacheSink for TimingSink {
    fn invalidate(&self, keys: &[String]) -> Result<u64, lifeguard_reflector::SinkError> {
        let n = self.inner.invalidate(keys)?;
        for k in keys {
            if let Some(id) = k.rsplit(':').next().and_then(|s| s.parse::<i64>().ok()) {
                mark(&self.done_ns, self.base, id);
            }
        }
        self.reflected.fetch_add(keys.len(), Ordering::Relaxed);
        Ok(n)
    }
    fn invalidate_matching_page(&self, p: &str, c: i64) -> Result<(i64, u64), lifeguard_reflector::SinkError> {
        self.inner.invalidate_matching_page(p, c)
    }
    fn get_i64(&self, k: &str) -> Result<Option<i64>, lifeguard_reflector::SinkError> {
        self.inner.get_i64(k)
    }
    fn set_i64(&self, k: &str, v: i64) -> Result<(), lifeguard_reflector::SinkError> {
        self.inner.set_i64(k, v)
    }
    fn ping(&self) -> Result<(), lifeguard_reflector::SinkError> {
        self.inner.ping()
    }
    fn eviction_policy(&self) -> Option<String> {
        self.inner.eviction_policy()
    }
}

/// The `lifeguard-reflector` crate end to end: write -> trigger -> NOTIFY -> batched UNLINK.
fn reflector_crate(c: &Cfg, host: &str, port: u16, admin: &may_postgres::Client, rows: &mut Vec<Row>) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("reflector_crate (lifeguard-reflector, invalidate)");
    let writers = env_usize("PERF_REFLECT_WRITERS", 16);
    let burst_ms = env_usize("PERF_REFLECT_BURST_MS", 3000) as u64;
    let paced_n = env_usize("PERF_REFLECT_PACED", 2000);
    let mut base: i64 = 100_000_000;
    // Optional: a large unrelated keyspace, so the start-up catch-up (a SCAN of every key) is
    // long, and writes that start immediately show whether live invalidation waits for it.
    let preload = env_usize("PERF_REFLECT_PRELOAD_KEYS", 0);
    let no_wait = matches!(env::var("PERF_REFLECT_NO_WAIT").as_deref(), Ok("1"));
    for (phase, paced) in [("paced_500_per_s", true), ("burst", false)] {
        if preload > 0 {
            let r = may_redis::RedisClient::connect(host, port)?;
            for chunk in (0..preload).collect::<Vec<_>>().chunks(1000) {
                let mut p = r.pipeline();
                for i in chunk {
                    p.add(may_redis::cmd("SET").arg(format!("perf:junk:{i}")).arg("v").arg("EX").arg(600u32));
                }
                p.execute_raw()?;
            }
            eprintln!("  crate/{phase}: preloaded {preload} unrelated keys");
        }
        admin.batch_execute("TRUNCATE perf_reflect")?;
        let cap = 2_000_000usize;
        let start_ns: Arc<Vec<AtomicU64>> = Arc::new((0..cap).map(|_| AtomicU64::new(0)).collect());
        let done_ns: Arc<Vec<AtomicU64>> = Arc::new((0..cap).map(|_| AtomicU64::new(0)).collect());
        let reflected = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let mut cfg = lifeguard_reflector::Config::new(c.admin_url.clone(), format!("redis://{host}:{port}"));
        cfg.channels = vec!["perf_reflect".into()];
        cfg.key_prefix = "perf:lr:".into();
        cfg.watermark_key = "perf:lr:__wm".into();
        cfg.metrics_addr = None;
        cfg.tables = vec![lifeguard_reflector::TableSpec { table: "perf_reflect".into(), catch_up: lifeguard_reflector::CatchUp::Flush }];
        cfg.lock_key = 0x5eed_0000 + i64::from(paced);
        let (url, b, d, r, st) = (cfg.redis_url.clone(), base, done_ns.clone(), reflected.clone(), stop.clone());
        let h = may::go!(move || {
            let factory = move || -> Result<Box<dyn lifeguard_reflector::CacheSink>, lifeguard_reflector::SinkError> {
                Ok(Box::new(TimingSink { inner: lifeguard_reflector::RedisSink::connect(&url)?, base: b, done_ns: d.clone(), reflected: r.clone() }))
            };
            lifeguard_reflector::run(cfg, &factory, &st)
        });
        // Wait for leadership and the initial catch-up (a full SCAN of the keyspace for a
        // flush-mode table), so the measurement is of steady-state invalidation.
        let m = lifeguard_reflector::metrics::metrics();
        let catch_ups_before = m.catch_ups.with_label_values(&["flush"]).get();
        let t_ready = Instant::now();
        while !no_wait
            && (m.leader.get() != 1 || m.catch_ups.with_label_values(&["flush"]).get() == catch_ups_before)
            && t_ready.elapsed() < Duration::from_secs(60)
        {
            may::coroutine::sleep(Duration::from_millis(10));
        }
        if no_wait {
            // Leadership itself is quick; only the catch-up is left running.
            while m.leader.get() != 1 && t_ready.elapsed() < Duration::from_secs(10) {
                may::coroutine::sleep(Duration::from_millis(2));
            }
            eprintln!("  crate/{phase}: leader after {:.0} ms; writing during the catch-up", t_ready.elapsed().as_secs_f64() * 1e3);
        } else {
            eprintln!("  crate/{phase}: leader and catch-up done after {:.0} ms", t_ready.elapsed().as_secs_f64() * 1e3);
        }
        reflected.store(0, Ordering::Relaxed);
        let written = Arc::new(AtomicUsize::new(0));
        let t0 = Instant::now();
        let n_writers = if paced { 1 } else { writers };
        let deadline = Instant::now() + Duration::from_millis(burst_ms);
        let mut hs = Vec::new();
        for w in 0..n_writers {
            let (url, start_ns, written) = (c.admin_url.clone(), start_ns.clone(), written.clone());
            hs.push(may::go!(move || {
                let cl = may_postgres::connect(&url).expect("writer connect");
                let st = cl.prepare("INSERT INTO perf_reflect (id, version, body) VALUES ($1, 1, $2)").expect("prep");
                let body = "x".repeat(200);
                let mut j = 0usize;
                loop {
                    let seq = if paced { j } else { j * n_writers + w };
                    if paced && seq >= paced_n || !paced && Instant::now() >= deadline || seq >= cap {
                        break;
                    }
                    start_ns[seq].store(now_ns(), Ordering::Release);
                    cl.execute(&st, &[&(base + seq as i64), &body]).expect("insert");
                    written.fetch_add(1, Ordering::Relaxed);
                    if paced {
                        may::coroutine::sleep(Duration::from_millis(2));
                    }
                    j += 1;
                }
            }));
        }
        for h in hs {
            let _ = h.join();
        }
        let write_wall = t0.elapsed();
        let total = written.load(Ordering::Relaxed);
        let drain_t = Instant::now();
        while reflected.load(Ordering::Relaxed) < total && drain_t.elapsed() < Duration::from_secs(30) {
            may::coroutine::sleep(Duration::from_millis(5));
        }
        let all_wall = t0.elapsed();
        if no_wait {
            eprintln!(
                "  crate/{phase}: catch-ups completed so far {} (keys removed {})",
                m.catch_ups.with_label_values(&["flush"]).get(),
                m.catch_up_keys.get()
            );
        }
        stop.store(true, Ordering::Relaxed);
        let _ = h.join();
        let mut e2e = Vec::with_capacity(total);
        let mut missing = 0usize;
        for s in 0..total.min(cap) {
            let (a, b) = (start_ns[s].load(Ordering::Acquire), done_ns[s].load(Ordering::Acquire));
            if a == 0 {
                continue;
            }
            if b == 0 {
                missing += 1;
            } else {
                e2e.push((b.saturating_sub(a)) as f64 / 1000.0);
            }
        }
        let batches = lifeguard_reflector::metrics::metrics().flushes.with_label_values(&["ok"]).get();
        let mut erow = summarize(
            "reflector_crate_e2e",
            phase,
            n_writers,
            e2e,
            all_wall,
            format!("write start -> UNLINK done; written {total}, missing {missing}, flushes so far {batches}, drain {:.0} ms", (all_wall - write_wall).as_secs_f64() * 1e3),
        );
        erow.ops_per_s = reflected.load(Ordering::Relaxed) as f64 / all_wall.as_secs_f64();
        eprintln!("  crate/{phase}: wrote {total}, e2e p50 {:.0}µs p99 {:.0}µs, {:.0} keys/s, missing {missing}", erow.p50_us, erow.p99_us, erow.ops_per_s);
        rows.push(erow);
        base += cap as i64;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn spawn_listener(
    c: &Cfg,
    host: &str,
    port: u16,
    mode: Mode,
    base: i64,
    start_ns: Arc<Vec<AtomicU64>>,
    done_ns: Arc<Vec<AtomicU64>>,
    reflected: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
) -> Result<may::coroutine::JoinHandle<()>, Box<dyn std::error::Error>> {
    let listen = may_postgres::connect(&c.admin_url)?;
    listen.batch_execute("LISTEN perf_reflect")?;
    let reader = may_postgres::connect(&c.admin_url)?;
    let one = reader.prepare("SELECT id, version, body FROM perf_reflect WHERE id = $1")?;
    let many = reader.prepare("SELECT id, version, body FROM perf_reflect WHERE id = ANY($1)")?;
    let redis = may_redis::RedisClient::connect(host, port)?;
    let poll = Duration::from_micros(env_usize("PERF_REFLECT_POLL_US", 200) as u64);
    let _ = start_ns;
    Ok(may::go!(move || {
        let mut batch: Vec<i64> = Vec::with_capacity(256);
        loop {
            batch.clear();
            while let Some(n) = listen.notifications().pop() {
                let v: serde_json::Value = serde_json::from_str(n.payload()).expect("payload json");
                batch.push(v["id"].as_i64().expect("id"));
                if mode == Mode::PerKey || batch.len() >= 256 {
                    break;
                }
            }
            if batch.is_empty() {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                // may_postgres exposes notifications as a queue with no wake-up: poll it.
                may::coroutine::sleep(poll);
                continue;
            }
            match mode {
                Mode::PerKey => {
                    let id = batch[0];
                    let r = reader.query_one(&one, &[&id]).expect("refresh");
                    let body: String = r.get(2);
                    let version: i64 = r.get(1);
                    let val = format!("{{\"v\":{version},\"b\":\"{body}\"}}");
                    redis
                        .execute::<String>(may_redis::cmd("SET").arg(format!("perf:reflect:{id}")).arg(val).arg("EX").arg(300u32))
                        .expect("redis set");
                    mark(&done_ns, base, id);
                    reflected.fetch_add(1, Ordering::Relaxed);
                }
                Mode::Batched => {
                    let rs = reader.query(&many, &[&batch]).expect("refresh many");
                    let mut p = redis.pipeline();
                    for r in &rs {
                        let id: i64 = r.get(0);
                        let version: i64 = r.get(1);
                        let body: String = r.get(2);
                        p.add(may_redis::cmd("SET").arg(format!("perf:reflect:{id}")).arg(format!("{{\"v\":{version},\"b\":\"{body}\"}}")).arg("EX").arg(300u32));
                    }
                    p.execute_raw().expect("pipeline");
                    for id in &batch {
                        mark(&done_ns, base, *id);
                    }
                    reflected.fetch_add(batch.len(), Ordering::Relaxed);
                }
            }
        }
    }))
}

fn mark(done_ns: &[AtomicU64], base: i64, id: i64) {
    let s = (id - base) as usize;
    if s < done_ns.len() {
        done_ns[s].store(now_ns(), Ordering::Release);
    }
}

// ---------------------------------------------------------------------------------------------
// tokio-postgres baseline (same SQL, prepared, same pool width)

fn run_tokio(c: &Cfg, rows: &mut Vec<Row>) -> Result<(), Box<dyn std::error::Error>> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(c.may_workers)
        .enable_all()
        .build()?;
    rt.block_on(async {
        let connect = |url: String| async move {
            let (cl, conn) = tokio_postgres::connect(&url, tokio_postgres::NoTls).await.expect("tokio connect");
            tokio::spawn(conn);
            cl
        };
        let cl = connect(c.admin_url.clone()).await;
        let pk = cl.prepare(PK_SQL).await?;
        let list = cl.prepare(LIST_SQL).await?;
        let ins = cl.prepare(INSERT_SQL).await?;
        let upd = cl.prepare(UPDATE_SQL).await?;
        let ups = cl.prepare(UPSERT_SQL).await?;
        let tmap = |r: &tokio_postgres::Row| item::ItemModel {
            id: r.get(0),
            account_id: r.get(1),
            name: r.get(2),
            qty: r.get(3),
            price: r.get(4),
            active: r.get(5),
            updated_at: r.get(6),
        };
        macro_rules! tseq {
            ($scn:expr, $var:expr, $note:expr, |$i:ident| $body:expr) => {{
                for $i in 0..c.warmup {
                    $body;
                }
                let mut s = Vec::with_capacity(c.iters);
                let wall = Instant::now();
                for k in 0..c.iters {
                    let $i = c.warmup + k;
                    let t = Instant::now();
                    $body;
                    s.push(us(t.elapsed()));
                }
                let row = summarize($scn, $var, 1, s, wall.elapsed(), $note);
                eprintln!("  {:<14} {:<26} p50 {:>8.1}µs p99 {:>8.1}µs", row.scenario, row.variant, row.p50_us, row.p99_us);
                rows.push(row);
            }};
        }
        if wants(c, "pk_select") {
            tseq!("pk_select", "raw_tokio_prepared", "tokio-postgres, cached Statement", |i| {
                let r = cl.query_one(&pk, &[&bulk_key(i)]).await?;
                let _ = tmap(&r);
            });
            tseq!("pk_select", "raw_tokio_unprepared", "tokio-postgres query_one(&str)", |i| {
                let r = cl.query_one(PK_SQL, &[&bulk_key(i)]).await?;
                let _ = tmap(&r);
            });
        }
        if wants(c, "list") {
            for n in [10i64, 100, 1000] {
                let scn = format!("list_{n}");
                tseq!(&scn, "raw_tokio_prepared", &format!("{n} rows"), |_i| {
                    let v: Vec<item::ItemModel> = cl.query(&list, &[&n]).await?.iter().map(tmap).collect();
                    assert_eq!(v.len() as i64, n);
                });
            }
        }
        if wants(c, "write") {
            let mut next = 40_000_000i64;
            tseq!("insert", "raw_tokio_prepared", "", |i| {
                next += 1;
                let name = format!("n{i}");
                cl.execute(&ins, &[&next, &7i64, &name, &1i32, &1.5f64, &true, &ts()]).await?;
            });
            tseq!("update", "raw_tokio_prepared", "", |i| {
                let r = cl.query_one(&upd, &[&bulk_key(i), &((i % 1000) as i32)]).await?;
                let _ = tmap(&r);
            });
            tseq!("upsert", "raw_tokio_prepared", "", |i| {
                let name = format!("n{i}");
                let r = cl.query_one(&ups, &[&bulk_key(i), &7i64, &name, &((i % 1000) as i32), &1.5f64, &true, &ts()]).await?;
                let _ = tmap(&r);
            });
        }
        if wants(c, "contention") {
            let mut clients = Vec::new();
            for _ in 0..c.pool_size {
                let cl = connect(c.admin_url.clone()).await;
                let st = cl.prepare(PK_SQL).await?;
                clients.push((cl, st));
            }
            let clients = Arc::new(clients);
            for &k in &c.concurrency {
                let stop = Arc::new(AtomicBool::new(false));
                let counter = Arc::new(AtomicUsize::new(0));
                let start = Instant::now();
                let mut hs = Vec::new();
                for _ in 0..k {
                    let (stop, counter, clients) = (stop.clone(), counter.clone(), clients.clone());
                    hs.push(tokio::spawn(async move {
                        let mut s = Vec::with_capacity(4096);
                        while !stop.load(Ordering::Relaxed) {
                            let i = counter.fetch_add(1, Ordering::Relaxed);
                            let (cl, st) = &clients[i % clients.len()];
                            let t = Instant::now();
                            let _ = cl.query_one(st, &[&bulk_key(i)]).await.expect("q");
                            s.push(us(t.elapsed()));
                        }
                        s
                    }));
                }
                tokio::time::sleep(Duration::from_millis(c.contention_ms)).await;
                stop.store(true, Ordering::Relaxed);
                let mut all = Vec::new();
                for h in hs {
                    all.extend(h.await?);
                }
                let row = summarize("pk_contention", "raw_tokio_shared_clients", k, all, start.elapsed(), format!("pool/clients={}", c.pool_size));
                eprintln!("  k={k:<5} raw_tokio_shared_clients {:>9.0} ops/s p50 {:>8.1}µs p99 {:>9.1}µs", row.ops_per_s, row.p50_us, row.p99_us);
                rows.push(row);
            }
        }
        Ok::<(), Box<dyn std::error::Error>>(())
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let c = cfg()?;
    let _ = epoch();
    may::config()
        .set_workers(c.may_workers)
        .set_stack_size(0x40000);
    eprintln!(
        "lifeguard-perf [{}]: pool_size={} iters={} warmup={} may_workers={} statement_cache_env={:?}",
        c.label,
        c.pool_size,
        c.iters,
        c.warmup,
        c.may_workers,
        env::var("LIFEGUARD_STATEMENT_CACHE_CAPACITY").ok()
    );

    let c = Arc::new(c);
    let may_rows = {
        let c = c.clone();
        may::go!(move || {
            let mut rows = Vec::new();
            if let Err(e) = run_may(&c, &mut rows) {
                eprintln!("may side failed: {e}");
                std::process::exit(2);
            }
            rows
        })
        .join()
        .map_err(|_| "may side panicked")?
    };
    let mut rows = may_rows;
    run_tokio(&c, &mut rows)?;

    // Group rows by scenario in first-seen order so ORM and baselines sit together.
    let mut order: Vec<String> = Vec::new();
    for r in &rows {
        if !order.contains(&r.scenario) {
            order.push(r.scenario.clone());
        }
    }
    rows.sort_by_key(|r| (order.iter().position(|s| s == &r.scenario), r.concurrency));

    println!("## lifeguard-perf: {}\n", c.label);
    println!("{}", markdown(&rows));
    if let Ok(path) = env::var("PERF_OUTPUT_JSON") {
        std::fs::write(&path, serde_json::to_string_pretty(&rows)?)?;
        eprintln!("json: {path}");
    }
    Ok(())
}
