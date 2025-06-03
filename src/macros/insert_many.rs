#[allow(unused_imports)]
use crate::pool::config::DatabaseConfig;
#[allow(unused_imports)]
use crate::DbPoolManager;

/// Insert many ActiveModels using `insert_many` and return last insert ID.
#[macro_export]
macro_rules! lifeguard_insert_many {
    ($pool:expr, $entity:path, $models:expr) => {{
        $pool.execute(|db| {
            Box::pin(async move {
                let res = < $entity as sea_orm::EntityTrait >::insert_many($models)
                    .exec(&db)
                    .await?;
                Ok::<_, sea_orm::DbErr>(res.last_insert_id)
            })
        })?
    }};
}
