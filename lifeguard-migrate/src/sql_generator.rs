//! SQL migration generator for entity-driven migrations.
//!
//! This module generates SQL from Lifeguard entity metadata for PostgreSQL.
//!
//! ## Idempotent / replay-safe DDL (bootstrap & `ON_ERROR_STOP` re-runs)
//!
//! - **`CREATE TABLE IF NOT EXISTS`** — skips if the table exists (already present).
//! - **`CREATE [UNIQUE] INDEX IF NOT EXISTS`** — skips if that index name exists.
//! - **Column `CHECK` constraints** — emitted as **`DROP CONSTRAINT IF EXISTS`** then
//!   **`ADD CONSTRAINT`** so re-applying the same file does not fail on duplicate constraint names.
//!   (PostgreSQL has no `ADD CONSTRAINT … IF NOT EXISTS` in core SQL.)
//!
//! ## Caveats (why not “IF NOT EXISTS everything”)
//!
//! - **`CREATE TABLE IF NOT EXISTS`** does **not** update an existing table: if the table was created
//!   from an older definition (missing columns), this statement is a no-op and **schema drift**
//!   remains. Evolving schema still needs **`ALTER TABLE … ADD COLUMN IF NOT EXISTS`** (deltas) or
//!   a proper migration ledger — not repeated full `CREATE TABLE` bodies alone.
//! - **`CREATE INDEX IF NOT EXISTS`** only matches on **index name**: an existing index with the same
//!   name but a **different definition** is left as-is; drift detection is a separate concern
//!   (e.g. `schema_migration_compare`).
//! - **Inline `UNIQUE` / composite `UNIQUE` / `REFERENCES` inside `CREATE TABLE`** are only applied
//!   when the table is first created; they are **not** re-evaluated when `IF NOT EXISTS` skips.
//! - **`COMMENT ON`** is naturally re-runnable (replaces the comment).

use lifeguard::{
    index_key_parts_coverage_columns, query::column::column_trait::ColumnDefHelper, ColumnTrait,
    LifeEntityName, LifeModelTrait, TableDefinition,
};
use sea_query::IdenStatic;
use std::fmt::Write;

