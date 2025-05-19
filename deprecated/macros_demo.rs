mod entity;
use entity::{owners, pets};
use lifeguard::{
    lifeguard_execute, lifeguard_go, lifeguard_insert_many, lifeguard_query, lifeguard_txn,
    pool::config::DatabaseConfig, DbPoolManager,
};

use sea_orm::*;

// fn main() -> Result<(), Box<dyn std::error::Error>> {
//     let config = DatabaseConfig::load();
//     let pool = DbPoolManager::from_config(&config).unwrap();
//
//     // lifeguard_execute!
//     let version: String = lifeguard_execute!(pool, {
//         let row = Statement::from_string(DatabaseBackend::Postgres, "SELECT version()".into())
//             .query_one(db)
//             .await?
//             .unwrap();
//         row.try_get("", "version")?
//     });
//     println!("✅ PostgreSQL version: {version}");
//
//     // lifeguard_go!
//     lifeguard_go!(pool, inserted_id, {
//         let model = owners::ActiveModel {
//             name: Set("Macro Bob".into()),
//             phone: Set(Some("123456".into())),
//             ..Default::default()
//         };
//         let res = owners::Entity::insert(model).exec(db).await?;
//         Ok::<_, DbErr>(res.last_insert_id)
//     });
//     println!("👤 Inserted owner with id: {inserted_id}");
//
//     // lifeguard_query!
//     let all_owners: Vec<owners::Model> = lifeguard_query!(pool, { owners::Entity::find().all(db) });
//     println!("📋 Total owners: {}", all_owners.len());
//
//     // lifeguard_insert_many!
//     let models = vec![
//         pets::ActiveModel {
//             name: Set("Spike".into()),
//             species: Set("Dog".into()),
//             owner_id: Set(inserted_id),
//             ..Default::default()
//         },
//         pets::ActiveModel {
//             name: Set("Whiskers".into()),
//             species: Set("Cat".into()),
//             owner_id: Set(inserted_id),
//             ..Default::default()
//         },
//     ];
//     let last_pet_id: i32 = lifeguard_insert_many!(pool, pets::Entity, models);
//     println!("🐾 Last inserted pet ID: {last_pet_id}");
//
//     // lifeguard_txn!
//     let Ok((id1, id2)) = lifeguard_txn!(pool, {
//         let o1 = owners::ActiveModel {
//             name: Set("Txn One".into()),
//             phone: Set(Some("000".into())),
//             ..Default::default()
//         };
//         let o2 = owners::ActiveModel {
//             name: Set("Txn Two".into()),
//             phone: Set(Some("111".into())),
//             ..Default::default()
//         };
//         let r1 = owners::Entity::insert(o1).exec(&txn).await?.last_insert_id;
//         let r2 = owners::Entity::insert(o2).exec(&txn).await?.last_insert_id;
//         Ok((r1, r2))
//     });
//     println!("✅ Inserted in transaction: {id1}, {id2}");
//     Ok(())
// }
