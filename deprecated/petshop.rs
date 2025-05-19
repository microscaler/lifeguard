use lifeguard::DbPoolManager;
use may::go;
use sea_orm::entity::*;
use sea_orm::*;
use std::time::Duration;

mod entity;
use entity::{
    appointments, appointments::Entity as Appointment, owners, owners::Entity as Owner, pets,
    pets::Entity as Pet,
};
use lifeguard::pool::config::DatabaseConfig;

// fn main() {
//     // let db_url = "postgres://postgres:postgres@localhost:5432/postgres";
//     // let pool = DbPoolManager::new(db_url, 5).expect("Failed to create pool");
//     let cfg = DatabaseConfig::default();
//     let pool = DbPoolManager::from_config(&cfg).unwrap();
//
//     // Create owner
//     go!(move || {
//         let result = pool.execute(|db| {
//             Box::pin(async move {
//                 let new_owner = owners::ActiveModel {
//                     name: Set("Bob".to_owned()),
//                     phone: Set(Some("123-456".to_owned())),
//                     ..Default::default()
//                 };
//
//                 let owner_res = Owner::insert(new_owner).exec(&db).await?;
//                 Ok::<_, DbErr>(owner_res.last_insert_id)
//             })
//         });
//
//         match result {
//             Ok(owner_id) => {
//                 println!("Created owner with ID: {:?}", owner_id);
//
//                 // Now create a pet for the owner
//                 let pool = pool.clone();
//                 go!(move || {
//                     let pet_result = pool.execute(move |db| {
//                         Box::pin(async move {
//                             let new_pet = pets::ActiveModel {
//                                 name: Set("Whiskers".to_owned()),
//                                 species: Set("Cat".to_owned()),
//                                 owner_id: Set(owner_id),
//                                 ..Default::default()
//                             };
//
//                             let pet_res = Pet::insert(new_pet).exec(db).await?;
//                             Ok::<_, DbErr>(pet_res.last_insert_id)
//                         })
//                     });
//
//                     match pet_result {
//                         Ok(pet_id) => {
//                             println!("Created pet with ID: {:?}", pet_id);
//
//                             // Create appointment
//                             let pool = pool.clone();
//                             go!(move || {
//                                 let appointment_result = pool.execute(move |db| {
//                                     Box::pin(async move {
//                                         let new_appointment = appointments::ActiveModel {
//                                             pet_id: Set(pet_id),
//                                             date: Set(chrono::Utc::now().naive_utc()),
//                                             notes: Set(Some("Yearly check-up".to_owned())),
//                                             ..Default::default()
//                                         };
//
//                                         let _ =
//                                             Appointment::insert(new_appointment).exec(db).await?;
//                                         Ok::<_, DbErr>(())
//                                     })
//                                 });
//
//                                 if let Err(e) = appointment_result {
//                                     eprintln!("Error creating appointment: {:?}", e);
//                                 } else {
//                                     println!("Appointment created for pet ID: {:?}", pet_id);
//                                 }
//                             });
//                         }
//                         Err(e) => eprintln!("Error creating pet: {:?}", e),
//                     }
//                 });
//             }
//             Err(e) => eprintln!("Error creating owner: {:?}", e),
//         }
//     });
//
//     // Query and join all appointments with pet and owner info
//     go!(move || {
//         let result = pool.execute(|db| {
//             Box::pin(async move {
//                 let appointments = Appointment::find()
//                     .find_also_related(Pet)
//                     .find_also_related(Owner)
//                     .all(db)
//                     .await?;
//
//                 Ok::<_, DbErr>(appointments)
//             })
//         });
//
//         match result {
//             Ok(appointments) => {
//                 println!("--- Appointments Report ---");
//                 for ((appt, pet_opt), owner_opt) in appointments {
//                     let pet_name = pet_opt.map(|p| p.name).unwrap_or("<Unknown Pet>".into());
//                     let owner_name = owner_opt
//                         .map(|o| o.name)
//                         .unwrap_or("<Unknown Owner>".into());
//                     println!(
//                         "{} with {} (owner: {})",
//                         appt.date.format("%Y-%m-%d %H:%M"),
//                         pet_name,
//                         owner_name
//                     );
//                 }
//             }
//             Err(e) => eprintln!("Query error: {:?}", e),
//         }
//     });
//
//     std::thread::sleep(Duration::from_secs(4));
// }
