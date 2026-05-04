# Optional `graphql` feature (`async_graphql`)

## Platform decision (read first)

**Hauliage / BFF / dashboards:** The platform has **explicitly chosen not to adopt GraphQL** for the BFF or multi-service dashboards. A GraphQL runtime does **not** fit the **OpenAPI-first, BRRTRouter-generated, coroutine/handler** model: composition belongs in **BFF view endpoints** (REST-shaped, spec-driven) and typed downstream calls (see the Hauliage BFF view-composition PRD and related wiki in the Hauliage repository). **Do not plan new work** that assumes a GraphQL API on top of Lifeguard for the Hauliage stack.

**This repository:** The optional **`graphql`** crate feature remains **only** for existing `#[cfg(feature = "graphql")]` code paths (`SimpleObject` emission on generated models, integration tests). It is **legacy / frozen** from a product perspective — **no new platform features** should depend on it until/unless that decision is revisited.

---

- **Status**: `partially-verified` (code exists; **product direction** is “no further investment” for Hauliage/BFF)
- **Source docs**: [`src/lib.rs`](../../../src/lib.rs) (GraphQL section), root [`Cargo.toml`](../../../Cargo.toml) features
- **Code anchors**: `#[cfg(feature = "graphql")]` in `src/lib.rs`; derive emits `SimpleObject` only when enabled
- **Last updated**: 2026-04-17

## What it is (technical)

When the **`graphql`** feature is enabled on **`lifeguard`** and **`lifeguard-derive`**, the derive can emit **`async_graphql::SimpleObject`** on generated models. The workspace pins **`async-graphql`** versions in **`[workspace.dependencies]`** — dependent crates must use a **compatible** version and enable the scalar features they need (`chrono`, `uuid`, etc.).

## Rules

- **New Hauliage/BFF work:** do not introduce GraphQL as the API shape; use OpenAPI + BRRTRouter + composed views per PRD.
- **Existing / tests only:** match **`async-graphql`** versions when the feature is enabled; do not expand GraphQL surface without an explicit architecture review.

## Cross-references

- [`entities/life-model-and-life-record.md`](../entities/life-model-and-life-record.md)
