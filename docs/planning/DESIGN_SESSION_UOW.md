# Design: Session / Unit of Work (Lifeguard)

**Status:** Companion to [PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md §9](./PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md). **`ModelIdentityMap`** in `src/session/mod.rs` implements identity (U-1), **dirty keys** + `flush_dirty` (U-2 partial — closure-based persistence; pool story still U-4).

## Goals (from PRD)

- **U-1:** Identity map — same PK → same in-memory handle (implemented).
- **U-2:** Dirty tracking + **flush** — **`mark_dirty`**, **`mark_dirty_key`**, **`flush_dirty`** ship; callers wire `LifeRecord::update` / `save` inside the flush closure. After mutating a **`#[derive(LifeRecord)]`** value, `record.identity_map_key()` + `map.mark_dirty_key(&key)` aligns dirty keys with the identity map (when the row was registered). Automatic derive integration (auto-mark on `set`) is **not** implemented.
- **U-3:** Explicit session — no thread-local global; maps are constructed by the app (satisfied).
- **U-4:** **LifeguardPool** — session must document whether it holds one executor, pins a worker, or uses another policy. **Decision (v0):** any future `Session` that performs I/O should hold **`&dyn LifeExecutor`** (or a generic bound) obtained **from the pool per operation** unless we add an explicit “pin slot for this UoW” API on [`LifeguardPool`](./PRD_CONNECTION_POOLING.md). Do **not** assume a session can outlive a single pooled checkout without a design that stores `PooledLifeExecutor` or equivalent.
- **U-5:** **`may` coroutines** — `ModelIdentityMap` uses `Rc`/`RefCell` and is **not** `Send`/`Sync`; treat it like other single-threaded cell state: one map per coroutine/thread, or external `Mutex` if shared.

## Fingerprint keys

`src/session/pk.rs` (`fingerprint_pk_values`) encodes a subset of `sea_query::Value` explicitly; unknown variants fall back to `Debug`. Extend the `match` when you rely on stable keys for production types.

## Flush (current + future)

- **Shipped:** `ModelIdentityMap::flush_dirty` walks dirty entries in **lexicographic PK fingerprint order** and invokes `Fn(&dyn LifeExecutor, Rc<RefCell<Model>>) -> Result<(), ActiveModelError>`. Wrap the executor in `lifeguard::Transaction` if you need a single DB transaction across rows.
- **Future:** auto-dirty on `LifeRecord::set` / derive hooks; optional `Session` struct holding executor + map.

## References

- [PRD_CONNECTION_POOLING.md](./PRD_CONNECTION_POOLING.md)
- [PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md §9](./PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md)
