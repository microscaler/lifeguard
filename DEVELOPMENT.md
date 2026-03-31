# Development Guide

## Preventing Clippy Errors

This guide explains how to prevent clippy errors from being introduced in the first place.

### Automated Checks

#### 1. **CI/CD Pipeline** ✅
- **Location**: `.github/workflows/ci.yaml`
- **Status**: CI now **fails** on clippy errors (removed `|| true`)
- The build will fail if any clippy warnings are found

#### 2. **Pre-commit (recommended)** ✅
- **Location**: `.pre-commit-config.yaml` in the repository root
- **Install** (once per clone): `pip install pre-commit && pre-commit install`
- Runs **`cargo clippy`** with the same flags as CI (`-D warnings`, `-W clippy::pedantic`). (CI does not run `rustfmt --check` yet; use `just fmt-check` before push if you want format guarantees.)
- The hook sets **`always_run: true`** so it is not skipped as `(no files to check)` when a commit only touches non-Rust paths (that skip was why local commits could miss CI clippy failures).
- To run manually: `pre-commit run cargo-clippy` or `pre-commit run --all-files`
- To bypass a single commit (not recommended): `git commit --no-verify`

#### 3. **Editor Integration** ✅
- **Location**: `.vscode/settings.json`
- Rust Analyzer runs clippy automatically and shows warnings as you type
- Format on save is enabled

#### 4. **Justfile Commands** ✅
```bash
just lint          # Check for clippy errors
just lint-fix      # Auto-fix clippy errors
just validate      # Run all checks (format, lint, check, tests)
```

### Rustdoc and test coverage (feature work)

For each PRD-driven or user-facing change, follow **`docs/planning/DEV_RUSTDOC_AND_COVERAGE.md`**: update **`///` rustdoc** for public API, add **unit/integration tests** as appropriate, and optionally run **`cargo llvm-cov`** (see `just test-coverage`) before merge.

### `lifeguard-migrate` and schema inference

- **CLI:** `cargo run -p lifeguard-migrate -- infer-schema --database-url …` (or set `DATABASE_URL` / `LIFEGUARD_DATABASE_URL`). See **`lifeguard-migrate/README.md`** (`infer-schema` section) and **`docs/planning/DESIGN_SCHEMA_INFERENCE_CLI_CODEGEN.md`**.
- **Emitter goldens:** changing `lifeguard-migrate/src/schema_infer.rs` output may require updating files under **`lifeguard-migrate/tests/golden/`**. Run **`cargo test -p lifeguard-migrate schema_infer`** before merge.
- **Live Postgres tests (optional):** `lifeguard-migrate/tests/infer_schema_postgres_smoke.rs` runs **`infer_schema_rust`** against `public`; `infer_schema_table_filter_si3.rs` covers SI-3 table filtering (creates/drops scratch tables). Both require **`TEST_DATABASE_URL`**, **`DATABASE_URL`**, or **`LIFEGUARD_DATABASE_URL`**; otherwise they skip. **`db_integration_suite`** includes **`column_f_update`** (F-style `UPDATE SET`, derived `set_*_expr` / `update()`, insert guard for `__update_exprs`), **`column_f_where`** (`WHERE` / `ORDER BY` with `Expr::expr` + `ColumnTrait::f_*`), and **`session_identity_flush`** (`ModelIdentityMap` + `flush_dirty` + `LifeRecord::update`, PRD §9).

### Development Workflow

**Before writing code:**
1. Ensure your editor (VS Code/Cursor) has Rust Analyzer installed
2. The editor will show clippy warnings in real-time as you type

**While writing code:**
1. Pay attention to red/yellow squiggles in your editor
2. Hover over warnings to see clippy suggestions
3. Many warnings can be auto-fixed with the "Quick Fix" action

**Before committing:**
1. Run `just lint` or `cargo clippy --all-targets --all-features -- -D warnings`
2. If errors found, run `just lint-fix` to auto-fix many issues
3. Fix remaining issues manually
4. With `pre-commit install`, the clippy hook runs before the commit completes

**If CI fails:**
1. Check the CI logs for clippy errors
2. Run `cargo clippy --all-targets --all-features -- -D warnings` locally
3. Fix all errors before pushing again

### Common Clippy Errors to Avoid

#### Format Strings
```rust
// ❌ Don't
format!("Error: {}", error)
write!(f, "Value: {}", value)

// ✅ Do
format!("Error: {error}")
write!(f, "Value: {value}")
```

#### Redundant Closures
```rust
// ❌ Don't
.map_err(|e| Error::Database(e))

// ✅ Do
.map_err(Error::Database)
```

#### Boolean Comparisons
```rust
// ❌ Don't
assert_eq!(value, false);
if value == false { }

// ✅ Do
assert!(!value);
if !value { }
```

#### String Slicing
```rust
// ❌ Don't
if s.starts_with("prefix") {
    let rest = &s[7..];
}

// ✅ Do
if let Some(rest) = s.strip_prefix("prefix") {
    // use rest
}
```

#### Comparisons with Copy Types
```rust
// ❌ Don't
if ident.to_string() == "test" { }

// ✅ Do
if *ident == "test" { }  // For Copy types like syn::Ident
```

### Auto-Fixing Clippy Errors

Many clippy errors can be automatically fixed:

```bash
# Auto-fix all fixable clippy errors
cargo clippy --all-targets --all-features --fix --allow-dirty --allow-staged

# Or use the justfile command
just lint-fix
```

**Note**: Auto-fix may not fix all errors. Review the changes and fix remaining issues manually.

### Configuration

- **CI**: `.github/workflows/ci.yaml` - Fails on clippy errors (with pedantic mode)
- **Editor**: `.vscode/settings.json` - Real-time clippy checking (with pedantic mode)
- **Pre-commit**: `.pre-commit-config.yaml` - clippy before commits (CI parity, `always_run`); install via `pre-commit install`
- **Justfile**: `justfile` - Convenient commands for linting (with pedantic mode)

### Pedantic Mode

Clippy is configured to run in **pedantic mode** (`-W clippy::pedantic`), which enables additional strict lints for even higher code quality. This will catch:

- Similar variable names
- Redundant else blocks
- Missing documentation backticks
- Functions that are too long
- Structs with too many boolean fields
- And many more style/quality improvements

**Note**: Pedantic mode is very strict and may flag some false positives. If a pedantic lint is too noisy for your use case, you can allow it with `#[allow(clippy::lint_name)]`.

### Best Practices

1. **Run clippy frequently** - Don't wait until the end
2. **Use auto-fix** - Many errors can be fixed automatically
3. **Fix CI failures immediately** - Don't let them accumulate
4. **Configure your editor** - Enable rust-analyzer clippy integration
5. **Use pre-commit** - Catch clippy errors before they are committed (`pre-commit install`)

### Troubleshooting

**Pre-commit not running?**
```bash
pip install pre-commit
pre-commit install
pre-commit run --all-files   # optional: verify hooks manually
```

**Editor not showing clippy warnings?**
- Ensure Rust Analyzer extension is installed
- Check `.vscode/settings.json` exists
- Restart the editor

**CI passing but local clippy fails?**
- Ensure you're using the same Rust toolchain: `rustup toolchain install nightly-2025-06-30`
- Run: `cargo clippy --all-targets --all-features -- -D warnings`
