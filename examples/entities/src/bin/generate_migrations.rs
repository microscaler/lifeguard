//! Generate SQL migrations from example entities
//!
//! Emits SQL under `migrations/generated/<service>/`. When a prior
//! `*_generated_from_entities.sql` exists in that folder, new runs emit **deltas**
//! (`ALTER TABLE ... ADD COLUMN IF NOT EXISTS`, new `CREATE INDEX IF NOT EXISTS`) instead
//! of duplicating full `CREATE TABLE` bodies for unchanged tables.

// Import modules, not structs, so we can access Entity nested structs
use example_entities::inventory::{category, inventory_item, product};

use lifeguard::LifeEntityName;
use lifeguard_migrate::generated_migration_diff;
use lifeguard_migrate::sql_dependency_order;
use lifeguard_migrate::sql_generator;
use std::fs;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Anchor to this crate so runs work regardless of process cwd (e.g. `cargo run` from repo root).
    let output_dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../migrations/generated");

    // Create output directory if it doesn't exist
    if !output_dir.exists() {
        fs::create_dir_all(&output_dir)?;
        println!("📁 Created output directory: {}", output_dir.display());
    }

    println!("🔨 Generating SQL migrations from entities...\n");

    // Group entities by service
    let mut sql_by_service: std::collections::HashMap<String, Vec<(String, String)>> =
        std::collections::HashMap::new();

    // Helper to generate SQL for each entity using its concrete Entity type
    // We'll call Entity::table_definition() directly on each Entity type

    // Inventory entities
    // Entity is a sibling struct in the same module, not nested inside the struct
    {
        type Entity = category::Entity;
        let entity = Entity::default();
        let table_name = entity.table_name();
        let table_def = Entity::table_definition();
        match sql_generator::generate_create_table_sql::<Entity>(table_def) {
            Ok(sql) => {
                sql_by_service
                    .entry("inventory".to_string())
                    .or_insert_with(Vec::new)
                    .push((table_name.to_string(), sql));
                println!("   ✓ Generated SQL for {}", table_name);
            }
            Err(e) => eprintln!("   ✗ Failed to generate SQL for {}: {}", table_name, e),
        }
    }

    {
        type Entity = product::Entity;
        let entity = Entity::default();
        let table_name = entity.table_name();
        let table_def = Entity::table_definition();
        match sql_generator::generate_create_table_sql::<Entity>(table_def) {
            Ok(sql) => {
                sql_by_service
                    .entry("inventory".to_string())
                    .or_insert_with(Vec::new)
                    .push((table_name.to_string(), sql));
                println!("   ✓ Generated SQL for {}", table_name);
            }
            Err(e) => eprintln!("   ✗ Failed to generate SQL for {}: {}", table_name, e),
        }
    }

    {
        type Entity = inventory_item::Entity;
        let entity = Entity::default();
        let table_name = entity.table_name();
        let table_def = Entity::table_definition();
        match sql_generator::generate_create_table_sql::<Entity>(table_def) {
            Ok(sql) => {
                sql_by_service
                    .entry("inventory".to_string())
                    .or_insert_with(Vec::new)
                    .push((table_name.to_string(), sql));
                println!("   ✓ Generated SQL for {}", table_name);
            }
            Err(e) => eprintln!("   ✗ Failed to generate SQL for {}: {}", table_name, e),
        }
    }

    // Write SQL files grouped by service
    let timestamp = chrono::Utc::now().format("%Y%m%d%H%M%S").to_string();

    for (service, tables) in sql_by_service {
        let service_dir = output_dir.join(&service);
        if !service_dir.exists() {
            fs::create_dir_all(&service_dir)?;
        }

        let prev_path = generated_migration_diff::find_latest_generated_migration(&service_dir);
        let prev_text = match prev_path
            .as_ref()
            .map(|p| fs::read_to_string(p))
            .transpose()
        {
            Ok(s) => s,
            Err(e) => {
                eprintln!(
                    "⚠️  Could not read previous migration in {}: {}",
                    service_dir.display(),
                    e
                );
                None
            }
        };

        let body = generated_migration_diff::build_service_migration_body(prev_text.as_deref(), &tables);

        // Ignore `-- Version` / `-- Generated` churn: if each table's SQL matches the latest file
        // sections, do not emit another timestamped copy.
        if let Some(prev) = prev_text.as_deref() {
            if generated_migration_diff::generated_tables_match_baseline(prev, &tables) {
                if !generated_migration_diff::service_migration_is_empty(&body) {
                    eprintln!(
                        "⚠️  Service `{}`: baseline table SQL matches entities but diff was non-empty; skipping new file (sanity).",
                        service
                    );
                }
                println!(
                    "   ◆ No schema changes for service `{}` — skipped new migration file (baseline matches entities).",
                    service
                );
                continue;
            }
        }

        if generated_migration_diff::service_migration_is_empty(&body) {
            println!(
                "   ◆ No schema changes for service `{}` — skipped new migration file (baseline matches entities).",
                service
            );
            continue;
        }

        let output_file = service_dir.join(format!("{}_generated_from_entities.sql", timestamp));

        let mut sql_content = String::new();
        sql_content.push_str("-- Migration: Generated from Lifeguard entities\n");
        sql_content.push_str(&format!("-- Service: {}\n", service));
        sql_content.push_str(&format!("-- Version: {}\n", timestamp));
        sql_content.push_str(&format!(
            "-- Generated: {}\n\n",
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        ));
        sql_content
            .push_str("-- This migration was automatically generated from entity definitions.\n");
        sql_content.push_str("-- DO NOT EDIT MANUALLY - regenerate from entities instead.\n\n");
        if prev_text.is_some() {
            sql_content.push_str(
                "-- Delta migration: ALTER / new tables vs latest *_generated_from_entities.sql in this directory.\n\n",
            );
        }
        sql_content.push_str(&body);

        fs::write(&output_file, sql_content)?;
        println!(
            "✅ Generated SQL migration for {}: {}",
            service,
            output_file.display()
        );
    }

    sql_dependency_order::write_apply_order_file(&output_dir).map_err(|e| {
        format!("FK-safe apply_order.txt: {e} (fix REFERENCES across migration files or merge SQL)")
    })?;
    println!(
        "\n✅ Wrote {}",
        output_dir.join("apply_order.txt").display()
    );

    println!("\n✅ Success - All migrations generated!");

    Ok(())
}
