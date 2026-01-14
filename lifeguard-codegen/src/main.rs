//! Lifeguard Codegen - Entity code generation tool
//!
//! This tool generates Entity, Model, Column, and related code from entity definitions.
//! Unlike procedural macros, this generates actual Rust source files, avoiding
//! macro expansion ordering issues (like E0223).

use clap::{Parser, Subcommand};
use std::path::PathBuf;

// Re-export from library for binary
use lifeguard_codegen::{EntityDefinition, EntityWriter};
use lifeguard_codegen::parser::{parse_entities_from_dir, parse_entity_from_file};

#[derive(Parser)]
#[command(name = "lifeguard-codegen")]
#[command(about = "Generate Lifeguard ORM entity code", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate entity code from definitions
    Generate {
        /// Input file or directory containing entity definitions
        #[arg(short, long)]
        input: PathBuf,

        /// Output directory for generated code
        #[arg(short, long, default_value = "src/entities")]
        output: PathBuf,

        /// Format: expanded (default) or compact
        #[arg(short, long, default_value = "expanded")]
        format: String,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Generate {
            input,
            output,
            format,
        } => {
            generate_entities(&input, &output, &format)?;
        }
    }

    Ok(())
}

fn generate_entities(input: &PathBuf, output: &PathBuf, format: &str) -> anyhow::Result<()> {
    println!("🔧 Lifeguard Codegen");
    println!("📥 Input: {}", input.display());
    println!("📤 Output: {}", output.display());
    println!("📝 Format: {}", format);

    // Create output directory if it doesn't exist
    std::fs::create_dir_all(output)?;

    // Parse entity definitions
    let entities = if input.is_file() {
        vec![parse_entity_from_file(input)?]
    } else if input.is_dir() {
        parse_entities_from_dir(input)?
    } else {
        // Fallback to example if input doesn't exist
        println!("⚠️  Input not found, using example entity");
        vec![EntityDefinition::example()]
    };

    if entities.is_empty() {
        anyhow::bail!("No entities found in input");
    }

    let writer = EntityWriter::new();
    let expanded = format == "expanded";

    let entity_count = entities.len();

    // Generate code for each entity
    for entity in entities {
        let code = writer.generate_entity_code(&entity, expanded)?;

        // Write to output file
        let output_file = output.join(format!("{}.rs", entity.name.to_string().to_lowercase()));
        std::fs::write(&output_file, code)?;

        println!("✅ Generated: {}", output_file.display());
    }

    println!(
        "✨ Generated {} entit{}",
        entity_count,
        if entity_count == 1 { "y" } else { "ies" }
    );

    Ok(())
}
