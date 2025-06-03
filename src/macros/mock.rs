/// A macro to test SeaORM logic inside a may coroutine with a mock database connection.
/// Supports optional setup/teardown logic.
#[macro_export]
macro_rules! with_mock_connection {
    (
        mock = $mock:expr,
        setup = $setup:block,
        test = $test:block,
        teardown = $teardown:block
    ) => {{
        may::go!(move || {
            let db = $mock.into_connection();

            (|| async $setup)().await;
            let result = (|| async $test)().await;
            (|| async $teardown)().await;

            result
        })
    }};

    (
        mock = $mock:expr,
        args = ($($arg:ident),*),
        test = $test:block
    ) => {{
        may::go!(move || {
            let db = $mock.into_connection();
            $(let $arg = $arg.clone();)*
            (|| async $test)().await
        })
    }};

    (
        mock = $mock:expr,
        test = $test:block
    ) => {{
        may::go!(move || {
            let db = $mock.into_connection();
            (|| async $test)().await
        })
    }};
}

/// Capture stdout logs during execution of a test block.
#[macro_export]
macro_rules! capture_logs {
    ($block:block) => {{
        use std::io::Write;
        let mut buf = Vec::new();
        let _ = std::io::set_output_capture(Some(Box::new(&mut buf)));
        let result = { $block };
        std::io::set_output_capture(None);
        (result, String::from_utf8_lossy(&buf).to_string())
    }};
}

#[cfg(test)]
mod tests {
    use sea_orm::{entity::*, query::*, DatabaseBackend, DbErr, MockDatabase, Statement, TryGetable};
    use fake::{Dummy, Fake, Faker};

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Dummy)]
    #[sea_orm(table_name = "cakes")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub name: String,
    }

    fn fake_cake() -> Model {
        Faker.fake()
    }

    #[test]
    fn test_mock_connection_with_lifecycle() {
        use crate::with_mock_connection;

        let mock = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results(vec![vec![Model { id: 1, name: "Chocolate".into() }]]);

        with_mock_connection! {
            mock = mock,
            setup = {
                println!("🛠 setup logic");
            },
            test = {
                let row = db.query_one(Statement::from_string(
                    DatabaseBackend::Postgres,
                    "SELECT 1 as id, 'Chocolate' as name".into(),
                )).await.unwrap().unwrap();

                let name: String = row.try_get("", "name").unwrap();
                assert_eq!(name, "Chocolate");
                Ok::<_, DbErr>(())
            },
            teardown = {
                println!("🧹 teardown logic");
            }
        };
    }

    #[test]
    fn test_mock_connection_with_args_and_fake_data() {
        use crate::with_mock_connection;

        let cake = fake_cake();
        let mock = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results(vec![vec![cake.clone()]]);

        with_mock_connection! {
            mock = mock,
            args = (cake),
            test = {
                let row = db.query_one(Statement::from_string(
                    DatabaseBackend::Postgres,
                    format!("SELECT {} as id, '{}' as name", cake.id, cake.name),
                )).await.unwrap().unwrap();

                let name: String = row.try_get("", "name").unwrap();
                assert_eq!(name, cake.name);
                Ok::<_, DbErr>(())
            }
        };
    }

    #[test]
    fn test_mock_connection_logs_capture() {
        use crate::{with_mock_connection, capture_logs};

        let mock = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results(vec![vec![Model { id: 3, name: "Lemon".into() }]]);

        let (_result, logs) = capture_logs!({
            with_mock_connection! {
                mock = mock,
                test = {
                    println!("🍋 testing with Lemon");
                    let row = db.query_one(Statement::from_string(
                        DatabaseBackend::Postgres,
                        "SELECT 3 as id, 'Lemon' as name".into(),
                    )).await.unwrap().unwrap();

                    let name: String = row.try_get("", "name").unwrap();
                    assert_eq!(name, "Lemon");
                    Ok::<_, DbErr>(())
                }
            };
        });

        assert!(logs.contains("🍋 testing with Lemon"));
    }

    #[test]
    fn test_mock_connection_error_case() {
        use crate::with_mock_connection;

        let mock = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_errors(vec![DbErr::Custom("Simulated failure".into())]);

        with_mock_connection! {
            mock = mock,
            test = {
                let err = db.query_one(Statement::from_string(
                    DatabaseBackend::Postgres,
                    "SELECT fail".into(),
                )).await;

                assert!(matches!(err, Err(DbErr::Custom(e)) if e.contains("Simulated failure")));
                Ok::<_, DbErr>(())
            }
        };
    }
}
