# Design: Session / Unit of Work (Lifeguard)

**Status:** Companion to [PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md §9](./PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md). **`ModelIdentityMap`** (`src/session/mod.rs`) implements identity (U-1). **`Session`** + **`SessionDirtyNotifier`** (`src/session/uow.rs`) wrap the map and a **`Send`/`Sync` pending-dirty queue** so derived `LifeRecord` can **`attach_session`** and auto-enqueue dirty keys on `set_*` / `ActiveModelTrait::set` / `set_*_expr` without breaking graph **`Send`** bounds. **`flush_dirty`** remains closure-based persistence (U-2); pool usage is U-4.

## Goals (from PRD)

- **U-1:** Identity map — same PK → same in-memory handle (implemented).
- **U-2:** Dirty tracking + **flush** — **`mark_dirty`**, **`mark_dirty_key`**, **`flush_dirty`** on **`ModelIdentityMap`**; **`Session::flush_dirty`** drains a mutex-backed pending set into the map then flushes. Callers wire `LifeRecord::update` / `save` inside the flush closure. **`LifeRecord::attach_session(&session)`** (PK entities only): mutating the record enqueues the PK fingerprint for flush when `identity_map_key()` is `Some`. Keep the registered **`Model`** in sync with record edits before flush if the closure reads from `Rc<RefCell<Model>>` (see integration test in `session_identity_flush.rs`).
- **U-3:** Explicit session — no thread-local global; maps are constructed by the app (satisfied).
- **U-4:** **LifeguardPool** — **`Session`** does not hold an executor; pass **`&dyn LifeExecutor`** (e.g. **`PooledLifeExecutor`**) into **`Session::flush_dirty`** per operation, or wrap in **`Transaction`** for one DB transaction across rows. Same “per-operation checkout” story as **`ModelIdentityMap::flush_dirty`**. A future “pin slot for this UoW” API would live on [`LifeguardPool`](./PRD_CONNECTION_POOLING.md) if needed.
- **U-5:** **`may` coroutines** — `ModelIdentityMap` uses `Rc`/`RefCell` and is **not** `Send`/`Sync`; treat it like other single-threaded cell state: one map per coroutine/thread, or external `Mutex` if shared.

## Fingerprint keys

`src/session/pk.rs` (`fingerprint_pk_values`) encodes a subset of `sea_query::Value` explicitly; unknown variants fall back to `Debug`. Extend the `match` when you rely on stable keys for production types.

## Flush (current + future)

- **Shipped:** `ModelIdentityMap::flush_dirty` walks dirty entries in **lexicographic PK fingerprint order** and invokes `Fn(&dyn LifeExecutor, Rc<RefCell<Model>>) -> Result<(), ActiveModelError>`. **`Session::flush_dirty`** merges **`SessionDirtyNotifier`** keys into the map first, then calls that logic. **`Session::flush_dirty_in_transaction(&MayPostgresExecutor, …)`** runs the same flush closure inside `BEGIN` / `COMMIT` (or `ROLLBACK` on error) on a **direct** client — **not** for `PooledLifeExecutor` (no single pinned connection for the whole flush).
- **Future:** insert-only flush / new entities in the map; optional pool API to pin one worker for a multi-statement transaction; optional executor-holding session type.

## References

- [PRD_CONNECTION_POOLING.md](./PRD_CONNECTION_POOLING.md)
- [PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md §9](./PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md)
