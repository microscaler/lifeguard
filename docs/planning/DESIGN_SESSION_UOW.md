# Design: Session / Unit of Work (Lifeguard)

**Status:** Companion to [PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md §9](./PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md). **`ModelIdentityMap`** (`src/session/mod.rs`) implements identity (U-1). **`Session`** + **`SessionDirtyNotifier`** (`src/session/uow.rs`) wrap the map and a **`Send`/`Sync` pending-dirty queue** so derived `LifeRecord` can **`attach_session`** and auto-enqueue dirty keys on `set_*` / `ActiveModelTrait::set` / `set_*_expr` without breaking graph **`Send`** bounds. **`flush_dirty`** remains closure-based persistence (U-2); pool usage is U-4.

## Goals (from PRD)

- **U-1:** Identity map — same PK → same in-memory handle (implemented).
- **U-2:** Dirty tracking + **flush** — **`mark_dirty`**, **`mark_dirty_key`**, **`flush_dirty`** on **`ModelIdentityMap`**; **`Session::flush_dirty`** drains a mutex-backed pending set into the map then flushes. Callers wire `LifeRecord::update` / `save` inside the flush closure. **`LifeRecord::attach_session(&session)`** (PK entities only): mutating the record enqueues the PK fingerprint for flush when `identity_map_key()` is `Some`. Keep the registered **`Model`** in sync with record edits before flush if the closure reads from `Rc<RefCell<Model>>` (see integration test in `session_identity_flush.rs`).
- **U-3:** Explicit session — no thread-local global; maps are constructed by the app (satisfied).
- **U-4:** **LifeguardPool** — **`Session`** does not hold an executor; pass **`&dyn LifeExecutor`** (e.g. **`PooledLifeExecutor`**) into **`Session::flush_dirty`**. For one DB transaction across a flush on the pool, use **`Session::flush_dirty_in_transaction_pooled`**, which pins one primary worker via **`LifeguardPool::exclusive_primary_write_executor`** (per-slot mutex + all statements on that slot). Plain **`PooledLifeExecutor`** still round-robins workers per call.
- **U-5:** **`may` coroutines** — `ModelIdentityMap` uses `Rc`/`RefCell` and is **not** `Send`/`Sync`; treat it like other single-threaded cell state: one map per coroutine/thread, or external `Mutex` if shared.

## Fingerprint keys

`src/session/pk.rs` (`fingerprint_pk_values`) encodes a subset of `sea_query::Value` explicitly; unknown variants fall back to `Debug`. Extend the `match` when you rely on stable keys for production types.

## Flush (current + future)

- **Shipped:** `ModelIdentityMap::flush_dirty` walks dirty entries in **lexicographic map-key order** (pending-insert keys under `PENDING_INSERT_KEY_PREFIX` sort before normal PK fingerprints) and invokes `Fn(&dyn LifeExecutor, Rc<RefCell<Model>>) -> Result<(), ActiveModelError>`. **`ModelIdentityMap::register_pending_insert`** / **`flush_dirty_with_map_key`** / **`promote_pending_to_loaded`** plus **`is_pending_insert_key`** support rows **without** a stable PK fingerprint until after `LifeRecord::insert` (callers branch on the map key in the flush closure, then promote). **`Session::flush_dirty`** merges **`SessionDirtyNotifier`** keys into the map first, then calls that logic. **`Session::flush_dirty_in_transaction(&MayPostgresExecutor, …)`** runs the flush inside **`Transaction`** on a **direct** client. **`Session::flush_dirty_in_transaction_pooled(&LifeguardPool, …)`** pins one primary slot (**`ExclusivePrimaryLifeExecutor`**) and uses raw `BEGIN` / `COMMIT` / `ROLLBACK` so the whole flush is one connection; map-key variants exist for insert vs update in one transaction. Per-slot mutexes also serialize unrelated jobs that target the same worker index.
- **LifeRecord → model auto-sync (PRD §9):** derived **`attach_session_with_model(&session, &Rc<RefCell<Model>>)`** — after each mutation that notifies the session, **`to_model()`** runs when it succeeds and writes into the linked `Rc` so flush closures read current literals without `*rc.borrow_mut() = rec.to_model()?`. **`attach_session`** without the `Rc` leaves prior manual sync behavior. F-style **`set_*_expr`** is not stored on the `Model` type; those edits stay on the record until `update()`.
- **Future:** optional executor-holding session type.

## References

- [PRD_CONNECTION_POOLING.md](./PRD_CONNECTION_POOLING.md)
- [PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md §9](./PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md)
