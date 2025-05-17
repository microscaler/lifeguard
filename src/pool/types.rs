use crossbeam_channel::Sender;
use sea_orm::{DatabaseConnection, DbErr, ExecResult, QueryResult, Statement};
use std::any::Any;
use tokio::sync::oneshot;

pub type DbQueryResult = Result<Box<dyn Any + Send>, DbErr>;

pub type BoxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;

pub type QueryCallback =
    Box<dyn FnOnce(DatabaseConnection) -> BoxFuture<'static, DbQueryResult> + Send>;

pub enum DbRequest {
    Execute {
        job: QueryCallback,
        response_tx: Sender<DbQueryResult>,
    },
}

pub enum DbTask {
    Execute(Statement, oneshot::Sender<Result<ExecResult, DbErr>>),
    ExecuteUnprepared(String, oneshot::Sender<Result<ExecResult, DbErr>>),
    QueryOne(
        Statement,
        oneshot::Sender<Result<Option<QueryResult>, DbErr>>,
    ),
    QueryAll(Statement, oneshot::Sender<Result<Vec<QueryResult>, DbErr>>),
}

pub enum LifeguardJob {
    Macro(DbRequest),
    Async(DbTask),
}
