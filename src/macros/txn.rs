#[allow(unused_imports)]
use crate::pool::config::DatabaseConfig;
#[allow(unused_imports)]
use crate::DbPoolManager;

/// Run a SeaORM transaction using Lifeguard, with commit/rollback handled automatically.
#[macro_export]
macro_rules! lifeguard_txn {
    ($pool:expr, $block:block) => {{
        $pool.execute(|db| Box::pin(async move {
            let txn = db.begin().await?;
            let out = (|| async $block)().await;
            match out {
                Ok(val) => {
                    txn.commit().await?;
                    Ok(val)
                }
                Err(e) => {
                    txn.rollback().await?;
                    Err(e)
                }
            }
        }))
    }};
}
