# Migration Prerequisites Discovery

## Executive Summary

**Key Finding:** The core infrastructure for migrations is **already implemented**. The missing derive macros listed in `SEAORM_LIFEGUARD_MAPPING.md` (lines 52-65) are **NOT prerequisites** for basic migration functionality. They are either:
1. **Not needed** (Lifeguard uses different patterns)
2. **Nice-to-have** (enhancements, not blockers)
3. **Future features** (can be added incrementally)

**Recommendation:** Proceed with migration implementation. The Phase 1 migration attributes (`default_expr`, `renamed_from`, `schema_name`) are already implemented and ready to use.

---

## Analysis: What Migrations Actually Need

### Core Migration Requirements

To generate migration SQL (CREATE TABLE, ALTER TABLE, etc.), a migration system needs:

1. **Entity Metadata**
   - ✅ Table name (`Entity::table_name()` via `LifeEntityName`)
   - ✅ Schema name (`Entity::schema_name()` via `LifeEntityName`)
   - ✅ All columns (`E::all_columns()` via `LifeModelTrait`)

2. **Column Metadata**
   - ✅ Column type (`ColumnDefinition::column_type`)
   - ✅ Nullability (`ColumnDefinition::nullable`)
   - ✅ Default expressions (`ColumnDefinition::default_expr`)
   - ✅ Auto-increment (`ColumnDefinition::auto_increment`)
   - ✅ Column renames (`ColumnDefinition::renamed_from`)
   - ✅ Comments (`ColumnDefinition::comment`)

3. **SQL Generation**
   - ✅ `ColumnDefinition::to_column_def()` - Converts to SeaQuery `ColumnDef`
   - ✅ `ColumnDefinition::apply_default_expr()` - Applies default expressions
   - ✅ `ColumnDefinition::comment_sql()` - Generates COMMENT ON COLUMN SQL
   - ✅ Type mapping (`type_mapping::apply_column_type()`)

4. **Primary Key Information**
   - ✅ Primary key columns (`PrimaryKey` enum)
   - ✅ Auto-increment status (`PrimaryKeyTrait::auto_increment()`)

**Status:** ✅ **ALL CORE REQUIREMENTS ARE IMPLEMENTED**

---

## Missing Derive Macros Analysis

### From `SEAORM_LIFEGUARD_MAPPING.md` Lines 52-65

| Derive Macro | Status | Needed for Migrations? | Reason |
|--------------|--------|------------------------|--------|
| `DeriveModelEx` | ❌ Missing | ❌ **NO** | Complex model with relational fields - not needed for basic migrations |
| `DeriveActiveModel` | ❌ Missing | ❌ **NO** | Lifeguard uses `LifeRecord`, not ActiveModel - different pattern |
| `DeriveActiveModelEx` | ❌ Missing | ❌ **NO** | Complex ActiveModel - not needed for migrations |
| `DeriveIntoActiveModel` | ❌ Missing | ❌ **NO** | Model → ActiveModel conversion - not needed for migrations |
| `DeriveActiveEnum` | ❌ Missing | ❌ **NO** | Enum support for ActiveModel - not needed for migrations |
| `DeriveMigrationName` | ❌ Missing | 🟡 **NICE-TO-HAVE** | Migration name generation - convenience feature, not a blocker |
| `DeriveValueType` | ❌ Missing | ❌ **NO** | ValueType trait for wrapper types - not needed for migrations |
| `DeriveDisplay` | ❌ Missing | ❌ **NO** | Display trait for ActiveEnum - not needed for migrations |
| `DeriveIden` | ❌ Missing | ❌ **NO** | Iden trait helper - `LifeModel` already generates Iden/IdenStatic |

**Conclusion:** **NONE of these are prerequisites for migrations.**

---

## What's Already Implemented (Migration-Ready)

### ✅ Phase 1: Critical Migration Attributes

1. **`default_expr`** - ✅ **COMPLETE**
   - Parsed by `LifeModel` macro
   - Stored in `ColumnDefinition::default_expr`
   - `apply_default_expr()` helper method available
   - Ready for migration SQL generation

2. **`renamed_from`** - ✅ **COMPLETE**
   - Parsed by `LifeModel` macro
   - Stored in `ColumnDefinition::renamed_from`
   - Ready for ALTER TABLE RENAME COLUMN migrations

3. **`schema_name`** - ✅ **COMPLETE**
   - Parsed by `LifeModel` macro
   - Generated as `Entity::schema_name()` method
   - Query builders use schema-qualified table names
   - Ready for schema-aware migrations

### ✅ Core Migration Infrastructure

1. **`ColumnDefinition::to_column_def()`** - ✅ **COMPLETE**
   - Converts column metadata to SeaQuery `ColumnDef`
   - Handles type mapping, nullability, auto-increment
   - Ready for CREATE TABLE generation

2. **`ColumnDefinition::apply_default_expr()`** - ✅ **COMPLETE**
   - Applies default SQL expressions to `ColumnDef`
   - Uses static string cache to prevent memory leaks
   - Ready for default expression handling

3. **`ColumnDefinition::comment_sql()`** - ✅ **COMPLETE**
   - Generates PostgreSQL COMMENT ON COLUMN SQL
   - Ready for column documentation in migrations

