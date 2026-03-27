# Story 04: Redis Integration for Transparent Caching

## Description

Integrate Redis into Lifeguard's read path. LifeModel queries should transparently check Redis first, falling back to database on cache miss.

## Acceptance Criteria

- [ ] LifeModel queries check Redis first
- [ ] Cache hit: return data from Redis (sub-millisecond)
- [ ] Cache miss: read from database, populate Redis
- [ ] TTL-based expiration (configurable per model)
- [ ] Cache key format: `lifeguard:model:<table>:<id>`
- [ ] Unit tests demonstrate transparent caching

## Technical Details

- Use `redis` crate for Redis client
- Read path:
  1. Check Redis: `GET lifeguard:model:<table>:<id>`
  2. If hit: deserialize and return
  3. If miss: read from database, serialize and cache
- Write path: LifeRecord triggers NOTIFY (LifeReflector handles cache refresh)
- TTL configuration: per-model or global default
- Serialization: use `serde` (JSON or MessagePack)

## Dependencies

- Epic 04: v1 Release
- Redis (external service)

## Notes

- This is the "transparent" part - application code doesn't change
- Cache hit rate should be 99%+ for Pricewhisperers scale
- Consider cache warming strategies

