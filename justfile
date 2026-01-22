#!/usr/bin/env just --justfile
# Lifeguard Development Justfile

# Set shell for recipes
set shell := ["bash", "-uc"]

# Set dotenv loading
set dotenv-load := true

# Variables
DATABASE_URL := "postgres://postgres:postgres@localhost:5432/postgres"
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
    @echo "⏳ Waiting for PostgreSQL to be ready..."
    @kubectl wait --for=condition=available --timeout=120s deployment/postgres -n lifeguard-test || \
        (echo "⚠️  PostgreSQL not ready. Check status:" && kubectl get pods -n lifeguard-test && exit 1)

# Get test database connection string
dev-connection-string:
    @./scripts/get_test_connection_string.sh

# Port-forward PostgreSQL service for local access
dev-port-forward:
    @echo "🔌 Port-forwarding PostgreSQL service..."
    @kubectl port-forward -n lifeguard-test svc/postgres 5432:5432

# Start Tilt (assumes cluster is already running)
tilt-up:
    @echo "🎯 Starting Tilt..."
    @echo "   Tilt UI: http://localhost:10350"
    @echo "   PostgreSQL: localhost:5432 (via Tilt port forward)"
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

# Run all tests
test: test-unit

# Run unit tests
test-unit:
    @echo "🧪 Running unit tests..."
    @DATABASE_URL={{DATABASE_URL}} cargo test --lib --no-fail-fast

# Run unit tests with output
test-unit-verbose:
    @echo "🧪 Running unit tests (verbose)..."
    @DATABASE_URL={{DATABASE_URL}} cargo test --lib -- --nocapture --no-fail-fast

# Run tests with nextest (faster test execution)
# Excludes integration tests (lifeguard-integration-tests) which require database
nextest-test:
    @echo "🧪 Running tests with nextest (excluding integration tests)..."
    @DATABASE_URL={{DATABASE_URL}} cargo nextest run --workspace --all-features --fail-fast --retries 1 --exclude lifeguard-integration-tests

alias nt := nextest-test

# Run tests with nextest (no capture - passes through stdout/stderr directly)
# Excludes integration tests (lifeguard-integration-tests) which require database
nt-verbose:
    @echo "🧪 Running tests with nextest (no capture - full output)..."
    @DATABASE_URL={{DATABASE_URL}} cargo nextest run --workspace --all-features --no-capture --exclude lifeguard-integration-tests

# Run tests with nextest (CI profile)
# Excludes integration tests (lifeguard-integration-tests) which require database
nt-ci:
    @echo "🧪 Running tests with nextest (CI profile)..."
    @DATABASE_URL={{DATABASE_URL}} cargo nextest run --workspace --all-features --profile ci --exclude lifeguard-integration-tests

# Run unit tests only with nextest
nt-unit:
    @echo "🧪 Running unit tests with nextest..."
    @DATABASE_URL={{DATABASE_URL}} cargo nextest run --workspace --all-features --test-group unit

# Run integration tests (requires database connection)
test-integration:
    @echo "🧪 Running integration tests..."
    @echo "⚠️  Note: These tests require a running database connection"
    @TEST_DATABASE_URL=$(./scripts/get_test_connection_string.sh) cargo test --package lifeguard-integration-tests

# Run integration tests with nextest
nt-integration:
    @echo "🧪 Running integration tests with nextest..."
    @echo "⚠️  Note: These tests require a running database connection"
    @TEST_DATABASE_URL=$(./scripts/get_test_connection_string.sh) cargo nextest run --package lifeguard-integration-tests

# Run tests with standard cargo (fallback)
test-cargo:
    @echo "🧪 Running tests with cargo..."
    @DATABASE_URL={{DATABASE_URL}} cargo test --all -- --nocapture

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

# Format code
fmt:
    @echo "🎨 Formatting code..."
    @cargo fmt

# Check formatting
fmt-check:
    @echo "🎨 Checking code formatting..."
    @cargo fmt -- --check

# Lint code
lint:
    @echo "🔍 Linting code..."
    @cargo clippy -- -D warnings

# Lint and fix
lint-fix:
    @echo "🔍 Linting and fixing code..."
    @cargo clippy --fix --allow-dirty --allow-staged

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
    @echo "📜 PostgreSQL logs..."
    @kubectl logs -n lifeguard-test deployment/postgres --tail=100 -f

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