/// Generate SQL CREATE TABLE statement from entity metadata
///
/// This function generates a complete SQL CREATE TABLE statement including:
/// - Column definitions with types, nullability, defaults
/// - Primary key constraints
/// - Foreign key constraints
/// - CHECK constraints (column and table level)
/// - Unique constraints (single and composite)
/// - Indexes
/// - Table comments
///
/// Foreign keys are generated as separate ALTER TABLE statements to allow
/// proper dependency ordering.
///
/// # Parameters
///
/// * `table_def` - Table-level metadata (composite unique, indexes, CHECK constraints, comments)
///   This should be obtained by calling `Entity::table_definition()` for the entity.
pub fn generate_create_table_sql<E>(table_def: TableDefinition) -> Result<String, String>
where
    E: LifeModelTrait + LifeEntityName + Default,
    E::Column: ColumnTrait + Copy + sea_query::IdenStatic + PartialEq,
{
    let entity = E::default();
    let table_name = entity.table_name();
    let schema_name = entity.schema_name();

    // Build full table name with schema if present
    let full_table_name = if let Some(schema) = schema_name {
        format!("{}.{}", schema, table_name)
    } else {
        table_name.to_string()
    };

    let mut sql = String::new();

    // Early exit for Views
    if table_def.is_view {
        let view_query = table_def
            .view_query
            .as_deref()
            .unwrap_or("SELECT 1; -- View Query Missing");
        writeln!(sql, "CREATE OR REPLACE VIEW {} AS", full_table_name)
            .map_err(|e| format!("Failed to write SQL: {}", e))?;
        writeln!(sql, "{};", view_query).map_err(|e| format!("Failed to write SQL: {}", e))?;

        // Return without columns, checks, index logic etc., since Views don't map to tables
        return Ok(sql);
    }

    // Generate CREATE TABLE statement
    writeln!(sql, "CREATE TABLE IF NOT EXISTS {} (", full_table_name)
        .map_err(|e| format!("Failed to write SQL: {}", e))?;

    // Get all columns
    let columns = E::all_columns();
    let mut column_defs = Vec::new();
    let mut primary_key_cols = Vec::new();
    let mut check_constraints = Vec::new();
    let mut single_column_indexes = Vec::new();
    let mut column_comments = Vec::new();

    // Process each column
    for col in columns {
        // Use column_def() (inherent method) instead of def() (trait method)
        // The macro generates column_def() with all the attribute metadata
        // The trait method def() has a default implementation that returns empty metadata
        let col_def = col.column_def();
        // Get column name - Column implements IdenStatic which has as_str()
        let col_name = col.as_str();

        // Build column definition
        let mut col_sql = format!("    {}", col_name);

        // Add column type
        if let Some(ref col_type) = col_def.column_type {
            col_sql.push_str(&format!(" {}", col_type));
        } else {
            // Infer type from Rust type (basic mapping)
            col_sql.push_str(" TEXT"); // Default fallback
        }

        // Check if this column is a primary key
        // Prioritize explicit #[primary_key] trait mapping, fall back to "id" heuristic or auto_increment
        let is_primary_key = col_def.primary_key || col_def.auto_increment || col_name == "id";
        if is_primary_key {
            primary_key_cols.push(col_name.to_string());
        }

        // Add nullability
        // For primary keys, omit NOT NULL (PostgreSQL allows it, and original doesn't have it)
        // For other columns, add NOT NULL if not nullable
        if is_primary_key {
            // Primary keys are implicitly NOT NULL, but we omit it to match original style
        } else if !col_def.nullable {
            col_sql.push_str(" NOT NULL");
        }
        // For nullable columns, we don't add explicit NULL (PostgreSQL default)

        // Add PRIMARY KEY constraint (before DEFAULT to match original style: "id UUID PRIMARY KEY DEFAULT ...")
        if is_primary_key {
            col_sql.push_str(" PRIMARY KEY");
        }

        // Add default value or expression.
        // Priority: explicit default_expr > explicit default_value > UUID primary key default
        //         > inferred zero-value default for NOT NULL non-FK numeric/boolean columns.
        //
        // The inferred-default branch is a safety net for `ALTER TABLE ADD COLUMN IF NOT EXISTS`
        // deltas that the consumer migrator emits from the same SQL body: PostgreSQL refuses to
        // add a NOT NULL column without a DEFAULT to a non-empty table, and hand-written seeds /
        // hand-written bulk inserts further require a DEFAULT so they can omit the column. We
        // restrict this to numeric + boolean types (where `0` / `false` is the universally
        // understood "zero" value) and explicitly exclude columns that already have an explicit
        // default, foreign-key references, or are primary keys — on those, silently auto-
        // defaulting would mask real bugs.
        if let Some(ref default_expr) = col_def.default_expr {
            col_sql.push_str(&format!(" DEFAULT {}", default_expr));
        } else if let Some(ref default_val) = col_def.default_value {
            col_sql.push_str(&format!(" DEFAULT {}", default_val));
        } else if is_primary_key {
            // For UUID primary keys, add gen_random_uuid() default if no explicit default is set
            if col_def
                .column_type
                .as_ref()
                .map(|s| s.contains("UUID"))
                .unwrap_or(false)
            {
                col_sql.push_str(" DEFAULT gen_random_uuid()");
            }
        } else if !col_def.nullable && col_def.foreign_key.is_none() {
            if let Some(col_type) = col_def.column_type.as_ref() {
                if let Some(inferred) = infer_zero_default_for_sql_type(col_type) {
                    col_sql.push_str(&format!(" DEFAULT {}", inferred));
                    warn_auto_inferred_default(&full_table_name, col_name, col_type, inferred);
                }
            }
        }

        // Track unique constraints (single column)
        if col_def.unique {
            col_sql.push_str(" UNIQUE");
        }

        // Track indexed flag (single column) - omit if natively indexed by PK or Unique constraints
        if col_def.indexed && !col_def.unique && !is_primary_key {
            // Deduplicate: avoid auto-generating if the user explicitly defined a table-level index for this column
            let already_covered = table_def
                .indexes
                .iter()
                .any(|idx| index_covers_only_column(idx, col_name));
            if !already_covered {
                single_column_indexes.push(col_name.to_string());
            }
        }

        // Track column comments
        if let Some(ref comment) = col_def.comment {
            column_comments.push((col_name.to_string(), comment.to_string()));
        }

        // Handle foreign keys - add inline to match original style
        // Format: "chart_of_accounts(id) ON DELETE SET NULL"
        if let Some(ref fk) = col_def.foreign_key {
            // Add inline REFERENCES clause
            col_sql.push_str(&format!(" REFERENCES {}", fk));
        }

        // Track CHECK constraints (column-level)
        if let Some(ref check) = col_def.check {
            check_constraints.push((col_name.to_string(), check.to_string()));
        }

        column_defs.push(col_sql);
    }

    // Write column definitions
    for (i, col_def) in column_defs.iter().enumerate() {
        let is_last_column = i == column_defs.len() - 1;
        let has_table_constraints =
            !table_def.check_constraints.is_empty() || !table_def.composite_unique.is_empty();

        if is_last_column && !has_table_constraints {
            // Last column and no table constraints - no comma
            writeln!(sql, "{}", col_def).map_err(|e| format!("Failed to write SQL: {}", e))?;
        } else {
            // Not last or has table constraints - add comma
            writeln!(sql, "{},", col_def).map_err(|e| format!("Failed to write SQL: {}", e))?;
        }
    }

    // Add table-level CHECK constraints
    for (i, (constraint_name, check_expr)) in table_def.check_constraints.iter().enumerate() {
        let is_last =
            i == table_def.check_constraints.len() - 1 && table_def.composite_unique.is_empty();
        // Use custom name if provided, otherwise generate from table name
        let constraint_name_str = constraint_name
            .as_ref()
            .map(|n| format!("check_{}", sanitize_constraint_name(n)))
            .unwrap_or_else(|| format!("check_{}", sanitize_constraint_name(&table_name)));
        if is_last {
            writeln!(
                sql,
                "    CONSTRAINT {} CHECK ({})",
                constraint_name_str, check_expr
            )
            .map_err(|e| format!("Failed to write SQL: {}", e))?;
        } else {
            writeln!(
                sql,
                "    CONSTRAINT {} CHECK ({}),",
                constraint_name_str, check_expr
            )
            .map_err(|e| format!("Failed to write SQL: {}", e))?;
        }
    }

    // Add composite unique constraints
    for (i, unique_cols) in table_def.composite_unique.iter().enumerate() {
        let is_last = i == table_def.composite_unique.len() - 1;
        let cols_str = unique_cols.join(", ");
        if is_last {
            writeln!(sql, "    UNIQUE({})", cols_str)
                .map_err(|e| format!("Failed to write SQL: {}", e))?;
        } else {
            writeln!(sql, "    UNIQUE({}),", cols_str)
                .map_err(|e| format!("Failed to write SQL: {}", e))?;
        }
    }

    // Close CREATE TABLE
    writeln!(sql, ");").map_err(|e| format!("Failed to write SQL: {}", e))?;
    writeln!(sql).map_err(|e| format!("Failed to write SQL: {}", e))?;

    // Get all column names for validation
    let all_column_names: std::collections::HashSet<String> =
        columns.iter().map(|col| col.as_str().to_string()).collect();

    // Generate indexes (only for columns that exist in the table)
    for index in &table_def.indexes {
        // Validate that all columns in the index exist in the table
        let mut missing_columns = Vec::new();
        for col_name in &index.columns {
            if !all_column_names.contains(col_name) {
                missing_columns.push(col_name.clone());
            }
        }
        for col_name in &index.include_columns {
            if !all_column_names.contains(col_name) {
                missing_columns.push(col_name.clone());
            }
        }

        // Skip index if any columns don't exist
        if !missing_columns.is_empty() {
            eprintln!("⚠️  Warning: Skipping index '{}' on table '{}' because column(s) {} do not exist in the table", 
                index.name, full_table_name, missing_columns.join(", "));
            continue;
        }

        let mut index_sql = String::new();

        // IF NOT EXISTS: safe when the same migration file is re-applied (e.g. psql + ON_ERROR_STOP
        // with CREATE TABLE IF NOT EXISTS but indexes that already exist from a prior run).
        if index.unique {
            index_sql.push_str("CREATE UNIQUE INDEX IF NOT EXISTS ");
        } else {
            index_sql.push_str("CREATE INDEX IF NOT EXISTS ");
        }

        index_sql.push_str(&index.name);
        index_sql.push_str(" ON ");
        index_sql.push_str(&full_table_name);
        index_sql.push('(');
        index_sql.push_str(&index_key_body_sql(index));
        index_sql.push(')');

        if !index.include_columns.is_empty() {
            index_sql.push_str(" INCLUDE (");
            index_sql.push_str(&index.include_columns.join(", "));
            index_sql.push_str(")");
        }

        if let Some(ref where_clause) = index.partial_where {
            index_sql.push_str(" WHERE ");
            index_sql.push_str(where_clause);
        }

        index_sql.push_str(";");
        writeln!(sql, "{}", index_sql).map_err(|e| format!("Failed to write SQL: {}", e))?;
    }

    // Foreign keys are now added inline in column definitions
    // No need for separate ALTER TABLE statements

    // Column-level CHECK: DROP IF EXISTS + ADD so the same migration file can be re-applied
    // (ADD CONSTRAINT alone fails if the constraint name already exists).
    for (col_name, check_expr) in &check_constraints {
        let cname = format!(
            "check_{}_{}",
            sanitize_constraint_name(table_name),
            sanitize_constraint_name(col_name)
        );
        writeln!(
            sql,
            "ALTER TABLE {} DROP CONSTRAINT IF EXISTS {};",
            full_table_name, cname
        )
        .map_err(|e| format!("Failed to write SQL: {}", e))?;
        writeln!(
            sql,
            "ALTER TABLE {} ADD CONSTRAINT {} CHECK ({});",
            full_table_name, cname, check_expr
        )
        .map_err(|e| format!("Failed to write SQL: {}", e))?;
    }

    // Generate single-column indexes
    for col_name in &single_column_indexes {
        let index_name = format!(
            "idx_{}_{}",
            sanitize_constraint_name(table_name),
            sanitize_constraint_name(col_name)
        );
        writeln!(
            sql,
            "CREATE INDEX IF NOT EXISTS {} ON {}({});",
            index_name, full_table_name, col_name
        )
        .map_err(|e| format!("Failed to write SQL: {}", e))?;
    }

    // Generate column comments
    for (col_name, comment) in &column_comments {
        let escaped_comment = comment.replace("'", "''");
        writeln!(
            sql,
            "COMMENT ON COLUMN {}.{} IS '{}';",
            full_table_name, col_name, escaped_comment
        )
        .map_err(|e| format!("Failed to write SQL: {}", e))?;
    }

    // Generate table comment
    if let Some(ref comment) = table_def.table_comment {
        let escaped_comment = comment.replace("'", "''");
        writeln!(
            sql,
            "COMMENT ON TABLE {} IS '{}';",
            full_table_name, escaped_comment
        )
        .map_err(|e| format!("Failed to write SQL: {}", e))?;
    }

    Ok(sql)
}

