# Development Guide

## Preventing Clippy Errors

This guide explains how to prevent clippy errors from being introduced in the first place.

### Automated Checks

#### 1. **CI/CD Pipeline** ✅
- **Location**: `.github/workflows/ci.yaml`
- **Status**: CI now **fails** on clippy errors (removed `|| true`)
- The build will fail if any clippy warnings are found

#### 2. **Pre-Commit Hook** ✅
- **Location**: `.git/hooks/pre-commit`
- **Status**: Installed and active
- Automatically runs clippy before allowing commits
- To bypass (not recommended): `git commit --no-verify`

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
4. The pre-commit hook will also run clippy automatically

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
- **Pre-commit**: `.git/hooks/pre-commit` - Runs clippy before commits (with pedantic mode)
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
5. **Use the pre-commit hook** - Catch errors before they're committed

### Troubleshooting

**Pre-commit hook not running?**
```bash
chmod +x .git/hooks/pre-commit
```

**Editor not showing clippy warnings?**
- Ensure Rust Analyzer extension is installed
- Check `.vscode/settings.json` exists
- Restart the editor

**CI passing but local clippy fails?**
- Ensure you're using the same Rust toolchain: `rustup toolchain install nightly-2025-06-30`
- Run: `cargo clippy --all-targets --all-features -- -D warnings`
