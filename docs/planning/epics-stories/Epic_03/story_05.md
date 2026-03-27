# Story 05: Programmatic Migrations and Data Seeding

## Description

Implement programmatic migrations (Rust code, not SQL files) and support data seeding within migrations. This replicates SeaORM's approach of defining migrations as Rust code with the ability to seed data.

## Acceptance Criteria

- [ ] Migrations are defined as Rust code (not SQL files)
- [ ] Migrations can use SeaQuery for SQL building
- [ ] Data seeding supported in migrations (insert initial data)
- [ ] Migrations can use LifeModel/LifeRecord for data operations
- [ ] Conditional operations: `has_column()`, `has_table()`, `has_index()`
- [ ] Unit tests demonstrate programmatic migrations and data seeding

## Technical Details

- Migrations are Rust structs implementing `LifeMigration` trait
- Use SeaQuery for SQL building (type-safe)
- Data seeding:
  ```rust
  fn up(&self, executor: &mut dyn LifeExecutor) -> Result<()> {
      // Schema changes
      executor.execute("CREATE TABLE users ...")?;
      
      // Data seeding
      let admin = UserRecord {
          email: Some("admin@example.com".to_string()),
          name: Some("Admin".to_string()),
          // ...
      };
      admin.insert(executor)?;
      Ok(())
  }
  ```
- Conditional operations:
  - `has_column(table, column)` - Check if column exists
  - `has_table(table)` - Check if table exists
  - `has_index(table, index)` - Check if index exists
- Use these for idempotent migrations

## Dependencies

- Story 01: Implement LifeMigration Trait
- Story 02: Build Migration Runner
- Epic 02: ORM Core (LifeModel/LifeRecord for data seeding)

## Notes

- Programmatic migrations are more powerful than SQL files
- Data seeding is essential for initial data setup
- Conditional operations prevent errors on re-runs

