use crate::pool::types::{DbRequest, DbTask, LifeguardJob};
use crossbeam_channel::Receiver;
use sea_orm::*;

/// The worker thread entrypoint that handles both macro and async jobs.
pub async fn run_worker_loop(rx: Receiver<LifeguardJob>, db: DatabaseConnection) {
    while let Ok(job) = rx.recv() {
        match job {
            LifeguardJob::Macro(DbRequest::Execute { job, response_tx }) => {
                let db = db.clone();
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
