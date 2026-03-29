//! Session row: indexed token fingerprint lookup and `last_seen` updates.

use lifeguard_derive::{LifeModel, LifeRecord};

#[derive(Debug, Clone, LifeModel, LifeRecord)]
#[table_name = "perf_sessions"]
#[table_comment = "Perf harness: session validation by token fingerprint"]
pub struct PerfSession {
    #[primary_key]
    pub id: uuid::Uuid,

    #[foreign_key = "perf_users(id) ON DELETE CASCADE"]
    #[indexed]
    pub user_id: uuid::Uuid,

    #[unique]
    #[indexed]
    #[column_type = "VARCHAR(128)"]
    pub token_fingerprint: String,

    pub expires_at: chrono::NaiveDateTime,

    #[default_expr = "CURRENT_TIMESTAMP"]
    pub last_seen_at: chrono::NaiveDateTime,
}
