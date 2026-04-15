//! PRD §9: `ModelIdentityMap` / `Session` + `flush_dirty` / `flush_dirty_in_transaction` with derived `LifeRecord::update`.

use std::sync::Arc;
use std::sync::Mutex;

use crate::context::get_test_context;
use lifeguard::executor::LifeError;
use lifeguard::session::{ModelIdentityMap, Session};
use lifeguard::test_helpers::TestDatabase;
use lifeguard::{
    ActiveModelTrait, LifeExecutor, LifeModelTrait, LifeguardPool, PooledLifeExecutor,
};
use lifeguard_derive::{LifeModel, LifeRecord};

static LOCK: Mutex<()> = Mutex::new(());

#[derive(LifeModel, LifeRecord, Clone, Debug)]
#[table_name = "lg_sess_flush_counter"]
pub struct Counter {
    #[primary_key]
    pub id: i32,
    pub n: i32,
}

fn setup(executor: &dyn LifeExecutor) -> Result<(), LifeError> {
    executor.execute("DROP TABLE IF EXISTS lg_sess_flush_counter CASCADE", &[])?;
    executor.execute(
        "CREATE TABLE lg_sess_flush_counter (id INTEGER PRIMARY KEY, n INTEGER NOT NULL)",
        &[],
    )?;
    executor.execute(
        "INSERT INTO lg_sess_flush_counter (id, n) VALUES (1, 0)",
        &[],
    )?;
    Ok(())
}

#[test]
fn identity_map_flush_dirty_persists_via_update() {
    let _guard = LOCK.lock().expect("session_identity_flush lock");

    let ctx = get_test_context();
    let mut db = TestDatabase::with_url(&ctx.pg_url);
    let executor = db.executor().expect("executor");

    setup(&executor).expect("setup");

    let mut map = ModelIdentityMap::<Entity>::new();
    let rc = map.register_loaded(CounterModel { id: 1, n: 0 });
    rc.borrow_mut().n = 8;

    map.mark_dirty(&CounterModel { id: 1, n: 0 });

    map.flush_dirty(&executor, |ex, mrc| {
        let model = mrc.borrow().clone();
        let rec = CounterRecord::from_model(&model);
        let _ = rec.update(ex)?;
        Ok(())
    })
    .expect("flush_dirty");

    let row = executor
        .query_one("SELECT n FROM lg_sess_flush_counter WHERE id = 1", &[])
        .expect("select");
    let n: i32 = row.get(0);
    assert_eq!(n, 8);
}

#[test]
fn identity_map_flush_dirty_with_mark_dirty_key_and_identity_map_key() {
    let _guard = LOCK.lock().expect("session_identity_flush lock");

    let ctx = get_test_context();
    let mut db = TestDatabase::with_url(&ctx.pg_url);
    let executor = db.executor().expect("executor");

    setup(&executor).expect("setup");

    let mut map = ModelIdentityMap::<Entity>::new();
    let rc = map.register_loaded(CounterModel { id: 1, n: 0 });
    rc.borrow_mut().n = 15;

    let key = CounterRecord::from_model(&rc.borrow())
        .identity_map_key()
        .expect("identity_map_key");
    map.mark_dirty_key(&key);

    map.flush_dirty(&executor, |ex, mrc| {
        let model = mrc.borrow().clone();
        let rec = CounterRecord::from_model(&model);
        let _ = rec.update(ex)?;
        Ok(())
    })
    .expect("flush_dirty");

    let row = executor
        .query_one("SELECT n FROM lg_sess_flush_counter WHERE id = 1", &[])
        .expect("select");
    let n: i32 = row.get(0);
    assert_eq!(n, 15);
}

