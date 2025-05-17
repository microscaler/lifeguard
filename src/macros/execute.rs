/// Execute a SeaORM async query inside a `may` coroutine context using a `DbPoolManager`.
///
/// Automatically clones the pool, wraps the block in `Box::pin(async move { ... })`,
/// and returns the result synchronously.
///
/// # Example
/// ```ignore
/// let version: String = lifeguard_execute!(pool, {
///     let row = sea_orm::Statement::from_string(
///         sea_orm::DatabaseBackend::Postgres,
///         "SELECT version()".into()
///     )
///     .query_one(db).await?
///     .unwrap();
///
///     row.try_get("", "version")?
/// });
/// ```
#[macro_export]
macro_rules! lifeguard_execute {
    ($pool:expr, $block:block) => {{
        let pool = $pool.clone();
        pool.execute(|db| {
            let _db = &db;
            let _db = _db.clone();
            Box::pin(async move {
                let result = (|| async $block)().await;
                Ok(result)
            })
        })
    }};
}

#[cfg(test)]
mod tests {
    use crate::pool::config::DatabaseConfig;
    use crate::pool::manager::DbPoolManager;
    use sea_orm::{ConnectionTrait, DbErr};
    async fn test_lifeguard_execute_macro_with_simple_value(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pool = DbPoolManager::from_config(&DatabaseConfig {
            url: "mock://test".to_string(),
            max_connections: 1,
        })?;

        let stmt = sea_orm::Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            "SELECT 42 AS version",
        );

        let value: Result<i32, DbErr> = lifeguard_execute!(pool, {
            let row = pool.query_one(stmt).await?;
            let version: i32 = row.expect("REASON").try_get("", "version")?;
            Ok::<_, DbErr>(version)
        })?;
        if let Ok(result) = value {
            assert_eq!(result, 42);
        } else {
            panic!("Failed to execute lifeguard_execute macro");
        }
        Ok(())
    }
}
