/// A coroutine + query combo macro. Spawns a `may::go!` coroutine, runs a query,
/// and returns the result in a binding you name.
///
/// # Example
/// ```ignore
/// lifeguard_go!(pool, version, {
///     let row = Entity::find_by_id(1).one(db).await?.unwrap();
///     Ok::<_, DbErr>(row.name)
/// });
/// ```
#[macro_export]
macro_rules! lifeguard_go {
    ($pool:expr, $ret:ident, $block:expr) => {
        let pool = $pool.clone();
        let $ret = {
            let handle = may::go!(move || {
                pool.execute(|conn| {
                    let _db = conn.clone(); // Clone db so it's accessible in the async block
                    Box::pin(async move {
                        // Execute the block with db in scope
                        $block
                    })
                })
            });
            match handle.join() {
                Ok(join_result) => join_result,
                Err(e) => return Err(sea_orm::DbErr::Custom(format!("{:?}", e))),
            }
        };
    };
}

#[cfg(test)]
mod tests {
    use sea_orm::{DbErr, EntityTrait};
    #[allow(unused_imports)]
    use crate::pool::config::DatabaseConfig;
    #[allow(unused_imports)]
    use crate::tests_cfg::entity::prelude::*;
    #[allow(unused_imports)]
    use crate::tests_cfg::entity::{
        appointments::Entity as Appointments, owners::Entity as Owners, pets::Entity as Pets,
    };
    #[allow(unused_imports)]
    use crate::DbPoolManager;
    use crate::test_pool;

    #[tokio::test]
    async fn test_lifeguard_go_macro_with_return_binding() -> Result<(), sea_orm::DbErr> {
        use sea_orm::{DatabaseBackend, MockDatabase};

        let pool = test_pool!(
            MockDatabase::new(DatabaseBackend::Postgres)
                .append_query_results(vec![vec![<Pets::Entity as sea_orm::EntityTrait>::Model {
                    id: 1,
                    name: "mocked name".to_string(),
                    species: "Dog".to_string(),
                    owner_id: None,
                }]])
        );

        lifeguard_go!(pool, pet_name, {
            let row = Pets::find_by_id(1)
                .one(&pool)
                .await?
                .unwrap();
            Ok::<_, DbErr>(row.name)
        });

        assert_eq!(pet_name?, "mocked name".to_string());
        Ok(())
    }
}

