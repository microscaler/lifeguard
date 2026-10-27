//! Narrow synthetic row (`perf_items`).

use lifeguard_derive::{LifeModel, LifeRecord};

#[derive(Debug, Clone, LifeModel, LifeRecord)]
#[table_name = "perf_items"]
pub struct Item {
    #[primary_key]
    pub id: i64,
    pub account_id: i64,
    pub name: String,
    pub qty: i32,
    pub price: f64,
    pub active: bool,
    pub updated_at: chrono::NaiveDateTime,
}
