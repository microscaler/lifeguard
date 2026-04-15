//! Postgres integration: `ColumnTrait::f_*` in `WHERE` and `ORDER BY` via `Expr::expr` (PRD §8, F-1/F-2).

use crate::context::get_test_context;
use lifeguard::executor::LifeError;
use lifeguard::test_helpers::TestDatabase;
use lifeguard::{ColumnTrait, LifeExecutor, LifeModelTrait};
use lifeguard_derive::{LifeModel, LifeRecord};
use sea_query::{Expr, ExprTrait, Order, PostgresQueryBuilder, Query};

#[derive(LifeModel, LifeRecord, Debug, Clone)]
#[table_name = "lg_f_where_counter"]
pub struct Counter {
    #[primary_key]
    #[auto_increment]
    pub id: i32,
    pub n: i32,
}

fn setup(executor: &dyn lifeguard::LifeExecutor) -> Result<(), LifeError> {
    executor.execute("DROP TABLE IF EXISTS lg_f_where_counter CASCADE", &[])?;
    executor.execute(
        "CREATE TABLE lg_f_where_counter (id SERIAL PRIMARY KEY, n INTEGER NOT NULL)",
        &[],
    )?;
    executor.execute("INSERT INTO lg_f_where_counter (n) VALUES (4), (6)", &[])?;
    Ok(())
}

#[test]
fn f_add_in_where_and_order_by_on_postgres() {
    let ctx = get_test_context();
    let mut db = TestDatabase::with_url(&ctx.pg_url);
    let executor = db.executor().expect("executor");

    setup(&executor).expect("setup");

    // Rows: id=1 n=4, id=2 n=6. Condition (n + 1) > 5 → only id=2 (7 > 5).
    let mut q_where = Query::select();
    q_where
        .column(<Entity as LifeModelTrait>::Column::Id)
        .from(Entity)
        .and_where(Expr::expr(<Entity as LifeModelTrait>::Column::N.f_add(1i32)).gt(5i32));

    let (sql, values) = q_where.build(PostgresQueryBuilder);
    let row = executor
        .query_one_values(&sql, &values)
        .expect("where query");
    let id: i32 = row.get(0);
    assert_eq!(id, 2);

    // Order by (n + 1) DESC: (4+1)=5, (6+1)=7 → first row id=2.
    let mut q_order = Query::select();
    q_order
        .column(<Entity as LifeModelTrait>::Column::Id)
        .from(Entity)
        .order_by_expr(
            Expr::expr(<Entity as LifeModelTrait>::Column::N.f_add(1i32)),
            Order::Desc,
        )
        .limit(1);

    let (sql, values) = q_order.build(PostgresQueryBuilder);
    let row = executor
        .query_one_values(&sql, &values)
        .expect("order by query");
    let id: i32 = row.get(0);
    assert_eq!(id, 2);
}
