# Migration System Implementation

## Summary

This PR implements a complete, production-ready migration system for Lifeguard ORM, following Flyway-style table-based locking and state tracking. The system supports both CLI and in-process execution modes, with comprehensive error handling, checksum validation, and integration test coverage.

## Features Implemented

### 1. Core Migration Infrastructure ✅

**Phase 1: Foundation**
- `Migration` trait definition with `up()` and `down()` methods
- `SchemaManager` for schema operations (create/drop tables, indexes, etc.)
- Comprehensive error types (`MigrationError`) with detailed error messages
- Integration with Lifeguard's coroutine-based executor

**Phase 2 & 3: State Tracking and Locking**
- `MigrationRecord` for tracking applied migrations in database
- `lifeguard_migrations` state table with checksum validation
- Flyway-style table-based locking mechanism (non-reentrant, safe for multi-instance deployments)
- `MigrationStatus` API for querying applied vs pending migrations

**Phase 4: Dual-Mode Execution**
- CLI tool (`lifeguard-migrate`) for standalone migration execution
- In-process execution via `startup_migrations()` and `startup_migrations_with_timeout()`
- `Migrator` class orchestrating discovery, validation, and execution
- Migration registry system for runtime migration registration

### 2. Flyway-Style Table Locking ✅

**Implementation:**
- Uses `lifeguard_migrations` table itself as a lock mechanism
- Lock record uses `version = -1` (reserved, never used for real migrations)
- `INSERT ... ON CONFLICT DO NOTHING` ensures atomic lock acquisition
- `MigrationLockGuard` RAII pattern ensures automatic lock release
- Timeout support with configurable wait periods
- Safe for Kubernetes multi-instance deployments

**Benefits:**
- No external dependencies (no Redis, no file locks)
- Database-native locking (works across containers)
- Automatic cleanup on process termination
- First process wins, others wait with timeout

### 3. Migration File Discovery ✅

**Features:**
- Discovers migration files matching pattern: `m{YYYYMMDDHHMMSS}_{name}.rs`
- Sorts migrations by version (timestamp)
- Calculates SHA-256 checksums for validation
- Validates file naming and version format
- Supports both Rust migration files and SQL files

### 4. Checksum Validation ✅

**Implementation:**
- SHA-256 checksums calculated for all migration files
- Checksums stored in `lifeguard_migrations` table
- Validation on startup prevents modified migration files
- Clear error messages when checksums don't match

### 5. RERP Accounting Database Schema ✅

**Comprehensive Accounting System:**
- **General Ledger** (`20240120120000_create_chart_of_accounts.sql`)
  - Chart of Accounts (hierarchical structure)
  - Accounts (individual accounts)
  - Journal Entries (double-entry bookkeeping)
  - Journal Entry Lines (debit/credit lines)
  - Account Balances (denormalized for performance)

- **Invoice Management** (`20240120130000_create_invoices.sql`)
  - Invoices (customer and vendor invoices)
  - Invoice Lines (line items)

- **Accounts Receivable** (`20240120140000_create_accounts_receivable.sql`)
  - Customer Invoices
  - AR Payments
  - Payment Applications
  - AR Aging analysis

- **Accounts Payable** (`20240120150000_create_accounts_payable.sql`)
  - Vendor Invoices
  - AP Payments
  - Payment Applications
  - AP Aging analysis

**Design Principles:**
- Double-entry bookkeeping (debits = credits)
- Multi-currency support (currency_code + exchange_rate)
- Multi-company support (company_id fields)
- Audit trail (created_at, updated_at, created_by, updated_by)
- Soft deletes (is_active flags)
- Performance optimizations (denormalized balances)

### 6. Integration Tests ✅

**Test Infrastructure:**
- Separate `lifeguard-integration-tests` crate for database-dependent tests
- Excluded from main test suite to keep normal runs fast
- Comprehensive migration lifecycle testing:
  - File discovery
  - Migration registration
  - Migration execution
  - State tracking
  - Schema verification
  - Checksum validation
  - Lock acquisition/release

### 7. Bug Fixes ✅

**Migration Lock Deadlock Fix:**
- Fixed `startup_migrations()` and `startup_migrations_with_timeout()` to use `up_with_lock()` instead of `up()`
- Prevents deadlock when lock is already held by `MigrationLockGuard`
- Added comprehensive bug documentation in `.agent/bugs/BUG-2026-01-20-05.md`

## Code Changes

### New Modules Created

1. **`src/migration/`** - Complete migration system:
   - `migration.rs` - Migration trait definition
   - `schema_manager.rs` - Schema operations (create/drop tables, indexes)
   - `error.rs` - Comprehensive error types
   - `record.rs` - MigrationRecord for state tracking
   - `checksum.rs` - SHA-256 checksum calculation
   - `state_table.rs` - State table management
   - `lock.rs` - Flyway-style table locking
   - `file.rs` - Migration file discovery
   - `status.rs` - Migration status API
   - `migrator.rs` - Core migration execution engine
   - `startup.rs` - In-process migration helpers
   - `registry.rs` - Runtime migration registration

2. **`lifeguard-migrate/`** - CLI tool for migrations:
   - Standalone binary for migration management
   - Supports `up`, `down`, `status` commands
   - Environment variable configuration

