//! Postgres integration: `ColumnTrait::f_add` on `UPDATE SET` (PRD §8, F-1/F-2).

use crate::context::get_test_context;
use lifeguard::executor::LifeError;
use lifeguard::test_helpers::TestDatabase;
use lifeguard::{ColumnTrait, LifeExecutor, LifeModelTrait};
use lifeguard_derive::{LifeModel, LifeRecord};
use sea_query::{PostgresQueryBuilder, Query};

#[derive(LifeModel, LifeRecord, Debug, Clone)]
#[table_name = "lg_f_update_counter"]
pub struct Counter {
    #[primary_key]
    #[auto_increment]
    pub id: i32,
    pub n: i32,
}

fn setup(executor: &dyn lifeguard::LifeExecutor) -> Result<(), LifeError> {
    executor.execute(
        "CREATE TABLE IF NOT EXISTS lg_f_update_counter (id SERIAL PRIMARY KEY, n INTEGER NOT NULL DEFAULT 0)",
        &[],
    )?;
    executor.execute("DELETE FROM lg_f_update_counter", &[])?;
    executor.execute("INSERT INTO lg_f_update_counter (n) VALUES (0)", &[])?;
    Ok(())
}

#[test]
fn f_add_update_increments_column_on_postgres() {
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

    let _ = executor.execute("DROP TABLE IF EXISTS lg_f_update_counter", &[]);
}
