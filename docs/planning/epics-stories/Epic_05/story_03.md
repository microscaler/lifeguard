# Story 03: LifeReflector - Redis Cache Coherence

## Description

Implement Redis cache coherence in LifeReflector. When a database write occurs, LifeReflector should refresh the corresponding Redis cache entry.

## Acceptance Criteria

- [ ] LifeReflector checks Redis for cache key existence
- [ ] If key exists, LifeReflector fetches fresh data from PostgreSQL
- [ ] LifeReflector updates Redis with fresh data (TTL-based)
- [ ] If key doesn't exist, LifeReflector ignores (inactive item)
- [ ] TTL-based active set (only active items cached)
- [ ] Unit tests demonstrate cache coherence

## Technical Details

- Redis key format: `lifeguard:model:<table>:<id>`
- On NOTIFY:
  1. Parse notification (table, id)
  2. Check Redis: `EXISTS lifeguard:model:<table>:<id>`
  3. If exists: `SELECT * FROM table WHERE id = $1` (from primary)
  4. Update Redis: `SETEX key TTL serialized_data`
  5. If not exists: ignore (TTL expired, item inactive)
- Use TTL to manage active set (inactive items expire)

## Dependencies

- Story 02: LifeReflector - PostgreSQL LISTEN/NOTIFY Integration
- Redis (external service)

## Notes

- This prevents cache stampedes
- TTL-based active set is key to scalability
- Only active users' data stays cached

