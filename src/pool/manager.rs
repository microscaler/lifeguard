use sea_orm::{ConnectOptions, Database, DatabaseConnection, DbErr, DatabaseBackend, ExecResult, QueryResult, Statement, ConnectionTrait};
use crossbeam_channel::{bounded, Receiver, Sender};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use crate::pool::types::DbRequest;
use crate::pool::worker::run_worker_loop;
use crossbeam_channel::bounded as crossbeam_bounded;
use std::any::Any;
use std::future::Future;
use std::pin::Pin;
use async_trait::async_trait;

pub struct DbPoolManager {
    senders: Vec<Sender<DbRequest>>,
    strategy: LoadBalancingStrategy,
}

#[derive(Clone)]
pub struct LifeguardConnection {
    sender: Sender<DbRequest>,
}

enum LoadBalancingStrategy {
    RoundRobin(AtomicUsize),
}

pub async fn run_worker_loop(rx: Receiver<DbRequest>, db: DatabaseConnection) {
    while let Ok(DbRequest::Run(job)) = rx.recv() {
        let conn = db.close();
        job(conn).await;
    }
}


impl LoadBalancingStrategy {
    fn next(&self, len: usize) -> usize {
        match self {
            LoadBalancingStrategy::RoundRobin(counter) => {
                counter.fetch_add(1, Ordering::SeqCst) % len
            }
        }
    }
}

impl DbPoolManager {
    pub fn new_with_params(database_url: &str, pool_size: usize) -> Result<Self, DbErr> {
        let mut senders = Vec::with_capacity(pool_size);
        for _ in 0..pool_size {
            let (tx, rx) = crossbeam_bounded::<DbRequest>(100);
            let db_url = database_url.to_string();

            may::go!(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("tokio runtime");
                rt.block_on(async move {
                    let db = Database::connect(ConnectOptions::new(&db_url))
                        .await
                        .expect("db connect");
                    run_worker_loop(rx, db).await;
                });
            });

            senders.push(tx);
        }

        Ok(Self {
            senders,
            strategy: LoadBalancingStrategy::RoundRobin(AtomicUsize::new(0)),
        })
    }

    pub fn lifeguard_sender(&self) -> Sender<DbRequest> {
        let idx = self.strategy.next(self.senders.len());
        self.senders[idx].clone()
    }

    pub fn execute<T, F, Fut>(&self, query_fn: F) -> Result<T, DbErr>
    where
        T: Send + 'static,
        F: FnOnce(DatabaseConnection) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<T, DbErr>> + Send + 'static,
    {
        use crate::metrics::METRICS;
        use std::time::Instant;

        METRICS.queue_depth.fetch_add(1, Ordering::Relaxed);
        let start = Instant::now();

        let sender = self.lifeguard_sender();
        let (tx, rx) = crossbeam_channel::bounded(1);

        let job: crate::pool::types::BoxedDbJob = Box::new(move |conn| {
            Box::pin(async move {
                let result = query_fn(conn).await;
                METRICS.observe_wait(start.elapsed());
                let _ = tx.send(result);
            })
        });

        sender
            .send(DbRequest::Run(job))
            .map_err(|e| DbErr::Custom(format!("Send error: {}", e)))?;

        let res = rx.recv().map_err(|e| DbErr::Custom(format!("Receive error: {}", e)))?;
        METRICS.queue_depth.fetch_sub(1, Ordering::Relaxed);
        METRICS.record_query(start.elapsed());
        res
    }

    pub fn connection(&self) -> LifeguardConnection {
        LifeguardConnection {
            sender: self.lifeguard_sender(),
        }
    }
}

#[async_trait]
impl ConnectionTrait for LifeguardConnection {
    fn get_database_backend(&self) -> DatabaseBackend {
        DatabaseBackend::Postgres
    }

