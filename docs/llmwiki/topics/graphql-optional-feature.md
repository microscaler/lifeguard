# Optional `graphql` feature (`async_graphql`)

- **Status**: `verified`
- **Source docs**: [`src/lib.rs`](../../../src/lib.rs) (GraphQL section), root [`Cargo.toml`](../../../Cargo.toml) features
- **Code anchors**: `#[cfg(feature = "graphql")]` in `src/lib.rs`; derive emits `SimpleObject` only when enabled
- **Last updated**: 2026-04-17

## What it is

When the **`graphql`** feature is enabled on **`lifeguard`** and **`lifeguard-derive`**, the derive can emit **`async_graphql::SimpleObject`** on generated models. The workspace pins **`async-graphql`** versions in **`[workspace.dependencies]`** — dependent crates must use a **compatible** version and enable the scalar features they need (`chrono`, `uuid`, etc.).

## Rules

- Do not enable GraphQL in crates that do not need it — it expands compile surface and dependency graph.
- Match **`async-graphql`** versions across **`lifeguard`** and consumers.

## Cross-references

- [`entities/life-model-and-life-record.md`](../entities/life-model-and-life-record.md)