4. **Type Mapping** - ✅ **COMPLETE**
   - `type_mapping::apply_column_type()` converts string types to SeaQuery types
   - Supports all common types (Integer, String, Json, Timestamp, etc.)

---

## Migration State Tracking: Deep Dive & Optimal Design

### Industry Standard Patterns

Migration state tracking is critical for production deployments. Industry-leading tools use database tables to track applied migrations, ensuring idempotency and preventing duplicate execution.

#### Tool Comparison

| Tool | State Table | Key Features | Locking Mechanism |
|------|------------|--------------|-------------------|
| **Flyway** | `flyway_schema_history` | Checksums, execution time, success status | Table-level lock during execution |
| **Rails** | `schema_migrations` | Simple version tracking | Advisory locks (PostgreSQL/MySQL) |
| **SeaORM** | `seaql_migrations` | Version, name, applied_at | Database transactions (no explicit lock) |
| **Loco-rs** | `seaql_migrations` | Inherits SeaORM pattern | Inherits SeaORM logic |

#### Optimal Design for Lifeguard

**State Tracking Table Schema: `lifeguard_migrations`**

```sql
CREATE TABLE lifeguard_migrations (
    version BIGINT PRIMARY KEY,           -- Timestamp: YYYYMMDDHHMMSS
    name VARCHAR(255) NOT NULL,            -- Human-readable name
    checksum VARCHAR(64) NOT NULL,         -- SHA-256 hash of migration content
    applied_at TIMESTAMP NOT NULL,        -- When migration was executed
    execution_time_ms INTEGER,            -- Duration in milliseconds
    success BOOLEAN NOT NULL DEFAULT true -- Whether migration succeeded
);

CREATE INDEX idx_lifeguard_migrations_applied_at ON lifeguard_migrations(applied_at);
```

**Key Design Decisions:**

1. **Checksum Validation** (Flyway-inspired)
   - Store SHA-256 hash of migration file content
   - On startup, validate checksums of all applied migrations
   - **Error if checksum mismatch:** Prevents silent schema drift from edited migration files
   - **Rationale:** Critical for production safety - ensures migration files haven't been modified after deployment

2. **Success Status Tracking**
   - Track whether migration completed successfully
   - Enables recovery from partial failures
   - **Use case:** If migration fails mid-execution, mark as `success = false` to prevent re-running

3. **Execution Time Tracking**
   - Useful for performance monitoring and debugging
   - Helps identify slow migrations that need optimization

### Concurrency & Locking Strategy

For **in-process execution** in distributed environments (Kubernetes, multiple app instances), we need locking to prevent concurrent migration execution.

#### Locking Mechanisms

**1. PostgreSQL: Advisory Locks (Recommended)**
```rust
// Acquire lock before migration
SELECT pg_advisory_lock(123456);  -- Unique lock ID for migrations

// Run migrations...

// Release lock after migration
SELECT pg_advisory_unlock(123456);
```

**Advantages:**
- Database-native, no additional table needed
- Automatically released on connection close (crash-safe)
- Non-blocking: other processes wait, don't fail

**2. Generic Lock Table (Fallback for SQLite/MySQL)**
```sql
CREATE TABLE lifeguard_migration_lock (
    id INTEGER PRIMARY KEY DEFAULT 1,
    locked BOOLEAN NOT NULL DEFAULT false,
    locked_by VARCHAR(255),        -- Process ID / hostname
    locked_at TIMESTAMP,
    CHECK (id = 1)                 -- Only one row allowed
);
```

**Lock Acquisition Logic:**
```rust
// Try to acquire lock
UPDATE lifeguard_migration_lock 
SET locked = true, 
    locked_by = 'hostname:pid', 
    locked_at = NOW()
WHERE id = 1 AND locked = false;

// If UPDATE affected 0 rows, another process has the lock
// Wait and retry, or fail with clear error message
```

**3. Hybrid Approach (Optimal)**
- Use advisory locks for PostgreSQL (preferred)
- Fall back to lock table for other databases
- Abstract via `MigrationLock` trait for database-specific implementations

### Dual-Mode Execution Architecture

#### Mode 1: Out-of-Band CLI Execution

**Use Cases:**
- CI/CD pipelines (run migrations before deploying new code)
- Manual database management
- Production deployments with separate migration step

**CLI Commands:**
```bash
lifeguard migrate status      # Show applied vs pending migrations
lifeguard migrate up          # Apply all pending migrations
lifeguard migrate down <n>    # Rollback last N migrations
lifeguard migrate fresh       # Drop all tables and re-run all migrations
lifeguard migrate validate    # Check checksums of applied migrations
```

**Implementation:**
- Standalone binary or `cargo` subcommand
- Uses same `Migrator` trait as in-process execution
- Can be run independently of application

#### Mode 2: In-Process Execution (Application Startup)

**Use Cases:**
- Self-migrating applications
- Development environments
- Single-instance deployments
- Containerized apps that need automatic schema updates

**Implementation Pattern:**
```rust
use lifeguard::migration::Migrator;

// On application startup
async fn startup_migrations(db: &dyn LifeExecutor) -> Result<(), LifeError> {
    // Acquire lock (prevents concurrent execution in multi-instance deployments)
    let lock = MigrationLock::acquire(db).await?;
    
    // Validate checksums of already-applied migrations
    Migrator::validate_checksums(db).await?;
    
    // Apply pending migrations
    Migrator::up(db, None).await?;
    
    // Lock automatically released when `lock` is dropped
    Ok(())
}
```

