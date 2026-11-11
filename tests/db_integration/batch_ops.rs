//! Integration tests for `lifeguard::batch`: multi-row writes, UNNEST, chunking, RETURNING,
//! per-chunk errors, batched reads, RLS context and the buffered `BatchWriter`.

use lifeguard::query::column::column_trait::ColumnTrait;
use lifeguard::{
    delete_many, find_by_ids, insert_many, test_helpers::TestDatabase, update_many, upsert_many,
    BatchFailure, BatchOptions, BatchWriter, BatchWriterConfig, LifeExecutor, LifeModelTrait,
    LifeguardPool, MayPostgresExecutor, OnConflict, PooledLifeExecutor, SessionContext, WriteMode,
};
use lifeguard_derive::{LifeModel, LifeRecord};
use sea_query::Value;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub mod batch_item {
    use super::{LifeModel, LifeRecord};

    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_batch_items"]
    pub struct BatchItem {
        #[primary_key]
        pub id: i64,
        pub sku: String,
        pub qty: i32,
        pub note: Option<String>,
        #[column_type = "JSONB"]
        pub meta: serde_json::Value,
    }
}
use batch_item::{BatchItemRecord, Column, Entity};

fn executor() -> MayPostgresExecutor {
    static SCHEMA: std::sync::Once = std::sync::Once::new();
    let ctx = crate::context::get_test_context();
    let mut db = TestDatabase::with_url(&ctx.pg_url);
    let _c = db.connect().expect("connect");
    let exec = db.executor().expect("executor");
    SCHEMA.call_once(|| {
        // Multi-statement text needs the simple protocol.
        let c = may_postgres::connect(&ctx.pg_url).expect("connect");
        c.batch_execute(
            "DROP TABLE IF EXISTS test_batch_items;
             CREATE TABLE test_batch_items (
               id BIGINT PRIMARY KEY, sku TEXT NOT NULL UNIQUE, qty INTEGER NOT NULL,
               note TEXT, meta JSONB NOT NULL)",
        )
        .expect("schema");
    });
    exec
}

/// Writers run on their own coroutine and need a `Send + Sync` executor: a pool.
fn pooled() -> PooledLifeExecutor {
    let _ = executor(); // schema
    let ctx = crate::context::get_test_context();
    PooledLifeExecutor::new(Arc::new(
        LifeguardPool::new(&ctx.pg_url, 2, Vec::new(), 0).expect("pool"),
    ))
}

/// Disjoint id ranges per test so tests can share the table in parallel.
fn id_base() -> i64 {
    static NEXT: AtomicI64 = AtomicI64::new(0);
    let run = i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_micros()
            % 1_000_000,
    )
    .expect("fits");
    (run * 1_000 + NEXT.fetch_add(1, Ordering::Relaxed)) * 100_000
}

fn rec(id: i64, qty: i32) -> BatchItemRecord {
    let mut r = BatchItemRecord::new();
    r.set_id(id)
        .set_sku(format!("sku-{id}"))
        .set_qty(qty)
        .set_note(if id % 2 == 0 {
            Some(format!("n{id}"))
        } else {
            None
        })
        .set_meta(serde_json::json!({ "id": id, "tags": ["a", "b"] }));
    r
}

fn ids(range: std::ops::Range<i64>) -> Vec<Value> {
    range.map(|i| Value::BigInt(Some(i))).collect()
}

fn count(exec: &MayPostgresExecutor, lo: i64, hi: i64) -> i64 {
    Entity::find()
        .filter(Column::Id.gte(lo))
        .filter(Column::Id.lt(hi))
        .count()
        .one(exec)
        .expect("count")
}

