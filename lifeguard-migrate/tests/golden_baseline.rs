//! Golden baseline test for `sql_generator::generate_create_table_sql`.
//!
//! Proves that migration SQL generation is deterministic and reproducible.
//! This test uses a simple, known-good entity and asserts the full SQL output
//! matches a hardcoded expected string.
//!
//! **Why this matters for RLS:** When we later add `#[rls_enabled]` or similar
//! entity attributes, the derive macros and sql_generator must not change the
//! output for entities that *don't* use RLS. This golden test is the baseline
//! guard against that regression.
//!
//! To refresh: update the `EXPECTED` constant and commit.

#![allow(warnings)]

use lifeguard_derive::LifeModel;
use lifeguard_migrate::sql_generator;

/// A minimal, well-known entity that produces deterministic SQL.
///
/// This entity has:
/// - A UUID primary key with `gen_random_uuid()` default
/// - A required text column with a comment
/// - A foreign key to a nonexistent table
/// - A unique+indexed column
/// - A nullable optional column
/// - A timestamp column with CURRENT_TIMESTAMP default
///
/// Note: the `#[derive(LifeModel)]` macro generates a separate `Entity`
/// unit struct with all required traits (`LifeEntityName`, `LifeModelTrait`,
/// `Default`, `table_definition()`).
#[derive(LifeModel)]
#[table_name = "golden_test_users"]
#[table_comment = "Golden baseline test users"]
#[index = "idx_gtu_email(email)"]
pub struct GoldenTestUser {
    #[primary_key]
    pub id: uuid::Uuid,

    #[column_type = "VARCHAR(255)"]
    #[comment = "The user's display name"]
    pub name: String,

    #[unique]
    #[indexed]
    #[column_type = "VARCHAR(255)"]
    pub email: String,

    pub bio: Option<String>,

    #[default_expr = "CURRENT_TIMESTAMP"]
    pub created_at: chrono::NaiveDateTime,

    #[foreign_key = "golden_test_organizations(id) ON DELETE SET NULL"]
    #[column_type = "UUID"]
    pub org_id: Option<uuid::Uuid>,
}

/// The expected SQL output for the golden entity.
///
/// This must match exactly what `generate_create_table_sql` produces.
/// Regenerate by running the test with the new expected output.
const EXPECTED: &str = r"CREATE TABLE IF NOT EXISTS golden_test_users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    email VARCHAR(255) NOT NULL UNIQUE,
    bio TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    org_id UUID REFERENCES golden_test_organizations(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_gtu_email ON golden_test_users(email);
COMMENT ON COLUMN golden_test_users.name IS 'The user''s display name';
COMMENT ON TABLE golden_test_users IS 'Golden baseline test users';";

#[test]
fn golden_baseline_create_table_sql_matches() {
    let sql = sql_generator::generate_create_table_sql::<Entity>(Entity::table_definition())
        .expect("should generate SQL for golden entity");

    assert_eq!(
        sql.trim(),
        EXPECTED.trim(),
        "Generated SQL does not match expected.\n\n--- EXPECTED ---\n{}\n\n--- ACTUAL ---\n{}",
        EXPECTED.trim(),
        sql.trim()
    );
}
