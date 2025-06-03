use std::future::Future;
use std::pin::Pin;
use sea_orm::DatabaseConnection;

pub type BoxedDbJob = Box<dyn FnOnce(DatabaseConnection) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send>;

pub enum DbRequest {
    Run(BoxedDbJob),
}
