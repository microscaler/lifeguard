-- Dev compatibility: some borrowed observability snippets (e.g. postgres_exporter samples) connect as user `pact`.
-- Idempotent: runs from docker-entrypoint-initdb.d on first init and from postStart on every pod start.
SET client_min_messages = WARNING;

DO $$
BEGIN
  IF NOT EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = 'pact') THEN
    CREATE ROLE pact WITH LOGIN PASSWORD 'pact' NOSUPERUSER NOCREATEDB NOCREATEROLE INHERIT NOREPLICATION;
  END IF;
END
$$;

GRANT CONNECT ON DATABASE postgres TO pact;
GRANT pg_monitor TO pact;
