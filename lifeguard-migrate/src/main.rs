//! Lifeguard Migration CLI Tool
//!
//! Command-line interface for managing database migrations in Lifeguard applications.
//! Supports both CLI execution and integration with CI/CD pipelines.

use clap::{Parser, Subcommand};
use lifeguard::{connect, MayPostgresExecutor, LifeExecutor};
use lifeguard::migration::{Migrator, MigrationError};
use std::path::PathBuf;
use std::process;

#[derive(Parser)]
#[command(name = "lifeguard-migrate")]
#[command(about = "Migration management tool for Lifeguard ORM")]
#[command(version = "0.1.0")]
struct Cli {
    /// Database connection URL
    #[arg(long)]
    database_url: Option<String>,
    
    /// Migrations directory path
    #[arg(long, default_value = "migrations")]
    migrations_dir: PathBuf,
    
    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
    
    /// Quiet output (errors only)
    #[arg(short, long)]
    quiet: bool,
    
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show migration status (applied vs pending)
    Status,
    
    /// Apply pending migrations
    Up {
        /// Number of migrations to apply (default: all pending)
        #[arg(long)]
        steps: Option<usize>,
        
        /// Dry run - show what would be executed without running
        #[arg(long)]
        dry_run: bool,
    },
    
    /// Rollback migrations
    Down {
        /// Number of migrations to rollback (default: 1)
        #[arg(long, default_value = "1")]
        steps: usize,
        
        /// Dry run - show what would be rolled back
        #[arg(long)]
        dry_run: bool,
    },
    
    /// Validate checksums of applied migrations
    Validate,
    
    /// Generate a new migration file
    Generate {
        /// Migration name (e.g., "create_users_table")
        name: String,
    },
    
    /// Show detailed migration information
    Info {
        /// Show information for a specific migration version
        #[arg(long)]
        version: Option<i64>,
    },
}

fn main() {
    let cli = Cli::parse();
    
    // Initialize logging
    if cli.quiet {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("error")).init();
    } else if cli.verbose {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    } else {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    }
    
    // Get database URL
    let database_url = cli.database_url
        .or_else(|| std::env::var("LIFEGUARD_DATABASE_URL").ok())
        .or_else(|| std::env::var("DATABASE_URL").ok())
        .ok_or_else(|| {
            eprintln!("Error: Database URL not provided. Use --database-url or set LIFEGUARD_DATABASE_URL or DATABASE_URL environment variable.");
            process::exit(1);
        })
        .unwrap();
    
    // Connect to database
    let client = match connect(&database_url) {
        Ok(client) => client,
        Err(e) => {
            eprintln!("Error connecting to database: {}", e);
            process::exit(1);
        }
    };
    
    let executor: Box<dyn LifeExecutor> = Box::new(MayPostgresExecutor::new(client));
    let migrator = Migrator::new(&cli.migrations_dir);
    
    // Execute command
    let result = match cli.command {
        Commands::Status => handle_status(&migrator, executor.as_ref()),
        Commands::Up { steps, dry_run } => handle_up(&migrator, executor, steps, dry_run),
        Commands::Down { steps, dry_run } => handle_down(&migrator, executor, steps, dry_run),
        Commands::Validate => handle_validate(&migrator, executor.as_ref()),
        Commands::Generate { name } => handle_generate(&cli.migrations_dir, &name),
        Commands::Info { version } => handle_info(&migrator, executor.as_ref(), version),
    };
    
    match result {
        Ok(()) => {
            if !cli.quiet {
                println!("✅ Success");
            }
            process::exit(0);
        }
        Err(e) => {
            eprintln!("❌ Error: {}", e);
            process::exit(1);
        }
    }
}

fn handle_status(migrator: &Migrator, executor: &dyn LifeExecutor) -> Result<(), MigrationError> {
    let status = migrator.status(executor)?;
    
    println!("\n📊 Migration Status\n");
    
    if !status.applied.is_empty() {
        println!("✅ Applied Migrations ({}):", status.applied_count);
        for record in &status.applied {
            let time_str = if let Some(ms) = record.execution_time_ms {
                format!("{}ms", ms)
            } else {
                "N/A".to_string()
            };
            println!("  ✓ m{}_{} ({}, {})", 
                record.version, 
                record.name,
                record.applied_at.format("%Y-%m-%d %H:%M:%S"),
                time_str
            );
        }
    } else {
        println!("✅ Applied Migrations: None");
    }
    
    println!();
    
    if !status.pending.is_empty() {
        println!("⏳ Pending Migrations ({}):", status.pending_count);
        for pending in &status.pending {
            println!("  ⏳ m{}_{} (pending)", pending.version, pending.name);
        }
    } else {
        println!("⏳ Pending Migrations: None");
    }
    
    println!("\n📈 Summary: {} applied, {} pending", status.applied_count, status.pending_count);
    
    Ok(())
}

fn handle_up(
    migrator: &Migrator,
    executor: Box<dyn LifeExecutor>,
    steps: Option<usize>,
    dry_run: bool,
) -> Result<(), MigrationError> {
    if dry_run {
        let status = migrator.status(executor.as_ref())?;
        if status.pending.is_empty() {
            println!("No pending migrations to apply");
            return Ok(());
        }
        
        let to_apply = steps.unwrap_or(status.pending.len());
        println!("Would apply {} migration(s):", to_apply);
        for (i, pending) in status.pending.iter().take(to_apply).enumerate() {
            println!("  {}. m{}_{}", i + 1, pending.version, pending.name);
        }
        return Ok(());
    }
    
    println!("Applying migrations...");
    let applied = migrator.up(executor, steps)?;
    
    if applied > 0 {
        println!("✅ Successfully applied {} migration(s)", applied);
    } else {
        println!("✅ No migrations to apply");
    }
    
    Ok(())
}

