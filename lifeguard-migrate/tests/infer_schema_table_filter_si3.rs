//! SI-3: `--table` / `InferOptions.tables` restricts emitted structs (PRD §5.5).
//!
//! Requires `TEST_DATABASE_URL`, `DATABASE_URL`, or `LIFEGUARD_DATABASE_URL`; otherwise skips.

use lifeguard::{connect, LifeExecutor, MayPostgresExecutor};
use lifeguard_migrate::schema_infer::{infer_schema_rust, InferOptions};

const T_ALPHA: &str = "lg_infer_si3_alpha";
const T_BETA: &str = "lg_infer_si3_beta";

fn postgres_url() -> Option<String> {
    std::env::var("TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .or_else(|_| std::env::var("LIFEGUARD_DATABASE_URL"))
        .ok()
        .filter(|s| !s.trim().is_empty())
}

#[test]
fn infer_schema_table_filter_excludes_other_tables() {
    let Some(url) = postgres_url() else {
        eprintln!("infer_schema_table_filter_excludes_other_tables: skipped (no DB URL)");
        return;
    };

    let client = connect(&url).expect("connect");
    let ex = MayPostgresExecutor::new(client);

    let create_a = format!(
        "CREATE TABLE IF NOT EXISTS public.{T_ALPHA} (id SERIAL PRIMARY KEY, note TEXT NOT NULL DEFAULT '')"
    );
    let create_b = format!(
        "CREATE TABLE IF NOT EXISTS public.{T_BETA} (id SERIAL PRIMARY KEY, note TEXT NOT NULL DEFAULT '')"
    );
    ex.execute(&create_a, &[]).expect("create alpha");
    ex.execute(&create_b, &[]).expect("create beta");

    let out = infer_schema_rust(
        &ex,
        &InferOptions {
            schema: "public".to_string(),
            tables: vec![T_ALPHA.to_string()],
        },
    )
    .expect("infer_schema_rust");

    assert!(
        out.contains(&format!("`{T_ALPHA}`")) || out.contains(T_ALPHA),
        "expected alpha table in output: {out}"
    );
    assert!(
        !out.contains(T_BETA),
        "beta table should be excluded when filtering to {T_ALPHA}: {out}"
    );

    let _ = ex.execute(&format!("DROP TABLE IF EXISTS public.{T_ALPHA}"), &[]);
    let _ = ex.execute(&format!("DROP TABLE IF EXISTS public.{T_BETA}"), &[]);
}
