use crate::metrics::METRICS;
use crate::pool::config::DatabaseConfig;
use crate::pool::types::{DbRequest, DbTask, LifeguardJob, QueryCallback};
use crate::pool::worker::run_worker_loop;
use sea_orm::ConnectionTrait;

use async_trait::async_trait;
use crossbeam_channel::{unbounded, Sender};
use sea_orm::{
    ConnectOptions, Database, DatabaseBackend, DatabaseConnection, DbErr, ExecResult, QueryResult,
    Statement,
};
use std::any::Any;
use std::thread;
use std::time::Instant;
use tokio::sync::oneshot;
use tracing::instrument;

// Internal enum representing database tasks for worker threads
// type AnyError = Box<dyn Error + Send + Sync>;

// channel to send tasks to the DB worker(s)
// ... (e.g. thread handles, config, etc.)
#[derive(Clone, Debug)]
pub struct DbPoolManager {
    pub(crate) request_tx: Sender<LifeguardJob>,
}

impl DbPoolManager {
    /// Public constructor: from config
    pub fn from_config(config: &DatabaseConfig) -> Result<Self, DbErr> {
        Self::new_with_params(&config.url, config.max_connections as u32)
    }

    /// Shorthand using default config (non-verbose mode)
    pub fn new() -> Result<Self, DbErr> {
        Self::new_with_verbose()
    }

    /// Shorthand using default config with configurable verbosity
    pub fn new_with_verbose() -> Result<Self, DbErr> {
        let config = DatabaseConfig::load()
            .map_err(|e| DbErr::Custom(format!("Failed to load database config: {}", e)))?;
        Self::from_config(&config)
    }

    /// Internal constructor that wires up the Lifeguard job channel and thread
    pub fn new_with_params(database_url: &str, max_connections: u32) -> Result<Self, DbErr> {
        let (tx, rx) = unbounded::<LifeguardJob>();
        let db_url = database_url.to_string();

        thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
            rt.block_on(async move {
                let mut options = ConnectOptions::new(db_url.clone());
                options.max_connections(max_connections);
                let db = Database::connect(options)
                    .await
                    .expect("Failed to connect to the database");

                run_worker_loop(rx, db).await;
            });
        });

        Ok(Self { request_tx: tx })
    }

    /// Coroutine-safe wrapper for running a query and downcasting the result
    #[instrument(level = "info", skip(query_fn), fields(pool = "DbPoolManager"))]
    pub fn execute<T: Send + 'static, F, Fut>(&self, query_fn: F) -> Result<T, DbErr>
    where
        F: FnOnce(DatabaseConnection) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<T, DbErr>> + Send + 'static,
    {
        let (response_tx, response_rx) = crossbeam_channel::bounded(1);
        let queue_depth = METRICS.queue_depth.clone();

        queue_depth.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let start = Instant::now();

        let job: QueryCallback = Box::new(move |conn| {
            Box::pin(async move {
                let result = query_fn(conn)
                    .await
                    .map(|v| Box::new(v) as Box<dyn Any + Send>);
                result
            })
        });

        let request = DbRequest::Execute { job, response_tx };
        self.request_tx
            .send(LifeguardJob::Macro(request))
            .map_err(|e| DbErr::Custom(format!("Send error: {}", e)))?;

        let response = response_rx
            .recv()
            .map_err(|e| DbErr::Custom(format!("Receive error: {}", e)))?;

        let elapsed = start.elapsed();
        METRICS.record_query(elapsed);
        METRICS
            .coroutine_wait_duration
            .record(elapsed.as_secs_f64(), &[]);
        queue_depth.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);

        let boxed = response?;
        let t = *boxed
            .downcast::<T>()
            .map_err(|_| DbErr::Custom("Type mismatch in lifeguard pool".into()))?;
        Ok(t)
    }

    /// Shared dispatch helper for async SeaORM tasks
    async fn send_db_task<R>(
        &self,
        task_builder: impl FnOnce(oneshot::Sender<R>) -> DbTask,
    ) -> Result<R, DbErr>
    where
        R: Send + 'static,
    {
        let (tx, rx) = oneshot::channel::<R>();
        let task = task_builder(tx);
        self.request_tx
            .send(LifeguardJob::Async(task))
            .map_err(|e| DbErr::Custom(format!("Failed to enqueue DbTask: {e}")))?;
        rx.await
            .map_err(|e| DbErr::Custom(format!("Worker dropped: {e}")))
    }

    /// Accessor for raw channel if needed (e.g. implementing ConnectionTrait)
    pub fn lifeguard_sender(&self) -> Sender<LifeguardJob> {
        self.request_tx.clone()
    }
}