#[test]
fn insert_many_values_and_unnest_with_returning() {
    let exec = executor();
    let b = id_base();
    // Below unnest_min_rows: multi-row VALUES.
    let small: Vec<_> = (b..b + 10).map(|i| rec(i, 1)).collect();
    let out = insert_many(&exec, &small, &BatchOptions::returning()).expect("values insert");
    assert_eq!(out.rows_affected, 10);
    assert_eq!(out.statements, 1);
    assert_eq!(out.models.len(), 10);
    assert!(out.models.iter().all(|m| m.meta["tags"][1] == "b"));

    // Above it: UNNEST arrays, one statement for 300 rows.
    let big: Vec<_> = (b + 10..b + 310).map(|i| rec(i, 2)).collect();
    let out = insert_many(&exec, &big, &BatchOptions::default()).expect("unnest insert");
    assert_eq!(out.rows_affected, 300);
    assert_eq!(out.statements, 1);
    assert_eq!(count(&exec, b, b + 310), 310);
    let back = find_by_ids::<Entity>(&exec, &ids(b + 300..b + 302), &BatchOptions::default())
        .expect("find");
    let mut back: Vec<_> = back.into_iter().map(|m| (m.id, m.note, m.meta)).collect();
    back.sort_by_key(|t| t.0);
    assert_eq!(back[0].1.as_deref(), Some(format!("n{}", b + 300).as_str()));
    assert_eq!(back[1].1, None);
    assert_eq!(back[0].2["id"], b + 300);

    // Chunking by max_rows.
    let opts = BatchOptions {
        max_rows: 50,
        ..BatchOptions::default()
    };
    let chunked: Vec<_> = (b + 400..b + 650).map(|i| rec(i, 3)).collect();
    let out = insert_many(&exec, &chunked, &opts).expect("chunked insert");
    assert_eq!(out.statements, 5);
    assert_eq!(count(&exec, b + 400, b + 650), 250);
}

#[test]
fn chunking_respects_the_parameter_cap() {
    let exec = executor();
    let b = id_base();
    // 5 columns per row; a 100-parameter cap forces 20-row chunks even with large max_rows and
    // no UNNEST.
    let opts = BatchOptions {
        max_rows: 10_000,
        max_params: 100,
        unnest_min_rows: usize::MAX,
        returning: false,
        ..BatchOptions::default()
    };
    let rows: Vec<_> = (b..b + 95).map(|i| rec(i, 1)).collect();
    let out = insert_many(&exec, &rows, &opts).expect("insert");
    assert_eq!(out.statements, 5);
    assert_eq!(out.rows_affected, 95);
}

#[test]
fn upsert_many_updates_existing_and_inserts_new() {
    let exec = executor();
    let b = id_base();
    let first: Vec<_> = (b..b + 20).map(|i| rec(i, 1)).collect();
    insert_many(&exec, &first, &BatchOptions::default()).expect("seed");
    let second: Vec<_> = (b + 10..b + 30).map(|i| rec(i, 7)).collect();
    let conflict = OnConflict::column(Column::Id)
        .update_columns([Column::Qty])
        .to_owned();
    let out = upsert_many(&exec, &second, conflict, &BatchOptions::returning()).expect("upsert");
    assert_eq!(out.rows_affected, 20);
    assert!(out.models.iter().all(|m| m.qty == 7));
    let all =
        find_by_ids::<Entity>(&exec, &ids(b..b + 30), &BatchOptions::default()).expect("find");
    assert_eq!(all.len(), 30);
    assert_eq!(all.iter().filter(|m| m.qty == 7).count(), 20);

    // DO NOTHING skips existing keys and reports only the new rows.
    let third: Vec<_> = (b + 25..b + 35).map(|i| rec(i, 9)).collect();
    let out = upsert_many(
        &exec,
        &third,
        OnConflict::column(Column::Id).do_nothing().to_owned(),
        &BatchOptions::default(),
    )
    .expect("do nothing");
    assert_eq!(out.rows_affected, 5);
}

#[test]
fn update_many_and_delete_many() {
    let exec = executor();
    let b = id_base();
    let seed: Vec<_> = (b..b + 100).map(|i| rec(i, 1)).collect();
    insert_many(&exec, &seed, &BatchOptions::default()).expect("seed");
    let changes: Vec<_> = (b..b + 100)
        .map(|i| {
            let mut r = BatchItemRecord::new();
            r.set_id(i).set_qty(i32::try_from(i - b).expect("small"));
            r
        })
        .collect();
    let out = update_many(&exec, &changes, &[Column::Qty], &BatchOptions::returning())
        .expect("update_many");
    assert_eq!(out.rows_affected, 100);
    assert_eq!(out.statements, 1);
    assert!(out.models.iter().all(|m| i64::from(m.qty) == m.id - b));
    assert!(
        out.models.iter().all(|m| m.sku == format!("sku-{}", m.id)),
        "other columns untouched"
    );

    let out = delete_many::<Entity>(&exec, &ids(b..b + 50), &BatchOptions::default())
        .expect("delete_many");
    assert_eq!(out.rows_affected, 50);
    let left =
        find_by_ids::<Entity>(&exec, &ids(b..b + 100), &BatchOptions::default()).expect("find");
    assert_eq!(left.len(), 50);
    assert!(left.iter().all(|m| m.id >= b + 50));
}

