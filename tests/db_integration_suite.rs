//! Single Cargo integration-test binary for all database-backed tests.
//!
//! One `context::TEST_CONTEXT` → one Postgres + one Redis (or env URLs) shared by every module
//! until this process exits, then `ctor::dtor` removes Docker containers.

// Integration tests: macro-generated structs, long scenarios, and shared `TEST_CONTEXT` runs share
// noisy patterns; keep strict `cargo clippy` on the library crate.
#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(unreachable_code)] // derive macros occasionally trip unreachable-code in test modules
#![allow(clippy::pedantic)]
#![allow(clippy::similar_names)]
#![allow(clippy::collapsible_match)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::items_after_statements)]
#![allow(clippy::struct_field_names)]

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
