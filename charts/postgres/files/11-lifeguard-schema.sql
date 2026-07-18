-- Dedicated application schema for Kind/Tilt + CI (avoids defaulting to public).
-- Idempotent: safe to run on every primary pod postStart; replicated to standbys.
SET client_min_messages = WARNING;

CREATE SCHEMA IF NOT EXISTS lifeguard;

GRANT USAGE ON SCHEMA lifeguard TO postgres;
GRANT CREATE ON SCHEMA lifeguard TO postgres;
ALTER DEFAULT PRIVILEGES IN SCHEMA lifeguard GRANT ALL ON TABLES TO postgres;
ALTER DEFAULT PRIVILEGES IN SCHEMA lifeguard GRANT ALL ON SEQUENCES TO postgres;
