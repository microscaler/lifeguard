use fake::{faker::name::en::*, faker::phone_number::en::*, Fake};
use lifeguard::pool::config::DatabaseConfig;
use lifeguard::DbPoolManager;
use rand::seq::SliceRandom;
use sea_orm::*;
use std::env;
use std::time::Instant;
mod animal;
mod entity;
pub use animal::{Animal, PetName};
#[allow(unused_imports)]
use entity::{
    appointments, appointments::Entity as Appointment, owners, owners::Entity as Owner, pets,
    pets::Entity as Pet,
};

#[derive(Clone)]
struct OwnerInput {
    name: String,
    phone: Option<String>,
}

#[derive(Clone)]
struct PetInput {
    name: String,
    species: String,
    owner_index: usize,
}

#[derive(Clone)]
struct AppointmentInput {
    pet_index: usize,
    notes: String,
}

// fn generate_fake_data(n: usize) -> (Vec<OwnerInput>, Vec<PetInput>, Vec<AppointmentInput>) {
//     let mut owners = Vec::with_capacity(n);
//     let mut pets = Vec::new();
//     let mut appts = Vec::new();
//     let species = ["Dog", "Cat", "Fish", "Rabbit", "Lizard"];
//
//     for i in 0..n {
//         owners.push(OwnerInput {
//             name: Name().fake(),
//             phone: Some(PhoneNumber().fake()),
//         });
//
//         for _ in 0..(i % 3 + 1) {
//             pets.push(PetInput {
//                 name: PetName.fake(),
//                 species: species.choose(&mut rand::thread_rng()).unwrap().to_string(),
//                 owner_index: i,
//             });
//         }
//     }
//
//     for (i, _) in pets.iter().enumerate() {
//         appts.push(AppointmentInput {
//             pet_index: i,
//             notes: format!("Checkup for {}", Animal.fake::<String>()),
//         });
//     }
//
//     (owners, pets, appts)
// }
//
// fn main() {
//     // let db_url = std::env::var("DATABASE_URL")
//     //     .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());
//     // let pool = DbPoolManager::new(&db_url, 10).expect("Pool init failed");
//     let cfg = DatabaseConfig::default();
//     let pool = DbPoolManager::from_config(&cfg).unwrap();
//
//     let args: Vec<String> = env::args().collect();
//     let total = args
//         .iter()
//         .position(|a| a == "--")
//         .and_then(|i| args.get(i + 1))
//         .and_then(|v| v.parse::<usize>().ok())
//         .unwrap_or(10000);
//
//     let batch_size = args
//         .iter()
//         .position(|a| a == "--batch-size")
//         .and_then(|i| args.get(i + 1))
//         .and_then(|v| v.parse::<usize>().ok())
//         .unwrap_or(500);
//
//     let (owners_all, pets_all, appts_all) = generate_fake_data(total);
//     let mut offset = 0;
//     let mut pet_offset = 0;
//
//     let start = Instant::now();
//
//     while offset < owners_all.len() {
//         let owners_chunk = &owners_all[offset..(offset + batch_size).min(owners_all.len())];
//
//         let owner_ids: Vec<i32> = pool
//             .execute(|db| {
//                 Box::pin(async move {
//                     let models: Vec<_> = owners_chunk
//                         .iter()
//                         .map(|o| owners::ActiveModel {
//                             name: Set(o.name.clone()),
//                             phone: Set(o.phone.clone()),
//                             ..Default::default()
//                         })
//                         .collect();
//
//                     let res = owners::Entity::insert_many(models).exec(db).await?;
//                     Ok::<_, DbErr>(res.last_insert_id)
//                 })
//             })
//             .unwrap();
//
//         let pets_chunk = &pets_all[pet_offset..pet_offset + owner_ids.len()];
//         let pet_ids: Vec<i32> = pool
//             .execute(|db| {
//                 Box::pin(async move {
//                     let models: Vec<_> = pets_chunk
//                         .iter()
//                         .enumerate()
//                         .map(|(i, p)| pets::ActiveModel {
//                             name: Set(p.name.clone()),
//                             species: Set(p.species.clone()),
//                             owner_id: Set(Option::from(
//                                 owner_ids.get(p.owner_index).copied().unwrap_or(1),
//                             )),
//                             ..Default::default()
//                         })
//                         .collect();
//
//                     let res = pets::Entity::insert_many(models).exec(db).await?;
//                     Ok::<_, DbErr>(res.last_insert_id)
//                 })
//             })
//             .unwrap();
//
//         let appt_chunk = &appts_all[pet_offset..pet_offset + pet_ids.len()];
//         pool.execute(|db| {
//             Box::pin(async move {
//                 let models: Vec<_> = appt_chunk
//                     .iter()
//                     .enumerate()
//                     .map(|(i, a)| appointments::ActiveModel {
//                         pet_id: Set(Some(pet_ids[i])),
//                         date: Set(chrono::Utc::now().naive_utc()),
//                         notes: Set(Some(a.notes.clone())),
//                         ..Default::default()
//                     })
//                     .collect();
//
//                 let _ = appointments::Entity::insert_many(models).exec(&db).await?;
//                 Ok::<_, DbErr>(())
//             })
//         })
//         .unwrap();
//
//         offset += batch_size;
//         pet_offset += pet_ids.len();
//     }
//
//     let elapsed = start.elapsed();
//     println!(
//         "✅ Seeded in {:.2?} ({:.2} rows/sec)",
//         elapsed,
//         (owners_all.len() + pets_all.len() + appts_all.len()) as f64 / elapsed.as_secs_f64()
//     );
// }
