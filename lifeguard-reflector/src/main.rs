//! `lifereflector` binary: configuration from `LIFEREFLECTOR_*` environment variables.

use lifeguard_reflector::{metrics, run, CacheSink, Config, RedisSink, SinkError};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("lifereflector: {e}");
            std::process::exit(2);
        }
    };
    may::config().set_workers(2).set_stack_size(0x10_0000);
    if let Some(addr) = &cfg.metrics_addr {
        if let Err(e) = metrics::serve(addr) {
            eprintln!("lifereflector: metrics endpoint {addr}: {e}");
            std::process::exit(2);
        }
    }
    let redis_url = cfg.redis_url.clone();
    let connect = move || -> Result<Box<dyn CacheSink>, SinkError> {
        Ok(Box::new(RedisSink::connect(&redis_url)?))
    };
    // Killing the process is safe: invalidations are idempotent and the successor catches up.
    let stop = Arc::new(AtomicBool::new(false));
    let h = may::go!(move || run(cfg, &connect, &stop));
    match h.join() {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            eprintln!("lifereflector: refusing to start: {e}");
            std::process::exit(1);
        }
        Err(_) => std::process::exit(1),
    }
}
