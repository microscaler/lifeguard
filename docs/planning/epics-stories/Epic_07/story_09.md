# Story 09: Read Preference Modes

## Description

Implement read preference modes that control where reads are executed (primary, replica, Redis, or mixed). This enables fine-grained control over read routing.

## Acceptance Criteria

- [ ] `read_preference = "primary"` - Always read from primary
- [ ] `read_preference = "replica"` - Prefer replicas when healthy
- [ ] `read_preference = "mixed"` - Choose best available (Redis → replica → primary)
- [ ] `read_preference = "cached_only"` - Only read from Redis (fail if miss)
- [ ] Per-query read preference override
- [ ] Integration with replica health monitoring
- [ ] Unit tests demonstrate all read preference modes

## Technical Details

- Read preference configuration:
  ```rust
  pub enum ReadPreference {
      Primary,      // Always primary
      Replica,      // Prefer replica if healthy
      Mixed,        // Redis → replica → primary
      CachedOnly,   // Only Redis
  }
  ```
- Read algorithm:
  ```
  IF read_preference == CachedOnly:
      IF Redis has key: return Redis value
      ELSE: return error
  
  IF read_preference == Mixed:
      IF Redis has key: return Redis value
      ELSE IF replica healthy: return replica read
      ELSE: return primary read
  
  IF read_preference == Replica:
      IF replica healthy: return replica read
      ELSE: return primary read
  
  IF read_preference == Primary:
      return primary read
  ```
- Per-query override:
  ```rust
  User::find_by_id(&pool, 42)
      .read_preference(ReadPreference::Replica)
      .one()?;
  ```

## Dependencies

- Story 07: WAL-Based Replica Health Monitoring (this epic)
- Story 02: Redis Integration - Write-Through Cache (this epic)

## Notes

- Essential for replica-aware reads
- Should match distributed database read preference patterns
- Default should be "primary" for safety

