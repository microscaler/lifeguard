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

// Legacy test code archived - was using SeaORM entities from tests_cfg
// This test will be rebuilt in Epic 03 using LifeModel/LifeRecord
// See .archive/legacy-petstore/ for archived files
//
// #[cfg(test)]
// mod tests {
//     // Test code using archived SeaORM entities
//     // Will be rebuilt with Lifeguard entities
// }

