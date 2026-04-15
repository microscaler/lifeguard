#!/usr/bin/env just --justfile
# Lifeguard Development Justfile

# Set shell for recipes
set shell := ["bash", "-uc"]

# Set dotenv loading
set dotenv-load := true

# Variables
# **Local dev:** use microscaler/shared-kind-cluster (`tilt up` there); it port-forwards Postgres primary to **localhost:5432**.
# **shared-kind-cluster** Tilt port-forwards replica-0 **6544** and Redis **6545** (see that repo’s `Tiltfile`). Manual `kubectl port-forward` only if you use another stack.
# **CI / docker-compose** uses host **6543** — set `LIFEGUARD_PG_PORT=6543` when running `just` against that stack only.
# libpq `options`: `search_path=lifeguard` (create schema once: `CREATE SCHEMA IF NOT EXISTS lifeguard;` on shared Postgres).
LG_PG_SEARCH_PATH := "?options=-c%20search_path%3Dlifeguard"
LIFEGUARD_PG_PORT := env_var_or_default("LIFEGUARD_PG_PORT", "5432")
LIFEGUARD_REPLICA_PORT := env_var_or_default("LIFEGUARD_REPLICA_PORT", "6544")
LIFEGUARD_REDIS_PORT := env_var_or_default("LIFEGUARD_REDIS_PORT", "6545")
LIFEGUARD_REPLICA2_PORT := env_var_or_default("LIFEGUARD_REPLICA2_PORT", "6546")
TEST_DATABASE_URL := "postgres://postgres:postgres@127.0.0.1:" + LIFEGUARD_PG_PORT + "/postgres" + LG_PG_SEARCH_PATH
TEST_REPLICA_URL := "postgres://postgres:postgres@127.0.0.1:" + LIFEGUARD_REPLICA_PORT + "/postgres" + LG_PG_SEARCH_PATH
TEST_REDIS_URL := "redis://127.0.0.1:" + LIFEGUARD_REDIS_PORT
# Optional second streaming replica (pool tests that target a specific standby)
TEST_REPLICA_URL_SECOND := "postgres://postgres:postgres@127.0.0.1:" + LIFEGUARD_REPLICA2_PORT + "/postgres" + LG_PG_SEARCH_PATH
# Primary URL for app code + migrate CLIs (same as TEST_DATABASE_URL for Kind)
DATABASE_URL := TEST_DATABASE_URL
ENTITY_DIR := "src/entity"

# Default recipe to display help
default:
    @just --list --unsorted

# ============================================================================
# Development Environment
# ============================================================================

# Start development environment (Kind + Tilt)
dev-up:
    @python3 scripts/dev_up.py

# Stop development environment (Kind + Tilt)
dev-down:
    @python3 scripts/dev_down.py

# Wait for shared **data** plane (microscaler/shared-kind-cluster) — namespace `data`
dev-wait-db:
    @echo "⏳ Waiting for Postgres primary + replicas + Redis (namespace data)..."
    @kubectl wait --for=condition=available --timeout=300s deployment/postgres-primary deployment/postgres-replica-0 deployment/postgres-replica-1 deployment/redis -n data || \
        (echo "⚠️  Stack not ready. Is shared-kind-cluster Tilt up? kubectl get pods -n data" && kubectl get pods -n data && exit 1)

# Get test database connection string
dev-connection-string:
    @./scripts/get_test_connection_string.sh

# Replica URL for host dev (port-forward the Service `kubectl get svc -n data` shows — usually `postgres-replica-0`)
dev-replica-connection-string:
    @./scripts/get_replica_connection_string.sh

# Print `export ...` lines for manual cargo runs (same URLs as `nt-db-suite` / `nt-workspace`)
kind-test-env:
    @echo "export DATABASE_URL='{{DATABASE_URL}}'"
    @echo "export TEST_DATABASE_URL='{{TEST_DATABASE_URL}}'"
    @echo "export TEST_REPLICA_URL='$(./scripts/get_replica_connection_string.sh)'"
    @echo "export TEST_REDIS_URL='{{TEST_REDIS_URL}}'"
    @echo "# optional: use second replica instead — export TEST_REPLICA_URL='{{TEST_REPLICA_URL_SECOND}}'"

