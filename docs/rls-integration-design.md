# Design Doc: PostgreSQL RLS Integration for Lifeguard

> **Status:** Draft
> **Date:** 2026-01-04
> **Scope:** Database-level Row Level Security (RLS) integration into Lifeguard's execution layer

---

## 1. Context

Multi-tenant consumers of Lifeguard (Hauliage, future projects) require **stateful visibility**: the set of rows visible to a user changes based on the lifecycle state of a record, not just their organizational membership.

Example: A `jobs` row is globally visible when `status = 'open'` (allowing transporters to bid), but must restrict visibility to only the shipper and allocated transporter once `status = 'allocated'`.

The trust boundary for RLS is external to Lifeguard:
1. An **identity provider** (Sesame-IDAM, Auth0, custom) issues a JWT containing verified claims (`user_id`, `user_org_id`, `user_type`, `org_type`, `permissions`)
2. These claims are injected into the PostgreSQL session as `SET LOCAL` variables before each transaction
3. PostgreSQL RLS policies evaluate these session variables transparently
4. **Lifeguard automates the injection** — the consuming application never writes `SET LOCAL` or RLS-dependent SQL by hand

**Why this lives in Lifeguard:** Without it, every consuming app must manually manage session variable lifecycle on every executor path (direct client, transaction, pooled worker). This is repetitive, error-prone, and duplicates across applications. Lifeguard provides the infrastructure; the identity provider and RLS policies are application-specific.

---

## 2. Current Architecture

Lifeguard currently provides:

| Component | Location | Purpose | RLS Relevance |
|---|---|---|---|
| `LifeExecutor` trait | `src/executor.rs` | Abstraction over `may_postgres::Client` (`execute`, `query_one`, `query_all`) | **Foundation** — RLS wrapper must implement this trait |
| `Transaction` | `src/transaction.rs` | ACID transactions with savepoints, implements `LifeExecutor` | **Injection point** — `SET LOCAL` can happen at `BEGIN` |
| `PooledLifeExecutor` | `src/pool/pooled.rs` | Multi-worker connection pool with primary/replica routing | **Routing** — needs RLS context attached to dispatched jobs |
| `Session` / `ModelIdentityMap` | `src/session/mod.rs` | ORM-level identity map and dirty tracking | **Data carrier** — can hold `SessionContext` between JWT parsing and DB query |
| `raw_sql` helpers | `src/raw_sql.rs` | Thin wrappers around `LifeExecutor` | **Transparent** — already use the trait, will inherit RLS automatically |

**Current Gap:** Zero code exists for:
- Injecting session variables (`SET LOCAL auth.user_id = '...'`)
- Running any session setup function or reading `current_setting()`
- Attaching session context to pooled worker jobs
- Shipping or verifying RLS policy DDL

---

## 3. Gap Analysis

| # | Gap | Severity | Description |
|---|---|---|---|
| 1 | **No Session Variable Injection** | **Critical** | No mechanism calls `SET LOCAL` before transactions or individual queries |
| 2 | **No PostgreSQL RLS Helper Functions** | **Critical** | SQL functions for setting/reading session variables don't exist in target databases |
| 3 | **No RLS-Enabled Executor Wrapper** | **Critical** | No executor exists that wraps `LifeExecutor` and injects session context |
| 4 | **`PooledLifeExecutor` is RLS-Blind** | **High** | Dispatched jobs carry no session context; workers execute raw SQL without RLS scoping |
| 5 | **No Generic Session Context Abstraction** | **High** | No `SessionContext` struct exists; no way to pass verified claims from identity provider to the database |
| 6 | **No RLS Policy Templates** | **Medium** | No mechanism to ship or verify `CREATE POLICY` statements for consuming apps |
| 7 | **No RLS Test Infrastructure** | **High** | Cannot test `SET LOCAL` scoping, RLS policy enforcement, or connection pool isolation |

---

## 4. Proposed Architecture

### 4.1 Phase 1: PostgreSQL Helper Functions (Identity Provider / Consuming App Ownership)

