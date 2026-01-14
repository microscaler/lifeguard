//! Test Helpers Module - Epic 01 Story 08
//!
//! Provides test infrastructure helpers for integration tests using Kind/Kubernetes.
//!
//! This module provides:
//! - Connection string retrieval from Kubernetes
//! - Test database setup/teardown helpers
//! - Integration test utilities

use crate::connection::{connect, ConnectionError};
use crate::executor::MayPostgresExecutor;
use may_postgres::Client;
use std::env;
use std::process::Command;
use std::time::Duration;

/// Test database configuration
pub struct TestDatabase {
    connection_string: String,
    client: Option<Client>,
}

impl TestDatabase {
    /// Create a new test database connection
    ///
    /// This function attempts to get the connection string from:
    /// 1. `TEST_DATABASE_URL` environment variable (highest priority)
    /// 2. `DATABASE_URL` environment variable
    /// 3. Kubernetes service (if running in Kind cluster)
    /// 4. Default localhost connection
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use lifeguard::test_helpers::TestDatabase;
    ///
    /// let db = TestDatabase::new()?;
    /// let executor = db.executor()?;
    /// // Use executor for tests...
    /// # Ok::<(), lifeguard::test_helpers::TestError>(())
    /// ```
    pub fn new() -> Result<Self, TestError> {
        let connection_string = Self::get_connection_string()?;
        Ok(Self {
            connection_string,
            client: None,
        })
    }

    /// Get connection string from various sources
    fn get_connection_string() -> Result<String, TestError> {
        // Priority 1: TEST_DATABASE_URL environment variable
        if let Ok(url) = env::var("TEST_DATABASE_URL") {
            return Ok(url);
        }

        // Priority 2: DATABASE_URL environment variable
        if let Ok(url) = env::var("DATABASE_URL") {
            return Ok(url);
        }

        // Priority 3: Try to get from Kubernetes (if running in Kind cluster)
        if let Ok(url) = Self::get_k8s_connection_string() {
            return Ok(url);
        }

        // Priority 4: Default localhost
        Ok("postgresql://postgres:postgres@localhost:5432/postgres".to_string())
    }

    /// Get connection string from Kubernetes service
    fn get_k8s_connection_string() -> Result<String, TestError> {
        // Try to get connection string from kubectl
        let output = Command::new("kubectl")
            .args(&[
                "get",
                "svc",
                "postgres",
                "-n",
                "lifeguard-test",
                "-o",
                "jsonpath={.spec.clusterIP}",
            ])
            .output()
            .map_err(|e| TestError::K8sError(format!("Failed to run kubectl: {}", e)))?;

        if !output.status.success() {
            return Err(TestError::K8sError(
                "kubectl command failed or service not found".to_string(),
            ));
        }

        let cluster_ip = String::from_utf8(output.stdout)
            .map_err(|e| TestError::K8sError(format!("Invalid kubectl output: {}", e)))?
            .trim()
            .to_string();

        if cluster_ip.is_empty() || cluster_ip == "None" {
            // Use service DNS name instead
            Ok("postgresql://postgres:postgres@postgres.lifeguard-test.svc.cluster.local:5432/postgres".to_string())
        } else {
            Ok(format!(
                "postgresql://postgres:postgres@{}:5432/postgres",
                cluster_ip
            ))
        }
    }

    /// Get the connection string
    pub fn connection_string(&self) -> &str {
        &self.connection_string
    }

    /// Connect to the database and return a client
    pub fn connect(&mut self) -> Result<Client, ConnectionError> {
        let client = connect(&self.connection_string)?;
        self.client = Some(client.clone());
        Ok(client)
    }

    /// Get an executor for the test database
    pub fn executor(&mut self) -> Result<MayPostgresExecutor, TestError> {
        let client = self.connect().map_err(TestError::ConnectionError)?;
        Ok(MayPostgresExecutor::new(client))
    }

    /// Wait for the database to be ready
    ///
    /// This function attempts to connect to the database, retrying up to `max_attempts` times
    /// with a delay of `delay_seconds` between attempts.
    pub fn wait_for_ready(
        &mut self,
        max_attempts: u32,
        delay_seconds: u64,
    ) -> Result<(), TestError> {
        for attempt in 1..=max_attempts {
            match self.connect() {
                Ok(_) => return Ok(()),
                Err(e) => {
                    if attempt < max_attempts {
                        std::thread::sleep(Duration::from_secs(delay_seconds));
                        continue;
                    } else {
                        return Err(TestError::ConnectionError(e));
                    }
                }
            }
        }
        Err(TestError::ConnectionError(ConnectionError::Other(
            "Failed to connect after max attempts".to_string(),
        )))
    }
}

/// Test error type
#[derive(Debug)]
pub enum TestError {
    /// Connection error
    ConnectionError(ConnectionError),
    /// Kubernetes-related error
    K8sError(String),
    /// Other test errors
    Other(String),
}

impl std::fmt::Display for TestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TestError::ConnectionError(e) => write!(f, "Connection error: {}", e),
            TestError::K8sError(s) => write!(f, "Kubernetes error: {}", s),
            TestError::Other(s) => write!(f, "Test error: {}", s),
        }
    }
}

impl std::error::Error for TestError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_connection_string_env_var() {
        // Test that environment variable is respected
        env::set_var(
            "TEST_DATABASE_URL",
            "postgresql://test:test@localhost:5432/test",
        );
        let url = TestDatabase::get_connection_string().unwrap();
        assert!(url.contains("test"));
        env::remove_var("TEST_DATABASE_URL");
    }

    #[test]
    fn test_get_connection_string_default() {
        // Test default connection string when no env vars are set
        // Note: If Kind cluster is running, it may return:
        // - Kubernetes service DNS (postgres.lifeguard-test.svc.cluster.local)
        // - Cluster IP address (10.x.x.x)
        // - Or default to localhost
        // All are valid connection strings
        env::remove_var("TEST_DATABASE_URL");
        env::remove_var("DATABASE_URL");
        let url = TestDatabase::get_connection_string().unwrap();
        // Should always be a valid PostgreSQL connection string
        assert!(
            url.starts_with("postgresql://"),
            "Should be a PostgreSQL connection string, got: {}",
            url
        );
        // Should contain postgres user and database
        assert!(
            url.contains("postgres"),
            "Should contain postgres user/database"
        );
    }
}
