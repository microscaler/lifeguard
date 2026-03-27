# Test Infrastructure Analysis: When to Implement Testcontainers

**Date:** 2025-01-XX  
**Purpose:** Determine when to implement Bollard/testcontainers for PostgreSQL integration testing

---

## Current Status

**Completed Stories:**
- ✅ Story 01: Remove SeaORM/Tokio
- ✅ Story 02: Integrate may_postgres
- ✅ Story 03: Implement LifeExecutor
- ✅ Story 04: Execute raw SQL queries (just completed)

**Current Testing:**
- Unit tests only (no database required)
- All tests passing but limited to error handling and validation

---

## When Integration Tests Are Needed

### Story 04: LifeguardPool (Upcoming)

**Requirements:**
- ✅ "Unit tests demonstrate connection acquisition and release"
- ✅ "Load tests show pool handles concurrent requests efficiently"
- ✅ "Health monitoring detects and reconnects failed connections"

**Needs Real Database:** **YES** ❗
- Cannot test connection pooling without actual connections
- Cannot test health monitoring without database to ping
- Cannot test load/performance without real database

**Critical:** This story **REQUIRES** integration tests with PostgreSQL.

---

### Story 06: Transaction Support

**Requirements:**
- ✅ "Unit tests demonstrate transaction usage"
- ✅ Test commit/rollback operations
- ✅ Test nested transactions (savepoints)
- ✅ Test isolation levels

**Needs Real Database:** **YES** ❗
- Cannot test transactions without actual database
- Need to verify data persistence/rollback
- Need to test isolation level behavior

**Critical:** This story **REQUIRES** integration tests with PostgreSQL.

---

### Story 07: Raw SQL Helpers (Just Completed)

**Current Status:**
- ✅ Unit tests for error handling only
- ❌ No integration tests with actual database
- Note in completion doc: "Integration tests will be added in Story 08"

**Needs Real Database:** **YES** (for comprehensive testing)
- Should test actual SQL execution
- Should test parameter binding
- Should test result extraction

---

## Recommendation: Implement Testcontainers NOW

### Why Now?

1. **Story 04 (LifeguardPool) is Next**
   - Cannot be properly tested without real database
   - Connection pooling requires actual connections
   - Health monitoring requires database to ping

2. **Story 06 (Transactions) is Coming Soon**
   - Transactions require real database
   - Need to test commit/rollback behavior

3. **Foundation for All Future Stories**
   - Epic 02 (LifeQuery) will need integration tests
   - Epic 03 (LifeModel/LifeRecord) will need integration tests
   - Better to set up infrastructure early

---

## Implementation Options

### Option 1: testcontainers-rs (Recommended)

**Crate:** `testcontainers` or `testcontainers-rs`

**Pros:**
- ✅ Built-in PostgreSQL support
- ✅ Automatic container lifecycle management
- ✅ Works with Docker (via Docker API)
- ✅ Isolated test environments
- ✅ No manual setup required

**Cons:**
- Requires Docker to be running
- Slightly slower than in-memory databases

**Example:**
```rust
use testcontainers::{clients, images::postgres::Postgres};

#[test]
fn test_connection_pool() {
    let docker = clients::Cli::default();
    let postgres_image = Postgres::default();
    let container = docker.run(postgres_image);
    
    let connection_string = format!(
        "postgresql://postgres:postgres@localhost:{}/postgres",
        container.get_host_port_ipv4(5432)
    );
    
    // Test with real database
    let pool = LifeguardPool::new(&connection_string)?;
    // ... test connection pooling
}
```

---

### Option 2: Bollard + Manual Container Management

**Crate:** `bollard` (Docker client for Rust)

**Pros:**
- ✅ Full control over container lifecycle
- ✅ Can customize container configuration
- ✅ More flexible

**Cons:**
- ❌ More boilerplate code
- ❌ Need to manage container lifecycle manually
- ❌ More complex setup

