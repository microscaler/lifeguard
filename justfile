set dotenv-load := true

DATABASE_URL := "postgres://postgres:postgres@localhost:5432/postgres"
ENTITY_DIR := "src/entity"

start-db:
	docker compose -f docker-compose.yaml up -d

stop-db:
	docker compose -f docker-compose.yaml down

wait-db:
	until pg_isready -h localhost -p 5432 -U postgres; do sleep 1; done

apply-schema:
	psql {{DATABASE_URL}} -f examples/db/schema.sql

migrate-up:
	sea-orm-cli migrate up -u {{DATABASE_URL}}

migrate-down:
	sea-orm-cli migrate down -u {{DATABASE_URL}}

migrate-refresh:
	sea-orm-cli migrate refresh -u {{DATABASE_URL}}

generate-entities:
	sea-orm-cli generate entity \
	--database-url {{DATABASE_URL}} \
	--output-dir {{ENTITY_DIR}} \
	--with-serde both

build:
	cargo build

# Run tests with nextest (faster, parallel test runner)
# Shows full test output for all tests (success and failure)
nt:
	@echo "🧪 Running tests with nextest (full output)..."
	@DATABASE_URL={{DATABASE_URL}} cargo nextest run --workspace --all-features

# Run tests with nextest (no capture - passes through stdout/stderr directly)
nt-verbose:
	@echo "🧪 Running tests with nextest (no capture - full output)..."
	@DATABASE_URL={{DATABASE_URL}} cargo nextest run --workspace --all-features --no-capture

# Run tests with nextest (CI profile)
nt-ci:
	@echo "🧪 Running tests with nextest (CI profile)..."
	@DATABASE_URL={{DATABASE_URL}} cargo nextest run --workspace --all-features --profile ci

# Run unit tests only with nextest
nt-unit:
	@echo "🧪 Running unit tests with nextest..."
	@DATABASE_URL={{DATABASE_URL}} cargo nextest run --workspace --all-features --test-group unit

# Run tests with standard cargo (fallback)
test:
	@echo "🧪 Running tests with cargo..."
	@DATABASE_URL={{DATABASE_URL}} cargo test --all -- --nocapture


setup:
	just start-db
	just wait-db
	just migrate-up

reset-and-test:
	just migrate-refresh
	just test

seed-db:
	cargo run --example seed_petshop

seed-db-heavy n:
	cargo run --release --example seed_petshop_heavy -- {{n}}

metrics-server:
	cargo run --example metrics_server
