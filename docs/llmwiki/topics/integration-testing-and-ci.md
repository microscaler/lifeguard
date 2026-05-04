# Integration testing, CI, and test helpers

- **Status**: `verified`
- **Source docs**: [`docs/TEST_INFRASTRUCTURE.md`](../../TEST_INFRASTRUCTURE.md), [`DEVELOPMENT.md`](../../../DEVELOPMENT.md), [`tests-integration/README.md`](../../../tests-integration/README.md)
- **Code anchors**: `lifeguard/src/test_helpers/`, `tests/`, `tests-integration/`
- **Last updated**: 2026-04-17

## What it is

Integration tests use **`TestDatabase`** / env (`TEST_DATABASE_URL`) and workspace `just` / `nextest` recipes per **`DEVELOPMENT.md`**. CI uses Compose/Kind patterns documented in **`TEST_INFRASTRUCTURE.md`**.

There is **no** `testkit` macro advertised in README — use **`test_helpers`** per root README.

## Cross-references

- [`reference/workspace-and-module-map.md`](../reference/workspace-and-module-map.md)