**Lock Behavior:**
- First process to start acquires the lock
- Other processes wait (with timeout) or skip migration if lock is held
- Lock automatically released on process exit (advisory locks) or explicit release

**Deployment Considerations for Kubernetes/Containerized Environments:**

For in-process migrations in Kubernetes, the migrations directory must be:
1. **Packaged in OCI Container:** Migration files are included in the container image
2. **Read-Only Mount:** Migrations are mounted as read-only volumes to prevent accidental modifications
3. **Immutable Deployment:** Ensures migration files match the application version
4. **SRE Inspectability:** Migration files remain easily accessible for inspection and debugging

**Container Structure:**
```dockerfile
# Dockerfile example
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
WORKDIR /app
# Copy compiled binary
COPY --from=builder /app/target/release/myapp /app/myapp
# Copy migration files as read-only resources
COPY --chmod=444 migrations/ /app/migrations/
# Mark as read-only
RUN chmod -R 444 /app/migrations/
```

**Kubernetes Deployment Pattern:**
```yaml
# Kubernetes deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: myapp
spec:
  template:
    spec:
      containers:
      - name: myapp
        image: myapp:latest
        volumeMounts:
        - name: migrations
          mountPath: /app/migrations
          readOnly: true
      volumes:
      - name: migrations
        # Migrations are baked into the container image
        # Mount the container's filesystem as read-only
        # OR use a ConfigMap/Secret if migrations need to be external
```

**Benefits of File-Based Approach:**
- ✅ **SRE Inspectability:** Migration files are easily accessible for inspection
- ✅ **Version Control:** Migration files match application version in container
- ✅ **Debugging:** Can inspect migration files directly in running containers
- ✅ **Audit Trail:** Migration files visible in container filesystem
- ✅ **Read-Only Safety:** Mounted as read-only prevents accidental modifications
- ✅ **Consistent:** Same approach for both CLI and in-process execution

### Migration Lifecycle

**1. Migration Generation**
```rust
// Generate new migration file
lifeguard migrate generate create_users_table

// Creates: migrations/m20240120120000_create_users_table.rs
```

**2. Migration Definition**
```rust
use sea_query::{Iden, Table};

pub struct Migration;

impl lifeguard::Migration for Migration {
    fn name(&self) -> &str {
        "create_users_table"
    }
    
    fn version(&self) -> i64 {
        20240120120000  // YYYYMMDDHHMMSS
    }
    
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(User::Table)
                    .col(...)
                    .to_owned(),
            )
            .await
    }
    
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(User::Table).to_owned())
            .await
    }
}
```

**3. Checksum Calculation**
```rust
// Calculate checksum from migration file content
fn calculate_checksum(migration_file_path: &Path) -> Result<String, LifeError> {
    use sha2::{Sha256, Digest};
    use std::fs;
    
    // Read migration file content
    let content = fs::read_to_string(migration_file_path)?;
    
    // Hash the file content
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    Ok(format!("{:x}", hasher.finalize()))
}
```

**Note:** Checksums are calculated from migration file content, ensuring that:
- File modifications are detected (checksum mismatch)
- SREs can inspect migration files directly
- Migration files remain human-readable and version-controlled

**4. Migration Execution Flow**

```
┌─────────────────────────────────────────────────────────┐
│ 1. Acquire Lock (advisory or table-based)              │
│    - Wait with timeout if lock held                    │
│    - Fail if timeout exceeded                          │
└─────────────────────────────────────────────────────────┘
                        ↓
┌─────────────────────────────────────────────────────────┐
│ 2. Validate Checksums                                   │
│    - Read all applied migrations from state table      │
│    - Calculate current checksums                        │
│    - Compare with stored checksums                      │
│    - ERROR if mismatch (migration file was edited)      │
└─────────────────────────────────────────────────────────┘
                        ↓
┌─────────────────────────────────────────────────────────┐
│ 3. Identify Pending Migrations                         │
│    - Scan migration files in migrations/ directory     │
│    - Compare versions with state table                  │
│    - Sort by version (ascending)                        │
└─────────────────────────────────────────────────────────┘
                        ↓
┌─────────────────────────────────────────────────────────┐
│ 4. Execute Pending Migrations                          │
│    For each pending migration:                         │
│    - Start transaction (if supported)                  │
│    - Execute migration.up()                            │
│    - Record in state table (version, name, checksum)   │
│    - Commit transaction                                │
│    - On error: mark success=false, rollback, abort     │
└─────────────────────────────────────────────────────────┘
                        ↓
┌─────────────────────────────────────────────────────────┐
│ 5. Release Lock                                         │
│    - Advisory lock: automatic on connection close      │
│    - Table lock: explicit UPDATE locked = false        │
└─────────────────────────────────────────────────────────┘
```

### Error Handling & Recovery

**Scenario 1: Checksum Mismatch**
```
Error: Migration 'create_users_table' (version 20240120120000) 
       has been modified after being applied.
       
       Stored checksum:  abc123...
       Current checksum: def456...
       
       This indicates the migration file was edited after deployment.
       To fix: Restore original migration file or create new migration.
```

