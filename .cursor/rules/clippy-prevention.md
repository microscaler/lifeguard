# Clippy Error Prevention Guidelines

## Overview
This document outlines how to prevent clippy errors from being introduced in the first place, rather than fixing them after the fact.

## Automated Checks

### 1. CI/CD Pipeline
- **Location**: `.github/workflows/ci.yaml`
- **Status**: ✅ Fixed - CI now fails on clippy errors
- The CI pipeline runs `cargo clippy --all-targets --all-features -- -D warnings` and will fail the build if any errors are found.

### 2. Editor Integration (VS Code/Cursor)
- **Location**: `.vscode/settings.json`
- **Status**: ✅ Configured
- Rust Analyzer is configured to run clippy automatically and show warnings as you type.
- Format on save is enabled.

### 3. Pre-Commit Workflow
Before committing code, always run:
```bash
# Check for clippy errors
cargo clippy --all-targets --all-features -- -D warnings

# Or use the justfile command
just lint

# Or use farm CLI (if available)
farm lint
```

### 4. Auto-Fix Common Issues
Many clippy errors can be auto-fixed:
```bash
cargo clippy --all-targets --all-features --fix --allow-dirty --allow-staged

# Or use the justfile command
just lint-fix
```

## Common Clippy Errors to Avoid

### Format Strings
❌ **Don't write:**
```rust
format!("Error: {}", error)
write!(f, "Value: {}", value)
```

✅ **Do write:**
```rust
format!("Error: {error}")
write!(f, "Value: {value}")
```

### Redundant Closures
❌ **Don't write:**
```rust
.map_err(|e| Error::Database(e))
```

✅ **Do write:**
```rust
.map_err(Error::Database)
```

### Comparisons
❌ **Don't write:**
```rust
if value.to_string() == "test" { }
assert_eq!(value, false);
```

✅ **Do write:**
```rust
if *value == "test" { }  // For Copy types
assert!(!value);  // For boolean assertions
```

### String Slicing
❌ **Don't write:**
```rust
if s.starts_with("prefix") {
    let rest = &s[7..];
}
```

✅ **Do write:**
```rust
if let Some(rest) = s.strip_prefix("prefix") {
    // use rest
}
```

## Development Workflow

1. **Write code** - Rust Analyzer will show clippy warnings in real-time
2. **Before committing** - Run `cargo clippy --all-targets --all-features -- -D warnings`
3. **Auto-fix** - Run `cargo clippy --fix` to automatically fix many issues
4. **Manual fixes** - Fix remaining issues manually
5. **Commit** - Only commit when clippy passes

## Configuration Files

- `clippy.toml` - Clippy configuration (allows certain lints that are acceptable)
- `.vscode/settings.json` - Editor configuration for real-time clippy checking
- `.github/workflows/ci.yaml` - CI configuration that fails on clippy errors

## Best Practices

1. **Run clippy frequently** - Don't wait until the end to check
2. **Use auto-fix** - Many errors can be fixed automatically
3. **Review CI failures** - If CI fails on clippy, fix it immediately
4. **Configure your editor** - Enable rust-analyzer clippy integration
5. **Use pre-commit hooks** - Consider setting up git hooks to run clippy before commits