# Port-forward shared primary (only if shared Tilt is not already forwarding :5432)
dev-port-forward:
    @echo "🔌 kubectl port-forward -n data svc/postgres 5432:5432  (shared-kind-cluster; skip if that Tilt already binds :5432)"
    @kubectl port-forward -n data svc/postgres 5432:5432

# Start Tilt (assumes cluster is already running)
tilt-up:
    @echo "🎯 Starting Lifeguard Tilt (builds/tests only — infra is shared-kind-cluster)..."
    @echo "   Tilt UI: http://localhost:10350"
    @tilt up

# Stop Tilt
tilt-down:
    @echo "🛑 Stopping Tilt..."
    @tilt down

# ============================================================================
# Building
# ============================================================================

# Build Rust binary (debug)
build:
    @echo "🔨 Building Rust binary..."
    @cargo build

# Build Rust binary (release)
build-release:
    @echo "🔨 Building Rust binary (release)..."
    @cargo build --release

# Check code (compile without building)
check:
    @echo "✅ Checking code..."
    @cargo check --all-targets

# ============================================================================
# Testing
# ============================================================================
#
# Plain `cargo test` does **not** match what CI / this repo calls the "workspace suite": it ignores
# `cargo nextest`, `.config/nextest.toml` (timeouts, `db_integration_suite` mutex), and the filters below.
#
# Nextest quick reference (see docs/TEST_INFRASTRUCTURE.md):
#   nt / nextest-test  — workspace nextest; excludes lifeguard-integration-tests + db_integration_suite binary (fast loop)
#   nt-workspace       — CI-parity workspace (includes lifeguard-integration-tests); still excludes db_suite
#   nt-db-suite        — lifeguard db_integration_suite only, serial (shared Postgres safe); same as:
#                        cargo nextest run -p lifeguard --all-features --profile db-serial --config-file .config/nextest.toml -E 'binary(db_integration_suite)'
#   nt-complete        — nextest-test then nt-db-suite (typical local: all workspace members except integration crate, plus DB suite)
#   nt-ci-parity / nt-full — nt-workspace then nt-db-suite (matches CI: integration crate + DB suite)
#   nt-integration       — lifeguard-integration-tests only (cluster URL from script)
#
# Coverage ladder (pick one path):
#   • Fast loop (~`just nt`):                    `just nt` or `just nextest-test` — skips `lifeguard-integration-tests` + `db_integration_suite`
#   • CI workspace step only:                    `just nt-workspace` — full workspace except `db_integration_suite`
#   • Serial ORM/DB integration (`db_integration_suite`): `just nt-db-suite`
#   • Typical “all Rust tests” locally:           `just nt-complete` (= `nt` + `nt-db-suite`)
#   • **Full CI parity** (workspace + db suite): `just nt-ci-parity` or `just nt-full`
# Tilt UI: `test-nextest` ≈ `nt-workspace`; `test-nextest-fast` ≈ `nt`; `test-db-suite` ≈ `nt-db-suite`; `test-migration` = integration crate only.

# Library unit tests only (`cargo test --lib`). For broad coverage use `just nt`, `just nt-complete`, or `just nt-ci-parity`.
test: test-unit

# Run unit tests
test-unit:
    @echo "🧪 Running unit tests..."
    @DATABASE_URL={{DATABASE_URL}} TEST_DATABASE_URL={{TEST_DATABASE_URL}} TEST_REPLICA_URL={{TEST_REPLICA_URL}} TEST_REDIS_URL={{TEST_REDIS_URL}} cargo test --lib --no-fail-fast

# Run unit tests with output
test-unit-verbose:
    @echo "🧪 Running unit tests (verbose)..."
    @DATABASE_URL={{DATABASE_URL}} TEST_DATABASE_URL={{TEST_DATABASE_URL}} TEST_REPLICA_URL={{TEST_REPLICA_URL}} TEST_REDIS_URL={{TEST_REDIS_URL}} cargo test --lib -- --nocapture --no-fail-fast