**Scenario 2: Partial Migration Failure**
```
Error: Migration 'add_index_to_users' (version 20240120130000) 
       failed during execution.
       
       Status: success = false
       
       To recover:
       1. Fix the underlying issue (e.g., constraint violation)
       2. Manually mark migration as successful:
          UPDATE lifeguard_migrations 
          SET success = true 
          WHERE version = 20240120130000;
       3. Or rollback and re-run: lifeguard migrate down 1
```

**Scenario 3: Concurrent Execution**
```
Warning: Another process is running migrations.
         Waiting for lock to be released...
         (Timeout: 60 seconds)
         
         If this message persists, check for:
         - Stuck migration process
         - Database connection issues
         - Manual lock in lifeguard_migration_lock table
```

### Implementation Requirements

**Core Traits & Types:**

1. **`Migration` Trait**
   ```rust
   pub trait Migration {
       fn name(&self) -> &str;
       fn version(&self) -> i64;  // YYYYMMDDHHMMSS
       async fn up(&self, manager: &SchemaManager) -> Result<(), LifeError>;
       async fn down(&self, manager: &SchemaManager) -> Result<(), LifeError>;
   }
   ```

2. **`SchemaManager` Struct**
   ```rust
   pub struct SchemaManager {
       executor: Box<dyn LifeExecutor>,
   }
   
   impl SchemaManager {
       pub fn new(executor: Box<dyn LifeExecutor>) -> Self;
       
       // Table operations
       pub async fn create_table(&self, table: Table) -> Result<(), LifeError>;
       pub async fn drop_table(&self, table: Table) -> Result<(), LifeError>;
       pub async fn alter_table(&self, table: AlterTable) -> Result<(), LifeError>;
       
       // Index operations
       pub async fn create_index(&self, index: Index) -> Result<(), LifeError>;
       pub async fn drop_index(&self, index: Index) -> Result<(), LifeError>;
       
       // Column operations
       pub async fn add_column(&self, table: &str, column: ColumnDef) -> Result<(), LifeError>;
       pub async fn drop_column(&self, table: &str, column: &str) -> Result<(), LifeError>;
       pub async fn rename_column(&self, table: &str, old: &str, new: &str) -> Result<(), LifeError>;
       
       // Raw SQL execution
       pub async fn execute(&self, sql: &str, params: &[&dyn ToSql]) -> Result<(), LifeError>;
   }
   ```

3. **`Migrator` Struct**
   ```rust
   pub struct Migrator {
       migrations: Vec<Box<dyn Migration>>,
       migrations_dir: PathBuf,
   }
   
   impl Migrator {
       pub fn new(migrations_dir: PathBuf) -> Self;
       
       // Load migrations from directory
       pub fn load_migrations(&mut self) -> Result<(), LifeError>;
       
       // Migration operations
       pub async fn up(&self, db: &dyn LifeExecutor, steps: Option<usize>) -> Result<(), LifeError>;
       pub async fn down(&self, db: &dyn LifeExecutor, steps: Option<usize>) -> Result<(), LifeError>;
       pub async fn status(&self, db: &dyn LifeExecutor) -> Result<MigrationStatus, LifeError>;
       pub async fn validate_checksums(&self, db: &dyn LifeExecutor) -> Result<(), LifeError>;
       
       // Migration discovery
       pub fn discover_migrations(&self) -> Result<Vec<MigrationFile>, LifeError>;
   }
   ```

4. **`MigrationLock` Trait**
   ```rust
   pub trait MigrationLock {
       async fn acquire(db: &dyn LifeExecutor) -> Result<LockGuard, LifeError>;
       async fn release(guard: LockGuard) -> Result<(), LifeError>;
   }
   
   pub struct LockGuard {
       // Automatically releases lock on drop
       executor: Box<dyn LifeExecutor>,
   }
   
   impl Drop for LockGuard {
       fn drop(&mut self) {
           // Release lock automatically
       }
   }
   
   // Database-specific implementations
   pub struct PostgresMigrationLock;
   pub struct GenericMigrationLock;  // For SQLite/MySQL
   ```

5. **`MigrationRecord` Struct**
   ```rust
   pub struct MigrationRecord {
       pub version: i64,
       pub name: String,
       pub checksum: String,
       pub applied_at: chrono::DateTime<Utc>,
       pub execution_time_ms: Option<i64>,
       pub success: bool,
   }
   ```

6. **`MigrationStatus` Struct**
   ```rust
   pub struct MigrationStatus {
       pub applied: Vec<MigrationRecord>,
       pub pending: Vec<MigrationFile>,
       pub total: usize,
       pub applied_count: usize,
       pub pending_count: usize,
   }
   ```

7. **`MigrationFile` Struct**
   ```rust
   pub struct MigrationFile {
       pub path: PathBuf,
       pub version: i64,
       pub name: String,
       pub checksum: String,
       pub migration: Box<dyn Migration>,
   }
   ```

