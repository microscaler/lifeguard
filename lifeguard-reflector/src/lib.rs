//! `LifeReflector` keeps a Redis cache of Lifeguard entities coherent with Postgres.
//!
//! Lifeguard's generated triggers publish `{"table","op","id","v","ts"}` on a NOTIFY channel.
//! One active reflector (a session advisory lock picks it; others wait as listening standbys)
//! turns those into batched `UNLINK`s of `{prefix}{table}:{id}`. It never writes row data:
//! readers refill on miss, so a duplicate or late invalidation costs one extra read and never
//! installs a value. After any period without a listener it catches up from a watermark (ranged
//! tables) or by flushing the table's keys.
//!
//! It refuses to start against a `noeviction` Redis. The crate README has the design, the
//! configuration and the metrics.

pub mod config;
pub mod engine;
pub mod metrics;
pub mod payload;
pub mod reflector;
pub mod sink;

pub use config::{CatchUp, Config, ConfigError, TableSpec};
pub use engine::{Engine, Role, TakeOver};
pub use payload::{Change, Op, PayloadError};
pub use reflector::{check_eviction_policy, run, SinkFactory, StartError};
pub use sink::{CacheSink, MemorySink, RedisSink, SinkError};
