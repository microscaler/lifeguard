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

test:
	DATABASE_URL={{DATABASE_URL}} cargo test --test integration -- --nocapture


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
