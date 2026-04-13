# Lifeguard Tiltfile
#
# This Tiltfile manages local development resources:
# - PostgreSQL deployment with port forwards
# - Test infrastructure
# - Observability (OTEL Collector, Prometheus, Loki, Grafana) with port forwards
#
# Usage: tilt up
#
# Resources are organized into parallel streams using labels:
# - 'infrastructure' label: PostgreSQL test database
# - 'migration' label: migration tooling + inventory generated SQL checks + lifeguard-integration-tests
# - 'replication' label: read-replica / pool tests (needs primary + replica-0 + Redis; see PRD read-replica)
# - 'perf' label: examples/perf-idam — unit tests (`idam-perf`) and ORM harness run (`idam-perf-run`; destructive DDL when PERF_RESET=1)
# - One label per component (no multi-label to avoid Tilt UI clutter).
# - 'inventory_entities' label: entities example crate build only

# ====================
# Configuration
# ====================

# Restrict to kind cluster
# Shared default cluster: kind-kind. Legacy: kind-lifeguard-test.
allow_k8s_contexts(['kind-kind', 'kind-lifeguard-test'])

# BRRTRouter paths: this Tiltfile does not invoke BRRTRouter. Expected layout is microscaler/lifeguard next to
# microscaler/BRRTRouter. From repo root use ../BRRTRouter; from docs/ use ../../BRRTRouter (see OBSERVABILITY_APP_INTEGRATION.md).

# Configure default registry for Kind cluster
# Tilt will automatically push docker_build images to this registry
# The registry is set up by scripts/setup_kind_cluster.sh
default_registry('localhost:5000')

# Note: Build storms are prevented by setting allow_parallel=False on test resources
# that share dependencies. This ensures tests run serially after builds complete,
# preventing multiple cargo processes from competing for resources.

# Get the directory where this Tiltfile is located
LIFEGUARD_DIR = '.'

# ====================
# Test stack (CI-parity topology)
# ====================
# Bitnami primary + 2 streaming replicas + Redis — same images/env shape as `.github/docker/docker-compose.yml`.
# Host ports match CI: 6543 primary, 6544 replica-0, 6545 redis, 6546 replica-1 (Tilt forwards to cluster services).

k8s_yaml(kustomize('%s/config/k8s/test-infrastructure' % LIFEGUARD_DIR))
# Observability: OTEL Collector, Prometheus, Loki, Grafana (namespace lifeguard-test from test-infrastructure).
k8s_yaml(kustomize('%s/config/k8s/observability' % LIFEGUARD_DIR))

k8s_resource(
    'postgresql-primary',
    labels=['infrastructure'],
    port_forwards=['6543:5432'],
    resource_deps=[],
    auto_init=True,
)
k8s_resource(
    'postgresql-replica-0',
    labels=['infrastructure'],
    port_forwards=['6544:5432'],
    resource_deps=['postgresql-primary'],
    auto_init=True,
)
k8s_resource(
    'postgresql-replica-1',
    labels=['infrastructure'],
    port_forwards=['6546:5432'],
    resource_deps=['postgresql-primary'],
    auto_init=True,
)
k8s_resource(
    'redis',
    labels=['infrastructure'],
    port_forwards=['6545:6379'],
    resource_deps=[],
    auto_init=True,
)

k8s_resource(
    'otel-collector',
    labels=['observability'],
    port_forwards=[
        '4317:4317',
        '4318:4318',
        '9464:9464',
    ],
    resource_deps=[],
    auto_init=True,
)
k8s_resource(
    'prometheus',
    labels=['observability'],
    port_forwards=['9090:9090'],
    resource_deps=[],
    auto_init=True,
)
k8s_resource(
    'loki',
    labels=['observability'],
    port_forwards=['3100:3100'],
    resource_deps=[],
    auto_init=True,
)
k8s_resource(
    'grafana',
    labels=['observability'],
    port_forwards=['3000:3000'],
    resource_deps=[],
    auto_init=True,
)

# ====================
# Build Resources
# ====================
# Compilation resources for catching build errors early (workspace members in Cargo.toml).
# Dependency graph (Cargo):
#   lifeguard-derive — standalone proc-macro
#   lifeguard-codegen — standalone CLI (clap/serde only)
#   lifeguard-reflector — standalone stub (no path deps yet)
#   lifeguard — depends on lifeguard-derive
#   lifeguard-migrate — depends on lifeguard + lifeguard-derive (must follow build-lifeguard)

