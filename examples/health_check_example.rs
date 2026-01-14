//! Health Check Example - Epic 01 Story 07
//!
//! Demonstrates connection health checking with Lifeguard:
//! - Checking connection health
//! - Using health checks with executors
//! - Handling unhealthy connections

use lifeguard::connection::check_connection_health;
use lifeguard::{connect, LifeError, LifeExecutor, MayPostgresExecutor};

fn main() -> Result<(), LifeError> {
    // Connect to database
    let connection_string = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost:5432/postgres".to_string());

    let client = connect(&connection_string)
        .map_err(|e| LifeError::Other(format!("Connection error: {}", e)))?;

    // Example 1: Check connection health directly
    println!("Example 1: Direct connection health check");
    match check_connection_health(&client) {
        Ok(true) => println!("✓ Connection is healthy"),
        Ok(false) => println!("✗ Connection is unhealthy"),
        Err(e) => println!("✗ Health check failed: {}", e),
    }
    println!();

    // Example 2: Check health via executor
    println!("Example 2: Health check via executor");
    let executor = MayPostgresExecutor::new(client);

    match executor.check_health() {
        Ok(true) => println!("✓ Executor connection is healthy"),
        Ok(false) => println!("✗ Executor connection is unhealthy - may need reconnection"),
        Err(e) => println!("✗ Health check failed: {}", e),
    }
    println!();

    // Example 3: Health check before executing queries
    println!("Example 3: Health check before query execution");
    let executor = MayPostgresExecutor::new(
        connect(&connection_string)
            .map_err(|e| LifeError::Other(format!("Connection error: {}", e)))?,
    );

    // Check health before executing a query
    if executor.check_health()? {
        println!("✓ Connection is healthy, proceeding with query");

        // Execute a simple query
        let row = executor.query_one("SELECT 1 as health_check", &[])?;
        let result: i32 = row.get(0);
        println!("✓ Query executed successfully: {}", result);
    } else {
        println!("✗ Connection is unhealthy, cannot execute query");
        return Err(LifeError::Other(
            "Connection health check failed".to_string(),
        ));
    }
    println!();

    // Example 4: Periodic health monitoring pattern
    println!("Example 4: Periodic health monitoring pattern");
    let executor = MayPostgresExecutor::new(
        connect(&connection_string)
            .map_err(|e| LifeError::Other(format!("Connection error: {}", e)))?,
    );

    // Simulate periodic health checks (in a real application, this would be in a loop)
    for i in 1..=3 {
        println!("Health check #{}:", i);
        match executor.check_health() {
            Ok(true) => println!("  ✓ Connection healthy"),
            Ok(false) => println!("  ✗ Connection unhealthy - should reconnect"),
            Err(e) => println!("  ✗ Health check error: {}", e),
        }
    }

    println!("\nAll health check examples completed!");
    Ok(())
}