fn index_covers_only_column(index: &lifeguard::IndexDefinition, col_name: &str) -> bool {
    let cov = if !index.key_parts.is_empty() {
        index_key_parts_coverage_columns(&index.key_parts)
    } else {
        index.columns.clone()
    };
    cov.len() == 1 && cov[0] == col_name
}

fn index_key_body_sql(index: &lifeguard::IndexDefinition) -> String {
    if !index.key_parts.is_empty() {
        lifeguard::format_index_key_list_sql(&index.key_parts)
    } else if let Some(ref k) = index.key_list_sql {
        k.clone()
    } else {
        index.columns.join(", ")
    }
}

/// Sanitize a name for use in constraint names
fn sanitize_constraint_name(name: &str) -> String {
    name.replace("-", "_").replace(".", "_").to_lowercase()
}

/// Emit a stderr warning that [`infer_zero_default_for_sql_type`] kicked in for a specific
/// column. Surfaces the "silent pragmatic default" so developers can replace it with an explicit
/// `#[default_expr]` / `#[default_value]` — once the warning line stops appearing for a given
/// field, the entity is telling the whole story instead of relying on the safety net.
///
/// Silenced when `LIFEGUARD_SILENCE_INFERRED_DEFAULTS=1` is set in the environment (useful in
/// CI or tests where the noise has been reviewed and accepted). Test helpers can also gate on
/// `cfg(test)` at the call site if individual suites want pristine stderr.
fn warn_auto_inferred_default(
    full_table_name: &str,
    col_name: &str,
    col_type: &str,
    inferred_default: &str,
) {
    if std::env::var("LIFEGUARD_SILENCE_INFERRED_DEFAULTS")
        .ok()
        .as_deref()
        == Some("1")
    {
        return;
    }
    // Keep to one concise line per occurrence; the postmortem / README go into the "why".
    let suggested_attr = match inferred_default {
        "false" => r#"#[default_expr = "false"]"#,
        _ => r#"#[default_expr = "0"]"#,
    };
    eprintln!(
        "warning: [lifeguard-migrate] auto-inferred DEFAULT {inferred_default} for {col_type} \
         NOT NULL column `{full_table_name}.{col_name}` (add {suggested_attr} to the LifeModel \
         field to silence; set LIFEGUARD_SILENCE_INFERRED_DEFAULTS=1 to silence all)",
    );
}

