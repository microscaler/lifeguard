# lifeguard-migrate

Migration CLI tool for Lifeguard ORM - manage database schema changes with version control and checksum validation.

## Overview

`lifeguard-migrate` is a command-line tool for managing database migrations in Lifeguard applications. It provides a complete migration lifecycle: discovery, validation, application, rollback, and status tracking.

## Features

- ✅ **File-based Migration Discovery** - Automatically discovers migration files from directory structure
- ✅ **Version Control** - Timestamp-based versioning ensures migration ordering
- ✅ **Checksum Validation** - Prevents modified migration files from being applied
- ✅ **Up/Down Migrations** - Full support for forward and rollback migrations
- ✅ **Status Tracking** - View applied vs pending migrations
- ✅ **Entity-Driven Generation** - Generate SQL migrations from Lifeguard entity definitions
- ✅ **CI/CD Integration** - Designed for automated deployment pipelines
- ✅ **Dry Run Mode** - Preview migrations without executing them

## Installation

The tool is part of the Lifeguard workspace. Build it with:

```bash
cargo build --bin lifeguard-migrate
```

Or install it:

```bash
cargo install --path lifeguard-migrate
```

## Quick Start

### 1. Generate a Migration

```bash
lifeguard-migrate generate create_users_table \
  --migrations-dir migrations
```

This creates a new migration file:
```
migrations/20240121120000_create_users_table.sql
```

### 2. Write Your Migration

Edit the generated file:

```sql
-- Up migration
CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    email VARCHAR(255) UNIQUE NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

-- Down migration
DROP TABLE IF EXISTS users;
```

### 3. Apply Migrations

```bash
lifeguard-migrate up \
  --database-url postgresql://user:pass@localhost/dbname \
  --migrations-dir migrations
```

### 4. Check Status

```bash
lifeguard-migrate status \
  --database-url postgresql://user:pass@localhost/dbname \
  --migrations-dir migrations
```

## Commands

### `status`

Show migration status (applied vs pending):

```bash
lifeguard-migrate status \
  --database-url $DATABASE_URL \
  --migrations-dir migrations
```

Output:
```
✅ Applied: 20240121120000_create_users_table
⏳ Pending: 20240121130000_add_user_indexes
```

### `up`

Apply pending migrations:

```bash
lifeguard-migrate up \
  --database-url $DATABASE_URL \
  --migrations-dir migrations
```

Options:
- `--steps N` - Apply only N migrations (default: all pending)
- `--dry-run` - Show what would be executed without running

### `down`

Rollback migrations:

```bash
lifeguard-migrate down \
  --database-url $DATABASE_URL \
  --migrations-dir migrations \
  --steps 1
```

Options:
- `--steps N` - Rollback N migrations (default: 1)
- `--dry-run` - Show what would be rolled back

### `validate`

Validate checksums of applied migrations:

```bash
lifeguard-migrate validate \
  --database-url $DATABASE_URL \
  --migrations-dir migrations
```

This ensures that migration files haven't been modified after being applied.

### `generate`

Generate a new migration file:

```bash
lifeguard-migrate generate add_user_bio \
  --migrations-dir migrations
```

Creates a timestamped migration file with up/down sections.

### `generate-from-entities`

Generate SQL migrations from Lifeguard entity definitions:

```bash
lifeguard-migrate generate-from-entities \
  --entities-dir examples/entities \
  --output-dir migrations/generated
```

This scans entity definitions and generates SQL migration files for tables, columns, indexes, and constraints.

### `info`

Show detailed migration information:

```bash
lifeguard-migrate info \
  --database-url $DATABASE_URL \
  --migrations-dir migrations
```

Options:
- `--version N` - Show info for specific migration version

## Configuration

### Environment Variables

The tool respects the `DATABASE_URL` environment variable:

```bash
export DATABASE_URL=postgresql://user:pass@localhost/dbname
lifeguard-migrate status
```

