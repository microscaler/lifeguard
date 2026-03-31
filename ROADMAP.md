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

Story-level detail: [docs/planning/epics-stories/](./docs/planning/epics-stories/) · Feature audit: [docs/planning/lifeguard-derive/SEAORM_LIFEGUARD_MAPPING.md](./docs/planning/lifeguard-derive/SEAORM_LIFEGUARD_MAPPING.md) · [docs/EPICS/](./docs/EPICS/) (curated notes).

---

[← README](./README.md) · [Comparison](./COMPARISON.md#repository-status)
