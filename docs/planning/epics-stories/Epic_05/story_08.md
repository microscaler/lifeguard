# Story 08: Query Cache Support

## Description

Implement query cache support that caches query results (not just primary key lookups). This replicates the query cache pattern discussed in operation_sledge_hammer conversations.

## Acceptance Criteria

- [ ] Query hash generation (hash of SQL + parameters)
- [ ] Query cache key format: `lifeguard:query:<table>:<hash>`
- [ ] Opt-in query caching: `.cache(ttl)` on query builder
- [ ] Query cache invalidation on table changes (via LifeReflector)
- [ ] Query cache statistics (hits, misses, size)
- [ ] Unit tests demonstrate query caching

## Technical Details

- Query hash: `sha256(sql_string + serialized_params)`
- Cache key: `lifeguard:query:<table>:<hash>`
- Usage:
  ```rust
  User::find()
      .filter(User::Email.eq("test@example.com"))
      .cache(ttl = 5)  // Cache for 5 seconds
      .all(&pool)?;
  ```
- LifeReflector invalidates query cache on table changes:
  - On NOTIFY for table, invalidate all `lifeguard:query:<table>:*` keys
  - Use Redis pattern matching: `KEYS lifeguard:query:<table>:*`
- TTL should be short (1-5 seconds) for query cache
- Query cache is opt-in (not automatic)

## Dependencies

- Story 04: Redis Integration for Transparent Caching
- Story 03: LifeReflector - Redis Cache Coherence
- Story 05: Type-Safe Query Builders (Epic 02)

## Notes

- Query cache is powerful but must be used carefully
- Short TTL prevents stale query results
- Pattern-based invalidation may be expensive (consider Redis SCAN)

