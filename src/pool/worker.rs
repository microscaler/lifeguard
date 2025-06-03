use crate::pool::types::{DbRequest, DbTask, LifeguardJob};
use crossbeam_channel::Receiver;
use sea_orm::*;
use std::sync::Arc;

/// The worker thread entrypoint that handles both macro and async jobs.
pub async fn run_worker_loop(rx: Receiver<LifeguardJob>, db: DatabaseConnection) {
    while let Ok(job) = rx.recv() {
        match job {
            LifeguardJob::Macro(DbRequest::Execute { job, response_tx }) => {
                let db = clone_connection(&db);
                let fut = job(db);
                let result = fut.await;
                let _ = response_tx.send(result);
            }

            LifeguardJob::Async(DbTask::Execute(stmt, tx)) => {
                let res = db.execute(stmt).await;
                let _ = tx.send(res);
            }

            LifeguardJob::Async(DbTask::ExecuteUnprepared(sql, tx)) => {
                let res = db.execute_unprepared(&sql).await;
                let _ = tx.send(res);
            }

            LifeguardJob::Async(DbTask::QueryOne(stmt, tx)) => {
                let res = db.query_one(stmt).await;
                let _ = tx.send(res);
            }

            LifeguardJob::Async(DbTask::QueryAll(stmt, tx)) => {
                let res = db.query_all(stmt).await;
                let _ = tx.send(res);
            }
        }
    }
}

#[cfg(not(feature = "mock"))]
fn clone_connection(db: &DatabaseConnection) -> DatabaseConnection {
    db.clone()
}

#[cfg(feature = "mock")]
fn clone_connection(db: &DatabaseConnection) -> DatabaseConnection {
    match db {
        DatabaseConnection::SqlxPostgresPoolConnection(conn) => {
            DatabaseConnection::SqlxPostgresPoolConnection(conn.clone())
        }
        #[cfg(feature = "sqlx-mysql")]
        DatabaseConnection::SqlxMySqlPoolConnection(conn) => {
            DatabaseConnection::SqlxMySqlPoolConnection(conn.clone())
        }
        #[cfg(feature = "sqlx-sqlite")]
        DatabaseConnection::SqlxSqlitePoolConnection(conn) => {
            DatabaseConnection::SqlxSqlitePoolConnection(conn.clone())
        }
        #[cfg(feature = "proxy")]
        DatabaseConnection::ProxyDatabaseConnection(conn) => {
            DatabaseConnection::ProxyDatabaseConnection(Arc::clone(conn))
        }
        DatabaseConnection::MockDatabaseConnection(conn) => {
            DatabaseConnection::MockDatabaseConnection(Arc::clone(conn))
        }
        DatabaseConnection::Disconnected => DatabaseConnection::Disconnected,
    }
}
