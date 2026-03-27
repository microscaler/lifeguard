# Story 01: Remove SeaORM and Tokio Dependencies

## Description

Remove all SeaORM and Tokio dependencies from the Lifeguard codebase. This is the first step in the complete rebuild, as these dependencies are fundamentally incompatible with the `may` coroutine runtime.

## Acceptance Criteria

- [ ] All SeaORM imports removed from codebase
- [ ] All Tokio imports removed from codebase
- [ ] `Cargo.toml` no longer lists `sea-orm` or `tokio` as dependencies
- [ ] Code compiles without SeaORM/Tokio dependencies
- [ ] All tests that depended on SeaORM/Tokio are removed or marked as TODO

## Technical Details

- Search codebase for: `use sea_orm::*`, `use tokio::*`
- Remove or replace any SeaORM-specific code
- Remove or replace any Tokio runtime code
- Update `Cargo.toml` to remove dependencies
- Document any functionality that needs to be rebuilt

## Dependencies

None (this is the starting point)

## Notes

This is a destructive change. Make sure to:
- Document what functionality is being removed
- Create TODO items for rebuilding removed features
- Keep a backup branch if needed

