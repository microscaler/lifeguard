# Transactions: API and semantics

- **Status**: `verified`
- **Source docs**: [`src/lib.rs`](../../../src/lib.rs) re-exports, rustdoc on `transaction::`
- **Code anchors**: [`src/transaction.rs`](../../../src/transaction.rs)
- **Last updated**: 2026-07-14

## What it is

Lifeguard exposes **`Transaction`**, **`IsolationLevel`**, and **`TransactionError`** from [`src/transaction.rs`](../../../src/transaction.rs). Callers obtain a transaction from the executor API (see **`LifeExecutor`** / pool docs) and commit or roll back explicitly.

## Agent notes

- Prefer **`cargo doc -p lifeguard transaction`** for exact method signatures at your revision.
- Streaming APIs (`stream_all`, etc.) document **txn** cleanup in module rustdocs — read before changing `query/stream` paths.
- `begin_with_session` calls the schema-qualified application helper
  `public.rls_set_session(...)` after `BEGIN`. The consumer owns that SQL
  function and its `auth.*` GUC mapping.

## Cross-references

- [`topics/query-select-and-active-model.md`](../topics/query-select-and-active-model.md)
- [`entities/life-executor-pool-and-routing.md`](./life-executor-pool-and-routing.md)
