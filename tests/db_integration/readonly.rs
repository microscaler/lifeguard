use lifeguard::executor::MayPostgresExecutor;
use lifeguard::test_helpers::TestDatabase;
use lifeguard::{ActiveModelTrait, LifeExecutor, LifeModelTrait};
use lifeguard_derive::{LifeModel, LifeRecord};

fn get_db() -> TestDatabase {
    let ctx = crate::context::get_test_context();
    TestDatabase::with_url(&ctx.pg_url)
}

#[derive(LifeModel, LifeRecord, Debug)]
#[table_name = "test_readonly_models"]
pub struct TestReadonlyModel {
    #[primary_key]
    #[auto_increment]
    pub id: i32,
    pub name: String,
    #[generated_always_as = "upper(name)"]
    pub generated_ref: String,
}

fn setup_schema(executor: &MayPostgresExecutor) -> Result<(), lifeguard::executor::LifeError> {
    executor.execute("DROP TABLE IF EXISTS test_readonly_models", &[])?;
    executor.execute(
        r"
        CREATE TABLE IF NOT EXISTS test_readonly_models (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            generated_ref TEXT GENERATED ALWAYS AS (upper(name)) STORED
        )
        ",
        &[],
    )?;
    Ok(())
}

#[test]
fn test_readonly_insert_and_update() {
    let mut db = get_db();
    let executor = db.executor().unwrap();

    setup_schema(&executor).unwrap();
    executor
        .execute(
            "TRUNCATE test_readonly_models RESTART IDENTITY CASCADE",
            &[],
        )
        .unwrap();

    // 1. Test Insert
    let mut model = TestReadonlyModelRecord::new();
    model.set_name("hello".to_string());

    let inserted = model
        .insert(&executor)
        .expect("Should insert correctly skipping generated_ref");

    // Check that generated_ref was fetched via RETURNING
    assert_eq!(inserted.name, "hello");
    assert_eq!(inserted.generated_ref, "HELLO");

    // 2. Test Update
    let mut update_record = TestReadonlyModelRecord::from_model(&inserted);
    update_record.set_name("world".to_string());

    // We shouldn't need to manually reset or touch generated_ref, it should be ignored in SET clauses
    let updated = update_record
        .update(&executor)
        .expect("Should update correctly skipping generated_ref");

    assert_eq!(updated.name, "world");
    // After update, we do a find_one(), so we get the fresh generated value
    assert_eq!(updated.generated_ref, "WORLD");
}
