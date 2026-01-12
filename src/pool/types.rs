//! Pool Types - TO BE REBUILT IN EPIC 04
//!
//! This file contained types for the old SeaORM-based pool manager.
//! New types will be defined in Epic 04 for the may_postgres-based pool.

// OLD IMPLEMENTATION - REMOVED (SeaORM dependencies)
/*
use std::future::Future;
use std::pin::Pin;
use sea_orm::DatabaseConnection;

pub type BoxedDbJob = Box<dyn FnOnce(DatabaseConnection) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send>;

pub enum DbRequest {
    Run(BoxedDbJob),
}
*/

// NEW TYPES WILL BE DEFINED HERE (Epic 04)
// - LifeConnectionSlot
// - Pool job types for may_postgres
// - Connection health types