#[test]
fn find_by_ids_chunks_and_ignores_missing() {
    let exec = executor();
    let b = id_base();
    let seed: Vec<_> = (b..b + 30).map(|i| rec(i, 1)).collect();
    insert_many(&exec, &seed, &BatchOptions::default()).expect("seed");
    let opts = BatchOptions {
        max_rows: 7,
        ..BatchOptions::default()
    };
    let found = find_by_ids::<Entity>(&exec, &ids(b - 5..b + 40), &opts).expect("find");
    assert_eq!(found.len(), 30);
    assert!(find_by_ids::<Entity>(&exec, &[], &opts)
        .expect("empty")
        .is_empty());
}

#[test]
fn error_reports_the_failing_chunk() {
    let exec = executor();
    let b = id_base();
    insert_many(&exec, &[rec(b + 25, 1)], &BatchOptions::default()).expect("seed duplicate");
    let rows: Vec<_> = (b..b + 30)
        .filter(|i| *i != b + 25)
        .chain([b + 25])
        .map(|i| rec(i, 1))
        .collect();
    let opts = BatchOptions {
        max_rows: 10,
        ..BatchOptions::default()
    };
    let err = insert_many(&exec, &rows, &opts).expect_err("duplicate key must fail");
    assert_eq!(err.chunk, 2, "{err}");
    assert_eq!(err.rows, 20..30);
    assert_eq!(err.rows_written_before, 20);
    assert!(err.to_string().contains("chunk 2"), "{err}");
    // Each chunk is its own statement: the first two are committed.
    assert_eq!(count(&exec, b, b + 30), 21);
}

#[test]
fn batch_writer_flushes_on_size_time_and_close_and_dedupes_keys() {
    let exec = executor();
    let b = id_base();
    let failed = Arc::new(Mutex::new(Vec::new()));
    let f = failed.clone();
    let writer: BatchWriter<BatchItemRecord> = BatchWriter::new(
        pooled(),
        WriteMode::Upsert {
            on_conflict: OnConflict::column(Column::Id)
                .update_columns([Column::Qty])
                .to_owned(),
            key: vec![Column::Id],
        },
        BatchWriterConfig {
            max_rows: 100,
            max_delay: Duration::from_millis(30),
            capacity: 1_000,
            ..BatchWriterConfig::default()
        },
        move |e, rows| f.lock().expect("lock").push((e.to_string(), rows.len())),
    );
    // Size trigger: 250 rows -> at least two full batches without waiting.
    for i in 0..250 {
        writer
            .push(rec(b + i, 1))
            .map_err(|_| "closed")
            .expect("push");
    }
    // Same key twice in one batch: the later row wins.
    writer
        .push(rec(b + 1_000, 1))
        .map_err(|_| "closed")
        .expect("push");
    writer
        .push(rec(b + 1_000, 5))
        .map_err(|_| "closed")
        .expect("push");
    // Time trigger: wait past max_delay without flushing explicitly.
    may::coroutine::sleep(Duration::from_millis(300));
    assert_eq!(count(&exec, b, b + 1_001), 251, "time-based flush");
    let one = find_by_ids::<Entity>(&exec, &ids(b + 1_000..b + 1_001), &BatchOptions::default())
        .expect("find");
    assert_eq!(one[0].qty, 5);

    // Close flushes the tail.
    for i in 2_000..2_010 {
        writer
            .push(rec(b + i, 1))
            .map_err(|_| "closed")
            .expect("push");
    }
    let stats = writer.close();
    assert_eq!(count(&exec, b + 2_000, b + 2_010), 10, "close flushes");
    assert_eq!(stats.pushed, 262);
    assert_eq!(stats.superseded, 1);
    assert_eq!(stats.failed, 0);
    assert!(failed.lock().expect("lock").is_empty());
}

