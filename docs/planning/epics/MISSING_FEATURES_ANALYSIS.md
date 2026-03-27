# Missing Features Analysis: SeaORM Parity & Cache Coherence

This document identifies missing stories needed to achieve complete SeaORM parity and full cache coherence functionality.

## Part 1: Missing SeaORM Features

### 1. Entity Hooks & Lifecycle Events

**SeaORM Feature**: Entity hooks (before_insert, after_insert, before_update, after_update, before_delete, after_delete)

**Missing**: No hooks system for LifeModel/LifeRecord

**Impact**: Cannot intercept operations for validation, logging, or side effects

**Recommended Epic**: Epic 02 (add to Story 02 or new Story 09)

**Story Needed**: 
- `LifeRecord` hooks: `before_insert()`, `after_insert()`, `before_update()`, `after_update()`, `before_delete()`, `after_delete()`
- `LifeModel` hooks: `after_load()`
- Hook registration and execution order
- Ability to abort operations in hooks

### 2. Validators

**SeaORM Feature**: Custom validators for field validation

**Missing**: No validation system

**Impact**: Cannot validate data before database operations

**Recommended Epic**: Epic 02 (new Story 10)

**Story Needed**:
- Validator trait definition
- Field-level validators
- Model-level validators
- Validation error collection
- Integration with hooks

### 3. Soft Deletes

**SeaORM Feature**: Soft delete support (deleted_at timestamp)

**Missing**: No soft delete functionality

**Impact**: Cannot implement soft deletes without manual handling

**Recommended Epic**: Epic 02 (new Story 11)

**Story Needed**:
- `#[soft_delete]` attribute on LifeModel
- Automatic filtering of soft-deleted records
- `with_deleted()` query option
- `restore()` method
- `force_delete()` method

### 4. Auto-Managed Timestamps

**SeaORM Feature**: Automatic `created_at` and `updated_at` management

**Missing**: No automatic timestamp handling

**Impact**: Must manually manage timestamps

**Recommended Epic**: Epic 02 (add to Story 01 or new Story 12)

**Story Needed**:
- `#[created_at]` and `#[updated_at]` attributes
- Automatic timestamp setting on insert/update
- Database-level defaults support
- Configurable timestamp fields

### 5. UUID Primary Keys

**SeaORM Feature**: UUID primary key generation and handling

**Missing**: UUID support mentioned but not detailed

**Impact**: UUID primary keys need explicit handling

**Recommended Epic**: Epic 02 (enhance Story 01)

**Story Needed**:
- UUID type support in LifeModel
- Automatic UUID generation on insert
- UUID primary key handling
- Database UUID extension support

### 6. Composite Primary Keys

**SeaORM Feature**: Composite primary keys (multiple columns)

**Missing**: Mentioned but not fully specified

**Impact**: Cannot handle composite keys properly

**Recommended Epic**: Epic 02 (enhance Story 01)

**Story Needed**:
- Multiple `#[primary_key]` attributes
- Composite key handling in queries
- Composite key in `find_by_id()`
- Composite key in relations

### 7. Virtual Fields / Computed Columns

**SeaORM Feature**: Virtual fields (not stored in database)

**Missing**: No virtual field support

**Impact**: Cannot have computed properties on models

**Recommended Epic**: Epic 02 (new Story 13)

**Story Needed**:
- `#[virtual]` attribute
- Computed field generation
- Virtual fields in serialization
- Virtual fields excluded from queries

### 8. Database Functions in Queries

**SeaORM Feature**: Using database functions in queries (e.g., `COUNT`, `SUM`, `NOW()`)

**Missing**: Basic aggregation exists, but not full function support

**Impact**: Limited query capabilities

**Recommended Epic**: Epic 02 (enhance Story 05)

**Story Needed**:
- Database function support in queries
- Custom function calls
- Function aliasing
- Window functions (mentioned but not detailed)

### 9. Subqueries

**SeaORM Feature**: Subquery support in queries

**Missing**: No subquery support

**Impact**: Complex queries require raw SQL

**Recommended Epic**: Epic 02 (new Story 14)

**Story Needed**:
- Subquery in WHERE clauses
- Subquery in SELECT
- Subquery in FROM (derived tables)
- Correlated subqueries

### 10. CTEs (Common Table Expressions)

**SeaORM Feature**: WITH clauses for CTEs

**Missing**: No CTE support

**Impact**: Complex queries require raw SQL

**Recommended Epic**: Epic 02 (new Story 15)

**Story Needed**:
- WITH clause support
- Recursive CTEs
- Multiple CTEs
- CTE in queries

### 11. Entity Generation from Database

