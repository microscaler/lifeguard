# PRD: PostgreSQL RLS Integration V2 — Augmenting Existing Executors

> **Status:** Draft V2
> **Date:** 2026-05-04
> **Scope:** Database-level Row Level Security (RLS) integration into Lifeguard's execution layer.
> **Key Change:** Removes the `RlsExecutor` wrapper type. Augments existing executor structs with optional session context, matching the `ReadPreference` pattern.

---

## 1. Architectural Philosophy

### 1.1 No Wrapper Types
Previous V1 proposed `RlsExecutor<'a, E: LifeExecutor>`. This was rejected because:
- It introduces complex lifetimes and generic noise for downstream apps (`RlsExecutor<'_, MayPostgresExecutor>`).
- It creates a new type in the call chain, potentially breaking downcasting or specific method expectations.
- It violates the existing platform convention (e.g., `ReadPreference` is a builder method on `PooledLifeExecutor`, not a separate type).

**V2 Strategy:** Augment existing structs (`MayPostgresExecutor`, `PooledLifeExecutor`, `Transaction`) with an `Option<SessionContext>` field and a `with_session_context(ctx)` builder method.

### 1.2 Minimal, Generic Session Carrier
`SessionContext` is a lightweight carrier for verified identity claims. It is **not** an identity provider. 
- Derives `Clone + Send + PartialEq` (must cross thread boundaries and be efficiently cloned for channel dispatch).
- Contains the minimal set of verified claims needed by most multi-tenant apps (user ID, org ID, roles, permissions).
- Serialization into `SET LOCAL` variables uses proper `LifeError` handling (no silent failures or `unwrap_or`).

### 1.3 Fail-Closed by Design
If `SessionContext` is `None` or claims are empty, RLS policies receive `NULL` or empty values via `current_setting(..., true)`. Rows are not visible unless the policy explicitly allows public access. This prevents privilege escalation bugs where a missing context defaults to "allow all".

---

## 2. Core Entities

### 2.1 `SessionContext` (New Type)
**Location:** `src/executor.rs` (exported via `lib.rs`)

```rust
/// Verified identity claims from the consuming application's identity provider.
/// Lifeguard does not parse JWTs or extract these claims; the application passes them here.
///
/// Derives Clone + Send to cross thread boundaries in the pool worker path.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionContext {
    pub user_id: Option<uuid::Uuid>,
    pub user_org_id: Option<uuid::Uuid>,
    pub user_type: Option<String>,
    pub org_type: Option<String>,
    pub permissions: Vec<String>,
    pub user_email: Option<String>,
}

impl SessionContext {
    /// Serialize this context into the SQL arguments expected by the session setup function.
    /// Returns a vector of values compatible with `may_postgres::types::ToSql`.
    pub fn to_sql_args(&self) -> Result<Vec<Box<dyn ToSql + '_>>, LifeError> {
        Ok(vec![
            Box::new(self.user_id),
            Box::new(self.user_org_id),
            Box::new(self.user_type.as_deref().unwrap_or("")),
            Box::new(self.org_type.as_deref().unwrap_or("")),
            Box::new(serde_json::to_value(&self.permissions).map_err(|e| {
                LifeError::Other(format!("failed to serialize session permissions: {}", e))
            })?),
            Box::new(self.user_email.as_deref().unwrap_or("")),
        ])
    }
}
```

### 2.2 `MayPostgresExecutor` Augmentation
**Location:** `src/executor.rs`

Add `session_context: Option<SessionContext>` to `MayPostgresExecutor`.

```rust
pub struct MayPostgresExecutor {
    client: Client,
    session_context: Option<SessionContext>,
}

impl MayPostgresExecutor {
    pub fn new(client: Client) -> Self {
        Self { client, session_context: None }
    }

    /// Attach session context for RLS-injected execution.
    #[must_use]
    pub fn with_session_context(mut self, ctx: SessionContext) -> Self {
        self.session_context = Some(ctx);
        self
    }

    // Overrides: execute, query_one, query_all -> delegate to execute_with_session, etc.
}
```

### 2.3 `Transaction` Augmentation
**Location:** `src/transaction.rs`

Add optional session context. Inject at `BEGIN` rather than every query, since `SET LOCAL` is transaction-scoped.

```rust
pub struct Transaction {
    client: Client,
    depth: u32,
    closed: bool,
    session_context: Option<SessionContext>, // NEW
}

impl Transaction {
    pub(crate) fn new_with_session(client: Client, ctx: Option<SessionContext>) -> Result<Self, TransactionError> {
        // 1. Begin transaction
        client.execute("BEGIN", &[])?;

        // 2. Inject session context if provided
        if let Some(ref ctx) = ctx {
            let args = ctx.to_sql_args()?;
            client.execute("SELECT rls_set_session($1, $2, $3, $4, $5, $6)", &args.iter().map(|a| a.as_ref()).collect::<Vec<_>>()[..])?;
        }

        Ok(Self { client, depth: 0, closed: false, session_context: ctx })
    }

    // Provide factory on MayPostgresExecutor:
    pub fn begin_with_session(&self, ctx: SessionContext) -> Result<Transaction, TransactionError> {
        Transaction::new_with_session(self.client.clone(), Some(ctx))
    }
}
```