# Build lifeguard-derive (procedural macros)
local_resource(
    'build-derive',
    cmd='cargo build -p lifeguard-derive',
    deps=[
        'lifeguard-derive/src',
        'lifeguard-derive/Cargo.toml',
        'lifeguard-derive/Cargo.lock',
    ],
    ignore=[
        'target/**',
        '**/target/**',
    ],
    resource_deps=[],
    labels=['build'],
    allow_parallel=True,
)

# Build lifeguard-codegen (code generation CLI tool)
local_resource(
    'build-codegen',
    cmd='cargo build -p lifeguard-codegen',
    deps=[
        'lifeguard-codegen/src',
        'lifeguard-codegen/Cargo.toml',
        'lifeguard-codegen/Cargo.lock',
    ],
    ignore=[
        'target/**',
        '**/target/**',
    ],
    resource_deps=[],
    labels=['build'],
    allow_parallel=True,
)

# Build lifeguard-reflector (cache coherence service; workspace member)
local_resource(
    'build-reflector',
    cmd='cargo build -p lifeguard-reflector',
    deps=[
        'lifeguard-reflector/src',
        'lifeguard-reflector/Cargo.toml',
    ],
    ignore=[
        'target/**',
        '**/target/**',
    ],
    resource_deps=[],
    labels=['build'],
    allow_parallel=True,
)

# Build main lifeguard crate (depends on lifeguard-derive)
local_resource(
    'build-lifeguard',
    cmd='cargo build',
    deps=[
        'src',
        'lifeguard-derive/src',
        'Cargo.toml',
        'Cargo.lock',
        'lifeguard-derive/Cargo.toml',
    ],
    ignore=[
        'target/**',
        '**/target/**',
    ],
    resource_deps=['build-derive'],
    labels=['build'],
    allow_parallel=False,
)

# Build lifeguard-migrate CLI (depends on lifeguard + lifeguard-derive)
local_resource(
    'build-migrate',
    cmd='cargo build -p lifeguard-migrate',
    deps=[
        'lifeguard-migrate/src',
        'lifeguard-migrate/Cargo.toml',
        'lifeguard-migrate/tests',
        'src',
        'lifeguard-derive/src',
        'Cargo.toml',
        'Cargo.lock',
    ],
    ignore=[
        'target/**',
        '**/target/**',
    ],
    resource_deps=['build-lifeguard'],
    labels=['build'],
    allow_parallel=False,
)

# Build example entities crate (showcase example)
# This monitors compilation errors in the entities
# Note: Only building library (--lib) to avoid binary compilation issues
# The binary (generate-migrations) has macro type resolution issues that need separate fixing
local_resource(
    'build-entities',
    cmd='cd examples/entities && cargo build --lib 2>&1',
    deps=[
        'examples/entities/src',
        'examples/entities/Cargo.toml',
        'examples/entities/Cargo.lock',
    ],
    ignore=[
        'target/**',
        '**/target/**',
        'examples/entities/src/bin/**',  # Ignore binary directory
    ],
    resource_deps=['build-lifeguard'],  # Wait for lifeguard to compile first (entities depend on it)
    labels=['inventory_entities'],
    allow_parallel=False,  # Serialize after lifeguard build to prevent storms
)

# ====================
# Inventory Service
# ====================
# Generate migrations from inventory entities (writes to migrations/generated/inventory/)

# Generate migrations from inventory entities
local_resource(
    'gen-migrations',
    cmd='cd examples/entities && cargo run --bin generate-migrations 2>&1',
    deps=[
        'examples/entities',
    ],
    ignore=[
        'target/**',
        '**/target/**',
    ],
    resource_deps=['build-entities'],
    labels=['migration'],
    allow_parallel=False,
)

# Verify generated migrations exist for inventory service
local_resource(
    'check-migrations-inventory',
    cmd=('ls migrations/generated/inventory/*.sql >/dev/null 2>&1 && ' +
         'echo "✅ inventory migrations OK" || ' +
         'echo "⚠️ No .sql in inventory/"'),
    deps=['migrations/generated/inventory'],
    resource_deps=['gen-migrations'],
    labels=['migration'],
)

# ====================
# Test Helpers
# ====================
# Local resources for running tests and examples

# Run unit tests
local_resource(
    'test-unit',
    cmd='cargo test --lib --no-fail-fast',
    deps=[
        'src',
        'Cargo.toml',
        'Cargo.lock',
    ],
    ignore=[
        'target/**',
        '**/target/**',
        '*.stderr',
        '*.stdout',
        'nexttest-errors.log',
        'test-derive-errors.log',
    ],
    resource_deps=['postgresql-primary', 'build-migrate'],
    labels=['tests'],
    allow_parallel=False,  # Serialize to prevent build storms
)

