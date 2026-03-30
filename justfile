#!/usr/bin/env just --justfile
# Lifeguard Development Justfile

# Set shell for recipes
set shell := ["bash", "-uc"]

# Set dotenv loading
set dotenv-load := true

# Variables
# Kind/Tilt port-forwards (see `config/k8s/test-infrastructure` + Tiltfile): 6543 primary, 6544 replica-0, 6545 redis, 6546 replica-1.
# Passwords match `config/k8s/test-infrastructure/postgresql-credentials-secret.yaml` (default `postgres`).
TEST_DATABASE_URL := "postgres://postgres:postgres@127.0.0.1:6543/postgres"
TEST_REPLICA_URL := "postgres://postgres:postgres@127.0.0.1:6544/postgres"
TEST_REDIS_URL := "redis://127.0.0.1:6545"
# Optional second streaming replica (pool tests that target a specific standby)
TEST_REPLICA_URL_SECOND := "postgres://postgres:postgres@127.0.0.1:6546/postgres"
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

# Wait for database to be ready
dev-wait-db:
    @echo "⏳ Waiting for Postgres primary + replicas + Redis..."
    @kubectl wait --for=condition=available --timeout=300s deployment/postgresql-primary deployment/postgresql-replica-0 deployment/postgresql-replica-1 deployment/redis -n lifeguard-test || \
        (echo "⚠️  Stack not ready. Check status:" && kubectl get pods -n lifeguard-test && exit 1)

# Get test database connection string
dev-connection-string:
    @./scripts/get_test_connection_string.sh

# Print `export ...` lines for manual cargo runs (same URLs as `nt-db-suite` / `nt-workspace`)
kind-test-env:
    @echo "export DATABASE_URL='{{DATABASE_URL}}'"
    @echo "export TEST_DATABASE_URL='{{TEST_DATABASE_URL}}'"
    @echo "export TEST_REPLICA_URL='{{TEST_REPLICA_URL}}'"
    @echo "export TEST_REDIS_URL='{{TEST_REDIS_URL}}'"
    @echo "# optional: use second replica instead — export TEST_REPLICA_URL='{{TEST_REPLICA_URL_SECOND}}'"

# Port-forward PostgreSQL service for local access
dev-port-forward:
    @echo "🔌 Primary Postgres only on 6543:5432 (use \`tilt up\` for full stack: primary 6543, replica-0 6544, redis 6545, replica-1 6546)."
    @kubectl port-forward -n lifeguard-test svc/postgresql-primary 6543:5432

# Start Tilt (assumes cluster is already running)
tilt-up:
    @echo "🎯 Starting Tilt..."
    @echo "   Tilt UI: http://localhost:10350"
    @echo "   Postgres primary: localhost:6543 | replica-0: 6544 | replica-1: 6546 | Redis: 6545"
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
# Nextest quick reference (see docs/TEST_INFRASTRUCTURE.md):
#   nt / nextest-test  — workspace, excludes lifeguard-integration-tests + db_integration_suite binary
#   nt-workspace       — CI-parity workspace (includes lifeguard-integration-tests); still excludes db_suite
#   nt-db-suite        — lifeguard db_integration_suite only, serial (shared Postgres safe)
#   nt-complete        — nt then nt-db-suite (typical local DB run)
#   nt-ci-parity         — nt-workspace then nt-db-suite (matches CI test steps)
#   nt-integration       — lifeguard-integration-tests only (cluster URL from script)

# Run all tests
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

# Lifeguard `tests/db_integration_suite.rs`: Postgres + optional Redis; must run serially on a shared DB
nt-db-suite:
    @echo "🧪 Running lifeguard db_integration_suite (serial profile; Kind/Tilt: TEST_* from justfile)..."
    @DATABASE_URL={{DATABASE_URL}} TEST_DATABASE_URL={{TEST_DATABASE_URL}} TEST_REPLICA_URL={{TEST_REPLICA_URL}} TEST_REDIS_URL={{TEST_REDIS_URL}} cargo nextest run -p lifeguard --all-features --profile db-serial -E 'binary(db_integration_suite)'

alias nt-db := nt-db-suite

# Verbose output for db suite only
nt-db-suite-verbose:
    @echo "🧪 Running lifeguard db_integration_suite (serial, no-capture)..."
    @DATABASE_URL={{DATABASE_URL}} TEST_DATABASE_URL={{TEST_DATABASE_URL}} TEST_REPLICA_URL={{TEST_REPLICA_URL}} TEST_REDIS_URL={{TEST_REDIS_URL}} cargo nextest run -p lifeguard --all-features --profile db-serial --no-capture -E 'binary(db_integration_suite)'

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
    @kubectl get pods -n lifeguard-test 2>/dev/null || echo "⚠️  No pods in lifeguard-test namespace"

# Show PostgreSQL logs
logs-db:
    @echo "📜 PostgreSQL primary logs..."
    @kubectl logs -n lifeguard-test deployment/postgresql-primary --tail=100 -f

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
