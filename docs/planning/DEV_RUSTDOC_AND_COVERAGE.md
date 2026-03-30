# Rustdoc and test coverage (ongoing process)

Use this checklist whenever you add or change **user-visible behavior** (public API, CLI, or behavior documented in a PRD).

## Rustdoc

1. **Public items** (`pub` types, traits, fns, enum variants): add or update `///` docs.
2. **Prefer** a one-line summary + `# Example` or `# Errors` where it helps; link to the PRD section for feature work (e.g. `PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md` §N).
3. **Modules**: `//!` module docs for new `src/**/mod.rs` or top-level files that encode a feature area.
4. **Limitations** (non-goals, raw-SQL escape hatches): document in the same place as the API.
5. **Verify** locally: `cargo doc -p lifeguard --no-deps --open` (optional) or `cargo doc -p lifeguard -p lifeguard-migrate --no-deps` for migrate-only changes.

## Test coverage

1. **Unit tests** next to the code (`#[cfg(test)]` or `*_tests.rs`) for pure logic (validators, scope composition, `F`-style expression builders, type mapping helpers).
2. **Integration tests** under `tests/` when behavior depends on Postgres, the derive pipeline, or the global migration registry—follow existing patterns (`db_integration_suite`, `TEST_DATABASE_URL`).
3. **Before merging** substantive library changes, run:
   - `cargo test -p lifeguard --lib` (and affected crates),
   - `cargo clippy -p lifeguard --all-targets -- -D warnings`,
   - optionally `just test-coverage` or `cargo llvm-cov -p lifeguard --lib --summary-only` to spot **uncovered new lines** (repo targets **≥65%**, aim **~80%** per project rules).
4. **Track gaps** in the relevant PRD “Implementation status” subsection (e.g. golden tests for schema inference) instead of leaving them implicit.

## Related

- `DEVELOPMENT.md` — Clippy / pre-commit workflow.
- `justfile` — `test-coverage`, `test-coverage-check`.