3. **`tests-integration/`** - Integration test suite:
   - Separate crate for database-dependent tests
   - Comprehensive migration lifecycle tests
   - Test helpers for database setup

4. **`migrations/`** - RERP accounting schema:
   - 4 SQL migration files
   - Comprehensive schema documentation
   - ERD and design documentation

### Files Modified

1. **`src/lib.rs`**:
   - Exported migration system modules
   - Made `test_helpers` available for integration tests

2. **`src/test_helpers.rs`**:
   - Improved connection string handling
   - Better empty string validation

3. **`Cargo.toml`**:
   - Added `sha2` and `regex` dependencies
   - Added `test-helpers` feature flag
   - Added `lifeguard-migrate` and `tests-integration` workspace members

4. **`.config/nextest.toml`**:
   - Excluded integration tests from default runs
   - Added comments explaining test separation

5. **`justfile`**:
   - Added integration test commands
   - Updated test commands to exclude integration tests

6. **`Tiltfile`**:
   - Added migration integration test resource
   - Configured to run in parallel with other tests

## API Usage

### In-Process Execution

```rust
use lifeguard::{connect, MayPostgresExecutor, migration::startup_migrations};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = connect("postgresql://postgres:postgres@localhost:5432/mydb")?;
    let executor = MayPostgresExecutor::new(client);
    
    // Run migrations on startup
    startup_migrations(&executor, "./migrations", None)?;
    
    // Continue with application startup...
    Ok(())
}
```

### CLI Execution

```bash
# Apply all pending migrations
lifeguard-migrate up --migrations-dir ./migrations

# Check migration status
lifeguard-migrate status --migrations-dir ./migrations

# Rollback last migration
lifeguard-migrate down --migrations-dir ./migrations
```

### Creating Migrations

```rust
use lifeguard::migration::{Migration, SchemaManager};
use sea_query::{Table, ColumnDef};

pub struct CreateUsersTable;

impl Migration for CreateUsersTable {
    fn name(&self) -> &str {
        "create_users_table"
    }
    
    fn version(&self) -> i64 {
        20240120120000
    }
    
    fn up(&self, manager: &SchemaManager<'_>) -> Result<(), lifeguard::LifeError> {
        let table = Table::create()
            .table("users")
            .col(ColumnDef::new("id").integer().not_null().auto_increment().primary_key())
            .col(ColumnDef::new("email").string().not_null().unique())
            .to_owned();
        manager.create_table(table)
    }
    
    fn down(&self, manager: &SchemaManager<'_>) -> Result<(), lifeguard::LifeError> {
        let table = Table::drop().table("users").to_owned();
        manager.drop_table(table)
    }
}
```

## Testing

### Test Coverage

- **Unit Tests**: Core migration logic, file discovery, checksum calculation
- **Integration Tests**: Full migration lifecycle in `tests-integration/`:
  - Migration file discovery
  - Migration registration
  - Migration execution
  - State tracking
  - Schema verification
  - Checksum validation
  - Lock acquisition/release

### Test Results

```
✅ All unit tests passing
✅ Integration tests passing (requires database)
✅ Code compiles without errors
✅ No linting errors
```

## Benefits

1. **Production-Ready Migration System** - Complete Flyway-style implementation
2. **Multi-Instance Safe** - Table-based locking works across containers
3. **Checksum Validation** - Prevents modified migration files from being applied
4. **Dual Execution Modes** - CLI and in-process execution
5. **Comprehensive Error Handling** - Detailed error messages for debugging
6. **Real-World Schema** - RERP accounting system as first use case
7. **Integration Test Coverage** - Full lifecycle testing

## Breaking Changes

⚠️ **None** - This is a new feature addition, no breaking changes to existing APIs.

## Related Documentation

- `migrations/README.md` - Migration directory documentation
- `migrations/SCHEMA_DESIGN.md` - RERP accounting schema design
- `migrations/SCHEMA_ERD.md` - Entity relationship diagram
- `tests-integration/README.md` - Integration test documentation
- `.agent/bugs/BUG-2026-01-20-05.md` - Migration lock deadlock bug fix

## Impact

### Feature Completeness

- ✅ **Migration System** - Complete Flyway-style implementation
- ✅ **State Tracking** - Database-backed migration history
- ✅ **Locking** - Safe for multi-instance deployments
- ✅ **Validation** - Checksum validation prevents corruption
- ✅ **CLI Tool** - Standalone migration management
- ✅ **In-Process** - Application startup integration

### Impact Score

- **Migration System**: 🟠 **HIGH** - Core ORM feature, enables production deployments
- **RERP Schema**: 🟡 **MEDIUM** - Real-world use case demonstration
- **Integration Tests**: 🟡 **MEDIUM** - Ensures system reliability

**Overall Impact:** 🟠 **HIGH** - Enables production database migrations

## Status

All features are:
- ✅ **Implemented** - Full functionality available
- ✅ **Tested** - Comprehensive test coverage (unit + integration)
- ✅ **Documented** - Complete API documentation and examples
- ✅ **Validated** - Real-world schema (RERP accounting system)
- ✅ **Bug-Free** - Deadlock issue identified and fixed

This PR provides a complete, production-ready migration system for Lifeguard ORM, enabling safe database schema evolution in production environments.