# Run nextest (CI profile: matches .github/workflows/ci.yaml workspace step shape).
# `db_integration_suite` is serialized via nextest test-group `lifeguard-shared-postgres` in .config/nextest.toml
# (shared Kind Postgres + fixed table names — parallel processes used to flake). Replica/Redis env for pool_read_replica.
local_resource(
    'test-nextest',
    cmd=(
        'TEST_DATABASE_URL=$(./scripts/get_test_connection_string.sh) '
        + 'TEST_REPLICA_URL=postgres://postgres:postgres@127.0.0.1:6544/postgres '
        + 'TEST_REDIS_URL=redis://127.0.0.1:6545 '
        + 'cargo nextest run --workspace --all-features --profile ci --config-file .config/nextest.toml'
    ),
    deps=[
        'src',
        'lifeguard-derive',
        'lifeguard-codegen',
        'lifeguard-migrate',
        'lifeguard-reflector',
        'tests-integration',
        'Cargo.toml',
        'Cargo.lock',
        '.config/nextest.toml',
        'scripts/get_test_connection_string.sh',
    ],
    ignore=[
        'target/**',
        '**/target/**',
        '*.stderr',
        '*.stdout',
        'nexttest-errors.log',
        'test-derive-errors.log',
    ],
    resource_deps=[
        'postgresql-primary',
        'postgresql-replica-0',
        'redis',
        'build-codegen',
        'build-reflector',
        'build-migrate',
    ],
    labels=['tests'],
    allow_parallel=False,  # Serialize to prevent build storms
)

# Read-replica pool tests only (`tests/db_integration/pool_read_replica.rs`).
# Filter matches only tests in that module (the only code paths using `TEST_REPLICA_URL`). Nextest would
# otherwise print every other test in the binary as SKIP; `--status-level pass` keeps logs to PASS lines only.
# Full DB integration with replica env: `test-db-integration-replica`. Ports: primary 6543, replica-0 6544, Redis 6545.
local_resource(
    'test-replication-pool',
    cmd=(
        'TEST_DATABASE_URL=$(./scripts/get_test_connection_string.sh) '
        + 'TEST_REPLICA_URL=postgres://postgres:postgres@127.0.0.1:6544/postgres '
        + 'TEST_REDIS_URL=redis://127.0.0.1:6545 '
        + 'cargo nextest run -p lifeguard --all-features --config-file .config/nextest.toml '
        + '--profile db-serial --status-level pass '
        + "-E 'binary(db_integration_suite) and test(~pool_read_replica)'"
    ),
    deps=[
        'tests/db_integration/pool_read_replica.rs',
        'tests/db_integration/replication_sync.rs',
        'tests/db_integration_suite.rs',
        'tests/context.rs',
        'Cargo.toml',
        'Cargo.lock',
        '.config/nextest.toml',
        'scripts/get_test_connection_string.sh',
    ],
    ignore=[
        'target/**',
        '**/target/**',
        '*.stderr',
        '*.stdout',
        'nexttest-errors.log',
        'test-derive-errors.log',
    ],
    resource_deps=[
        'postgresql-primary',
        'postgresql-replica-0',
        'redis',
        'build-migrate',
    ],
    labels=['replication'],
    allow_parallel=False,
)

# Full `db_integration_suite` with replica + Redis env (serial profile — same shape as `just nt-db-suite`).
# Use for PRD read-replica / integration work; avoids running the entire workspace.
local_resource(
    'test-db-integration-replica',
    cmd=(
        'TEST_DATABASE_URL=$(./scripts/get_test_connection_string.sh) '
        + 'TEST_REPLICA_URL=postgres://postgres:postgres@127.0.0.1:6544/postgres '
        + 'TEST_REDIS_URL=redis://127.0.0.1:6545 '
        + 'cargo nextest run -p lifeguard --all-features --config-file .config/nextest.toml '
        + "--profile db-serial -E 'binary(db_integration_suite)'"
    ),
    deps=[
        'tests/db_integration',
        'tests/db_integration_suite.rs',
        'tests/context.rs',
        'Cargo.toml',
        'Cargo.lock',
        '.config/nextest.toml',
        'scripts/get_test_connection_string.sh',
    ],
    ignore=[
        'target/**',
        '**/target/**',
        '*.stderr',
        '*.stdout',
        'nexttest-errors.log',
        'test-derive-errors.log',
    ],
    resource_deps=[
        'postgresql-primary',
        'postgresql-replica-0',
        'redis',
        'build-migrate',
    ],
    labels=['replication'],
    allow_parallel=False,
)