#[test]
fn batch_writer_hands_poison_batches_to_the_error_handler() {
    let exec = executor();
    let b = id_base();
    insert_many(&exec, &[rec(b, 1)], &BatchOptions::default()).expect("seed");
    let failed = Arc::new(Mutex::new(Vec::new()));
    let f = failed.clone();
    let writer: BatchWriter<BatchItemRecord> = BatchWriter::new(
        pooled(),
        WriteMode::Insert,
        BatchWriterConfig {
            retries: 1,
            retry_backoff: Duration::from_millis(5),
            // Strict: the whole batch is retried, then handed over.
            options: BatchOptions::default(),
            ..BatchWriterConfig::default()
        },
        move |_e, rows| f.lock().expect("lock").push(rows.len()),
    );
    writer.push(rec(b, 1)).map_err(|_| "closed").expect("push");
    let stats = writer.close();
    assert_eq!(stats.failed, 1);
    assert_eq!(stats.retries, 1);
    assert_eq!(*failed.lock().expect("lock"), vec![1]);
}

#[test]
fn batch_writer_bisect_hands_over_only_the_bad_rows() {
    let exec = executor();
    let b = id_base();
    insert_many(
        &exec,
        &[rec(b + 3, 1), rec(b + 11, 1)],
        &BatchOptions::default(),
    )
    .expect("seed");
    let failed = Arc::new(Mutex::new(Vec::new()));
    let f = failed.clone();
    let writer: BatchWriter<BatchItemRecord> = BatchWriter::new(
        pooled(),
        WriteMode::Insert,
        BatchWriterConfig {
            max_rows: 1_000,
            max_delay: Duration::from_secs(60),
            retries: 1,
            retry_backoff: Duration::from_millis(5),
            options: BatchOptions::bisect(),
            ..BatchWriterConfig::default()
        },
        move |e, rows| {
            f.lock().expect("lock").push((e.rows.start, rows.len()));
        },
    );
    for i in 0..16 {
        writer
            .push(rec(b + i, 1))
            .map_err(|_| "closed")
            .expect("push");
    }
    let stats = writer.close();
    assert_eq!(stats.failed, 2);
    assert_eq!(stats.retries, 0, "row errors are bisected, not retried");
    assert_eq!(stats.written, 14);
    let mut got = failed.lock().expect("lock").clone();
    got.sort_unstable();
    assert_eq!(got, vec![(3, 1), (11, 1)]);
    assert_eq!(count(&exec, b, b + 16), 16, "14 new rows plus the 2 seeded");
}

pub mod batch_rls {
    use super::{LifeModel, LifeRecord};

    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_batch_rls"]
    #[schema_name = "public"]
    pub struct BatchRls {
        #[primary_key]
        pub id: i64,
        pub org: String,
        pub qty: i32,
    }
}

const ORG_A: &str = "550e8400-e29b-41d4-a716-4466554400a1";
const ORG_B: &str = "550e8400-e29b-41d4-a716-4466554400b2";

fn org_ctx(org: &str) -> SessionContext {
    SessionContext {
        tenant_id: "batch".to_string(),
        subject_id: uuid::Uuid::new_v4(),
        organization_id: uuid::Uuid::parse_str(org).expect("uuid"),
        session_id: format!("batch-{}", uuid::Uuid::new_v4()),
        roles: vec!["writer".into()],
        permissions: vec!["write".into()],
        user_type: Some("writer".into()),
        org_type: Some("tenant".into()),
    }
}