**Module Structure for Main Lifeguard Crate:**
```
src/
├── migration/
│   ├── mod.rs              # Public API exports
│   ├── migration.rs        # Migration trait
│   ├── migrator.rs         # Migrator struct
│   ├── schema_manager.rs   # SchemaManager struct
│   ├── lock.rs             # MigrationLock trait and implementations
│   ├── record.rs           # MigrationRecord struct
│   ├── status.rs           # MigrationStatus struct
│   ├── file.rs             # MigrationFile struct and discovery
│   ├── checksum.rs         # Checksum calculation
│   └── error.rs            # Migration-specific errors
```

**Migration File Format:**
- File naming: `m{YYYYMMDDHHMMSS}_{name}.rs`
- Each file must implement `Migration` trait
- File structure:
  ```rust
  use lifeguard::migration::{Migration, SchemaManager};
  use sea_query::{Iden, Table};
  
  pub struct Migration;
  
  impl Migration for Migration {
      fn name(&self) -> &str { "migration_name" }
      fn version(&self) -> i64 { 20240120120000 }
      async fn up(&self, manager: &SchemaManager) -> Result<(), LifeError> {
          // Migration logic
      }
      async fn down(&self, manager: &SchemaManager) -> Result<(), LifeError> {
          // Rollback logic
      }
  }
  ```

**Migration Discovery:**
- Scan `migrations/` directory for files matching pattern `m\d{14}_\w+\.rs`
- Parse version from filename (first 14 digits)
- Parse name from filename (after version and underscore)
- Load and instantiate migration struct
- Sort by version (ascending)

### Migration CLI Tool: Detailed Implementation Plan

**Status:** 🔴 **REQUIRED FOR PRODUCTION**  
**Priority:** High - Essential for CI/CD pipelines and manual database management

The Migration CLI Tool is a critical component for production deployments, enabling:
- CI/CD pipeline integration (run migrations before deploying new code)
- Manual database management and troubleshooting
- Development workflow automation
- Production-safe migration execution

#### CLI Architecture

**Tool Name:** `lifeguard-migrate` (or `lifeguard migrate` as subcommand)

**Implementation Options:**
1. **Standalone Binary:** Separate `lifeguard-migrate` crate/bin
2. **Cargo Subcommand:** `cargo lifeguard migrate` (via `cargo` plugin)
3. **Farm CLI Integration:** `farm migrate` (integrated into existing farm CLI)

**Recommended:** Standalone binary with optional farm CLI integration

#### Core Commands

**1. `lifeguard-migrate status`**
```bash
lifeguard-migrate status [--database-url <URL>] [--migrations-dir <DIR>]
```
- **Purpose:** Show migration status (applied vs pending)
- **Output:**
  ```
  Applied Migrations (3):
    ✓ m20240120120000_create_users_table (2024-01-20 12:00:00, 45ms)
    ✓ m20240120130000_add_email_index (2024-01-20 13:00:00, 12ms)
    ✓ m20240120140000_add_roles_table (2024-01-20 14:00:00, 23ms)
  
  Pending Migrations (2):
    ⏳ m20240120150000_add_user_preferences (pending)
    ⏳ m20240120160000_migrate_to_jsonb (pending)
  
  Status: 3 applied, 2 pending
  ```
- **Implementation:**
  - Scan migrations directory for migration files
  - Query `lifeguard_migrations` table for applied migrations
  - Compare and display status
  - Show checksum validation status

**2. `lifeguard-migrate up [--steps N]`**
```bash
lifeguard-migrate up [--steps <N>] [--database-url <URL>] [--migrations-dir <DIR>]
```
- **Purpose:** Apply pending migrations
- **Options:**
  - `--steps N`: Apply only next N migrations (default: all pending)
  - `--dry-run`: Show what would be executed without running
- **Behavior:**
  - Acquire migration lock
  - Validate checksums of applied migrations
  - Identify pending migrations
  - Execute migrations in order (with transactions where supported)
  - Record in state table
  - Release lock
- **Output:**
  ```
  Applying migrations...
  ✓ m20240120150000_add_user_preferences (23ms)
  ✓ m20240120160000_migrate_to_jsonb (156ms)
  
  Successfully applied 2 migrations
  ```

**3. `lifeguard-migrate down [--steps N]`**
```bash
lifeguard-migrate down [--steps <N>] [--database-url <URL>] [--migrations-dir <DIR>]
```
- **Purpose:** Rollback migrations
- **Options:**
  - `--steps N`: Rollback last N migrations (default: 1)
  - `--dry-run`: Show what would be rolled back
- **Behavior:**
  - Acquire migration lock
  - Identify last N applied migrations (in reverse order)
  - Execute `down()` for each migration
  - Remove from state table
  - Release lock
- **Note:** Requires `down()` implementations in migration files

**4. `lifeguard-migrate validate`**
```bash
lifeguard-migrate validate [--database-url <URL>] [--migrations-dir <DIR>]
```
- **Purpose:** Validate checksums of applied migrations
- **Behavior:**
  - Read all applied migrations from state table
  - Calculate current checksums from migration files
  - Compare with stored checksums
  - Report any mismatches
- **Output:**
  ```
  Validating migration checksums...
  ✓ m20240120120000_create_users_table (checksum valid)
  ✓ m20240120130000_add_email_index (checksum valid)
  ✗ m20240120140000_add_roles_table (checksum mismatch!)
      Stored:  abc123...
      Current: def456...
      ERROR: Migration file has been modified after being applied
  
  Validation failed: 1 mismatch found
  ```