#[test]
fn session_flush_dirty_after_attach_session_and_set_n_on_record() {
    let _guard = LOCK.lock().expect("session_identity_flush lock");

    let ctx = get_test_context();
    let mut db = TestDatabase::with_url(&ctx.pg_url);
    let executor = db.executor().expect("executor");

    setup(&executor).expect("setup");

    let session = Session::<Entity>::new();
    let rc = session.register_loaded(CounterModel { id: 1, n: 0 });

    let mut rec = CounterRecord::from_model(&rc.borrow());
    rec.attach_session_with_model(&session, &rc);
    rec.set_n(42);

    session
        .flush_dirty(&executor, |ex, mrc| {
            let model = mrc.borrow().clone();
            let r = CounterRecord::from_model(&model);
            let _ = r.update(ex)?;
            Ok(())
        })
        .expect("flush_dirty");

    let row = executor
        .query_one("SELECT n FROM lg_sess_flush_counter WHERE id = 1", &[])
        .expect("select");
    let n: i32 = row.get(0);
    assert_eq!(n, 42);
}

#[test]
fn session_flush_dirty_in_transaction_persists_via_update() {
    let _guard = LOCK.lock().expect("session_identity_flush lock");

    let ctx = get_test_context();
    let mut db = TestDatabase::with_url(&ctx.pg_url);
    let executor = db.executor().expect("executor");

    setup(&executor).expect("setup");

    let session = Session::<Entity>::new();
    let rc = session.register_loaded(CounterModel { id: 1, n: 0 });
    rc.borrow_mut().n = 99;
    session.mark_dirty(&CounterModel { id: 1, n: 0 });

    session
        .flush_dirty_in_transaction(&executor, |ex, mrc| {
            let model = mrc.borrow().clone();
            let rec = CounterRecord::from_model(&model);
            let _ = rec.update(ex)?;
            Ok(())
        })
        .expect("flush_dirty_in_transaction");

    let row = executor
        .query_one("SELECT n FROM lg_sess_flush_counter WHERE id = 1", &[])
        .expect("select");
    let n: i32 = row.get(0);
    assert_eq!(n, 99);
}

#[test]
fn session_flush_dirty_in_transaction_pooled_persists_via_update() {
    let _guard = LOCK.lock().expect("session_identity_flush lock");

    let ctx = get_test_context();
    let pool = Arc::new(
        LifeguardPool::new(&ctx.pg_url, 1, vec![], 0).expect("LifeguardPool primary-only"),
    );
    let setup_ex = PooledLifeExecutor::new(pool.clone());
    setup(&setup_ex).expect("setup");

    let session = Session::<Entity>::new();
    let rc = session.register_loaded(CounterModel { id: 1, n: 0 });
    rc.borrow_mut().n = 77;
    session.mark_dirty(&CounterModel { id: 1, n: 0 });

    session
        .flush_dirty_in_transaction_pooled(&pool, |ex, mrc| {
            let model = mrc.borrow().clone();
            let rec = CounterRecord::from_model(&model);
            let _ = rec.update(ex)?;
            Ok(())
        })
        .expect("flush_dirty_in_transaction_pooled");

    let verify = PooledLifeExecutor::new(pool);
    let row = verify
        .query_one("SELECT n FROM lg_sess_flush_counter WHERE id = 1", &[])
        .expect("select");
    let n: i32 = row.get(0);
    assert_eq!(n, 77);
}