# Workspace nextest: excludes lifeguard-integration-tests and db_integration_suite for speed (use nt-db-suite / nt-complete).
# Note: db_integration_suite is safe in parallel with other *packages* — nextest test-group `lifeguard-shared-postgres`
# serializes tests inside that binary only (see .config/nextest.toml).
nextest-test:
    @echo "🧪 Running tests with nextest (excluding DB-heavy integration binaries)..."
    @DATABASE_URL={{DATABASE_URL}} TEST_DATABASE_URL={{TEST_DATABASE_URL}} TEST_REPLICA_URL={{TEST_REPLICA_URL}} TEST_REDIS_URL={{TEST_REDIS_URL}} cargo nextest run --workspace --all-features --fail-fast --retries 1 --exclude lifeguard-integration-tests -E 'not binary(db_integration_suite)'

alias nt := nextest-test

# Broadest automated suite aligned with CI (workspace including lifeguard-integration-tests + db_integration_suite).
alias nt-full := nt-ci-parity

# CI-parity workspace nextest (same filter as .github/workflows/ci.yaml "Run workspace tests").
# Includes lifeguard-integration-tests; requires DATABASE_URL (and any deps those tests need).
nt-workspace:
    @echo "🧪 Running workspace nextest (CI selection: all members except db_integration_suite binary)..."
    @DATABASE_URL={{DATABASE_URL}} TEST_DATABASE_URL={{TEST_DATABASE_URL}} TEST_REPLICA_URL={{TEST_REPLICA_URL}} TEST_REDIS_URL={{TEST_REDIS_URL}} cargo nextest run --workspace --all-features --profile ci -E 'not binary(db_integration_suite)'

# Run tests with nextest (no capture - passes through stdout/stderr directly)
nt-verbose:
    @echo "🧪 Running tests with nextest (no capture - full output)..."
    @DATABASE_URL={{DATABASE_URL}} TEST_DATABASE_URL={{TEST_DATABASE_URL}} TEST_REPLICA_URL={{TEST_REPLICA_URL}} TEST_REDIS_URL={{TEST_REDIS_URL}} cargo nextest run --workspace --all-features --no-capture --exclude lifeguard-integration-tests -E 'not binary(db_integration_suite)'

# Same as `nt-workspace` (alias for discoverability)
nt-ci:
    @just nt-workspace

# Run unit tests only with nextest (same selection as nextest-test)
nt-unit:
    @echo "🧪 Running tests with nextest (excluding DB-heavy integration binaries)..."
    @DATABASE_URL={{DATABASE_URL}} TEST_DATABASE_URL={{TEST_DATABASE_URL}} TEST_REPLICA_URL={{TEST_REPLICA_URL}} TEST_REDIS_URL={{TEST_REDIS_URL}} cargo nextest run --workspace --all-features --fail-fast --retries 1 --exclude lifeguard-integration-tests -E 'not binary(db_integration_suite)'

# lifeguard-derive (matches CI)
nt-derive:
    @echo "🧪 Running lifeguard-derive tests..."
    @cd lifeguard-derive && cargo test --no-fail-fast

# lifeguard-codegen (matches CI; may be absent in some checkouts)
nt-codegen:
    @echo "🧪 Running lifeguard-codegen tests..."
    @cd lifeguard-codegen && cargo test --no-fail-fast

# Lifeguard `tests/db_integration_suite.rs`: Postgres + optional Redis; must run serially on a shared DB.
# Tilt UI: `test-db-suite` (same command + `LG_NEXTTEST_ENV`).
nt-db-suite:
    @echo "🧪 Running lifeguard db_integration_suite (serial profile; Kind/Tilt: TEST_* from justfile)..."
    @DATABASE_URL={{DATABASE_URL}} TEST_DATABASE_URL={{TEST_DATABASE_URL}} TEST_REPLICA_URL={{TEST_REPLICA_URL}} TEST_REDIS_URL={{TEST_REDIS_URL}} cargo nextest run -p lifeguard --all-features --profile db-serial --config-file .config/nextest.toml -E 'binary(db_integration_suite)'

alias nt-db := nt-db-suite
# Same as `nt-db-suite` (CI step name / copy-paste alias)
alias db-integration-suite := nt-db-suite

