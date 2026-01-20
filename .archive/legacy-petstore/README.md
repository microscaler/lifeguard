# Legacy Petstore Tables - Archived

This directory contains legacy SeaORM entity definitions and schema files for the petstore example (appointments, owners, pets tables).

## Status

**ARCHIVED** - These files are legacy SeaORM code that was used for testing/comparison purposes. They are not part of the active Lifeguard codebase.

## Contents

- `tests_cfg_entity/` - SeaORM entity definitions from `src/tests_cfg/entity/`
- `tests_cfg_db/` - Schema SQL from `src/tests_cfg/db/`
- `examples_entity/` - Example SeaORM entities from `examples/entity/`
- `examples_db/` - Example schema SQL from `examples/db/`

## Why Archived

These files were:
- Using SeaORM (not Lifeguard)
- Marked as "TO BE REBUILT IN EPIC 03" in `src/tests_cfg/mod.rs`
- Not actually creating tables in the database
- Causing confusion in IDE database viewers (showing DDL data source instead of actual database state)

## Migration Path

If these tables are needed in the future, they should be:
1. Rebuilt using Lifeguard's `LifeModel` and `LifeRecord` derives
2. Created via proper migrations using the migration system
3. Not using SeaORM dependencies

## Date Archived

2026-01-20
