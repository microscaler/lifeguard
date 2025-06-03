//! # Lifeguard
//!
//! Coroutine-safe PostgreSQL pool for SeaORM using the `may` runtime.
//!
//! See [README on GitHub](https://github.com/microscaler/lifeguard) for full architecture.

//! # Lifeguard
//! Coroutine-safe PostgreSQL pool for SeaORM using the `may` runtime.
//!
//! See [README on GitHub](https://github.com/microscaler/lifeguard)

pub mod config;

mod macros;
pub mod metrics;
pub mod pool;
mod test_helpers;
mod tests_cfg;

pub use pool::DbPoolManager;
