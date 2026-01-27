# Lifeguard Tiltfile
#
# This Tiltfile manages local development resources:
# - PostgreSQL deployment with port forwards
# - Test infrastructure
#
# Usage: tilt up
#
# Resources are organized into parallel streams using labels:
# - 'infrastructure' label: PostgreSQL test database
# - 'migration' label: Migration integration tests (runs in parallel with other tests)
# - One label per component (no multi-label to avoid Tilt UI clutter).
# - Inventory service: 'inventory_entities', 'inventory_gen_migrations', 'inventory_migrations'

# ====================
# Configuration
# ====================

# Restrict to kind cluster
allow_k8s_contexts(['kind-lifeguard-test'])

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
# PostgreSQL Deployment
# ====================
# PostgreSQL is deployed via kustomize for test infrastructure
# Port forwards are configured here for convenient access
# Note: Namespace and PVC are created by setup_kind_cluster.sh
# Tilt only needs to deploy the deployment and service

# Deploy PostgreSQL deployment and service (namespace and PVC already exist)
k8s_yaml([
    '%s/config/k8s/test-infrastructure/postgres-deployment.yaml' % LIFEGUARD_DIR,
    '%s/config/k8s/test-infrastructure/postgres-service.yaml' % LIFEGUARD_DIR,
])

# Configure PostgreSQL resource with port forwards
# Forward to service port 5432 for database access
# Wait for deployment to be ready before marking resource as ready
k8s_resource(
    'postgres',
    labels=['infrastructure'],
    port_forwards=[
        '5432:5432',  # PostgreSQL: localhost:5432 -> service:5432
    ],
    resource_deps=[],  # No dependencies - namespace and PVC already exist
    # Ensure port-forward is established before dependent resources start
    auto_init=True,
)

# ====================
# Build Resources
# ====================
# Compilation resources for catching build errors early

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
    resource_deps=[],  # No dependencies - standalone crate
    labels=['build'],
    allow_parallel=True,  # Can build in parallel with codegen
)

# Build lifeguard-codegen (code generation CLI tool)
# Only build if Cargo.toml exists (crate may not be created yet)
local_resource(
    'build-codegen',
    cmd='if [ -f lifeguard-codegen/Cargo.toml ]; then cargo build -p lifeguard-codegen; else echo "⚠️  lifeguard-codegen crate not found, skipping build"; fi',
    deps=[
        'lifeguard-codegen',
    ],
    ignore=[
        'target/**',
        '**/target/**',
    ],
    resource_deps=[],  # No dependencies - standalone crate
    labels=['build'],
    allow_parallel=True,  # Can build in parallel with derive
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
    resource_deps=['build-derive'],  # Wait for lifeguard-derive to compile first
    labels=['build'],
    allow_parallel=False,  # Serialize after derive build to prevent storms
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
    labels=['inventory_gen_migrations'],
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
    labels=['inventory_migrations'],
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
    resource_deps=['postgres', 'build-lifeguard'],  # Wait for PostgreSQL and build to be ready
    labels=['tests'],
    allow_parallel=False,  # Serialize to prevent build storms
)

# Run nextest (faster test execution for main crate)
# Using nextest as the primary test runner
# Excludes integration tests (lifeguard-integration-tests) which require database and run separately
# Note: ignore patterns prevent infinite loops from test output files in target/
local_resource(
    'test-nextest',
    cmd='TEST_DATABASE_URL=$(./scripts/get_test_connection_string.sh) cargo nextest run --workspace --all-features',
    deps=[
        'src',
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
    resource_deps=['postgres', 'build-lifeguard'],  # Wait for PostgreSQL and build to be ready
    labels=['tests'],
    allow_parallel=False,  # Serialize to prevent build storms
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
    resource_deps=['postgres', 'build-lifeguard'],  # Wait for PostgreSQL and build to be ready
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
#     resource_deps=['postgres'],  # Wait for PostgreSQL to be ready
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
    cmd='if [ -f lifeguard-codegen/Cargo.toml ]; then cd lifeguard-codegen && cargo test --no-fail-fast; else echo "⚠️  lifeguard-codegen crate not found, skipping tests"; fi',
    deps=[
        'lifeguard-codegen',
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
    resource_deps=['postgres'],  # Wait for PostgreSQL to be ready
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
    resource_deps=['postgres'],  # Wait for PostgreSQL to be ready
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
    resource_deps=['postgres'],  # Wait for PostgreSQL to be ready
    labels=['examples'],
    allow_parallel=True,
)
