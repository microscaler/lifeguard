use once_cell::sync::Lazy;
use testcontainers::clients;
use testcontainers_modules::{postgres::Postgres, redis::Redis};

pub struct LifeguardTestContext {
    pub pg_url: String,
    pub redis_url: String,
}

pub static TEST_CONTEXT: Lazy<LifeguardTestContext> = Lazy::new(|| {
    // Global Docker CLI

    let docker = Box::leak(Box::new(clients::Cli::default()));
    
    let pg_node = docker.run(Postgres::default());
    let pg_port = pg_node.get_host_port_ipv4(5432);
    let pg_url = format!("postgres://postgres:postgres@127.0.0.1:{pg_port}/postgres");
    
    // Leak the node to keep the container running for the lifetime of the test binary.
    // The testcontainers test runner will clean up containers via its reaper if the process exits.
    Box::leak(Box::new(pg_node));

    let redis_node = docker.run(Redis);
    let redis_port = redis_node.get_host_port_ipv4(6379);
    let redis_url = format!("redis://127.0.0.1:{redis_port}");
    
    Box::leak(Box::new(redis_node));

    LifeguardTestContext {
        pg_url,
        redis_url,
    }
});

pub fn get_test_context() -> &'static LifeguardTestContext {
    &TEST_CONTEXT
}

// Helper to clean the database before a test if needed
pub fn clean_db(pg_url: &str, tables: &[&str]) {
    if let Ok(client) = may_postgres::connect(pg_url) {
        for table in tables {
            let _ = client.execute(format!("DROP TABLE IF EXISTS {table} CASCADE;").as_str(), &[]);
        }
    }
}