Each consuming application ships these SQL functions as migration files. The functions are minimal, schema-agnostic, and application-specific — they define the session variable namespace and provide safe getter patterns for RLS policies.

```sql
-- Sets all session variables in one call (atomic within transaction)
-- The function name and variable namespace are application-defined.
-- This example uses 'auth' as the namespace (common convention).
CREATE OR REPLACE FUNCTION public.rls_set_session(
    p_user_id       uuid        DEFAULT NULL,
    p_user_org_id   uuid        DEFAULT NULL,
    p_user_type     text        DEFAULT NULL,
    p_org_type      text        DEFAULT NULL,
    p_permissions   text[]      DEFAULT '{}',
    p_user_email    text        DEFAULT NULL
) RETURNS void
LANGUAGE sql
SET LOCAL search_path = public
AS $$
    SELECT SET LOCAL 'auth.user_id'::text, COALESCE(p_user_id::text, '');
    SELECT SET LOCAL 'auth.user_org_id'::text, COALESCE(p_user_org_id::text, '');
    SELECT SET LOCAL 'auth.user_type'::text, COALESCE(p_user_type, '');
    SELECT SET LOCAL 'auth.org_type'::text, COALESCE(p_org_type, '');
    SELECT SET LOCAL 'auth.permissions'::text, COALESCE(p_permissions::text, '{}');
    SELECT SET LOCAL 'auth.user_email'::text, COALESCE(p_user_email, '');
$$;

-- Convenience getters for RLS policy templates
CREATE OR REPLACE FUNCTION rls_current_user_id()       RETURNS uuid       LANGUAGE sql AS $$ SELECT NULLIF(current_setting('auth.user_id', true), '')::uuid; $$;
CREATE OR REPLACE FUNCTION rls_current_user_org_id()   RETURNS uuid       LANGUAGE sql AS $$ SELECT NULLIF(current_setting('auth.user_org_id', true), '')::uuid; $$;
CREATE OR REPLACE FUNCTION rls_current_user_type()     RETURNS text       LANGUAGE sql AS $$ SELECT NULLIF(current_setting('auth.user_type', true), '')::text; $$;
CREATE OR REPLACE FUNCTION rls_current_org_type()      RETURNS text       LANGUAGE sql AS $$ SELECT NULLIF(current_setting('auth.org_type', true), '')::text; $$;
CREATE OR REPLACE FUNCTION rls_current_permissions()   RETURNS text[]     LANGUAGE sql AS $$ SELECT NULLIF(current_setting('auth.permissions', true), '')::text[]; $$;
CREATE OR REPLACE FUNCTION rls_current_user_email()    RETURNS text       LANGUAGE sql AS $$ SELECT NULLIF(current_setting('auth.user_email', true), '')::text; $$;
```

**Key Design Decisions:**
- Function name (`rls_set_session`) and variable namespace (`auth.*`) are **application-defined conventions**, not Lifeguard mandates. Lifeguard's executor wrapper supports any function name via configuration.
- `SET LOCAL` inside function ensures variables are transaction-scoped and automatically cleaned up
- `current_setting(..., true)` prevents errors if variable isn't set (returns `NULL` instead of raising)
- Default values are `NULL`/empty — no assumed roles. The consuming app decides what makes sense

**Lifeguard's Role:** The executor wrapper (Phase 2) accepts a configurable function name (default: `rls_set_session`) so applications can rename it or use a different convention. Lifeguard does **not** ship or enforce this function.

### 4.2 Phase 2: RLS-Enabled Executor Wrapper (Lifeguard Ownership)

A thin wrapper type that implements `LifeExecutor` and injects session context.

