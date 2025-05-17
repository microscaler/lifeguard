use entity::owners;
use lifeguard::{
    lifeguard_execute, lifeguard_query, lifeguard_txn, pool::config::DatabaseConfig, DbPoolManager,
};
use sea_orm::*;

// #[test]
// fn test_macro_insert_and_query() -> Result<(), Box<dyn std::error::Error>> {
//     let cfg = DatabaseConfig::default();
//     let pool = DbPoolManager::from_config(&cfg).unwrap();
//
//     let owner_id: i32 = lifeguard_execute!(pool, {
//         let model = owners::ActiveModel {
//             name: Set("MacroTester".into()),
//             phone: Set(None),
//             ..Default::default()
//         };
//         let res = owners::Entity::insert(model).exec(db).await?;
//         Ok::<_, DbErr>(res.last_insert_id)
//     });
//
//     let count: u64 = lifeguard_query!(pool, {
//         owners::Entity::find()
//             .filter(owners::Column::Id.eq(owner_id))
//             .count(db)
//     });
//
//     assert_eq!(count, 1);
//     Ok(())
// }
//
// #[test]
// fn test_macro_txn_commit() -> Result<(), Box<dyn std::error::Error>> {
//     let cfg = DatabaseConfig::default();
//     let pool = DbPoolManager::from_config(&cfg).unwrap();
//
//     let Ok((a, b)) = lifeguard_txn!(pool, {
//         let a = owners::ActiveModel {
//             name: Set("Txn A".into()),
//             phone: Set(None),
//             ..Default::default()
//         };
//         let b = owners::ActiveModel {
//             name: Set("Txn B".into()),
//             phone: Set(None),
//             ..Default::default()
//         };
//         let r1 = owners::Entity::insert(a).exec(&txn).await?.last_insert_id;
//         let r2 = owners::Entity::insert(b).exec(&txn).await?.last_insert_id;
//         Ok((r1, r2))
//     });
//
//     assert!(a > 0 && b > 0);
//     Ok(())
// }