# Single integration test: `LifeguardPool` smoke against streaming replica (fastest loop for pool/replication changes).
local_resource(
    'test-replication-pool-smoke',
    cmd=(
        'TEST_DATABASE_URL=$(./scripts/get_test_connection_string.sh) '
        + 'TEST_REPLICA_URL=postgres://postgres:postgres@127.0.0.1:6544/postgres '
        + 'TEST_REDIS_URL=redis://127.0.0.1:6545 '
        + 'cargo nextest run -p lifeguard --all-features --config-file .config/nextest.toml '
        + '--profile db-serial --status-level pass '
        + "-E 'binary(db_integration_suite) and test(pooled_pool_construct_write_read_with_replica)'"
    ),
    deps=[
        'tests/db_integration/pool_read_replica.rs',
        'tests/db_integration/replication_sync.rs',
        'tests/db_integration_suite.rs',
        'tests/context.rs',
        'Cargo.toml',
        'Cargo.lock',
        '.config/nextest.toml',
        'scripts/get_test_connection_string.sh',
    ],
    ignore=[
        'target/**',
        '**/target/**',
    ],
    resource_deps=[
        'postgresql-primary',
        'postgresql-replica-0',
        'redis',
        'build-migrate',
    ],
    labels=['replication'],
    allow_parallel=False,
)

# Run migration integration tests (requires database connection)
# These tests are separated from the main test suite to avoid slowing down normal test runs
# Runs in parallel with other tests since it's in a separate crate
local_resource(
    'test-migration',
    cmd='TEST_DATABASE_URL=$(./scripts/get_test_connection_string.sh) cargo nextest run --package lifeguard-integration-tests',
    deps=[
        'tests-integration',
        'src/migration',
        'Cargo.toml',
        'Cargo.lock',
        'scripts/get_test_connection_string.sh',
    ],
    ignore=[
        'target/**',
        '**/target/**',
        '*.stderr',
        '*.stdout',
        'nexttest-errors.log',
        'test-derive-errors.log',
    ],
    resource_deps=['postgresql-primary', 'build-migrate'],
    labels=['migration'],
    allow_parallel=True,  # Can run in parallel with other tests (separate crate)
)

# Run integration tests (requires database)
# NOTE: Disabled by default to reduce build storms. Use test-nextest instead.
# Uncomment if you need separate integration test output.
# local_resource(
#     'test-integration',
#     cmd='TEST_DATABASE_URL=$(./scripts/get_test_connection_string.sh) cargo test --test integration --no-fail-fast || echo "⚠️  No integration tests found. Create tests/integration/ directory."',
#     deps=[
#         'src',
#         'tests',
#         'Cargo.toml',
#         'Cargo.lock',
#         'scripts/get_test_connection_string.sh',
#     ],
#     resource_deps=['postgresql-primary'],  # Primary DB for examples
#     labels=['tests'],
#     allow_parallel=False,
# )

# Run lifeguard-derive tests with nextest (faster execution)
# Using nextest as the primary test runner for derive tests
local_resource(
    'test-derive-nextest',
    cmd='cd lifeguard-derive && cargo nextest run --all-features',
    deps=[
        'lifeguard-derive/src',
        'lifeguard-derive/tests',
        'lifeguard-derive/Cargo.toml',
        'lifeguard-derive/Cargo.lock',
        '.config/nextest.toml',  # Use workspace nextest config
    ],
    ignore=[
        'target/**',
        '**/target/**',
        '*.stderr',
        '*.stdout',
        'nexttest-errors.log',
        'test-derive-errors.log',
    ],
    resource_deps=['build-derive'],  # Wait for build to complete first
    labels=['tests'],
    allow_parallel=False,  # Serialize to prevent build storms
)

# Run lifeguard-derive tests (compile-time macro verification tests)
# These tests don't require a database - they verify macro code generation
# NOTE: Disabled by default to reduce build storms. Use test-derive-nextest instead.
# Uncomment if you need standard cargo test output.
# local_resource(
#     'test-derive',
#     cmd='cd lifeguard-derive && cargo test --no-fail-fast',
#     deps=[
#         'lifeguard-derive/src',
#         'lifeguard-derive/tests',
#         'lifeguard-derive/Cargo.toml',
#         'lifeguard-derive/Cargo.lock',
#     ],
#     resource_deps=['build-derive'],  # Wait for build to complete first
#     labels=['tests'],
#     allow_parallel=False,
# )