# Verbose output for db suite only
nt-db-suite-verbose:
    @echo "🧪 Running lifeguard db_integration_suite (serial, no-capture)..."
    @DATABASE_URL={{DATABASE_URL}} TEST_DATABASE_URL={{TEST_DATABASE_URL}} TEST_REPLICA_URL={{TEST_REPLICA_URL}} TEST_REDIS_URL={{TEST_REDIS_URL}} cargo nextest run -p lifeguard --all-features --profile db-serial --config-file .config/nextest.toml --no-capture -E 'binary(db_integration_suite)'

# Typical local run: fast workspace (no cluster integration crate) + serial DB suite
nt-complete: nextest-test nt-db-suite
    @echo "✅ Workspace + db_integration_suite complete."

# Matches CI order: workspace nextest + serial db_integration_suite
nt-ci-parity: nt-workspace nt-db-suite
    @echo "✅ CI-parity test run complete (workspace + db_integration_suite)."

# Run integration tests (requires database connection)
test-integration:
    @echo "🧪 Running integration tests..."
    @echo "⚠️  Note: These tests require a running database connection"
    @DATABASE_URL={{DATABASE_URL}} TEST_DATABASE_URL={{TEST_DATABASE_URL}} TEST_REPLICA_URL={{TEST_REPLICA_URL}} TEST_REDIS_URL={{TEST_REDIS_URL}} cargo test --package lifeguard-integration-tests

# Run integration tests with nextest
nt-integration:
    @echo "🧪 Running integration tests with nextest..."
    @echo "⚠️  Note: These tests require a running database connection"
    @DATABASE_URL={{DATABASE_URL}} TEST_DATABASE_URL={{TEST_DATABASE_URL}} TEST_REPLICA_URL={{TEST_REPLICA_URL}} TEST_REDIS_URL={{TEST_REDIS_URL}} cargo nextest run --package lifeguard-integration-tests

# Run tests with standard cargo (fallback)
test-cargo:
    @echo "🧪 Running tests with cargo..."
    @DATABASE_URL={{DATABASE_URL}} TEST_DATABASE_URL={{TEST_DATABASE_URL}} TEST_REPLICA_URL={{TEST_REPLICA_URL}} TEST_REDIS_URL={{TEST_REDIS_URL}} cargo test --all -- --nocapture

# Run tests with LLVM coverage
test-coverage:
    @echo "🧪 Running tests with LLVM coverage..."
    @echo "📦 Installing cargo-llvm-cov if needed..."
    @cargo install cargo-llvm-cov --locked 2>/dev/null || true
    @echo "🔍 Generating coverage report..."
    @cargo llvm-cov --lib --lcov --output-path lcov.info
    @cargo llvm-cov --lib --html --output-dir target/llvm-cov/html
    @echo "✅ Coverage report generated:"
    @echo "   📊 HTML: target/llvm-cov/html/index.html"
    @echo "   📄 LCOV: lcov.info"
    @cargo llvm-cov --lib --summary-only

# Open coverage report in browser
test-coverage-open:
    @echo "🌐 Opening coverage report..."
    @open target/llvm-cov/html/index.html || xdg-open target/llvm-cov/html/index.html || echo "Please open target/llvm-cov/html/index.html manually"

# Check coverage meets minimum (65%)
test-coverage-check:
    @echo "📊 Checking test coverage (minimum 65%)..."
    @cargo install cargo-llvm-cov --locked 2>/dev/null || true
    @cargo llvm-cov --lib --summary-only | grep -E "^\s*Total\s+\|\s+[0-9]+\s+\|\s+[0-9]+\s+\|\s+([0-9]+)%" || echo "⚠️  Could not parse coverage, run 'just test-coverage' for full report"

# Rewrite `lifeguard-migrate/tests/golden/*.expected.rs` from current `emit_inferred_rust` (no DB). Set only when changing the emitter; review `git diff` before commit. CI must not set `LIFEGUARD_BLESS_INFER_SCHEMA_GOLDENS`.
bless-infer-schema-goldens:
    @echo "📝 Blessing infer-schema goldens (LIFEGUARD_BLESS_INFER_SCHEMA_GOLDENS=1)..."
    LIFEGUARD_BLESS_INFER_SCHEMA_GOLDENS=1 cargo test -p lifeguard-migrate golden_ -- --nocapture
    @echo "✅ Done. Review changes under lifeguard-migrate/tests/golden/"

# ============================================================================
# Code Quality
# ============================================================================

