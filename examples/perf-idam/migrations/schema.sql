-- IDAM-shaped perf tables in `public` (must match entities in src/perf_idam/).
-- Dropped in FK-safe order; recreated for a clean run each time.
DROP TABLE IF EXISTS perf_sessions CASCADE;
DROP TABLE IF EXISTS perf_users CASCADE;
DROP TABLE IF EXISTS perf_tenants CASCADE;

CREATE TABLE perf_tenants (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL
);

CREATE TABLE perf_users (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES perf_tenants (id) ON DELETE CASCADE,
    email TEXT NOT NULL,
    display_name VARCHAR(255) NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT perf_users_tenant_email_unique UNIQUE (tenant_id, email)
);

CREATE TABLE perf_sessions (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES perf_users (id) ON DELETE CASCADE,
    token_fingerprint VARCHAR(128) NOT NULL,
    expires_at TIMESTAMP NOT NULL,
    last_seen_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT perf_sessions_token_unique UNIQUE (token_fingerprint)
);

CREATE INDEX idx_perf_sessions_user_id ON perf_sessions (user_id);
