# Story 01: Complete PostgreSQL Feature Support

## Description

Ensure all core PostgreSQL features are supported in LifeModel and LifeRecord: views, materialized views, JSONB, full-text search, arrays, and custom types.

## Acceptance Criteria

- [ ] Views can be queried via LifeModel
- [ ] Materialized views supported
- [ ] JSONB fields work in LifeModel/LifeRecord
- [ ] Full-text search queries work
- [ ] Array types (text[], integer[], etc.) supported
- [ ] Custom PostgreSQL types supported
- [ ] Unit tests demonstrate all features

## Technical Details

- Views: Treat as read-only LifeModel
- Materialized views: Support `REFRESH MATERIALIZED VIEW`
- JSONB: Use `serde_json::Value` or custom types
- Full-text search: Support `tsvector`, `tsquery`, `@@` operator
- Arrays: Use Rust `Vec<T>` types
- Custom types: Allow custom serialization/deserialization

## Dependencies

- Epic 02: ORM Core (must be complete)

## Notes

- This is a broad story - break into smaller stories if needed
- Focus on commonly used features first
- Document PostgreSQL-specific features