**Example:**
```rust
use bollard::Docker;

#[test]
fn test_connection_pool() {
    let docker = Docker::connect_with_local_defaults()?;
    
    // Create container
    let container_config = /* ... */;
    let container = docker.create_container(/* ... */, container_config).await?;
    docker.start_container(&container.id, None::<StartContainerOptions>).await?;
    
    // Get connection string
    let connection_string = /* ... */;
    
    // Test with real database
    // ...
    
    // Cleanup
    docker.stop_container(&container.id, None).await?;
    docker.remove_container(&container.id, None).await?;
}
```

---

### Option 3: testcontainers-rs with Bollard Backend

**Crate:** `testcontainers` with `bollard` feature

**Pros:**
- ✅ Best of both worlds
- ✅ Uses testcontainers API (simple)
- ✅ Uses Bollard backend (reliable)

**Cons:**
- Slightly more dependencies

---

## Recommended Approach

### Use `testcontainers-rs` with PostgreSQL Image

**Why:**
1. **Simplest API** - Minimal boilerplate
2. **Automatic Cleanup** - Containers destroyed after tests
3. **Isolated Tests** - Each test gets fresh database
4. **Well Maintained** - Active Rust community support

**Implementation:**
```toml
[dev-dependencies]
testcontainers = "0.15"
testcontainers-modules = { version = "0.15", features = ["postgres"] }
```

---

## When to Implement

### **IMMEDIATELY - Before Story 04 (LifeguardPool)**

**Reasoning:**
1. Story 04 **cannot** be properly tested without real database
2. Connection pooling requires actual connections
3. Health monitoring requires database to ping
4. Load tests require real database performance

**Timeline:**
- **Now:** Set up testcontainers infrastructure
- **Story 04:** Use it for LifeguardPool tests
- **Story 06:** Use it for transaction tests
- **All Future Stories:** Use it for integration tests

---

## Implementation Plan

### Step 1: Add Dependencies

```toml
[dev-dependencies]
testcontainers = "0.15"
testcontainers-modules = { version = "0.15", features = ["postgres"] }
```

### Step 2: Create Test Helper Module

```rust
// src/test_helpers.rs
use testcontainers::{clients, images::postgres::Postgres, Container};

pub fn setup_test_db() -> (Container<Postgres>, String) {
    let docker = clients::Cli::default();
    let postgres_image = Postgres::default();
    let container = docker.run(postgres_image);
    
    let connection_string = format!(
        "postgresql://postgres:postgres@localhost:{}/postgres",
        container.get_host_port_ipv4(5432)
    );
    
    (container, connection_string)
}
```

### Step 3: Use in Story 04 Tests

```rust
#[test]
fn test_connection_pool_acquisition() {
    let (_container, connection_string) = setup_test_db();
    let pool = LifeguardPool::new(&connection_string, 5)?;
    
    // Test connection acquisition
    let executor = pool.acquire()?;
    // ... test operations
}
```

---

## Alternative: Docker Compose for CI/CD

If testcontainers doesn't work well in CI/CD, consider:

1. **Docker Compose** - Pre-spin containers in CI
2. **GitHub Actions Services** - Use PostgreSQL service
3. **Hybrid Approach** - testcontainers locally, Docker Compose in CI

---

## Conclusion

### **Implement Testcontainers NOW (Before Story 04)**

**Critical Path:**
1. ✅ Stories 01-04: Unit tests only (completed)
2. ⚠️ **Story 04 (LifeguardPool): NEEDS integration tests** ← **WE ARE HERE**
3. ⚠️ Story 06 (Transactions): NEEDS integration tests
4. ⚠️ All future stories: Will benefit from integration tests

**Action Items:**
1. Add `testcontainers` to `[dev-dependencies]`
2. Create test helper module (`src/test_helpers.rs`)
3. Set up PostgreSQL test container infrastructure
4. Use in Story 04 tests immediately

**Timeline:** Implement testcontainers **before starting Story 04 (LifeguardPool)**

---

## References

- [testcontainers-rs](https://github.com/testcontainers/testcontainers-rs)
- [testcontainers-modules](https://github.com/testcontainers/testcontainers-modules-rs)
- [Bollard](https://github.com/fussybeaver/bollard)
