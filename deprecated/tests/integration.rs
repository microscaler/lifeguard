use lifeguard::DbPoolManager;
use may::go;
use sea_orm::entity::*;
use sea_orm::*;
use std::time::Duration;

use entity::{
    appointments, appointments::Entity as Appointment, owners, owners::Entity as Owner, pets,
    pets::Entity as Pet,
};

// #[test]
// fn test_lifeguard_petshop_flow() {
//     // Set up test database URL
//     let db_url = std::env::var("DATABASE_URL")
//         .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());
//
//     let pool = DbPoolManager::new().expect("Failed to create pool");
//
//     // Insert owner -> pet -> appointment in coroutine
//     let mut owner_id = 0;
//     let mut pet_id = 0;
//
//     go!(|| {
//         let result = pool.execute(|db| {
//             Box::pin(async move {
//                 let model = owners::ActiveModel {
//                     name: Set("TestOwner".to_string()),
//                     phone: Set(Some("000-000".to_string())),
//                     ..Default::default()
//                 };
//                 let insert = Owner::insert(model).exec(&db).await?;
//                 Ok::<_, DbErr>(insert.last_insert_id)
//             })
//         });
//         owner_id = result.unwrap();
//     });
//
//     go!(|| {
//         let result = pool.execute(|db| {
//             Box::pin(async move {
//                 let model = pets::ActiveModel {
//                     name: Set("TestPet".to_string()),
//                     species: Set("Turtle".to_string()),
//                     owner_id: Set(Some(owner_id)),
//                     ..Default::default()
//                 };
//                 let insert = Pet::insert(model).exec(&db).await?;
//                 Ok::<_, DbErr>(insert.last_insert_id)
//             })
//         });
//         pet_id = result.unwrap();
//     });
//
//     go!(|| {
//         let result = pool.execute(|db| {
//             Box::pin(async move {
//                 let model = appointments::ActiveModel {
//                     pet_id: Set(Some(pet_id)),
//                     date: Set(chrono::Utc::now().naive_utc()),
//                     notes: Set(Some("Shell check-up".into())),
//                     ..Default::default()
//                 };
//                 let insert = Appointment::insert(model).exec(&db).await?;
//                 Ok::<_, DbErr>(insert.last_insert_id)
//             })
//         });
//
//         assert!(result.unwrap() > 0);
//     });
//
//     // Verify join query
//     go!(|| {
//         let result = pool.execute(|db| {
//             Box::pin(async move {
//                 let rows = Appointment::find()
//                     .find_also_related(Pet)
//                     .find_also_related(Owner)
//                     .all(db)
//                     .await?;
//                 Ok::<_, DbErr>(rows)
//             })
//         });
//
//         let rows = result.unwrap();
//         assert!(rows.len() > 0);
//         let ((_, Some(pet)), Some(owner)) = &rows[0];
//         assert_eq!(pet.name, "TestPet");
//         assert_eq!(owner.name, "TestOwner");
//     });
//
//     std::thread::sleep(Duration::from_secs(2));
// }
