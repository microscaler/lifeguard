//! Test SQL generation from entities
//!
//! This example demonstrates generating SQL from Lifeguard entities and comparing
//! with the original SQL migrations.

// Note: This example is disabled because:
// 1. sql_generator is in lifeguard-migrate and examples don't have access to it
// 2. Entities have been moved to examples/entities/ which is a separate crate
// Use lifeguard-migrate tests instead.

use std::fs;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Testing SQL Generation from Entities\n");
    
    // Test ChartOfAccount
    println!("📋 Testing ChartOfAccount entity...");
    test_chart_of_accounts()?;
    
    // Test Account
    println!("\n📋 Testing Account entity...");
    test_account()?;
    
    // Test JournalEntry
    println!("\n📋 Testing JournalEntry entity...");
    test_journal_entry()?;
    
    println!("\n✅ All tests completed!");
    Ok(())
}

fn test_chart_of_accounts() -> Result<(), Box<dyn std::error::Error>> {
    // Note: SQL generation is disabled in examples - use lifeguard-migrate tests instead
    println!("⚠️  SQL generation test disabled in examples - use lifeguard-migrate tests");
    println!("   This example cannot access entities or sql_generator");
    Ok(())
}

fn test_account() -> Result<(), Box<dyn std::error::Error>> {
    println!("⚠️  SQL generation test disabled in examples - use lifeguard-migrate tests");
    Ok(())
}

fn test_journal_entry() -> Result<(), Box<dyn std::error::Error>> {
    println!("⚠️  SQL generation test disabled in examples - use lifeguard-migrate tests");
    Ok(())
}

fn load_original_sql(filename: &str) -> Result<String, Box<dyn std::error::Error>> {
    let path = PathBuf::from("migrations/original").join(filename);
    Ok(fs::read_to_string(&path)?)
}

fn extract_table_sql(sql: &str, table_name: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Extract CREATE TABLE statement for the specified table
    let start_marker = format!("CREATE TABLE IF NOT EXISTS {}", table_name);
    let end_marker = ");";
    
    if let Some(start) = sql.find(&start_marker) {
        if let Some(end) = sql[start..].find(end_marker) {
            let table_sql = &sql[start..start + end + end_marker.len()];
            
            // Also extract indexes and comments for this table
            let mut result = table_sql.to_string();
            result.push('\n');
            
            // Extract indexes
            for line in sql.lines() {
                if line.contains(&format!("ON {}", table_name)) {
                    result.push_str(line);
                    result.push('\n');
                }
            }
            
            // Extract comment
            for line in sql.lines() {
                if line.contains(&format!("COMMENT ON TABLE {}", table_name)) {
                    result.push_str(line);
                    result.push('\n');
                }
            }
            
            return Ok(result);
        }
    }
    
    Err(format!("Could not find table definition for {}", table_name).into())
}

fn normalize_sql(sql: &str) -> String {
    sql.lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty() && !line.starts_with("--"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn compare_sql(generated: &str, original: &str) {
    let gen_lines: Vec<&str> = generated.lines().collect();
    let orig_lines: Vec<&str> = original.lines().collect();
    
    let max_len = gen_lines.len().max(orig_lines.len());
    
    for i in 0..max_len {
        let gen_line = gen_lines.get(i).copied().unwrap_or("");
        let orig_line = orig_lines.get(i).copied().unwrap_or("");
        
        if gen_line != orig_line {
            println!("  Line {}:", i + 1);
            if !gen_line.is_empty() {
                println!("    Generated: {}", gen_line);
            }
            if !orig_line.is_empty() {
                println!("    Original:  {}", orig_line);
            }
        }
    }
}
