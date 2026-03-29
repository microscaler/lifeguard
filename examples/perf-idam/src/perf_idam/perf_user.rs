//! User row: PK fetch and composite (tenant_id, email) resolution.

use lifeguard_derive::{LifeModel, LifeRecord};

#[derive(Debug, Clone, LifeModel, LifeRecord)]
#[table_name = "perf_users"]
#[table_comment = "Perf harness: users scoped to tenant (composite unique email)"]
#[composite_unique = "tenant_id, email"]
pub struct PerfUser {
    #[primary_key]
    pub id: uuid::Uuid,

    #[foreign_key = "perf_tenants(id) ON DELETE CASCADE"]
    #[indexed]
    pub tenant_id: uuid::Uuid,

    #[indexed]
    pub email: String,

    #[column_type = "VARCHAR(255)"]
    pub display_name: String,

    #[default_expr = "CURRENT_TIMESTAMP"]
    pub created_at: chrono::NaiveDateTime,
}
