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
