# Code Coverage Setup

**Tool:** `cargo-llvm-cov` (LLVM-based coverage)

**Replaced:** `cargo-tarpaulin` (switched on 2025-01-XX)

---

## CI Configuration

The CI workflow (`.github/workflows/ci.yaml`) uses `cargo-llvm-cov` for code coverage:

```yaml
- name: Install llvm-tools
  run: rustup component add llvm-tools-preview

- name: Install cargo-llvm-cov
  run: cargo install cargo-llvm-cov --locked

- name: Code coverage
  run: |
    cargo llvm-cov --all-features --workspace --lcov --output-path lcov.info
    cargo llvm-cov --all-features --workspace --summary-only
```

---

## Local Usage

### Install

```bash
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov --locked
```

### Run Coverage

```bash
# Generate LCOV report
cargo llvm-cov --all-features --workspace --lcov --output-path lcov.info

# Show summary only
cargo llvm-cov --all-features --workspace --summary-only

# Generate HTML report
cargo llvm-cov --all-features --workspace --html
```

### View HTML Report

After generating HTML report:
```bash
open target/llvm-cov/html/index.html
```

---

## Coverage Threshold

**Note:** The CI workflow does not currently enforce a coverage threshold. The previous tarpaulin setup had `--fail-under 80`, but this has been removed.

To add a threshold check, you can:

1. **Parse the summary output** and fail if below threshold
2. **Use a coverage service** (e.g., Codecov) that enforces thresholds
3. **Add a script** to check coverage percentage

---

## Advantages of llvm-cov over tarpaulin

1. **Faster** - Uses LLVM's built-in coverage instrumentation
2. **More accurate** - Better branch coverage detection
3. **Better integration** - Works with Rust's native tooling
4. **HTML reports** - Better visualization
5. **LCOV format** - Compatible with more tools

---

## Current Coverage Status

As of Story 04 completion:
- **Coverage:** ~26.67% (24/90 lines)
- **Reason:** Most code is foundation layer without integration tests
- **Expected:** Coverage will increase as integration tests are added (Story 08+)

---

## Next Steps

1. Add integration tests with testcontainers (before Story 04 - LifeguardPool)
2. Increase test coverage for all modules
3. Consider adding coverage threshold once baseline is established