```rust
/// Verified claims from the identity provider.
/// Lifeguard does not create these — the consuming app extracts them from the JWT
/// and passes them here.
pub struct SessionContext {
    pub user_id: Uuid,
    pub user_org_id: Uuid,
    pub user_type: String,
    pub org_type: String,
    pub permissions: Vec<String>,
    pub user_email: Option<String>,
}

/// An executor that injects session context into every database call.
/// Wraps any `LifeExecutor` and runs the RLS session setup function
/// before delegating to the inner executor.
///
/// # Example
///
/// ```no_run
/// let executor = MayPostgresExecutor::new(client);
/// let ctx = SessionContext { user_id: ..., user_org_id: ..., .. };
/// let rls_exec = RlsExecutor::new(&executor, ctx);
/// let users = User::find().all(&rls_exec)?;
/// ```
pub struct RlsExecutor<'a, E: LifeExecutor> {
    inner: &'a E,
    context: SessionContext,
    /// Configurable function name; defaults to "rls_set_session"
    set_session_fn: String,
}

impl<'a, E: LifeExecutor> RlsExecutor<'a, E> {
    pub fn new(inner: &'a E, context: SessionContext) -> Self {
        Self {
            inner,
            context,
            set_session_fn: "rls_set_session".to_string(),
        }
    }

    /// Set a custom session setup function name (e.g. if the app renamed it)
    pub fn with_set_session_fn(mut self, name: &str) -> Self {
        self.set_session_fn = name.to_string();
        self
    }

    fn run_with_session<F, T>(&self, f: F) -> Result<T, LifeError>
    where
        F: FnOnce() -> Result<T, LifeError>,
    {
        // Build SET LOCAL call — parameters serialized via sea_query::Value path
        let sql = format!(
            "SELECT {}($1, $2, $3, $4, $5, $6)",
            self.set_session_fn
        );
        
        // Execute session setup
        self.inner.execute(&sql, &[
            &self.context.user_id,
            &self.context.user_org_id,
            &self.context.user_type,
            &self.context.org_type,
            &serde_json::to_string(&self.context.permissions).unwrap_or("[]".into()),
            &self.context.user_email.as_deref().unwrap_or("")
        ])?;
        
        // Execute wrapped operation
        f()
    }
}

