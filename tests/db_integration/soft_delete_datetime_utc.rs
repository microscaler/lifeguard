//! Soft delete with `deleted_at: Option<DateTime<Utc>>` — generated UPDATE must use `ChronoDateTimeUtc`.

use chrono::{DateTime, Utc};
use lifeguard::query::column::column_trait::ColumnTrait;
use lifeguard::test_helpers::TestDatabase;
use lifeguard::{ActiveModelTrait, LifeExecutor, LifeModelTrait};
use lifeguard_derive::{LifeModel, LifeRecord};

fn get_db() -> TestDatabase {
    let ctx = crate::context::get_test_context();
    TestDatabase::with_url(&ctx.pg_url)
}

pub mod soft_delete_utc {
    use super::*;

    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_soft_delete_users_utc"]
    #[soft_delete]
    pub struct TestSoftDeleteUserUtc {
        #[primary_key]
        #[auto_increment]
        pub id: i32,
        pub name: String,
        pub deleted_at: Option<DateTime<Utc>>,
    }
}

fn setup_schema(executor: &dyn LifeExecutor) -> Result<(), lifeguard::executor::LifeError> {
    executor.execute(
        r"
        CREATE TABLE IF NOT EXISTS test_soft_delete_users_utc (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            deleted_at TIMESTAMPTZ
        )
        ",
        &[],
    )?;
    Ok(())
}

fn cleanup(executor: &dyn LifeExecutor) -> Result<(), lifeguard::executor::LifeError> {
    executor.execute("DELETE FROM test_soft_delete_users_utc", &[])?;
    Ok(())
}

#[test]
fn soft_delete_sets_timestamptz_via_chrono_datetime_utc() {
    let mut test_db = get_db();
    let executor = test_db.executor().expect("executor");
    setup_schema(&executor).expect("setup");
    cleanup(&executor).expect("cleanup");

    let mut insert_record = soft_delete_utc::TestSoftDeleteUserUtcRecord::new();
    insert_record.set_name("Utc Soft Delete".to_string());
    let model = insert_record.insert(&executor).expect("insert");

    let delete_record = soft_delete_utc::TestSoftDeleteUserUtcRecord::from_model(&model);
    delete_record.delete(&executor).expect("soft delete");

    let rows = executor
        .query_all(
            "SELECT deleted_at FROM test_soft_delete_users_utc WHERE id = $1",
            &[&model.id],
        )
        .expect("select deleted_at");
    assert_eq!(rows.len(), 1);
    let dt: Option<DateTime<Utc>> = rows[0].get(0);
    assert!(dt.is_some(), "deleted_at should be set");

    let found = soft_delete_utc::Entity::find()
        .filter(soft_delete_utc::Column::Id.eq(model.id))
        .find_one(&executor)
        .expect("find");
    assert!(found.is_none(), "soft-deleted row hidden from find()");
}
