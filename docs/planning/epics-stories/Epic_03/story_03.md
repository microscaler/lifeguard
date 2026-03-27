# Story 03: Create CLI Tooling (lifeguard migrate)

## Description

Build a CLI tool that provides commands for managing migrations: `migrate`, `status`, `rollback`, and `create`.

## Acceptance Criteria

- [ ] `lifeguard migrate` command applies pending migrations
- [ ] `lifeguard migrate status` shows migration status
- [ ] `lifeguard migrate rollback` rolls back last migration
- [ ] `lifeguard migrate create <name>` creates new migration file
- [ ] CLI reads database connection from environment/config
- [ ] CLI provides clear output and error messages
- [ ] Unit tests cover all CLI commands

## Technical Details

- Use `clap` crate for CLI argument parsing
- Commands (replicating `sea-orm-cli`):
  - `migrate init`: Initialize migration directory
  - `migrate create <name>`: Create new migration file
  - `migrate up`: Apply pending migrations
  - `migrate down [count]`: Rollback last N migrations
  - `migrate refresh`: Rollback all and reapply all
  - `migrate reset`: Rollback all migrations
  - `migrate status`: Show detailed migration status
- Configuration: `DATABASE_URL` environment variable or config file
- Migration files: `migrations/YYYYMMDDHHMMSS_<name>.rs`

## Dependencies

- Story 02: Build Migration Runner

## Notes

- CLI should be a separate binary (`lifeguard-cli` or `lifeguard` command)
- Consider adding `--dry-run` flag
- Migration templates should include boilerplate