### 2.4 `PooledLifeExecutor` Augmentation
**Location:** `src/pool/pooled.rs`

This is the most invasive change due to the worker model. Session context must travel across `crossbeam_channel`.

**2.4.1 Extend `WorkerJob`**
Add `session: Option<SessionContext>` to all variants.

```rust
enum WorkerJob {
    Execute { enqueued_at: Instant, query: String, params: Vec<OwnedParam>, reply: ..., session: Option<SessionContext> },
    QueryOne  { enqueued_at: Instant, query: String, params: Vec<OwnedParam>, reply: ..., session: Option<SessionContext> },
    QueryAll  { enqueued_at: Instant, query: String, params: Vec<OwnedParam>, reply: ..., session: Option<SessionContext> },
}
```

**2.4.2 Update `PooledLifeExecutor`**
Add field and builder, update dispatch methods.

```rust
pub struct PooledLifeExecutor {
    pool: Arc<LifeguardPool>,
    read_preference: ReadPreference,
    session_context: Option<SessionContext>, // NEW
}

impl PooledLifeExecutor {
    pub fn with_session_context(mut self, ctx: SessionContext) -> Self {
        self.session_context = Some(ctx);
        self
    }
}
```

**2.4.3 Worker Thread Injection**
Update `dispatch_worker_job` in `pooled.rs`.

```rust
fn dispatch_worker_job(tier: &str, conn: &str, client: &mut Client, job: WorkerJob) {
    // Extract session from job and run SET LOCAL if present
    // ...
}
```

---

## 3. Implementation Roadmap (V2)

| Phase | Component | Priority | Est. Effort | Notes |
|-------|-----------|----------|-------------|-------|
| **P1** | `SessionContext` struct in `executor.rs` | Critical | 1 hour | Derive `Clone, Send, PartialEq`. Add `to_sql_args()` with proper error handling. |
| **P2** | `MayPostgresExecutor` augmentation | Critical | 4 hours | Add `session_context` field, `with_session_context()` builder, and wrapped `execute/query_one/query_all`. Update `lib.rs` exports. |
| **P3** | `Transaction` augmentation | Critical | 3 hours | Add `new_with_session()`, `begin_with_session()` on `MayPostgresExecutor`. Inject at `BEGIN`. |
| **P4** | `PooledLifeExecutor` worker job extension | High | 6 hours | Add `session` field to `WorkerJob` variants. Update `with_enqueued_at`. Update `PooledLifeExecutor` dispatch. |
| **P5** | `PooledLifeExecutor` worker thread injection | High | 3 hours | Modify `dispatch_worker_job` to run `SELECT rls_set_session(...)` if `session` is present. |
| **P6** | Integration tests | High | 10 hours | Test 1: Direct executor RLS injection. Test 2: Transaction RLS injection. Test 3: Pool worker RLS pass-through. Test 4: Fail-closed behavior. |
| **P7** | Documentation | Medium | 3 hours | Example showing JWT middleware integration with `PooledLifeExecutor::with_session_context()`. |

**Total Estimated Effort:** ~30 hours

---

## 4. Open Decisions & Clarifications

### 4.1 Serialization Strategy
We are using `serde_json` for the permissions array (`$5`) because `may_postgres` handles JSON well, and it avoids manual array string building. If the consuming app prefers a different serialization, they can map their claims into `SessionContext` before passing it to the executor. The executor is a dumb carrier.

### 4.2 `SET LOCAL` Redundancy in Pool Workers
In the pool path, a worker processes many jobs. If `session` is `Some`, we run `SET LOCAL` before every job. 
- **Pushback considered:** Could we cache the context on the worker connection? 
- **Decision:** No. Sessions change per request. The worker connection might process Job A (User 1) then Job B (User 2). We must inject on every job. Since `SET LOCAL` is transaction-scoped and cheap, per-job injection is correct.

### 4.3 Error Handling in Workers
If `to_sql_args()` fails inside a worker thread, it cannot easily propagate `LifeError` through the existing `dispatch_worker_job` signature without significant restructuring. 
- **Mitigation:** `SessionContext` construction and validation should ideally happen on the main thread *before* the job is dispatched to the pool. If it somehow fails in the worker, we log a warning and proceed without RLS (fail-closed or best-effort).

---

## 5. What is Out of Scope for Lifeguard
- **SQL Functions:** `rls_set_session`, `rls_current_user_*` are shipped by the consuming application as migration files.
- **RLS Policies:** `CREATE POLICY ... USING (...)` is schema-specific and shipped by the consuming application.
- **JWT Parsing:** Extraction of claims from the JWT remains in the application's web/middleware layer.
