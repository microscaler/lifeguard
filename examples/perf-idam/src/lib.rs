//! IDAM-shaped entities for Lifeguard ORM performance testing.
//!
//! Runbook: repository file `docs/PERF_ORM.md`.

pub mod perf_idam;

#[allow(missing_docs)]
pub mod entity_registry {
    include!(concat!(env!("OUT_DIR"), "/entity_registry.rs"));
}
