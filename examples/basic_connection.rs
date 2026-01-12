//! Basic Connection Example - Epic 01 Story 02
//!
//! Demonstrates how to establish a connection to PostgreSQL using may_postgres.
//!
//! Run with:
//! ```bash
//! cargo run --example basic_connection
//! ```

use lifeguard::connection::{connect, validate_connection_string};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Example connection strings
    let uri_format = "postgresql://postgres:postgres@localhost:5432/postgres";
    let key_value_format = "host=localhost user=postgres dbname=postgres";

    println!("Validating connection strings...");
    
    // Validate URI format
    match validate_connection_string(uri_format) {
        Ok(_) => println!("✅ URI format connection string is valid"),
        Err(e) => println!("❌ URI format validation failed: {}", e),
    }

    // Validate key-value format
    match validate_connection_string(key_value_format) {
        Ok(_) => println!("✅ Key-value format connection string is valid"),
        Err(e) => println!("❌ Key-value format validation failed: {}", e),
    }

    println!("\nAttempting to connect to PostgreSQL...");
    println!("Note: This will fail if PostgreSQL is not running or credentials are incorrect.");

    // Try to connect (this will fail if DB is not available, which is expected)
    match connect(uri_format) {
        Ok(_client) => {
            println!("✅ Successfully connected to PostgreSQL!");
            println!("   Connection established with may_postgres");
            println!("   Client is ready for queries");
            // Note: In a real application, you would use the client here
            // For example: let rows = _client.query("SELECT 1", &[])?;
            Ok(())
        }
        Err(e) => {
            println!("❌ Connection failed: {}", e);
            println!("   This is expected if PostgreSQL is not running.");
            println!("   To test a real connection, ensure PostgreSQL is running and credentials are correct.");
            // Return Ok since this is just a demonstration
            Ok(())
        }
    }
}
