//! Integration tests for `lifeguard::batch`: multi-row writes, UNNEST, chunking, RETURNING,
//! per-chunk errors, batched reads, RLS context and the buffered `BatchWriter`.

use lifeguard::query::column::column_trait::ColumnTrait;
use lifeguard::{
    delete_many, find_by_ids, insert_many, test_helpers::TestDatabase, update_many, upsert_many,
    BatchOptions, BatchWriter, BatchWriterConfig, LifeExecutor, LifeModelTrait, LifeguardPool,
    MayPostgresExecutor, OnConflict, PooledLifeExecutor, SessionContext, WriteMode,
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
    PooledLifeExecutor::new(Arc::new(LifeguardPool::new(&ctx.pg_url, 2, Vec::new(), 0).expect("pool")))
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
        .set_note(if id % 2 == 0 { Some(format!("n{id}")) } else { None })
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
    let opts = BatchOptions { max_rows: 50, ..BatchOptions::default() };
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
    let all = find_by_ids::<Entity>(&exec, &ids(b..b + 30), &BatchOptions::default()).expect("find");
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
    assert!(out.models.iter().all(|m| m.sku == format!("sku-{}", m.id)), "other columns untouched");

    let out = delete_many::<Entity>(&exec, &ids(b..b + 50), &BatchOptions::default())
        .expect("delete_many");
    assert_eq!(out.rows_affected, 50);
    let left = find_by_ids::<Entity>(&exec, &ids(b..b + 100), &BatchOptions::default())
        .expect("find");
    assert_eq!(left.len(), 50);
    assert!(left.iter().all(|m| m.id >= b + 50));
}

#[test]
fn find_by_ids_chunks_and_ignores_missing() {
    let exec = executor();
    let b = id_base();
    let seed: Vec<_> = (b..b + 30).map(|i| rec(i, 1)).collect();
    insert_many(&exec, &seed, &BatchOptions::default()).expect("seed");
    let opts = BatchOptions { max_rows: 7, ..BatchOptions::default() };
    let found = find_by_ids::<Entity>(&exec, &ids(b - 5..b + 40), &opts).expect("find");
    assert_eq!(found.len(), 30);
    assert!(find_by_ids::<Entity>(&exec, &[], &opts).expect("empty").is_empty());
}

#[test]
fn error_reports_the_failing_chunk() {
    let exec = executor();
    let b = id_base();
    insert_many(&exec, &[rec(b + 25, 1)], &BatchOptions::default()).expect("seed duplicate");
    let rows: Vec<_> = (b..b + 30).filter(|i| *i != b + 25).chain([b + 25]).map(|i| rec(i, 1)).collect();
    let opts = BatchOptions { max_rows: 10, ..BatchOptions::default() };
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
        writer.push(rec(b + i, 1)).map_err(|_| "closed").expect("push");
    }
    // Same key twice in one batch: the later row wins.
    writer.push(rec(b + 1_000, 1)).map_err(|_| "closed").expect("push");
    writer.push(rec(b + 1_000, 5)).map_err(|_| "closed").expect("push");
    // Time trigger: wait past max_delay without flushing explicitly.
    may::coroutine::sleep(Duration::from_millis(300));
    assert_eq!(count(&exec, b, b + 1_001), 251, "time-based flush");
    let one = find_by_ids::<Entity>(&exec, &ids(b + 1_000..b + 1_001), &BatchOptions::default())
        .expect("find");
    assert_eq!(one[0].qty, 5);

    // Close flushes the tail.
    for i in 2_000..2_010 {
        writer.push(rec(b + i, 1)).map_err(|_| "closed").expect("push");
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
               USING (org = NULLIF(current_setting('sesame.organization_id', true), ''))
               WITH CHECK (org = NULLIF(current_setting('sesame.organization_id', true), ''));
             GRANT SELECT, INSERT, UPDATE, DELETE ON public.test_batch_rls TO rls_test_role;
             SELECT pg_advisory_unlock(7211915001);",
        )
        .expect("rls fixture");
    // Run as the non-superuser role (superusers bypass RLS) on the suite's own URL, whatever its
    // credentials: the role and `rls_set_session` come from `rls_integration`'s ctor.
    let as_role = || {
        let conn = may_postgres::connect(&ctx.pg_url).expect("connect");
        conn.batch_execute("SET ROLE rls_test_role").expect("SET ROLE");
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
    assert!(find_by_ids::<batch_rls::Entity>(&bexec, &keys, &BatchOptions::default())
        .expect("find as B")
        .is_empty());
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
    let out = update_many(&a, &upd, &[batch_rls::Column::Qty], &BatchOptions::default())
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
