//! PRD §9: `ModelIdentityMap` + `flush_dirty` persists changes via derived `LifeRecord::update`.

use std::sync::Mutex;

use crate::context::get_test_context;
use lifeguard::executor::LifeError;
use lifeguard::session::{ModelIdentityMap, Session};
use lifeguard::test_helpers::TestDatabase;
use lifeguard::{ActiveModelTrait, LifeExecutor, LifeModelTrait};
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

    let mut rec = CounterRecord::from_model(&*rc.borrow());
    rec.attach_session(&session);
    rec.set_n(42);
    *rc.borrow_mut() = rec.to_model().expect("to_model");

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