**5. `lifeguard-migrate generate <name>`**
```bash
lifeguard-migrate generate <name> [--migrations-dir <DIR>]
```
- **Purpose:** Generate new migration file template
- **Behavior:**
  - Create timestamp: `YYYYMMDDHHMMSS`
  - Generate migration file: `m{timestamp}_{name}.rs`
  - Create template with `up()` and `down()` methods
- **Output:**
  ```
  Created migration: migrations/m20240120170000_add_user_avatar.rs
  ```
- **Template:**
  ```rust
  use sea_query::{Iden, Table};
  use lifeguard::migration::{Migration, SchemaManager};
  
  pub struct Migration;
  
  impl Migration for Migration {
      fn name(&self) -> &str {
          "add_user_avatar"
      }
      
      fn version(&self) -> i64 {
          20240120170000
      }
      
      async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
          // TODO: Implement migration logic
          Ok(())
      }
      
      async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
          // TODO: Implement rollback logic
          Ok(())
      }
  }
  ```

**6. `lifeguard-migrate fresh`**
```bash
lifeguard-migrate fresh [--database-url <URL>] [--migrations-dir <DIR>]
```
- **Purpose:** Drop all tables and re-run all migrations (development only)
- **Warning:** Destructive operation - should require confirmation
- **Behavior:**
  - Drop all tables (or entire database schema)
  - Clear `lifeguard_migrations` table
  - Run all migrations from scratch
- **Use Case:** Development/testing environments only

**7. `lifeguard-migrate info`**
```bash
lifeguard-migrate info [--database-url <URL>] [--migrations-dir <DIR>]
```
- **Purpose:** Show detailed information about migrations
- **Output:**
  ```
  Migration Information:
  
  Database: postgresql://localhost/mydb
  Migrations Directory: ./migrations
  State Table: lifeguard_migrations
  
  Total Migrations: 5
  Applied: 3
  Pending: 2
  
  Latest Applied: m20240120140000_add_roles_table (2024-01-20 14:00:00)
  Next Pending: m20240120150000_add_user_preferences
  ```

#### Configuration

**Environment Variables:**
- `LIFEGUARD_DATABASE_URL`: Default database connection string
- `LIFEGUARD_MIGRATIONS_DIR`: Default migrations directory (default: `./migrations`)

**Configuration File (Optional):**
```toml
# lifeguard.toml or .lifeguard/config.toml
[database]
url = "postgresql://localhost/mydb"

[migrations]
directory = "./migrations"
table_name = "lifeguard_migrations"
```

**Command-Line Flags:**
- `--database-url <URL>`: Override database URL
- `--migrations-dir <DIR>`: Override migrations directory
- `--config <PATH>`: Path to configuration file
- `--verbose`: Enable verbose output
- `--quiet`: Suppress non-error output

#### Error Handling

**Common Error Scenarios:**

1. **Database Connection Failure**
   ```
   Error: Failed to connect to database
   URL: postgresql://localhost/mydb
   Cause: Connection refused
   
   Suggestion: Check database is running and URL is correct
   ```

2. **Migration Lock Held**
   ```
   Warning: Migration lock is held by another process
   Waiting for lock to be released... (timeout: 60s)
   
   If this persists, check for:
   - Stuck migration process
   - Manual lock in lifeguard_migration_lock table
   ```

3. **Checksum Mismatch**
   ```
   Error: Migration checksum mismatch
   Migration: m20240120140000_add_roles_table
   Stored:    abc123...
   Current:   def456...
   
   This indicates the migration file was modified after being applied.
   To fix: Restore original migration file or create new migration.
   ```

4. **Missing Migration File**
   ```
   Error: Applied migration file not found
   Migration: m20240120120000_create_users_table
   Expected: migrations/m20240120120000_create_users_table.rs
   
   Suggestion: Ensure all migration files are present in migrations directory
   ```

#### Integration Points

**1. CI/CD Pipeline Integration**
```yaml
# GitHub Actions example
- name: Run Migrations
  run: |
    lifeguard-migrate validate
    lifeguard-migrate up
  env:
    LIFEGUARD_DATABASE_URL: ${{ secrets.DATABASE_URL }}
```

**2. Docker/Kubernetes Integration**
```dockerfile
# Dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release --bin lifeguard-migrate

FROM debian:bookworm-slim
WORKDIR /app
COPY --from=builder /app/target/release/lifeguard-migrate /usr/local/bin/
COPY migrations/ /app/migrations/
```

**3. Farm CLI Integration (Optional)**
```bash
# If integrated into farm CLI
farm migrate status
farm migrate up
farm migrate validate
```

#### Implementation Requirements