#[async_trait]
impl ConnectionTrait for DbPoolManager {
    fn get_database_backend(&self) -> DatabaseBackend {
        DatabaseBackend::Postgres
    }

    async fn execute(&self, stmt: Statement) -> Result<ExecResult, DbErr> {
        self.send_db_task(|tx| DbTask::Execute(stmt.clone(), tx))
            .await?
    }

    async fn execute_unprepared(&self, sql: &str) -> Result<ExecResult, DbErr> {
        let owned_sql = sql.to_owned();
        self.send_db_task(|tx| DbTask::ExecuteUnprepared(owned_sql, tx))
            .await?
    }

    async fn query_one(&self, stmt: Statement) -> Result<Option<QueryResult>, DbErr> {
        self.send_db_task(|tx| DbTask::QueryOne(stmt.clone(), tx))
            .await?
    }

    async fn query_all(&self, stmt: Statement) -> Result<Vec<QueryResult>, DbErr> {
        self.send_db_task(|tx| DbTask::QueryAll(stmt.clone(), tx))
            .await?
    }
}

#[cfg(test)]
mod tests {
    use crate::pool::config::DatabaseConfig;
    use crate::pool::manager::DbPoolManager;
    use crate::test_helpers::{create_temp_table, drop_temp_table};
    use crate::{
        lifeguard_execute, lifeguard_insert_many, lifeguard_query, lifeguard_txn,
        insert_test_rows, seed_test, update_test_rows, with_temp_table,
    };
    use may::go;
    use sea_orm::{
        ColumnTrait, ConnectionTrait, DatabaseBackend, DbErr, EntityTrait, PaginatorTrait,
        QueryFilter, Statement, TransactionTrait, TryGetable,
    };

    #[tokio::test]
    async fn test_macro_and_async_work_together() -> Result<(), DbErr> {
        let pool = DbPoolManager::from_config(&DatabaseConfig {
            url: "postgres://postgres:postgres@localhost:5432/postgres".to_string(),
            max_connections: 1,
            pool_timeout_seconds: 5,
        })?;

        // Async: use ConnectionTrait
        let version_row =
            Statement::from_string(DatabaseBackend::Postgres, "SELECT version()".to_string());
        let result = pool.query_one(version_row).await?;
        assert!(result.is_some());
        let version: String = result.unwrap().try_get("", "version")?;
        println!("✅ Async version: {version}");

        // Coroutine: use lifeguard_execute! macro
        go!(move || {
            let version: String = lifeguard_execute!(pool, {
                let row = pool
                    .query_one(Statement::from_string(
                        DatabaseBackend::Postgres,
                        "SELECT version()",
                    ))
                    .await
                    .expect("Failed to execute query")
                    .expect("No result returned");

                String::try_get(&row, "", "version").expect("Failed to get version")
            })
            .expect("Failed to execute lifeguard_execute macro");

            println!("✅ Coroutine version: {version}");
        });

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        Ok(())
    }

    #[tokio::test]
    async fn test_execute_and_unprepared() -> Result<(), sea_orm::DbErr> {
        let db = DbPoolManager::from_config(&DatabaseConfig {
            url: "postgres://postgres:postgres@localhost:5432/postgres".to_string(),
            max_connections: 1,
            pool_timeout_seconds: 5,
        })?;

        db.execute_unprepared("CREATE TEMP TABLE IF NOT EXISTS temp_table (id SERIAL)")
            .await?;
        db.execute_unprepared("INSERT INTO temp_table DEFAULT VALUES")
            .await?;

        let stmt = Statement::from_string(
            DatabaseBackend::Postgres,
            "INSERT INTO temp_table DEFAULT VALUES",
        );
        let res = db.execute(|conn| async move { conn.execute(stmt).await })?;
        assert_eq!(res.rows_affected(), 1);

        Ok(())
    }