/// Zero-value default for a PostgreSQL type, used as a safety net for NOT NULL non-FK columns
/// that lack an explicit `#[default_expr]` / `#[default_value]`.
///
/// Intentionally narrow: only numeric and boolean types get an inferred default. Text, JSON,
/// UUID, timestamps, and bytea require an explicit default — auto-defaulting `VARCHAR(255) NOT
/// NULL` to `''` or `TIMESTAMP NOT NULL` to `NOW()` would happily mask real "forgot to set X"
/// bugs. Length / scale / precision modifiers and trailing `NOT NULL` / inline `REFERENCES` are
/// accepted transparently; the base type token at the start decides the answer.
fn infer_zero_default_for_sql_type(col_type: &str) -> Option<&'static str> {
    // Take only the base type identifier (everything before a `(`, whitespace, or end).
    let trimmed = col_type.trim();
    let base_end = trimmed
        .find(|c: char| c == '(' || c.is_whitespace())
        .unwrap_or(trimmed.len());
    let base = trimmed[..base_end].to_ascii_uppercase();
    // For two-word base types (`DOUBLE PRECISION`, `CHARACTER VARYING`) we also check the first
    // two tokens joined by a single space.
    let two_word = {
        let rest = trimmed[base_end..].trim_start();
        let second_end = rest
            .find(|c: char| c == '(' || c.is_whitespace())
            .unwrap_or(rest.len());
        if second_end == 0 {
            None
        } else {
            Some(format!("{base} {}", rest[..second_end].to_ascii_uppercase()))
        }
    };

    match base.as_str() {
        "SMALLINT" | "INT" | "INTEGER" | "INT2" | "INT4" | "INT8" | "BIGINT" | "NUMERIC"
        | "DECIMAL" | "REAL" | "FLOAT" | "FLOAT4" | "FLOAT8" | "MONEY" => Some("0"),
        "BOOLEAN" | "BOOL" => Some("false"),
        _ => match two_word.as_deref() {
            Some("DOUBLE PRECISION") => Some("0"),
            _ => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests would require actual entity definitions
    // For now, we'll test the sanitize function
    #[test]
    fn test_sanitize_constraint_name() {
        assert_eq!(
            sanitize_constraint_name("chart-of-accounts"),
            "chart_of_accounts"
        );
        assert_eq!(
            sanitize_constraint_name("journal.entries"),
            "journal_entries"
        );
        assert_eq!(sanitize_constraint_name("UPPERCASE"), "uppercase");
    }

    // --- infer_zero_default_for_sql_type ---

    #[test]
    fn infer_zero_default_numeric_types() {
        for t in [
            "SMALLINT",
            "smallint",
            "INT",
            "INTEGER",
            "BIGINT",
            "NUMERIC(10, 2)",
            "DECIMAL(5,0)",
            "REAL",
            "FLOAT",
            "FLOAT8",
            "DOUBLE PRECISION",
        ] {
            assert_eq!(
                infer_zero_default_for_sql_type(t),
                Some("0"),
                "expected 0 for {t}"
            );
        }
    }

    #[test]
    fn infer_zero_default_boolean_types() {
        assert_eq!(infer_zero_default_for_sql_type("BOOLEAN"), Some("false"));
        assert_eq!(infer_zero_default_for_sql_type("BOOL"), Some("false"));
    }

    #[test]
    fn infer_zero_default_does_not_apply_to_text_uuid_json_timestamps() {
        for t in [
            "TEXT",
            "VARCHAR(255)",
            "CHAR(10)",
            "UUID",
            "JSONB",
            "JSON",
            "TIMESTAMP",
            "TIMESTAMP WITH TIME ZONE",
            "TIMESTAMPTZ",
            "DATE",
            "TIME",
            "BYTEA",
        ] {
            assert_eq!(
                infer_zero_default_for_sql_type(t),
                None,
                "text / uuid / json / temporal types must require explicit defaults: {t}"
            );
        }
    }

    #[test]
    fn infer_zero_default_ignores_trailing_constraint_tokens() {
        // `column_type` is usually just the type, but be tolerant if a caller passes a snippet.
        assert_eq!(infer_zero_default_for_sql_type("SMALLINT NOT NULL"), Some("0"));
        assert_eq!(
            infer_zero_default_for_sql_type("DOUBLE PRECISION NOT NULL"),
            Some("0")
        );
    }
}
