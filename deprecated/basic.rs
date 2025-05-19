use lifeguard::pool::config::DatabaseConfig;
use lifeguard::DbPoolManager;
use may::go;
use sea_orm::entity::prelude::*;
use std::time::Duration;
mod entity;

// #[tokio::main]
// async fn main() {
//     // Example assumes an accessible Postgres DB
//     // let db_url = "postgres://postgres:postgres@localhost:5432/postgres";
//     // let pool = DbPoolManager::new(db_url, 5).expect("Failed to create pool");
//     let cfg = DatabaseConfig::default();
//     let pool = DbPoolManager::from_config(&cfg).unwrap();
//
//     // Spawn multiple coroutines doing DB work
//     for i in 0..5 {
//         let pool = pool.clone();
//         go!(move || {
//             let result = pool.execute(move |db| {
//                 Box::pin(async move {
//                     // Replace with a real SeaORM entity query
//                     println!("[{}] Executing fake DB task...", i);
//                     tokio::time::sleep(Duration::from_secs(1)).await;
//                     Ok::<_, sea_orm::DbErr>(format!("Hello from coroutine {}", i))
//                 })
//             });
//
//             match result {
//                 Ok(msg) => println!("[{}] Got: {}", i, msg),
//                 Err(e) => eprintln!("[{}] Error: {:?}", i, e),
//             }
//         });
//     }
//
//     // Give coroutines time to run
//     std::thread::sleep(Duration::from_secs(3));
// }