#[test]
fn batch_operations_run_under_rls_session_context() {
    let ctx = crate::context::get_test_context();
    let admin = may_postgres::connect(&ctx.pg_url).expect("admin");
    admin
        .batch_execute(
            "SELECT pg_advisory_lock(7211915001);
             DROP TABLE IF EXISTS public.test_batch_rls;
             CREATE TABLE public.test_batch_rls (id BIGINT PRIMARY KEY, org TEXT NOT NULL, qty INTEGER NOT NULL);
             ALTER TABLE public.test_batch_rls ENABLE ROW LEVEL SECURITY;
             CREATE POLICY org_only ON public.test_batch_rls
               USING (org = NULLIF(current_setting('app.organization_id', true), ''))
               WITH CHECK (org = NULLIF(current_setting('app.organization_id', true), ''));
             GRANT SELECT, INSERT, UPDATE, DELETE ON public.test_batch_rls TO rls_test_role;
             SELECT pg_advisory_unlock(7211915001);",
        )
        .expect("rls fixture");
    // Run as the non-superuser role (superusers bypass RLS) on the suite's own URL, whatever its
    // credentials: the role and `rls_set_session` come from `rls_integration`'s ctor.
    let as_role = || {
        let conn = may_postgres::connect(&ctx.pg_url).expect("connect");
        conn.batch_execute("SET ROLE rls_test_role")
            .expect("SET ROLE");
        MayPostgresExecutor::new(conn)
    };
    let a = as_role().with_session_context(org_ctx(ORG_A));
    let bexec = as_role().with_session_context(org_ctx(ORG_B));

    let rows: Vec<_> = (1..=100)
        .map(|i| {
            let mut r = batch_rls::BatchRlsRecord::new();
            r.set_id(i).set_org(ORG_A.to_string()).set_qty(1);
            r
        })
        .collect();
    let out = insert_many(&a, &rows, &BatchOptions::default()).expect("insert as A");
    assert_eq!(out.rows_affected, 100);

    // B cannot see A's rows, so batched reads, updates and deletes touch nothing.
    let keys: Vec<Value> = (1..=100).map(|i| Value::BigInt(Some(i))).collect();
    assert!(
        find_by_ids::<batch_rls::Entity>(&bexec, &keys, &BatchOptions::default())
            .expect("find as B")
            .is_empty()
    );
    assert_eq!(
        delete_many::<batch_rls::Entity>(&bexec, &keys, &BatchOptions::default())
            .expect("delete as B")
            .rows_affected,
        0
    );
    // B cannot write rows for A (WITH CHECK).
    let err = insert_many(
        &bexec,
        &[{
            let mut r = batch_rls::BatchRlsRecord::new();
            r.set_id(1_000).set_org(ORG_A.to_string()).set_qty(1);
            r
        }],
        &BatchOptions::default(),
    );
    assert!(err.is_err(), "RLS WITH CHECK must reject B writing A's org");

    let upd: Vec<_> = (1..=100)
        .map(|i| {
            let mut r = batch_rls::BatchRlsRecord::new();
            r.set_id(i).set_qty(2);
            r
        })
        .collect();
    let out = update_many(
        &a,
        &upd,
        &[batch_rls::Column::Qty],
        &BatchOptions::default(),
    )
    .expect("update as A");
    assert_eq!(out.rows_affected, 100);
    assert_eq!(
        find_by_ids::<batch_rls::Entity>(&a, &keys, &BatchOptions::default())
            .expect("find as A")
            .len(),
        100
    );
}

#[test]
fn cached_statements_survive_a_schema_change() {
    let ctx = crate::context::get_test_context();
    let admin = may_postgres::connect(&ctx.pg_url).expect("admin");
    admin
        .batch_execute(
            "DROP TABLE IF EXISTS test_stmt_cache_drift;
             CREATE TABLE test_stmt_cache_drift (id BIGINT PRIMARY KEY, a TEXT);
             INSERT INTO test_stmt_cache_drift VALUES (1, 'x');",
        )
        .expect("fixture");
    let exec = executor();
    let pool = pooled();
    let sql = "SELECT * FROM test_stmt_cache_drift WHERE id = $1";
    assert_eq!(exec.query_all(sql, &[&1_i64]).expect("before").len(), 1);
    assert_eq!(
        pool.query_all_values(sql, &sea_query::Values(vec![Value::BigInt(Some(1))]))
            .expect("pool before")
            .len(),
        1
    );
    // `SELECT *` now returns a different row type: the cached plan is invalid
    // ("cached plan must not change result type"); the cache must re-prepare.
    admin
        .batch_execute("ALTER TABLE test_stmt_cache_drift ADD COLUMN b INTEGER DEFAULT 7")
        .expect("alter");
    let rows = exec.query_all(sql, &[&1_i64]).expect("after: re-prepared");
    assert_eq!(rows[0].len(), 3);
    let rows = pool
        .query_all_values(sql, &sea_query::Values(vec![Value::BigInt(Some(1))]))
        .expect("pool after: re-prepared");
    assert_eq!(rows[0].len(), 3);
}