# Format all Rust trees: root workspace + standalone example workspaces (each has its own Cargo.toml / lockfile).
fmt:
    @echo "🎨 Formatting root workspace (lifeguard + members)..."
    @cargo fmt
    @echo "🎨 Formatting examples/entities..."
    @cd examples/entities && cargo fmt
    @echo "🎨 Formatting examples/perf-idam..."
    @cd examples/perf-idam && cargo fmt

# Check formatting (same directories as `fmt`)
fmt-check:
    @echo "🎨 Checking code formatting (root workspace)..."
    @cargo fmt -- --check
    @echo "🎨 Checking examples/entities..."
    @cd examples/entities && cargo fmt -- --check
    @echo "🎨 Checking examples/perf-idam..."
    @cd examples/perf-idam && cargo fmt -- --check

# Lint code
lint:
    @echo "🔍 Linting code..."
    @cargo clippy --all-targets --all-features -- -D warnings -W clippy::pedantic

# Lint and fix
lint-fix:
    @echo "🔍 Linting and fixing code..."
    @cargo clippy --all-targets --all-features --fix --allow-dirty --allow-staged

# Audit dependencies
audit:
    @echo "🔒 Auditing dependencies..."
    @cargo audit

# Validate all (format, lint, check, tests, coverage)
validate: fmt-check lint check test-unit test-coverage-check
    @echo "✅ All validations passed!"

# ============================================================================
# Database Management
# ============================================================================

# Apply schema
apply-schema:
    @echo "📝 Applying database schema..."
    @psql {{DATABASE_URL}} -f examples/db/schema.sql

# Migrate up (legacy - uses sea-orm-cli)
migrate-up:
    @echo "⬆️  Running migrations..."
    @sea-orm-cli migrate up -u {{DATABASE_URL}}

# Migrate down (legacy - uses sea-orm-cli)
migrate-down:
    @echo "⬇️  Rolling back migrations..."
    @sea-orm-cli migrate down -u {{DATABASE_URL}}

# Migrate refresh (legacy - uses sea-orm-cli)
migrate-refresh:
    @echo "🔄 Refreshing migrations..."
    @sea-orm-cli migrate refresh -u {{DATABASE_URL}}

# Generate entities (legacy - uses sea-orm-cli)
generate-entities:
    @echo "📦 Generating entities..."
    @sea-orm-cli generate entity \
        --database-url {{DATABASE_URL}} \
        --output-dir {{ENTITY_DIR}} \
        --with-serde both


# Reset database and run tests
reset-and-test:
    @echo "🔄 Resetting database and running tests..."
    @just migrate-refresh
    @just test-cargo

# ============================================================================
# Examples & Utilities
# ============================================================================

# Seed database (ARCHIVED - legacy petstore example removed)
# seed-db:
#     @echo "🌱 Seeding database..."
#     @cargo run --example seed_petshop
#
# # Seed database (heavy load) (ARCHIVED)
# seed-db-heavy n:
#     @echo "🌱 Seeding database (heavy load)..."
#     @cargo run --release --example seed_petshop_heavy -- {{n}}

# Run metrics server example
metrics-server:
    @echo "📊 Starting metrics server..."
    @cargo run --example metrics_server

# ============================================================================
# Utilities
# ============================================================================

# Clean build artifacts
clean:
    @echo "🧹 Cleaning build artifacts..."
    @cargo clean
    @echo "✅ Cleaned"

# Show cluster status
status:
    @echo "📊 Cluster Status..."
    @kubectl get nodes 2>/dev/null || echo "⚠️  No Kind cluster running"
    @kubectl get pods -n data 2>/dev/null || echo "⚠️  No pods in data namespace (start shared-kind-cluster Tilt)"

# Show PostgreSQL logs
logs-db:
    @echo "📜 PostgreSQL primary logs (namespace data)..."
    @kubectl logs -n data deployment/postgres-primary --tail=100 -f

# ============================================================================
# Documentation
# ============================================================================

# Generate documentation
docs:
    @echo "📚 Generating documentation..."
    @cargo doc --no-deps --open

# Generate documentation (without opening)
docs-build:
    @echo "📚 Building documentation..."
    @cargo doc --no-deps
    @echo "✅ Documentation built: target/doc/"