fn handle_down(
    migrator: &Migrator,
    executor: Box<dyn LifeExecutor>,
    steps: usize,
    dry_run: bool,
) -> Result<(), MigrationError> {
    if dry_run {
        let status = migrator.status(executor.as_ref())?;
        if status.applied.is_empty() {
            println!("No applied migrations to rollback");
            return Ok(());
        }
        
        let mut applied = status.applied;
        applied.sort_by_key(|m| std::cmp::Reverse(m.version));
        
        let to_rollback = steps.min(applied.len());
        println!("Would rollback {} migration(s):", to_rollback);
        for (i, record) in applied.iter().take(to_rollback).enumerate() {
            println!("  {}. m{}_{}", i + 1, record.version, record.name);
        }
        return Ok(());
    }
    
    println!("Rolling back migrations...");
    let rolled_back = migrator.down(executor, Some(steps))?;
    
    if rolled_back > 0 {
        println!("✅ Successfully rolled back {} migration(s)", rolled_back);
    } else {
        println!("✅ No migrations to rollback");
    }
    
    Ok(())
}

fn handle_validate(migrator: &Migrator, executor: &dyn LifeExecutor) -> Result<(), MigrationError> {
    println!("Validating checksums...");
    migrator.validate_checksums(executor)?;
    println!("✅ All checksums valid");
    Ok(())
}

fn handle_generate(migrations_dir: &PathBuf, name: &str) -> Result<(), MigrationError> {
    use std::fs;
    use chrono::Utc;
    
    // Generate timestamp
    let now = Utc::now();
    let timestamp_str = now.format("%Y%m%d%H%M%S").to_string();
    let timestamp_num: i64 = timestamp_str.parse().unwrap_or(0);
    
    // Create migrations directory if it doesn't exist
    fs::create_dir_all(migrations_dir)
        .map_err(|e| MigrationError::FileNotFound(format!("Failed to create migrations directory: {}", e)))?;
    
    // Generate migration file name
    let filename = format!("m{}_{}.rs", timestamp_str, name);
    let filepath = migrations_dir.join(&filename);
    
    // Generate migration template
    let generated_time = now.format("%Y-%m-%d %H:%M:%S UTC").to_string();
    let template = format!(
        r#"//! Migration: {}
//! Version: {}
//! Generated: {}

use lifeguard::{{LifeError, migration::{{Migration, SchemaManager}}}};
use sea_query::{{Table, ColumnDef}};

pub struct Migration;

impl Migration for Migration {{
    fn name(&self) -> &str {{
        "{}"
    }}
    
    fn version(&self) -> i64 {{
        {}
    }}
    
    fn up(&self, manager: &SchemaManager) -> Result<(), LifeError> {{
        // TODO: Implement migration logic
        // Example:
        // let table = Table::create()
        //     .table("example")
        //     .col(ColumnDef::new("id").integer().not_null().primary_key())
        //     .to_owned();
        // manager.create_table(table)?;
        Ok(())
    }}
    
    fn down(&self, manager: &SchemaManager) -> Result<(), LifeError> {{
        // TODO: Implement rollback logic
        // Example:
        // let table = Table::drop().table("example").to_owned();
        // manager.drop_table(table)?;
        Ok(())
    }}
}}
"#,
        name, timestamp_str, generated_time, name, timestamp_num
    );
    
    // Write migration file
    fs::write(&filepath, template)
        .map_err(|e| MigrationError::FileNotFound(format!("Failed to write migration file: {}", e)))?;
    
    println!("✅ Generated migration: {}", filepath.display());
    println!("   Edit the file to implement up() and down() methods");
    
    Ok(())
}

fn handle_info(migrator: &Migrator, executor: &dyn LifeExecutor, version: Option<i64>) -> Result<(), MigrationError> {
    let status = migrator.status(executor)?;
    
    if let Some(version) = version {
        // Show info for specific migration
        if let Some(record) = status.applied.iter().find(|r| r.version == version) {
            println!("\n📋 Migration Information\n");
            println!("Version: {}", record.version);
            println!("Name: {}", record.name);
            println!("Checksum: {}", record.checksum);
            println!("Applied At: {}", record.applied_at.format("%Y-%m-%d %H:%M:%S UTC"));
            if let Some(ms) = record.execution_time_ms {
                println!("Execution Time: {}ms", ms);
            }
            println!("Success: {}", record.success);
        } else if let Some(pending) = status.pending.iter().find(|p| p.version == version) {
            println!("\n📋 Migration Information (Pending)\n");
            println!("Version: {}", pending.version);
            println!("Name: {}", pending.name);
            println!("Checksum: {}", pending.checksum);
            println!("Status: Pending");
            println!("Path: {}", pending.path.display());
        } else {
            return Err(MigrationError::InvalidVersion(version));
        }
    } else {
        // Show summary info
        println!("\n📋 Migration System Information\n");
        println!("Total Migrations: {}", status.total);
        println!("Applied: {}", status.applied_count);
        println!("Pending: {}", status.pending_count);
        
        if let Some(latest) = status.latest_applied_version() {
            println!("Latest Applied Version: {}", latest);
        }
        
        if let Some(next) = status.next_pending_version() {
            println!("Next Pending Version: {}", next);
        }
    }
    
    Ok(())
}
