#[allow(unused_imports)]
use crate::pool::config::DatabaseConfig;
#[allow(unused_imports)]
use crate::DbPoolManager;

/// Run a SeaORM query and automatically handle `.await?`.
#[macro_export]
macro_rules! lifeguard_query {
    ($pool:expr, $query:expr) => {{
        $pool.execute(|db| Box::pin(async move { Ok($query.await?) }))?
    }};
}