**SeaORM Feature**: `sea-orm-cli generate entity` - generate entities from existing database

**Missing**: Schema introspection exists (Epic 06) but not entity generation

**Impact**: Cannot generate LifeModel from existing database

**Recommended Epic**: Epic 06 (enhance Story 04 or new story)

**Story Needed**:
- Database schema introspection
- Generate `#[derive(LifeModel)]` code
- Generate relations
- Generate migrations from schema diff

### 12. Query Logging

**SeaORM Feature**: Query logging and debugging

**Missing**: No query logging system

**Impact**: Difficult to debug queries

**Recommended Epic**: Epic 01 (new Story 08)

**Story Needed**:
- Query logging configuration
- SQL query logging
- Parameter logging
- Execution time logging
- Integration with tracing

### 13. Prepared Statement Caching

**SeaORM Feature**: Prepared statement caching for performance

**Missing**: No prepared statement caching

**Impact**: Performance overhead on repeated queries

**Recommended Epic**: Epic 01 (new Story 09)

**Story Needed**:
- Prepared statement cache
- Cache key generation
- Cache eviction policy
- Cache statistics

## Part 2: Missing Cache Coherence Features

### 1. Query Cache Support

**From Conversations**: Query cache with `lifeguard:query:<table>:<hash>` keys

**Missing**: No query cache implementation

**Impact**: Only primary key caching, no query result caching

**Recommended Epic**: Epic 05 (new Story 08)

**Story Needed**:
- Query hash generation
- Query cache key format: `lifeguard:query:<table>:<hash>`
- Opt-in query caching: `.cache(ttl)`
- Query cache invalidation on table changes
- Query cache statistics

### 2. Cache Warming Strategies

**From Conversations**: Cache warming for critical data

**Missing**: No cache warming support

**Impact**: Cold starts have poor performance

**Recommended Epic**: Epic 05 (new Story 09)

**Story Needed**:
- Cache warming configuration
- Pre-populate cache on startup
- Background cache warming
- Cache warming for critical models
- Cache warming metrics

### 3. Cache Statistics & Monitoring

**From Conversations**: Cache hit/miss rates, latency metrics

**Missing**: Basic metrics exist but not comprehensive

**Impact**: Cannot monitor cache effectiveness

**Recommended Epic**: Epic 05 (new Story 10)

**Story Needed**:
- Cache hit/miss counters
- Cache latency histograms
- Cache size metrics
- Cache eviction metrics
- Cache key distribution
- Per-model cache statistics

### 4. Cache Key Strategies

**From Conversations**: Primary, query, and aggregate cache keys

**Missing**: Only primary keys implemented

**Impact**: Limited caching capabilities

**Recommended Epic**: Epic 05 (enhance Story 04)

**Story Needed**:
- Primary key cache (already exists)
- Query cache keys (needs implementation)
- Aggregate cache keys: `lifeguard:agg:<table>:<tag>`
- Custom cache key strategies
- Cache key versioning

### 5. Cache Versioning

**From Conversations**: Cache versioning for schema changes

**Missing**: No cache versioning

**Impact**: Schema changes break cache

**Recommended Epic**: Epic 05 (new Story 11)

**Story Needed**:
- Cache version in keys
- Version bump on schema change
- Automatic cache invalidation on version change
- Version migration strategies

### 6. Per-Model Cache Configuration

**From Conversations**: `#[cache(primary = true, ttl_seconds = 900, reflector = true)]`

**Missing**: Basic cache attributes exist but not comprehensive

**Impact**: Limited cache configuration options

**Recommended Epic**: Epic 02 (enhance Story 01)

**Story Needed**:
- `#[cache(primary = true)]` - Enable primary key caching
- `#[cache(ttl_seconds = 900)]` - Per-model TTL
- `#[cache(reflector = true)]` - Enable LifeReflector
- `#[cache(query = true)]` - Enable query caching
- `#[cache(strategy = "refresh")]` - Refresh vs invalidate

### 7. Cache Invalidation Patterns

**From Conversations**: Refresh vs invalidate strategies

**Missing**: Only refresh implemented

**Impact**: Cannot choose invalidation strategy

**Recommended Epic**: Epic 05 (enhance Story 03)

**Story Needed**:
- Refresh strategy (read from DB, update cache)
- Invalidate strategy (delete cache key)
- Configurable per-model
- Bulk invalidation
- Pattern-based invalidation (wildcards)

### 8. WAL-Based Replica Health Monitoring

**From Conversations**: Track replica lag using WAL positions

**Missing**: Mentioned in Epic 05 but not detailed

**Impact**: Cannot safely use replicas

