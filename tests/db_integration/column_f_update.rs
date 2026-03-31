//! Postgres integration: `ColumnTrait::f_add` on `UPDATE SET` (PRD §8, F-1/F-2).

use std::sync::Mutex;

use crate::context::get_test_context;
use lifeguard::executor::LifeError;
use lifeguard::test_helpers::TestDatabase;
use lifeguard::{
    ActiveModelError, ActiveModelTrait, ColumnTrait, LifeExecutor, LifeModelTrait,
};
use lifeguard_derive::{LifeModel, LifeRecord};
use sea_query::{PostgresQueryBuilder, Query};

static LOCK: Mutex<()> = Mutex::new(());

#[derive(LifeModel, LifeRecord, Debug, Clone)]
#[table_name = "lg_f_update_counter"]
pub struct Counter {
    #[primary_key]
    #[auto_increment]
    pub id: i32,
    pub n: i32,
}

fn setup(executor: &dyn lifeguard::LifeExecutor) -> Result<(), LifeError> {
    executor.execute("DROP TABLE IF EXISTS lg_f_update_counter CASCADE", &[])?;
    executor.execute(
        "CREATE TABLE lg_f_update_counter (id SERIAL PRIMARY KEY, n INTEGER NOT NULL DEFAULT 0)",
        &[],
    )?;
    executor.execute("INSERT INTO lg_f_update_counter (n) VALUES (0)", &[])?;
    Ok(())
}

#[test]
fn f_add_update_increments_column_on_postgres() {
    let _guard = LOCK.lock().expect("column_f_update lock");

    let ctx = get_test_context();
    let mut db = TestDatabase::with_url(&ctx.pg_url);
    let executor = db.executor().expect("executor");

    setup(&executor).expect("setup");

    let mut q = Query::update();
    q.table(Entity);
    q.value(
        <Entity as LifeModelTrait>::Column::N,
        <Entity as LifeModelTrait>::Column::N.f_add(1),
    );
    q.and_where(<Entity as LifeModelTrait>::Column::Id.eq(1i32));

    let (sql, values) = q.build(PostgresQueryBuilder);
    let n = executor
        .execute_values(&sql, &values)
        .expect("execute update");

    assert_eq!(n, 1, "one row updated");

    let row = executor
        .query_one("SELECT n FROM lg_f_update_counter WHERE id = 1", &[])
        .expect("select");
    let n_after: i32 = row.get(0);
    assert_eq!(n_after, 1, "column incremented in database");
}

/// `LifeRecord::set_*_expr` + `ActiveModelTrait::update` (F-1 on ORM path).
#[test]
fn record_set_n_expr_update_increments_on_postgres() {
    let _guard = LOCK.lock().expect("column_f_update lock");

    let ctx = get_test_context();
    let mut db = TestDatabase::with_url(&ctx.pg_url);
    let executor = db.executor().expect("executor");

    setup(&executor).expect("setup");

    let row = executor
        .query_one("SELECT id, n FROM lg_f_update_counter WHERE id = 1", &[])
        .expect("select seed row");
    let id: i32 = row.get(0);
    let n0: i32 = row.get(1);
    assert_eq!((id, n0), (1, 0));

    let model = CounterModel { id: 1, n: 0 };
    let mut rec = CounterRecord::from_model(&model);
    rec.set_n_expr(<Entity as LifeModelTrait>::Column::N.f_add(1i32));

    let updated = rec
        .update(&executor)
        .expect("record update with f_add expr");

    assert_eq!(updated.n, 1);

    let row = executor
        .query_one("SELECT n FROM lg_f_update_counter WHERE id = 1", &[])
        .expect("select after");
    let n_after: i32 = row.get(0);
    assert_eq!(n_after, 1);
}

#[test]
fn insert_rejects_when_set_expr_pending() {
    let _guard = LOCK.lock().expect("column_f_update lock");

    let ctx = get_test_context();
    let mut db = TestDatabase::with_url(&ctx.pg_url);
    let executor = db.executor().expect("executor");

    setup(&executor).expect("setup");

    let mut rec = CounterRecord::new();
    rec.set_n_expr(<Entity as LifeModelTrait>::Column::N.f_add(1i32));

    let err = rec.insert(&executor).expect_err("insert must reject pending __update_exprs");
    match err {
        ActiveModelError::Other(msg) => {
            assert!(
                msg.contains("set_*_expr") || msg.contains("__update_exprs"),
                "unexpected message: {msg}"
            );
        }
        e => panic!("expected Other, got {e:?}"),
    }
}