    #[tokio::test]
    async fn test_query_one_and_query_all() -> Result<(), sea_orm::DbErr> {
        let db = DbPoolManager::from_config(&DatabaseConfig {
            url: "postgres://postgres:postgres@localhost:5432/postgres".to_string(),
            max_connections: 1,
            pool_timeout_seconds: 5,
        })?;

        db.execute_unprepared(
            "CREATE TEMP TABLE IF NOT EXISTS temp_table2 (id SERIAL, label TEXT)",
        )
        .await?;
        db.execute_unprepared("INSERT INTO temp_table2 (label) VALUES ('A'), ('B'), ('C')")
            .await?;

        let stmt = Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT label FROM temp_table2 WHERE label = 'B'",
        );
        let row = db.query_one(stmt).await?;
        assert!(row.is_some());
        let val: String = row.unwrap().try_get("", "label")?;
        assert_eq!(val, "B");

        let stmt =
            Statement::from_string(DatabaseBackend::Postgres, "SELECT label FROM temp_table2");
        let rows = db.query_all(stmt).await?;
        assert_eq!(rows.len(), 3);

        Ok(())
    }

    #[tokio::test]
    async fn test_query_with_setup_teardown() -> Result<(), sea_orm::DbErr> {
        let db = DbPoolManager::from_config(&DatabaseConfig {
            url: "postgres://postgres:postgres@localhost:5432/postgres".to_string(),
            max_connections: 1,
            pool_timeout_seconds: 5,
        })?;
        let table = "temp_lifeguard_test";

        create_temp_table(&db, table, "(id SERIAL, label TEXT)").await?;

        db.execute_unprepared(&format!(
            "INSERT INTO {} (label) VALUES ('A'), ('B'), ('C')",
            table
        ))
        .await?;

        let stmt = Statement::from_string(
            DatabaseBackend::Postgres,
            format!("SELECT label FROM {} WHERE label = 'B'", table),
        );
        let row = db.query_one(stmt).await?;
        let label: String = row.unwrap().try_get("", "label")?;
        assert_eq!(label, "B");

        drop_temp_table(&db, table).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_with_temp_table_macro() -> Result<(), sea_orm::DbErr> {
        let db = DbPoolManager::from_config(&DatabaseConfig {
            url: "postgres://postgres:postgres@localhost:5432/postgres".to_string(),
            max_connections: 1,
            pool_timeout_seconds: 5,
        })?;

        with_temp_table!("temp_macro", "(id SERIAL, label TEXT)", db, {
            db.execute_unprepared("INSERT INTO temp_macro (label) VALUES ('Z')")
                .await?;

            let row = db
                .query_one(Statement::from_string(
                    DatabaseBackend::Postgres,
                    "SELECT label FROM temp_macro".to_string(),
                ))
                .await?;

            let label: String = row.unwrap().try_get("", "label")?;
            assert_eq!(label, "Z");
            Ok(())
        })
    }

    #[tokio::test]
    async fn test_insert_test_rows_macro() -> Result<(), sea_orm::DbErr> {
        let db = DbPoolManager::from_config(&DatabaseConfig {
            url: "postgres://postgres:postgres@localhost:5432/postgres".to_string(),
            max_connections: 1,
            pool_timeout_seconds: 5,
        })?;
        let table_name = "temp_data";

        create_temp_table(&db, table_name, "(id INTEGER, name TEXT)").await?;

        insert_test_rows!(temp_data, [
            { id: 1, name: "Alice" },
            { id: 2, name: "Bob" }
        ], db);

        let stmt = Statement::from_string(
            DatabaseBackend::Postgres,
            format!("SELECT COUNT(*) as count FROM {}", table_name),
        );

        let row = db.query_one(stmt).await?.unwrap();
        let count: i64 = row.try_get("", "count")?;
        assert_eq!(count, 2);

        drop_temp_table(&db, table_name).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_seed_test_macro() -> Result<(), sea_orm::DbErr> {
        let db = DbPoolManager::from_config(&DatabaseConfig {
            url: "postgres://postgres:postgres@localhost:5432/postgres".to_string(),
            max_connections: 1,
            pool_timeout_seconds: 5,
        })?;

        seed_test!(owners, "(id INT, name TEXT, phone TEXT)", [
            { id: 1, name: "Alice", phone: "123" },
            { id: 2, name: "Bob", phone: "456" },
            { id: 3, name: "Charlie", phone: "789" },
            { id: 4, name: "Dave", phone: "012" }
        ], db, {
            let stmt = Statement::from_string(
                DatabaseBackend::Postgres,
                "SELECT COUNT(*) as count FROM owners",
            );

            let row = db.query_one(stmt).await?.unwrap();
            let count: i64 = row.try_get("", "count")?;
            assert_eq!(count, 4);

            Ok(())
        })
    }

    #[tokio::test]
    async fn test_lifeguard_query_macro() -> Result<(), sea_orm::DbErr> {
        let pool = DbPoolManager::from_config(&DatabaseConfig {
            url: "postgres://postgres:postgres@localhost:5432/postgres".to_string(),
            max_connections: 1,
            pool_timeout_seconds: 5,
        })?;

        let table = "temp_query";
        create_temp_table(&pool, table, "(id SERIAL, label TEXT)").await?;
        pool.execute_unprepared(&format!("INSERT INTO {} (label) VALUES ('A')", table))
            .await?;

        let stmt = Statement::from_string(
            DatabaseBackend::Postgres,
            format!("SELECT label FROM {} WHERE id = 1", table),
        );

        let pool2 = pool.clone();
        let row = lifeguard_query!(pool2.clone(), pool2.query_one(stmt))
            .unwrap();
        let label: String = row.try_get("", "label")?;
        assert_eq!(label, "A");

        drop_temp_table(&pool, table).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_lifeguard_insert_many_macro() -> Result<(), sea_orm::DbErr> {
        use crate::tests_cfg::entity::owners;
        use sea_orm::ActiveModelTrait;

        let pool = DbPoolManager::from_config(&DatabaseConfig {
            url: "postgres://postgres:postgres@localhost:5432/postgres".to_string(),
            max_connections: 1,
            pool_timeout_seconds: 5,
        })?;

        pool.execute_unprepared("TRUNCATE TABLE owners RESTART IDENTITY")
            .await?;

        let models = vec![
            owners::ActiveModel {
                name: sea_orm::Set("InsertMany One".to_string()),
                phone: sea_orm::Set(None),
                ..Default::default()
            },
            owners::ActiveModel {
                name: sea_orm::Set("InsertMany Two".to_string()),
                phone: sea_orm::Set(None),
                ..Default::default()
            },
        ];

        let last_id: i32 = lifeguard_insert_many!(pool.clone(), owners::Entity, models);
        assert!(last_id >= 2);

        let pool3 = pool.clone();
        let count: u64 = lifeguard_query!(pool3.clone(), owners::Entity::find().count(&pool3));
        assert_eq!(count, 2);

        pool.execute_unprepared("TRUNCATE TABLE owners").await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_lifeguard_txn_macro() -> Result<(), sea_orm::DbErr> {
        use crate::tests_cfg::entity::owners;
        use sea_orm::ActiveModelTrait;

        let pool = DbPoolManager::from_config(&DatabaseConfig {
            url: "postgres://postgres:postgres@localhost:5432/postgres".to_string(),
            max_connections: 1,
            pool_timeout_seconds: 5,
        })?;

        let result: Result<(), sea_orm::DbErr> = lifeguard_txn!(pool.clone(), {
            Ok(())
        });
        assert!(result.is_ok());
        Ok(())
    }

    #[tokio::test]
    async fn test_insert_and_update_test_data_macros() -> Result<(), sea_orm::DbErr> {
        let pool = DbPoolManager::from_config(&DatabaseConfig {
            url: "postgres://postgres:postgres@localhost:5432/postgres".to_string(),
            max_connections: 1,
            pool_timeout_seconds: 5,
        })?;

        let table = "temp_data_macro2";
        create_temp_table(&pool, table, "(id INTEGER, name TEXT)").await?;

        insert_test_rows!(temp_data_macro2, [
            { id: 1, name: "Before" },
            { id: 2, name: "Other" }
        ], pool);

        update_test_rows!(temp_data_macro2, { name: "After" }, "id = 1", pool);

        let stmt = Statement::from_string(
            DatabaseBackend::Postgres,
            format!("SELECT name FROM {} WHERE id = 1", table),
        );
        let row = pool.query_one(stmt).await?.unwrap();
        let name: String = row.try_get("", "name")?;
        assert_eq!(name, "After");

        drop_temp_table(&pool, table).await?;
        Ok(())
    }
}