**Recommended Epic**: Epic 01 (new Story 10) or Epic 05 (enhance Story 05)

**Story Needed**:
- `pg_current_wal_lsn()` on primary
- `pg_last_wal_replay_lsn()` on replicas
- Lag calculation (bytes and seconds)
- Replica health status
- Automatic replica routing based on health
- Metrics for replica lag

### 9. Read Preference Modes

**From Conversations**: Primary, replica, mixed, cached_only modes

**Missing**: No read preference system

**Impact**: Cannot control read routing

**Recommended Epic**: Epic 01 (new Story 11)

**Story Needed**:
- `read_preference = "primary"` - Always read from primary
- `read_preference = "replica"` - Prefer replicas when healthy
- `read_preference = "mixed"` - Choose best available
- `read_preference = "cached_only"` - Only read from Redis
- Per-query read preference override

### 10. Strong Consistency Mode (Causal Reads)

**From Conversations**: LSN-based read-your-writes consistency

**Missing**: Not implemented

**Impact**: Cannot guarantee read-your-writes with replicas

**Recommended Epic**: Epic 05 (new Story 12)

**Story Needed**:
- Track commit LSN on writes
- Wait for replica to catch up to commit LSN
- Causal read consistency
- Configurable consistency levels
- Fallback to primary if replica lagged

### 11. LifeReflector Metrics & Observability

**From Conversations**: Comprehensive metrics for LifeReflector

**Missing**: Basic metrics mentioned but not comprehensive

**Impact**: Cannot monitor LifeReflector effectively

**Recommended Epic**: Epic 05 (enhance Story 01-03)

**Story Needed**:
- `reflector_notifications_total`
- `reflector_refreshes_total`
- `reflector_ignored_total`
- `reflector_active_keys`
- `reflector_redis_latency_seconds`
- `reflector_pg_latency_seconds`
- `reflector_leader_changes_total`
- `reflector_slot_reconnects_total`
- Leader election metrics

### 12. LifeReflector Connection Resilience

**From Conversations**: Auto-reconnect, lag tracking

**Missing**: Basic reconnection but not comprehensive

**Impact**: LifeReflector may fail silently

**Recommended Epic**: Epic 05 (enhance Story 02)

**Story Needed**:
- Auto-reconnect on LISTEN connection drop
- Connection health monitoring
- Reconnection backoff
- Connection pool for LifeReflector
- Metrics for connection issues

## Summary of Missing Stories

### Epic 01: Foundation
- Story 08: Query Logging
- Story 09: Prepared Statement Caching
- Story 10: WAL-Based Replica Health Monitoring
- Story 11: Read Preference Modes

### Epic 02: ORM Core
- Story 09: Entity Hooks & Lifecycle Events
- Story 10: Validators
- Story 11: Soft Deletes
- Story 12: Auto-Managed Timestamps
- Story 13: Virtual Fields / Computed Columns
- Story 14: Subqueries
- Story 15: CTEs (Common Table Expressions)
- Enhance Story 01: UUID and Composite Primary Keys
- Enhance Story 05: Database Functions

### Epic 05: Advanced Features
- Story 08: Query Cache Support
- Story 09: Cache Warming Strategies
- Story 10: Cache Statistics & Monitoring
- Story 11: Cache Versioning
- Story 12: Strong Consistency Mode (Causal Reads)
- Enhance Story 01-03: LifeReflector Metrics & Observability
- Enhance Story 02: LifeReflector Connection Resilience
- Enhance Story 04: Cache Key Strategies
- Enhance Story 03: Cache Invalidation Patterns

### Epic 06: Enterprise Features
- Enhance Story 04: Entity Generation from Database (not just introspection)

## Priority Recommendations

### High Priority (Core Functionality)
1. Entity Hooks (Epic 02, Story 09)
2. Query Cache Support (Epic 05, Story 08)
3. WAL-Based Replica Health Monitoring (Epic 01, Story 10)
4. Read Preference Modes (Epic 01, Story 11)
5. Cache Statistics & Monitoring (Epic 05, Story 10)

### Medium Priority (Important Features)
1. Soft Deletes (Epic 02, Story 11)
2. Auto-Managed Timestamps (Epic 02, Story 12)
3. Validators (Epic 02, Story 10)
4. Cache Warming (Epic 05, Story 09)
5. LifeReflector Metrics (Epic 05, enhancements)

### Low Priority (Nice to Have)
1. Virtual Fields (Epic 02, Story 13)
2. Subqueries (Epic 02, Story 14)
3. CTEs (Epic 02, Story 15)
4. Query Logging (Epic 01, Story 08)
5. Prepared Statement Caching (Epic 01, Story 09)

