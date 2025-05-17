use lifeguard::DbPoolManager;
use may::go;
use sea_orm::*;
use std::thread::sleep;
use std::time::Duration;

mod entity;
use entity::{owners, owners::Entity as Owner};
use lifeguard::pool::config::DatabaseConfig;

// fn main() {
//     // let db_url = std::env::var("DATABASE_URL")
//     //     .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());
//     // let pool = DbPoolManager::new(&db_url, 4).expect("Pool init failed");
//     let cfg = DatabaseConfig::default();
//     let pool = DbPoolManager::from_config(&cfg).unwrap();
//
//     for i in 0..5 {
//         let pool = pool.clone();
//         go!(move || {
//             let result = pool.execute(move |db| {
//                 Box::pin(async move {
//                     let model = owners::ActiveModel {
//                         name: Set(format!("Owner {}", i)),
//                         phone: Set(Some(format!("555-000{}", i))),
//                         ..Default::default()
//                     };
//                     let insert = Owner::insert(model).exec(db).await?;
//                     Ok::<_, DbErr>(insert.last_insert_id)
//                 })
//             });
//
//             match result {
//                 Ok(id) => println!("[{}] Created owner ID: {}", i, id),
//                 Err(e) => eprintln!("[{}] Failed: {:?}", i, e),
//             }
//         });
//     }
//
//     sleep(Duration::from_secs(2));
// }
