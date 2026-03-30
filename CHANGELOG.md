# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- **Pool slot heal (PRD §5.5):** Worker threads replace the `may_postgres::Client` after connectivity-class `Postgres` errors (SQLSTATE 08\*, shutdown codes, closed connection, transport `io` kinds). One reconnect attempt per job; application SQL errors do not trigger heal. See `src/pool/connectivity.rs`.

### Fixed

- **`DatabaseConfig::load`:** Correctly reads `config/config.toml` `[database]` by deserializing a `database` key (nested TOML). Previously, `[database]` values were not applied to the flat struct, so defaults (e.g. 30s pool timeout) could mask TOML. Environment overrides use **`LIFEGUARD__DATABASE__*`** (e.g. `LIFEGUARD__DATABASE__POOL_TIMEOUT_SECONDS`) so they match the file layout (PRD R2.2).

### Changed

- **Environment overrides** for database fields must use **`LIFEGUARD__DATABASE__<FIELD>`** (e.g. `LIFEGUARD__DATABASE__URL`). If you relied on root-style names such as `LIFEGUARD__POOL_TIMEOUT_SECONDS`, switch to the nested form.

### Documentation

- **Pool acquire default (PRD R1.3):** Default maximum wait for a worker slot is **30 seconds**, matching `DatabaseConfig::default().pool_timeout_seconds` and `LifeguardPoolSettings::default().acquire_timeout`. Documented on `LifeguardPool::new`, `LifeguardPoolSettings`, and `DatabaseConfig` in source.