# Run lifeguard-codegen tests (code generation CLI tests)
# These tests verify the code generation tool works correctly
# Only test if Cargo.toml exists (crate may not be created yet)
local_resource(
    'test-codegen',
    cmd='cargo test -p lifeguard-codegen --no-fail-fast',
    deps=[
        'lifeguard-codegen/src',
        'lifeguard-codegen/Cargo.toml',
        'lifeguard-codegen/Cargo.lock',
    ],
    ignore=[
        'target/**',
        '**/target/**',
        '*.stderr',
        '*.stdout',
        'nexttest-errors.log',
        'test-derive-errors.log',
    ],
    resource_deps=['build-codegen'],  # Wait for build to complete first
    labels=['tests'],
    allow_parallel=False,  # Serialize to prevent build storms
)

# Test the minimal working pattern (verifies basic LifeModel flow)
# NOTE: This is redundant with test-derive-nextest. Disabled to reduce build storms.
# Uncomment if you need to test only the minimal pattern.
# local_resource(
#     'test-minimal-pattern',
#     cmd='cd lifeguard-derive && cargo test --test test_minimal',
#     deps=[
#         'lifeguard-derive/src',
#         'lifeguard-derive/tests/test_minimal.rs',
#         'lifeguard-derive/Cargo.toml',
#         'lifeguard-derive/Cargo.lock',
#     ],
#     resource_deps=['build-derive'],  # Wait for build to complete first
#     labels=['tests'],
#     allow_parallel=False,
# )

# ====================
# IDAM perf (standalone `examples/perf-idam` workspace)
# ====================
# See docs/PERF_ORM.md. Kind/Tilt host ports: primary 6543, replica-0 6544, Redis 6545 (CI Compose uses Toxiproxy :6547 for replica).

local_resource(
    'idam-perf',
    cmd='cd examples/perf-idam && cargo test --locked --no-fail-fast',
    deps=[
        'examples/perf-idam',
    ],
    ignore=[
        'examples/perf-idam/target/**',
        'target/**',
        '**/target/**',
    ],
    resource_deps=['build-lifeguard'],
    labels=['perf'],
    allow_parallel=False,
)

local_resource(
    'idam-perf-run',
    cmd=(
        'export PERF_DATABASE_URL=postgres://postgres:postgres@127.0.0.1:6543/postgres && '
        + 'export PERF_REPLICA_URL=postgres://postgres:postgres@127.0.0.1:6544/postgres && '
        + 'export REDIS_URL=redis://127.0.0.1:6545 && export TEST_REDIS_URL=redis://127.0.0.1:6545 && '
        + 'export PERF_RESET=1 && export PERF_POOL_SIZE=8 && export PERF_TENANT_COUNT=10 && '
        + 'export PERF_USER_ROWS=2000 && export PERF_SESSION_ROWS=2000 && '
        + 'export PERF_WARMUP=100 && export PERF_ITERATIONS=500 && export PERF_OUTPUT=perf-results.json && '
        + 'cd examples/perf-idam && cargo run --release --locked --bin perf-orm'
    ),
    deps=[
        'examples/perf-idam',
        'src',
        'lifeguard-derive',
        'Cargo.toml',
    ],
    ignore=[
        'examples/perf-idam/target/**',
        'target/**',
        '**/target/**',
    ],
    resource_deps=['postgresql-primary', 'postgresql-replica-0', 'redis', 'build-lifeguard'],
    labels=['perf'],
    allow_parallel=False,
)

# ====================
# Examples
# ====================
# Run example applications

# Run basic connection example
local_resource(
    'example-basic-connection',
    cmd='DATABASE_URL=$(./scripts/get_test_connection_string.sh) cargo run --example basic_connection',
    deps=[
        'examples/basic_connection.rs',
        'src',
        'Cargo.toml',
    ],
    ignore=[
        'target/**',
        '**/target/**',
    ],
    resource_deps=['postgresql-primary'],  # Primary DB for examples
    labels=['examples'],
    allow_parallel=True,
)

# Run transaction example
local_resource(
    'example-transaction',
    cmd='DATABASE_URL=$(./scripts/get_test_connection_string.sh) cargo run --example transaction_example',
    deps=[
        'examples/transaction_example.rs',
        'src',
        'Cargo.toml',
    ],
    ignore=[
        'target/**',
        '**/target/**',
    ],
    resource_deps=['postgresql-primary'],  # Primary DB for examples
    labels=['examples'],
    allow_parallel=True,
)

# Run health check example
local_resource(
    'example-health-check',
    cmd='DATABASE_URL=$(./scripts/get_test_connection_string.sh) cargo run --example health_check_example',
    deps=[
        'examples/health_check_example.rs',
        'src',
        'Cargo.toml',
    ],
    ignore=[
        'target/**',
        '**/target/**',
    ],
    resource_deps=['postgresql-primary'],  # Primary DB for examples
    labels=['examples'],
    allow_parallel=True,
)