    async fn execute(&self, stmt: Statement) -> Result<ExecResult, DbErr> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let job = crate::pool::types::DbRequest::Run(Box::new(move |conn| {
            Box::pin(async move {
                let res = conn.execute(stmt).await;
                let _ = tx.send(res);
            })
        }));
        self.sender.send(job).map_err(|e| DbErr::Custom(e.to_string()))?;
        rx.await.map_err(|e| DbErr::Custom(e.to_string()))?
    }

    async fn execute_unprepared(&self, sql: &str) -> Result<ExecResult, DbErr> {
        let sql = sql.to_string();
        let (tx, rx) = tokio::sync::oneshot::channel();
        let job = crate::pool::types::DbRequest::Run(Box::new(move |conn| {
            Box::pin(async move {
                let res = conn.execute_unprepared(&sql).await;
                let _ = tx.send(res);
            })
        }));
        self.sender.send(job).map_err(|e| DbErr::Custom(e.to_string()))?;
        rx.await.map_err(|e| DbErr::Custom(e.to_string()))?
    }

    async fn query_one(&self, stmt: Statement) -> Result<Option<QueryResult>, DbErr> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let job = crate::pool::types::DbRequest::Run(Box::new(move |conn| {
            Box::pin(async move {
                let res = conn.query_one(stmt).await;
                let _ = tx.send(res);
            })
        }));
        self.sender.send(job).map_err(|e| DbErr::Custom(e.to_string()))?;
        rx.await.map_err(|e| DbErr::Custom(e.to_string()))?
    }

    async fn query_all(&self, stmt: Statement) -> Result<Vec<QueryResult>, DbErr> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let job = crate::pool::types::DbRequest::Run(Box::new(move |conn| {
            Box::pin(async move {
                let res = conn.query_all(stmt).await;
                let _ = tx.send(res);
            })
        }));
        self.sender.send(job).map_err(|e| DbErr::Custom(e.to_string()))?;
        rx.await.map_err(|e| DbErr::Custom(e.to_string()))?
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{entity::*, query::*, DatabaseBackend, DbErr, DeriveEntityModel, DeriveRelation, EntityTrait, EnumIter, Statement, TryGetable};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use rand::Rng;
    use tokio::task;

    #[tokio::test]
    async fn test_lifeguard_connection_raw_query() {
        let pool = DbPoolManager::new_with_params("postgres://postgres:postgres@localhost:5432/postgres", 2).unwrap();
        let conn = pool.connection();

        let stmt = Statement::from_string(DatabaseBackend::Postgres, "SELECT 42 AS answer");
        let row = conn.query_one(stmt).await.unwrap().unwrap();
        let answer: i32 = row.try_get("", "answer").unwrap();
        assert_eq!(answer, 42);
    }

    #[tokio::test]
    async fn test_lifeguard_high_volume_insert_and_query() -> Result<(), DbErr> {
        #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
        #[sea_orm(table_name = "stress_entity")]
        pub struct Model {
            #[sea_orm(primary_key)]
            pub id: i32,
            pub value: String,
        }

        #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
        pub enum Relation {}

        impl ActiveModelBehavior for ActiveModel {}

        let pool = DbPoolManager::new_with_params("postgres://postgres:postgres@localhost:5432/postgres", 4)?;
        let conn = pool.connection();

        conn.execute_unprepared("DROP TABLE IF EXISTS stress_entity").await?;
        conn.execute_unprepared("CREATE TABLE stress_entity (id SERIAL PRIMARY KEY, value TEXT NOT NULL)").await?;

        let mut inserts = vec![];
        for i in 0..1000 {
            inserts.push(ActiveModel {
                value: Set(format!("val_{}", i)),
                ..Default::default()
            });
        }

        Entity::insert_many(inserts).exec(&conn).await?;

        let results = Entity::find().all(&conn).await?;
        assert_eq!(results.len(), 1000);
        assert!(results.iter().any(|m| m.value == "val_42"));
        Ok(())
    }

    #[tokio::test]
    async fn test_transaction_rollback_on_error() -> Result<(), DbErr> {
        #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
        #[sea_orm(table_name = "rollback_test")]
        pub struct Model {
            #[sea_orm(primary_key)]
            pub id: i32,
            pub data: String,
        }

        #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
        pub enum Relation {}

        impl ActiveModelBehavior for ActiveModel {}

        let pool = DbPoolManager::new_with_params("postgres://postgres:postgres@localhost:5432/postgres", 2)?;
        let conn = pool.connection();

        conn.execute_unprepared("DROP TABLE IF EXISTS rollback_test").await?;
        conn.execute_unprepared("CREATE TABLE rollback_test (id SERIAL PRIMARY KEY, data TEXT NOT NULL)").await?;

        let tx = conn.begin().await?;

        ActiveModel {
            data: Set("will rollback".into()),
            ..Default::default()
        }.insert(&tx).await?;

        tx.rollback().await?;

        let results = Entity::find().all(&conn).await?;
        assert!(results.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_concurrent_insert_and_read() -> Result<(), DbErr> {
        #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
        #[sea_orm(table_name = "concurrent_entity")]
        pub struct Model {
            #[sea_orm(primary_key)]
            pub id: i32,
            pub name: String,
        }

        #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
        pub enum Relation {}

        impl ActiveModelBehavior for ActiveModel {}

        let pool = DbPoolManager::new_with_params("postgres://postgres:postgres@localhost:5432/postgres", 4)?;
        let conn = pool.connection();

        conn.execute_unprepared("DROP TABLE IF EXISTS concurrent_entity").await?;
        conn.execute_unprepared("CREATE TABLE concurrent_entity (id SERIAL PRIMARY KEY, name TEXT NOT NULL)").await?;

        let mut handles = vec![];
        for i in 0..10 {
            let conn = pool.connection();
            let name = format!("user_{}", i);
            handles.push(task::spawn(async move {
                let model = ActiveModel {
                    name: Set(name),
                    ..Default::default()
                };
                Entity::insert(model).exec(&conn).await
            }));
        }

        for handle in handles {
            handle.await.unwrap()?;
        }

        let results = Entity::find().all(&conn).await?;
        assert_eq!(results.len(), 10);
        Ok(())
    }

    #[tokio::test]
    async fn test_retry_on_flaky_connection_simulation() -> Result<(), DbErr> {
        #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
        #[sea_orm(table_name = "flaky_entity")]
        pub struct Model {
            #[sea_orm(primary_key)]
            pub id: i32,
            pub label: String,
        }

        #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
        pub enum Relation {}

        impl ActiveModelBehavior for ActiveModel {}

        let pool = DbPoolManager::new_with_params("postgres://postgres:postgres@localhost:5432/postgres", 2)?;
        let conn = pool.connection();

        conn.execute_unprepared("DROP TABLE IF EXISTS flaky_entity").await?;
        conn.execute_unprepared("CREATE TABLE flaky_entity (id SERIAL PRIMARY KEY, label TEXT NOT NULL)").await?;

        let retries = Arc::new(Mutex::new(0));
        let mut last_err = None;

        for attempt in 0..5 {
            let conn = conn.clone();
            let retries = retries.clone();

            let result = Entity::insert(ActiveModel {
                label: Set("retry-test".to_string()),
                ..Default::default()
            }).exec(&conn).await;

            match result {
                Ok(_) => {
                    println!("✅ Insert succeeded on attempt {}", attempt + 1);
                    break;
                }
                Err(e) => {
                    *retries.lock().unwrap() += 1;
                    println!("⚠️ Retry {} failed: {}", attempt + 1, e);
                    last_err = Some(e);
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }

        assert!(Entity::find().filter(Column::Label.eq("retry-test")).one(&conn).await?.is_some());
        assert!(retries.lock().unwrap().to_owned() < 5);
        Ok(())
    }
}