impl LifeExecutor for RlsExecutor<'_, dyn LifeExecutor> {
    fn execute(&self, query: &str, params: &[&dyn ToSql]) -> Result<u64, LifeError> {
        self.run_with_session(|| self.inner.execute(query, params))
    }
    fn query_one(&self, query: &str, params: &[&dyn ToSql]) -> Result<Row, LifeError> {
        self.run_with_session(|| self.inner.query_one(query, params))
    }
    fn query_all(&self, query: &str, params: &[&dyn ToSql]) -> Result<Vec<Row>, LifeError> {
        self.run_with_session(|| self.inner.query_all(query, params))
    }
}
```

**Design Rationale:**
- `run_with_session()` wraps each executor method — simple, explicit, correct for direct clients
- For transactions: wrap at `Transaction::begin()` instead of every method call (avoids redundant `SET LOCAL`)
- `set_session_fn` is configurable — if an app renames the SQL function, they tell the executor, no code change needed
- `SessionContext` is a **generic container** — not tied to any identity provider. Any app can construct it from any JWT claims format
- Lifeguard does **not** parse JWTs or extract claims — that stays in the application's web/middleware layer

### 4.3 Phase 3: `PooledLifeExecutor` Integration (Lifeguard Ownership)

When using the pool, session context must travel with dispatched jobs. The pool's worker model requires a different approach than the direct-client wrapper.

```rust
// In src/pool/pooled.rs — extend WorkerJob variants
enum WorkerJob {
    Execute { sql: String, params: Vec<Value>, reply: ... },
    QueryOne { sql: String, params: Vec<Value>, reply: ... },
    QueryAll { sql: String, params: Vec<Value>, reply: ... },
    // NEW: session context field attached to each variant
    Execute { sql: String, params: Vec<Value>, session: Option<SessionContext>, reply: ... },
    QueryOne { sql: String, params: Vec<Value>, session: Option<SessionContext>, reply: ... },
    QueryAll { sql: String, params: Vec<Value>, session: Option<SessionContext>, reply: ... },
}
```

**Worker Thread Lifecycle (existing):**
1. Worker blocks on `crossbeam_channel::Receiver::recv()`
2. Receives `WorkerJob`
3. Executes job on its `may_postgres::Client`
4. Sends reply back

**RLS Integration Point (within existing worker):**
After receiving `WorkerJob` but before executing SQL:

```rust
// Inside the worker's job execution loop:
match &job {
    WorkerJob::QueryAll { session, .. } |
    WorkerJob::QueryOne { session, .. } |
    WorkerJob::Execute { session, .. } => {
        if let Some(ctx) = session {
            self.client.execute(
                "SELECT rls_set_session($1, $2, $3, $4, $5, $6)",
                &[&ctx.user_id, &ctx.user_org_id, ...]
            )?;
        }
    }
}
```

**Why attach to `WorkerJob` rather than wrap with `RlsExecutor`?**
Because `PooledLifeExecutor` dispatches async jobs to worker threads. Wrapping it would require boxing async closures and adding latency. Attaching context to `WorkerJob` is zero-overhead for non-RLS callers (they pass `session: None`) and handles the concurrent worker scenario correctly.

**Exposing it to consumers:**
```rust
impl PooledLifeExecutor {
    /// Create an executor that carries session context on dispatched jobs.
    /// Non-RLS callers can use `Self::new(pool)` as before.
    pub fn with_session_context(self, context: SessionContext) -> Self {
        Self {
            pool: self.pool,
            read_preference: self.read_preference,
            session_context: Some(context),
        }
    }
}
```

### 4.4 Phase 4: RLS Policy Templates (Consuming App Ownership)

RLS policies are **schema-specific** — they reference table names, column names, and visibility logic that are unique to each application. Lifeguard does not generate or ship policy DDL.

Instead, the consuming application writes policies using the helper functions. The helper functions (`rls_current_user_*`) are the **only** database-side primitive Lifeguard provides. Everything else is the application's responsibility.

```sql
-- Example: Hauliage jobs table policy
-- Written by the Hauliage application, not Lifeguard
CREATE POLICY jobs_visibility ON public.jobs
    FOR ALL
    USING (
        status = 'open'
        OR shipper_org_id = rls_current_user_org_id()
        OR allocated_transporter_user_id = rls_current_user_id()
    );

-- Example: Bids table policy
CREATE POLICY bids_visibility ON public.bids
    FOR ALL
    USING (
        transporter_user_id = rls_current_user_id()
        OR EXISTS (
            SELECT 1 FROM public.jobs j
            WHERE j.id = job_id
            AND j.shipper_org_id = rls_current_user_org_id()
        )
    );