**Dependencies:**
- `clap`: Command-line argument parsing
- `tokio` or `may`: Async runtime (match Lifeguard's runtime)
- `serde`: Configuration file parsing
- `sha2`: Checksum calculation
- `chrono`: Timestamp handling

**Project Structure:**
```
lifeguard-migrate/
├── Cargo.toml
├── src/
│   ├── main.rs              # CLI entry point
│   ├── commands/
│   │   ├── mod.rs
│   │   ├── status.rs
│   │   ├── up.rs
│   │   ├── down.rs
│   │   ├── validate.rs
│   │   ├── generate.rs
│   │   ├── fresh.rs
│   │   └── info.rs
│   ├── config.rs            # Configuration management
│   ├── migrator.rs          # Shared Migrator logic
│   └── utils.rs             # Helper functions
└── README.md
```

**Shared Code:**
- Migrator logic should be shared between CLI and in-process execution
- Consider extracting to `lifeguard-migration` crate
- CLI uses same `Migrator` struct as in-process code

#### Testing Strategy

**Unit Tests:**
- Command parsing and validation
- Configuration loading
- Migration file discovery
- Checksum calculation

**Integration Tests:**
- Full migration lifecycle (up/down)
- Lock acquisition/release
- Checksum validation
- Error scenarios

**E2E Tests:**
- Complete CLI workflow in test database
- CI/CD pipeline simulation
- Multi-process concurrency testing

#### Migration Priority

**Phase 4 Implementation:**
- CLI tool is part of Phase 4: Dual-Mode Execution
- Should be implemented alongside in-process execution
- Both share the same `Migrator` core implementation

### What's Missing (Not Blockers)

1. **`DeriveMigrationName`** - 🟡 **NICE-TO-HAVE**
   - **Purpose:** Generate migration file names from entity names
   - **Example:** `User` entity → `m20240101_000001_create_user_table.rs`
   - **Status:** Can be implemented later or use manual naming
   - **Blocker?** ❌ No - migrations can use manual naming
   - **Implementation:** Simple macro that generates timestamp + entity name

---

## Migration Implementation Readiness

### ✅ Ready to Implement

**All prerequisites are met.** You can proceed with migration implementation:

1. **CREATE TABLE migrations** - ✅ Ready
   - Use `Entity::table_name()` and `Entity::schema_name()`
   - Use `E::all_columns()` to iterate columns
   - Use `ColumnDefinition::to_column_def()` for each column
   - Use `ColumnDefinition::apply_default_expr()` for default expressions

2. **ALTER TABLE migrations** - ✅ Ready
   - Use `ColumnDefinition::renamed_from` for column renames
   - Use `ColumnDefinition` metadata for column modifications

3. **Schema-aware migrations** - ✅ Ready
   - Use `Entity::schema_name()` for schema-qualified table names

### Example Migration Code (Conceptual)

```rust
use lifeguard::{LifeModelTrait, ColumnDefHelper};
use sea_query::{Table, ColumnDef, Iden};

fn create_table_migration<E>() -> Table
where
    E: LifeModelTrait,
    E::Column: ColumnDefHelper + Iden,
{
    let mut table = Table::create();
    
    // Get table name and schema
    let table_name = E::default().table_name();
    let schema_name = E::default().schema_name();
    
    // Create table
    table.if_not_exists().table(
        if let Some(schema) = schema_name {
            // Schema-qualified: schema.table
            format!("{}.{}", schema, table_name)
        } else {
            table_name.to_string()
        }
    );
    
    // Add columns
    for col in E::all_columns() {
        let col_def = <E::Column as ColumnDefHelper>::column_def(*col);
        let mut sea_def = col_def.to_column_def(*col);
        
        // Apply default expression if present
        col_def.apply_default_expr(&mut sea_def);
        
        table.col(&mut sea_def);
    }
    
    table
}
```

---

## Recommendations

### ✅ Proceed with Migration Implementation

**Rationale:**
1. All core infrastructure is in place
2. Phase 1 attributes (`default_expr`, `renamed_from`, `schema_name`) are complete
3. No missing derive macros are blockers
4. Can implement migrations incrementally

### 📋 Implementation Plan

1. **Phase 1: Basic Migration Builder** (Week 1-2)
   - Create `Migration` trait and `SchemaManager`
   - Implement `create_table()` method using `LifeModelTrait`
   - Implement `alter_table()` method
   - Generate SQL using SeaQuery
   - Support for `default_expr`, `renamed_from`, `schema_name`

2. **Phase 2: State Tracking & Locking** (Week 3-4)
   - Create `lifeguard_migrations` state table schema
   - Implement `MigrationRecord` struct
   - Implement checksum calculation (SHA-256)
   - Implement `MigrationLock` trait (advisory locks for PG, table locks for others)
   - Implement checksum validation logic
   - Add error handling for checksum mismatches and partial failures

3. **Phase 3: Migration Runner** (Week 5-6)
   - Implement `Migrator` struct with `up()` and `down()` methods
   - Implement migration discovery (scan migration files)
   - Implement status tracking (applied vs pending)
   - Add transaction support for migration execution
   - Implement recovery mechanisms for failed migrations

4. **Phase 4: Dual-Mode Execution** (Week 7-8)
   - **In-Process Execution:**
     - Implement `startup_migrations()` helper function
     - Add lock acquisition/release for concurrent execution
     - Add timeout handling for lock acquisition
     - File-based migration loading from directory
     - Document Kubernetes/containerized deployment patterns (read-only mounts)
   - **Out-of-Band CLI Tool (REQUIRED):**
     - Create `lifeguard-migrate` standalone binary
     - Implement core commands:
       - `status` - Show applied vs pending migrations with detailed output
       - `up [--steps N]` - Apply pending migrations with transaction support
       - `down [--steps N]` - Rollback migrations (requires down() implementations)
       - `validate` - Validate checksums of applied migrations
       - `generate <name>` - Create new migration file template
       - `fresh` - Drop all and re-run (dev only, with confirmation)
       - `info` - Show detailed migration information
     - Configuration management:
       - Environment variables (`LIFEGUARD_DATABASE_URL`, `LIFEGUARD_MIGRATIONS_DIR`)
       - Configuration file support (`lifeguard.toml`)
       - Command-line flags (--database-url, --migrations-dir, --verbose, --quiet)
     - Error handling:
       - User-friendly error messages
       - Database connection failure handling
       - Migration lock timeout handling
       - Checksum mismatch reporting
       - Missing migration file detection
     - Integration examples:
       - CI/CD pipeline integration (GitHub Actions, GitLab CI)
       - Docker/Kubernetes deployment patterns
       - Optional farm CLI integration
     - Testing:
       - Unit tests for command parsing and validation
       - Integration tests for full migration lifecycle
       - E2E tests for complete CLI workflows
     - Share same `Migrator` implementation as in-process mode
     - Both modes use file-based migrations from directory

5. **Phase 5: Enhanced Features** (Week 9-10, Optional)
   - Add `DeriveMigrationName` macro for automatic naming
   - Add migration file generation from entity diffs
   - Add rollback support with `down()` implementations
   - Add migration templates for common operations

### 🟡 Optional: Add `DeriveMigrationName` Later

**When to add:**
- After basic migration system is working
- If migration file naming becomes tedious
- As a convenience feature

**Implementation complexity:** Low (simple macro that generates migration names from entity names)

---

## Conclusion

**The missing derive macros are NOT prerequisites for migrations.**

All core migration functionality is already implemented:
- ✅ Column metadata (`ColumnDefinition`)
- ✅ SQL generation helpers (`to_column_def()`, `apply_default_expr()`)
- ✅ Entity metadata (`table_name()`, `schema_name()`, `all_columns()`)
- ✅ Phase 1 attributes (`default_expr`, `renamed_from`, `schema_name`)

**Recommendation:** Proceed with migration implementation. The missing derives can be added later as enhancements, not blockers.

---

## Summary: Optimal Migration Design

### Core Architecture Decisions

1. **State Tracking Table: `lifeguard_migrations`**
   - Stores version, name, checksum, applied_at, execution_time, success
   - **Checksum validation** prevents edited migration files (critical for production)
   - **Success tracking** enables recovery from partial failures

2. **Dual-Mode Execution**
   - **Out-of-Band CLI:** For CI/CD pipelines, manual management
   - **In-Process:** For self-migrating applications, development
   - **Shared Core:** Both modes use same `Migrator` implementation

3. **Concurrency Protection**
   - **PostgreSQL:** Advisory locks (`pg_advisory_lock`)
   - **Other DBs:** Lock table with timeout mechanism
   - **First Process Wins:** Other processes wait or skip gracefully

4. **Containerized Deployment**
   - **File-Based Only:** Migrations stored as files in container image
   - **Read-Only Mounts:** Mount migrations as read-only volumes in Kubernetes
   - **SRE Inspectability:** Migration files easily accessible for inspection
   - **Immutable:** Ensures migrations match application version

4. **Migration Lifecycle**
   - Generate → Define → Calculate Checksum → Execute → Track
   - Validation ensures integrity across environments
   - Recovery mechanisms for edge cases

### Key Advantages Over Basic Approaches

| Feature | Basic (SeaORM default) | Lifeguard Optimal Design |
|---------|----------------------|-------------------------|
| **Checksum Validation** | ❌ None | ✅ SHA-256 validation |
| **Concurrent Execution** | ⚠️ Race conditions possible | ✅ Advisory locks / lock table |
| **Failure Recovery** | ⚠️ Manual intervention | ✅ Success tracking + recovery |
| **Execution Modes** | CLI only | ✅ CLI + In-process |
| **Production Safety** | ⚠️ Medium | ✅ High (checksums prevent drift) |

### Implementation Priority

**Must Have (Phase 1-3):**
- ✅ Migration builder (CREATE/ALTER TABLE)
- ✅ State tracking table
- ✅ Checksum calculation & validation
- ✅ Locking mechanism
- ✅ Basic `up()` execution

**Should Have (Phase 4):**
- ✅ In-process execution
- ✅ CLI tool
- ✅ Status/validation commands

**Nice to Have (Phase 5):**
- 🟡 `DeriveMigrationName` macro
- 🟡 Auto-generation from entity diffs
- 🟡 Migration templates

---

## Next Steps

1. ✅ Review this discovery document
2. ✅ Confirm migration implementation approach
3. ✅ Begin Phase 1: Basic Migration Builder
4. ✅ Implement Phase 2: State Tracking & Locking (critical for production)
5. ✅ Implement Phase 3: Migration Runner
6. ✅ Implement Phase 4: Dual-Mode Execution
7. 🟡 Add Phase 5: Enhanced Features (optional)

---

*Generated: 2026-01-20*
*Status: Ready for Migration Implementation*
*Design: Production-Ready with Checksums, Locking, and Dual-Mode Execution*