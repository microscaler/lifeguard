//! Single Cargo integration-test binary for all database-backed tests.
//!
//! One `context::TEST_CONTEXT` → one Postgres + one Redis (or env URLs) shared by every module
//! until this process exits, then `ctor::dtor` removes Docker containers.
//! Optional `TEST_REPLICA_URL` enables read-replica pool tests (`pool_read_replica`).

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

#[path = "db_integration/toxiproxy_control.rs"]
mod toxiproxy_control;

#[path = "db_integration/replication_sync.rs"]
mod replication_sync;

#[path = "db_integration/active_model_crud.rs"]
mod active_model_crud;

#[path = "db_integration/column_f_update.rs"]
mod column_f_update;

#[path = "db_integration/column_f_where.rs"]
mod column_f_where;

#[path = "db_integration/session_identity_flush.rs"]
mod session_identity_flush;

#[path = "db_integration/active_model_graph.rs"]
mod active_model_graph;

#[path = "db_integration/dataloader_n_plus_one.rs"]
mod dataloader_n_plus_one;

#[path = "db_integration/stream_and_cursor.rs"]
mod stream_and_cursor;

#[path = "db_integration/related_trait.rs"]
mod related_trait;

#[path = "db_integration/json_value_from_row.rs"]
mod json_value_from_row;

#[path = "db_integration/pool_read_replica.rs"]
mod pool_read_replica;

#[path = "db_integration/pool_idle_liveness.rs"]
mod pool_idle_liveness;