/// Pending-insert keys (`register_pending_insert`) + `flush_dirty_with_map_key` + `promote_pending_to_loaded`
/// (PRD Phase E insert-only flush path).
#[test]
fn identity_map_pending_insert_flush_and_promote_persists_on_postgres() {
    let _guard = LOCK.lock().expect("session_identity_flush lock");

    let ctx = get_test_context();
    let mut db = TestDatabase::with_url(&ctx.pg_url);
    let executor = db.executor().expect("executor");

    setup(&executor).expect("setup");

    let mut map = ModelIdentityMap::<Entity>::new();
    let (pending_key, rc) = map.register_pending_insert(CounterModel { id: 0, n: 33 });

    map.flush_dirty_with_map_key(&executor, |ex, mrc, key| {
        assert!(lifeguard::is_pending_insert_key(key));
        let mut m = mrc.borrow().clone();
        m.id = 2;
        *mrc.borrow_mut() = m.clone();
        let rec = CounterRecord::from_model(&m);
        let inserted = rec.insert(ex)?;
        *mrc.borrow_mut() = inserted;
        Ok(())
    })
    .expect("flush_dirty_with_map_key");

    map.promote_pending_to_loaded(&pending_key, rc.borrow().clone())
        .expect("promote_pending_to_loaded");

    let row = executor
        .query_one("SELECT n FROM lg_sess_flush_counter WHERE id = 2", &[])
        .expect("select");
    let n: i32 = row.get(0);
    assert_eq!(n, 33);

    assert_eq!(map.dirty_len(), 0);
    assert!(map.get_existing(&CounterModel { id: 2, n: 0 }).is_some());
}

#[test]
fn session_pending_insert_flush_in_transaction_with_map_key_persists_on_postgres() {
    let _guard = LOCK.lock().expect("session_identity_flush lock");

    let ctx = get_test_context();
    let mut db = TestDatabase::with_url(&ctx.pg_url);
    let executor = db.executor().expect("executor");

    setup(&executor).expect("setup");

    let session = Session::<Entity>::new();
    let (pending_key, rc) = session.register_pending_insert(CounterModel { id: 0, n: 44 });

    session
        .flush_dirty_in_transaction_with_map_key(&executor, |ex, mrc, key| {
            assert!(lifeguard::is_pending_insert_key(key));
            let mut m = mrc.borrow().clone();
            m.id = 2;
            *mrc.borrow_mut() = m.clone();
            let rec = CounterRecord::from_model(&m);
            let inserted = rec.insert(ex)?;
            *mrc.borrow_mut() = inserted;
            Ok(())
        })
        .expect("flush_dirty_in_transaction_with_map_key");

    session
        .promote_pending_to_loaded(&pending_key, rc.borrow().clone())
        .expect("promote_pending_to_loaded");

    let row = executor
        .query_one("SELECT n FROM lg_sess_flush_counter WHERE id = 2", &[])
        .expect("select");
    let n: i32 = row.get(0);
    assert_eq!(n, 44);
}

#[test]
fn session_pending_insert_flush_in_transaction_pooled_with_map_key_persists_on_postgres() {
    let _guard = LOCK.lock().expect("session_identity_flush lock");

    let ctx = get_test_context();
    let pool = Arc::new(
        LifeguardPool::new(&ctx.pg_url, 1, vec![], 0).expect("LifeguardPool primary-only"),
    );
    let setup_ex = PooledLifeExecutor::new(pool.clone());
    setup(&setup_ex).expect("setup");

    let session = Session::<Entity>::new();
    let (pending_key, rc) = session.register_pending_insert(CounterModel { id: 0, n: 55 });

    session
        .flush_dirty_in_transaction_pooled_with_map_key(&pool, |ex, mrc, key| {
            assert!(lifeguard::is_pending_insert_key(key));
            let mut m = mrc.borrow().clone();
            m.id = 2;
            *mrc.borrow_mut() = m.clone();
            let rec = CounterRecord::from_model(&m);
            let inserted = rec.insert(ex)?;
            *mrc.borrow_mut() = inserted;
            Ok(())
        })
        .expect("flush_dirty_in_transaction_pooled_with_map_key");

    session
        .promote_pending_to_loaded(&pending_key, rc.borrow().clone())
        .expect("promote_pending_to_loaded");

    let verify = PooledLifeExecutor::new(pool);
    let row = verify
        .query_one("SELECT n FROM lg_sess_flush_counter WHERE id = 2", &[])
        .expect("select");
    let n: i32 = row.get(0);
    assert_eq!(n, 55);
}
