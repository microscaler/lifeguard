# Roadmap

Epic-style checklists in older docs were overstated relative to this crate. Use this table instead:

| Area | Status |
|------|--------|
| `may_postgres`, `LifeExecutor`, transactions, raw SQL | Shipped |
| `LifeModel` / `LifeRecord`, query builder, relations, loaders | Shipped (ongoing hardening) |
| Migrations (`lifeguard::migration`, `lifeguard-migrate`, example `generate-migrations`) | Shipped (tooling evolves) |
| Optional metrics / tracing / channel logging | Shipped behind features |
| `LifeguardPool` / `PooledLifeExecutor` (primary/replica, WAL, heal, metrics) | Shipped (see [POOLING_OPERATIONS.md](./docs/POOLING_OPERATIONS.md), PRD for remaining parity) |
| **`ReadPreference`** on `PooledLifeExecutor` (explicit primary-tier reads); transparent Redis on every query | Partial — API shipped; “Redis on every read” remains vision / reflector path |
| LifeReflector, enterprise cache coherence | In-tree [`lifeguard-reflector`](./lifeguard-reflector/) (evolving) |
| **`compare-schema` index parity (T1 / T2b / T3 / T4)** | [`docs/planning/DESIGN_INDEX_COMPARE_ROADMAP.md`](./docs/planning/DESIGN_INDEX_COMPARE_ROADMAP.md) — **T1**, **T2**, **T4** shipped; **T2b** **partial** (live vs merged explicit or default opclass; **T1** opclass-only dedupe); **T3** **partial** (v1 + v2 normalized expression slots; explicit ordering/collation vs `pg_index`); derive-side expression IR optional |

Story-level detail: [docs/planning/epics-stories/](./docs/planning/epics-stories/) · Feature audit: [docs/planning/lifeguard-derive/SEAORM_LIFEGUARD_MAPPING.md](./docs/planning/lifeguard-derive/SEAORM_LIFEGUARD_MAPPING.md) · [docs/EPICS/](./docs/EPICS/) (curated notes).

---

[← README](./README.md) · [Comparison](./COMPARISON.md#repository-status)
