//! Tenant row (organization / tenant scope).

use lifeguard_derive::{LifeModel, LifeRecord};

#[derive(Debug, Clone, LifeModel, LifeRecord)]
#[table_name = "perf_tenants"]
#[table_comment = "Perf harness: tenant scope for IDAM-shaped lookups"]
pub struct PerfTenant {
    #[primary_key]
    pub id: uuid::Uuid,

    #[column_type = "VARCHAR(255)"]
    pub name: String,
}
