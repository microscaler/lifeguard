use super::provider::{CacheProvider, CacheError, CachedResult};
use redis::{Client, Connection, Commands};

/// A Redis implementation of the `CacheProvider` trait.
/// Provides distributed cache coherence mechanisms conforming to LifeReflector needs.
pub struct RedisCacheProvider {
    client: Client,
}

impl RedisCacheProvider {
    /// Creates a new RedisCacheProvider connected to the provided Redis URL.
    pub fn new(connection_url: &str) -> Result<Self, CacheError> {
        let client = Client::open(connection_url)
            .map_err(|e| CacheError::Connection(e.to_string()))?;
        Ok(Self { client })
    }

    fn get_conn(&self) -> Result<Connection, CacheError> {
        self.client.get_connection()
            .map_err(|e| CacheError::Connection(e.to_string()))
    }
}

impl CacheProvider for RedisCacheProvider {
    fn get(&self, key: &str) -> Result<CachedResult<String>, CacheError> {
        let mut conn = self.get_conn()?;
        let value: Option<String> = conn.get(key)
            .map_err(|e| CacheError::Internal(format!("Redis get error: {e}")))?;

        match value {
            Some(v) => Ok(CachedResult::Hit(v)),
            None => Ok(CachedResult::Miss),
        }
    }

    fn set(&self, key: &str, value: &str, ttl_seconds: Option<u64>) -> Result<(), CacheError> {
        let mut conn = self.get_conn()?;
        
        match ttl_seconds {
            Some(ttl) => {
                let _: () = conn.set_ex::<&str, &str, ()>(key, value, ttl)
                    .map_err(|e| CacheError::Internal(format!("Redis set_ex error: {e}")))?;
            }
            None => {
                let _: () = conn.set::<&str, &str, ()>(key, value)
                    .map_err(|e| CacheError::Internal(format!("Redis set error: {e}")))?;
            }
        }
        
        Ok(())
    }

    fn invalidate(&self, key: &str) -> Result<(), CacheError> {
        let mut conn = self.get_conn()?;
        let _: () = conn.del::<&str, ()>(key)
            .map_err(|e| CacheError::Internal(format!("Redis del error: {e}")))?;
        Ok(())
    }
}
