# Story 05: Code Generation from Database

## Description

Generate complete LifeModel/LifeRecord code from database schema. This feeds back into BRRTRouter controller generation, enabling full-stack code generation.

## Acceptance Criteria

- [ ] Code generation reads database schema
- [ ] Generates: LifeModel, LifeRecord, CRUD operations
- [ ] Generates: BRRTRouter controllers (if applicable)
- [ ] CLI command: `lifeguard generate`
- [ ] Generated code is idiomatic and customizable
- [ ] Unit tests demonstrate code generation

## Technical Details

- Schema introspection (reuse Story 04)
- Code generation:
  - LifeModel structs
  - LifeRecord structs
  - CRUD methods
  - BRRTRouter controller templates
- Templates: use `tera` or `handlebars`
- Customization: allow template overrides
- CLI: `lifeguard generate --template-dir templates/`

## Dependencies

- Story 04: Schema Introspection Tools
- BRRTRouter integration (if generating controllers)

## Notes

- This enables full-stack code generation
- Feeds into BRRTRouter controller generation
- Consider adding OpenAPI schema generation