### Command-Line Options

Global options:

- `--database-url URL` - Database connection URL (or use `DATABASE_URL` env var)
- `--migrations-dir PATH` - Migrations directory (default: `migrations`)
- `--verbose` / `-v` - Verbose output
- `--quiet` / `-q` - Quiet output (errors only)

## Migration File Format

Migration files follow this naming convention:

```
{timestamp}_{name}.sql
```

Example: `20240121120000_create_users_table.sql`

The file contains both up and down migrations:

```sql
-- Up migration
CREATE TABLE users (...);

-- Down migration
DROP TABLE IF EXISTS users;
```

The tool automatically detects the `-- Down migration` comment to separate up and down sections.

## Migration Registry

Migrations are tracked in a `_lifeguard_migrations` table:

```sql
CREATE TABLE _lifeguard_migrations (
    version BIGINT PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    checksum VARCHAR(64) NOT NULL,
    applied_at TIMESTAMP NOT NULL DEFAULT NOW()
);
```

The checksum ensures migration files haven't been modified after application.

## Entity-Driven Generation

The `generate-from-entities` command analyzes Lifeguard entity definitions and generates SQL migrations:

### Supported Entity Features

- Table creation with columns
- Primary keys
- Foreign keys
- Unique constraints
- Indexes
- Check constraints
- Column types and nullability
- Default values

### Example

Given an entity:

```rust
#[derive(LifeModel)]
#[lifeguard(table_name = "users")]
pub struct User {
    #[lifeguard(primary_key, auto_increment)]
    pub id: i32,
    
    pub name: String,
    
    #[lifeguard(unique, indexed)]
    pub email: String,
}
```

The tool generates:

```sql
CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    email VARCHAR(255) NOT NULL
);

CREATE UNIQUE INDEX users_email_unique ON users(email);
CREATE INDEX users_email_idx ON users(email);
```

## CI/CD Integration

### GitHub Actions

```yaml
- name: Run migrations
  run: |
    lifeguard-migrate up \
      --database-url ${{ secrets.DATABASE_URL }} \
      --migrations-dir migrations
```

### Docker

```dockerfile
FROM rust:latest AS builder
WORKDIR /app
COPY . .
RUN cargo build --release --bin lifeguard-migrate

FROM postgres:15
COPY --from=builder /app/target/release/lifeguard-migrate /usr/local/bin/
```

## Error Handling

The tool provides clear error messages for common issues:

- **Migration already applied** - Prevents duplicate application
- **Checksum mismatch** - Detects modified migration files
- **Missing down migration** - Warns about incomplete rollback support
- **Version conflicts** - Detects duplicate version numbers
- **Database connection errors** - Clear connection failure messages

## Best Practices

1. **Always include down migrations** - Enables safe rollbacks
2. **Use descriptive names** - Migration names should clearly describe the change
3. **Test migrations** - Test both up and down migrations before deploying
4. **Version control** - Commit migration files to version control
5. **Checksum validation** - Run `validate` in CI/CD to catch modifications
6. **Atomic migrations** - Keep migrations focused and atomic
7. **Backup before major changes** - Always backup before applying destructive migrations

## Troubleshooting

### Migration Already Applied

If a migration is already applied but you need to modify it:

1. Rollback the migration: `lifeguard-migrate down --steps 1`
2. Modify the migration file
3. Re-apply: `lifeguard-migrate up`

**Note**: This should only be done in development. In production, create a new migration instead.

### Checksum Mismatch

If validation fails due to checksum mismatch:

1. Check if the migration file was modified after being applied
2. If intentional, rollback and re-apply (development only)
3. If unintentional, restore the original file from version control

### Missing Migration Files

If a migration is recorded as applied but the file is missing:

1. Restore the file from version control
2. Run `validate` to verify checksum matches
3. If checksum doesn't match, you may need to manually fix the migration table

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](../LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
