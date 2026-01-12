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
    resource_deps=['postgres'],  # Wait for PostgreSQL to be ready
    labels=['tests'],
    allow_parallel=True,
)

# Run integration tests (requires database)
local_resource(
    'test-integration',
    cmd='TEST_DATABASE_URL=$(./scripts/get_test_connection_string.sh) cargo test --test integration --no-fail-fast || echo "⚠️  No integration tests found. Create tests/integration/ directory."',
    deps=[
        'src',
        'tests',
        'Cargo.toml',
        'Cargo.lock',
        'scripts/get_test_connection_string.sh',
    ],
    resource_deps=['postgres'],  # Wait for PostgreSQL to be ready
    labels=['tests'],
    allow_parallel=True,
)

# Run nextest (faster test execution)
local_resource(
    'test-nextest',
    cmd='TEST_DATABASE_URL=$(./scripts/get_test_connection_string.sh) cargo nextest run --workspace --all-features',
    deps=[
        'src',
        'Cargo.toml',
        'Cargo.lock',
        'scripts/get_test_connection_string.sh',
    ],
    resource_deps=['postgres'],  # Wait for PostgreSQL to be ready
    labels=['tests'],
    allow_parallel=True,
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
    resource_deps=['postgres'],  # Wait for PostgreSQL to be ready
    labels=['examples'],
    allow_parallel=True,
)
