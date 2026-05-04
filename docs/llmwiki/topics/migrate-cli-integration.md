# Using `lifeguard-migrate` in apps and CI

- **Status**: `verified`
- **Source docs**: [`lifeguard-migrate/README.md`](../../../lifeguard-migrate/README.md), [`docs/TEST_INFRASTRUCTURE.md`](../../TEST_INFRASTRUCTURE.md)
- **Code anchors**: `lifeguard-migrate/src/main.rs` (CLI), consumer `examples/entities/generate_migrations` pattern
- **Last updated**: 2026-04-17

## What it is

The **`lifeguard-migrate`** binary emits SQL, **`apply_order.txt`**, and (in Hauliage) **`seed_order.txt`**. **`compare-schema`** compares merged migration SQL to a live DB for drift reporting.

## Integration pattern

- Libraries embed **`startup_migrations`** from `lifeguard::migration` when apps run migrations at boot — see rustdoc on `migration` module in [`src/lib.rs`](../../../src/lib.rs).

## Cross-references

- [`entities/migrate-compare-and-sql-generation.md`](../entities/migrate-compare-and-sql-generation.md)
- [`reference/planning-docs-index.md`](../reference/planning-docs-index.md)
