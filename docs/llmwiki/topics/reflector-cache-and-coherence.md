# LifeReflector, cache coherence, Redis

- **Status**: `partially-verified`
- **Source docs**: [`VISION.md`](../../../VISION.md) (LifeReflector narrative), [`ARCHITECTURE.md`](../../../ARCHITECTURE.md), [LifeReflector README](https://github.com/microscaler/lifeguard-reflector)
- **Code anchors**: [microscaler/lifeguard-reflector](https://github.com/microscaler/lifeguard-reflector) (separate repository), `lifeguard/src/cache/`, [`docs/NOTIFY_CONTRACT.md`](../../NOTIFY_CONTRACT.md)
- **Last updated**: 2026-04-17

## What it is

**LifeReflector** is a **background** process that listens for PostgreSQL **NOTIFY** after commits and refreshes **Redis** keys that are already warm — it is **not** on the synchronous hot path of every `SELECT` (see numbered diagram in [`ARCHITECTURE.md`](../../../ARCHITECTURE.md)).

The core `lifeguard` crate exposes **`CacheProvider`** traits under `cache/` for optional cache-aside patterns.

## Cross-references

- [`entities/life-executor-pool-and-routing.md`](../entities/life-executor-pool-and-routing.md)
- [`reference/workspace-and-module-map.md`](../reference/workspace-and-module-map.md)
