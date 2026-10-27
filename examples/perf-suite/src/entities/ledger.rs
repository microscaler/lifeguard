//! Tenant-scoped synthetic row behind a row-level-security policy (`perf_ledger`).

use lifeguard_derive::{LifeModel, LifeRecord};

#[derive(Debug, Clone, LifeModel, LifeRecord)]
#[table_name = "perf_ledger"]
pub struct Ledger {
    #[primary_key]
    pub id: i64,
    pub tenant_id: String,
    pub amount: f64,
    pub memo: String,
}