```

---

## 5. Ownership Split

| Responsibility | Owner | Rationale |
|---|---|---|
| JWT parsing & claim extraction | **Consuming Application** | Identity provider is external to Lifeguard; claim format varies by provider |
| `SessionContext` struct | **Lifeguard** (defines) / **App** (populates) | Lifeguard provides the container; app fills it with verified claims |
| `RlsExecutor` wrapper | **Lifeguard** | Generic ORM infrastructure, reusable by any app with any identity provider |
| `PooledLifeExecutor` RLS integration | **Lifeguard** | Pool internals, worker lifecycle management |
| `rls_set_session()` SQL function | **Consuming Application** | Application chooses namespace, variable names, and default values |
| RLS policy DDL (`CREATE POLICY`) | **Consuming Application** | Schema-specific, references app tables and column names |

**Key Principle:** Lifeguard owns the **infrastructure** (executor wrapper, session injection, pool integration). The consuming application owns everything **above** the database — JWT parsing, claim extraction, SQL function naming, and RLS policy DDL. Lifeguard never assumes a specific identity provider, JWT format, or claim namespace.

---

## 6. Implementation Roadmap

| Phase | Component | Priority | Est. Effort |
|---|---|---|---|
| **P1** | `SessionContext` struct | Critical | 2 hours |
| **P2** | `RlsExecutor` wrapper for direct `LifeExecutor` | Critical | 6 hours |
| **P3** | `PooledLifeExecutor` worker job extension + session context attachment | High | 8 hours |
| **P4** | Integration tests: `SET LOCAL` scoping, pool isolation, session context pass-through | High | 10 hours |
| **P5** | Documentation: example showing integration with a JWT middleware | Medium | 3 hours |

**Total Estimated Effort:** ~29 hours

**Note:** The SQL functions (`rls_set_session`, `rls_current_*`) are **not Lifeguard work**. They are shipped by the consuming application. Lifeguard only provides the wrapper that calls them.

---

## 7. Open Decisions

### 7.1: Session Injection Granularity
Should `RlsExecutor` inject `SET LOCAL` on **every executor method call** or only at **transaction boundaries**?

- **Every call:** Simpler, works correctly with connection pooling (each `SET LOCAL` is transaction-scoped, PostgreSQL handles it efficiently)
- **Transaction boundary:** Fewer round-trips, but requires wrapping `Transaction::begin()` and tracking state across the transaction lifetime
- **Recommendation:** Inject on every call for direct executors. Inject at `Transaction::begin()` for explicit transaction blocks. Pool workers always inject per-job (as designed in 4.3).

### 7.2: Handling Missing/Invalid Claims
What should happen if claims are `None` or an empty context is passed?

- `SET LOCAL 'auth.user_id' = ''` — RLS policies using `current_setting('auth.user_id', true)` will return `NULL`, causing row visibility to be empty (safe fail-closed)
- **Recommendation:** Fail-closed. If claims are missing, the user sees 0 rows rather than all rows. This prevents privilege escalation bugs.

### 7.3: Default `org_type` Value
Should `org_type` default to a specific role (e.g. `'consumer'`) or be empty/`NULL`?

- **Empty/`NULL`:** Semantically accurate when the identity provider doesn't send this claim. RLS policies must handle `NULL` explicitly.
- **Default role:** Easier for apps that don't need org-level scoping. But assumes a role the app may not have.
- **Recommendation:** Default to empty/`NULL`. Let the consuming application decide. If they use a 3-persona model, they set the default at the identity provider level, not in Lifeguard.

### 7.4: Configurable Function Name
Should Lifeguard hardcode the session setup function name or make it configurable?

- **Hardcoded `rls_set_session`:** Simpler API, convention-driven
- **Configurable:** Allows apps to rename, migrate from custom functions, or use different namespaces
- **Recommendation:** Configurable via `with_set_session_fn()` (as designed in 4.2). Default is `rls_set_session` for discoverability, but the override path exists.

### 7.5: Session Context Lifespan
How long should `SessionContext` live? Should it be cloneable? Sendable?

- **Per-request:** Created per HTTP request, cloned into the executor, dropped at request end
- **Clone:** Required because the executor borrows it, but the executor may be passed across function boundaries
- **Recommendation:** Derive `Clone + Send` on `SessionContext`. It's small (UUIDs + strings), cheap to clone, and must cross thread boundaries in the pool worker path.

---

## 8. Summary

This design adds PostgreSQL RLS support to Lifeguard as a **generic capability**. The core mechanism is an `RlsExecutor` wrapper (for direct clients) and session context attachment (for pooled workers), both injecting verified claims as transaction-scoped `SET LOCAL` variables. PostgreSQL RLS policies then read these variables to enforce row-level visibility transparently.

The architecture is:
- **Generic:** Zero assumptions about identity providers, JWT formats, or claim namespaces
- **Minimal:** Zero overhead for non-RLS callers (use `PooledLifeExecutor::new()` as before, don't use `RlsExecutor` for direct executors)
- **Composable:** Wrappers layer cleanly on top of existing `LifeExecutor` implementations
- **Fail-closed:** Missing claims → empty results, never unintended visibility
- **Configurable:** Session function name, claim mapping, and pool integration are all extensible

Ready for implementation review.