/// `insert` and `update` read back through `RETURNING`; those statements are writes and must
/// reach the primary even when the pool routes reads to a replica (a replica answers
/// "cannot execute UPDATE in a read-only transaction"). Skips without `TEST_REPLICA_URL`.
#[test]
fn returning_writes_go_to_the_primary_when_reads_use_a_replica() {
    use lifeguard::{ActiveModelTrait, LifeguardPoolSettings};
    let ctx = crate::context::get_test_context();
    let Some(replica) = ctx.replica_pg_url.clone() else {
        return;
    };
    let _ = executor(); // schema
    let settings = LifeguardPoolSettings {
        wal_lag_poll_interval: Duration::from_millis(25),
        ..LifeguardPoolSettings::default()
    };
    let pool = Arc::new(
        LifeguardPool::new_with_settings(&ctx.pg_url, 2, vec![replica], 2, &settings)
            .expect("pool"),
    );
    let exec = PooledLifeExecutor::new(pool);
    // Wait until reads are really served by the replica, so the test cannot pass by falling back.
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        let on_replica = exec
            .query_one_values("SELECT pg_is_in_recovery()", &sea_query::Values(vec![]))
            .map(|r| r.get::<_, bool>(0))
            .unwrap_or(false);
        if on_replica {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "reads never moved to the replica"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    let b = id_base();
    let inserted = rec(b, 1)
        .insert(&exec)
        .expect("insert with RETURNING on the primary");
    assert_eq!(inserted.qty, 1);
    let mut change = BatchItemRecord::new();
    change.set_id(b).set_qty(2);
    let updated = change
        .update(&exec)
        .expect("update with RETURNING on the primary");
    assert_eq!(updated.qty, 2);
}

/// `BatchFailure::Bisect`: seed rows that collide on the primary key at `bad` offsets, insert
/// `n` rows starting at the same base, and check that exactly the bad rows come back.
fn bisect_case(n: i64, bad: &[i64]) -> lifeguard::BatchOutcome<<Entity as LifeModelTrait>::Model> {
    let exec = executor();
    let b = id_base();
    let seed: Vec<_> = bad.iter().map(|o| rec(b + o, 1)).collect();
    insert_many(&exec, &seed, &BatchOptions::default()).expect("seed");
    let rows: Vec<_> = (0..n).map(|o| rec(b + o, 2)).collect();
    let opts = BatchOptions {
        unnest_min_rows: usize::MAX,
        ..BatchOptions::bisect()
    };
    let out = insert_many(&exec, &rows, &opts).expect("bisect never fails on row errors");
    let got: Vec<usize> = out.failed.iter().map(|f| f.index).collect();
    let want: Vec<usize> = bad
        .iter()
        .map(|o| usize::try_from(*o).expect("offset"))
        .collect();
    assert_eq!(got, want, "failed rows, in input order");
    assert!(out
        .failed
        .iter()
        .all(|f| f.sqlstate.as_deref() == Some("23505")));
    assert_eq!(
        out.rows_affected,
        u64::try_from(n).expect("n") - bad.len() as u64
    );
    assert_eq!(
        count(&exec, b, b + n),
        n,
        "every good row written, seeds kept"
    );
    out
}

#[test]
fn bisect_isolates_a_bad_first_row() {
    let out = bisect_case(16, &[0]);
    // 16 → 8+8 → 4+4 → 2+2 → 1+1: one failed statement, then two per level.
    assert_eq!(out.statements, 9);
}

#[test]
fn bisect_isolates_a_bad_middle_row() {
    assert_eq!(bisect_case(16, &[7]).statements, 9);
}

#[test]
fn bisect_isolates_a_bad_last_row() {
    assert_eq!(bisect_case(16, &[15]).statements, 9);
}

#[test]
fn bisect_isolates_several_bad_rows_in_one_chunk() {
    let out = bisect_case(64, &[0, 9, 33, 63]);
    // At most 1 + 2·k·log2(n) statements, far below n = 64 row-by-row.
    assert!(
        out.statements <= 1 + 2 * 4 * 6,
        "statements: {}",
        out.statements
    );
}

#[test]
fn bisect_with_no_bad_rows_costs_one_statement() {
    assert_eq!(bisect_case(16, &[]).statements, 1);
}

#[test]
fn strict_still_fails_the_whole_chunk() {
    let exec = executor();
    let b = id_base();
    insert_many(&exec, &[rec(b + 5, 1)], &BatchOptions::default()).expect("seed");
    let rows: Vec<_> = (0..10).map(|o| rec(b + o, 2)).collect();
    let err = insert_many(&exec, &rows, &BatchOptions::default()).expect_err("strict");
    assert_eq!(err.rows, 0..10);
    assert_eq!(count(&exec, b, b + 10), 1, "nothing of the chunk written");
}

pub mod batch_item_bisect {
    use super::{LifeModel, LifeRecord};

    /// The same table, declared with independent rows.
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_batch_items"]
    #[batch_failure = "bisect"]
    pub struct BatchItemB {
        #[primary_key]
        pub id: i64,
        pub sku: String,
        pub qty: i32,
        pub note: Option<String>,
        #[column_type = "JSONB"]
        pub meta: serde_json::Value,
    }
}

fn rec_b(id: i64, qty: i32) -> batch_item_bisect::BatchItemBRecord {
    let mut r = batch_item_bisect::BatchItemBRecord::new();
    r.set_id(id)
        .set_sku(format!("sku-{id}"))
        .set_qty(qty)
        .set_note(None)
        .set_meta(serde_json::json!({ "id": id }));
    r
}

#[test]
fn an_unconfigured_batch_is_strict() {
    let exec = executor();
    let b = id_base();
    insert_many(&exec, &[rec(b + 2, 1)], &BatchOptions::default()).expect("seed");
    let rows: Vec<_> = (0..8).map(|o| rec(b + o, 2)).collect();
    // No per-call policy and no entity declaration: the chunk fails as a whole.
    let err = insert_many(&exec, &rows, &BatchOptions::default()).expect_err("strict");
    assert_eq!(err.rows, 0..8);
    assert_eq!(count(&exec, b, b + 8), 1);
}

#[test]
fn an_entity_declared_bisect_applies_and_a_per_call_setting_overrides_it() {
    let exec = executor();
    let b = id_base();
    insert_many(&exec, &[rec(b + 3, 1)], &BatchOptions::default()).expect("seed");
    // Entity says bisect: the bad row is isolated with default options.
    let rows: Vec<_> = (0..8).map(|o| rec_b(b + o, 2)).collect();
    let out = insert_many(&exec, &rows, &BatchOptions::default()).expect("entity bisect");
    assert_eq!(out.policy_applied, BatchFailure::bisect());
    assert!(!out.is_complete());
    assert_eq!(
        out.failed.iter().map(|f| f.index).collect::<Vec<_>>(),
        vec![3]
    );
    assert_eq!(count(&exec, b, b + 8), 8);

    // Per call: Strict wins over the entity.
    let b = id_base();
    insert_many(&exec, &[rec(b + 3, 1)], &BatchOptions::default()).expect("seed");
    let rows: Vec<_> = (0..8).map(|o| rec_b(b + o, 2)).collect();
    let strict = BatchOptions {
        failure: Some(BatchFailure::Strict),
        ..BatchOptions::default()
    };
    assert!(insert_many(&exec, &rows, &strict).is_err());
    assert_eq!(count(&exec, b, b + 8), 1);
}

#[test]
fn duplicate_keys_fall_back_to_strict_with_a_reason() {
    let exec = executor();
    let b = id_base();
    let seed: Vec<_> = (0..4).map(|o| rec(b + o, 1)).collect();
    insert_many(&exec, &seed, &BatchOptions::default()).expect("seed");
    // Key b+1 twice: which update wins would depend on how the batch is split.
    let mut upd = vec![rec(b, 5), rec(b + 1, 6), rec(b + 1, 7), rec(b + 2, 8)];
    for r in &mut upd {
        r.set_note(Some("u".into()));
    }
    let out = update_many(&exec, &upd, &[Column::Qty], &BatchOptions::bisect()).expect("update");
    assert_eq!(out.policy_applied, BatchFailure::Strict);
    assert!(out
        .fallback_reason
        .as_deref()
        .is_some_and(|r| r.contains("duplicate keys")));
    // Ordered also forces Strict.
    let ordered = BatchOptions {
        ordered: true,
        ..BatchOptions::bisect()
    };
    let out = delete_many::<Entity>(&exec, &ids(b + 3..b + 4), &ordered).expect("delete");
    assert_eq!(out.policy_applied, BatchFailure::Strict);
    assert!(out.fallback_reason.is_some());
}

#[test]
fn bisect_isolates_a_bad_row_in_update_many() {
    let exec = executor();
    let b = id_base();
    let seed: Vec<_> = (0..8).map(|o| rec(b + o, 1)).collect();
    insert_many(&exec, &seed, &BatchOptions::default()).expect("seed");
    // Row 5 takes row 6's sku while row 6 keeps it: a unique violation (class 23) for row 5 only.
    let mut upd: Vec<_> = (0..8).map(|o| rec(b + o, 9)).collect();
    upd[5].set_sku(format!("sku-{}", b + 6));
    let out = update_many(
        &exec,
        &upd,
        &[Column::Sku, Column::Qty],
        &BatchOptions::bisect(),
    )
    .expect("bisect update");
    assert_eq!(
        out.failed.iter().map(|f| f.index).collect::<Vec<_>>(),
        vec![5]
    );
    assert_eq!(out.failed[0].sqlstate.as_deref(), Some("23505"));
    assert_eq!(out.rows_affected, 7);
}

/// Microbench (ignored): 1000-row ingest through a delayed connection.
#[test]
#[ignore = "microbench: run explicitly"]
fn bench_ingest_row_by_row_vs_batched_vs_bisect() {
    use crate::session_batch::{proxied, report, rtt_us, time_ms};
    let _ = executor(); // schema
    let ctx = crate::context::get_test_context();
    for rtt in [0, rtt_us()] {
        let exec = PooledLifeExecutor::new(Arc::new(
            LifeguardPool::new(&proxied(&ctx.pg_url, rtt), 2, Vec::new(), 0).expect("pool"),
        ));
        let n = 1000_i64;
        // Row by row: one statement (one round trip, one commit) per row.
        let b = id_base();
        let rows: Vec<_> = (0..n).map(|o| rec(b + o, 1)).collect();
        let ms = time_ms(|| {
            for r in &rows {
                insert_many(&exec, std::slice::from_ref(r), &BatchOptions::default()).expect("row");
            }
        });
        report("1000-row ingest, row by row", rtt, vec![ms], "1000");
        // One batch, all good.
        let b = id_base();
        let rows: Vec<_> = (0..n).map(|o| rec(b + o, 1)).collect();
        let mut stmts = 0;
        let ms = time_ms(|| {
            stmts = insert_many(&exec, &rows, &BatchOptions::default())
                .expect("batch")
                .statements;
        });
        report(
            "1000-row ingest, insert_many (1 chunk)",
            rtt,
            vec![ms],
            &stmts.to_string(),
        );
        // Three bad rows (duplicate keys of existing rows) under Strict: the chunk fails whole.
        let b = id_base();
        insert_many(
            &exec,
            &[rec(b + 10, 1), rec(b + 500, 1), rec(b + 990, 1)],
            &BatchOptions::default(),
        )
        .expect("seed");
        let rows: Vec<_> = (0..n).map(|o| rec(b + o, 2)).collect();
        let ms = time_ms(|| {
            assert!(insert_many(&exec, &rows, &BatchOptions::default()).is_err());
        });
        report(
            "1000-row ingest, 3 bad rows, Strict (nothing written)",
            rtt,
            vec![ms],
            "1",
        );
        // The same under Bisect: 997 written, 3 reported.
        let b = id_base();
        insert_many(
            &exec,
            &[rec(b + 10, 1), rec(b + 500, 1), rec(b + 990, 1)],
            &BatchOptions::default(),
        )
        .expect("seed");
        let rows: Vec<_> = (0..n).map(|o| rec(b + o, 2)).collect();
        let mut out = None;
        let ms = time_ms(|| {
            out = Some(insert_many(&exec, &rows, &BatchOptions::bisect()).expect("bisect"));
        });
        let out = out.expect("outcome");
        assert_eq!(out.failed.len(), 3);
        assert_eq!(out.rows_affected, 997);
        report(
            "1000-row ingest, 3 bad rows, Bisect (997 written)",
            rtt,
            vec![ms],
            &out.statements.to_string(),
        );
    }
}
