//! Single Cargo integration-test binary for all database-backed tests.
//!
//! One `context::TEST_CONTEXT` → one Postgres + one Redis (or env URLs) shared by every module
//! until this process exits, then `ctor::dtor` removes Docker containers.

mod context;

#[path = "db_integration/active_model_crud.rs"]
mod active_model_crud;

#[path = "db_integration/active_model_graph.rs"]
mod active_model_graph;

#[path = "db_integration/dataloader_n_plus_one.rs"]
mod dataloader_n_plus_one;

#[path = "db_integration/stream_and_cursor.rs"]
mod stream_and_cursor;

#[path = "db_integration/related_trait.rs"]
mod related_trait;
