-- Synthetic, generic schema for the Lifeguard perf suite. Disposable: every run drops and
-- recreates these objects (the harness refuses to run without PERF_RESET=1).

DROP TABLE IF EXISTS perf_items, perf_wide, perf_ledger, perf_reflect CASCADE;
DROP FUNCTION IF EXISTS public.rls_set_session(text, uuid, uuid, text, jsonb, jsonb, text, text);
DROP FUNCTION IF EXISTS perf_reflect_notify();

-- Narrow row: PK lookups, filtered lists, insert/update/upsert.
CREATE TABLE perf_items (
    id BIGINT PRIMARY KEY,
    account_id BIGINT NOT NULL,
    name TEXT NOT NULL,
    qty INTEGER NOT NULL,
    price DOUBLE PRECISION NOT NULL,
    active BOOLEAN NOT NULL,
    updated_at TIMESTAMP NOT NULL
);
CREATE INDEX perf_items_account_idx ON perf_items (account_id);

-- Accounts 10 / 100 / 1000 hold exactly that many rows (list_N scenarios); the rest are
-- spread over accounts >= 1_000_000 so the table is not trivially small.
INSERT INTO perf_items
SELECT g, 10, 'item-' || g, g % 97, g * 0.25, true, TIMESTAMP '2026-01-01' + (g || ' seconds')::interval
FROM generate_series(1, 10) g;
INSERT INTO perf_items
SELECT 100 + g, 100, 'item-' || g, g % 97, g * 0.25, true, TIMESTAMP '2026-01-01' + (g || ' seconds')::interval
FROM generate_series(1, 100) g;
INSERT INTO perf_items
SELECT 1000 + g, 1000, 'item-' || g, g % 97, g * 0.25, true, TIMESTAMP '2026-01-01' + (g || ' seconds')::interval
FROM generate_series(1, 1000) g;
INSERT INTO perf_items
SELECT 10000 + g, 1000000 + (g % 5000), 'item-' || g, g % 97, g * 0.25, g % 3 <> 0,
       TIMESTAMP '2026-01-01' + (g || ' seconds')::interval
FROM generate_series(1, 100000) g;

-- RLS: a tenant-scoped table and a helper with the same signature Lifeguard calls
-- (`public.rls_set_session`, transaction-local `set_config(..., true)`).
CREATE TABLE perf_ledger (
    id BIGINT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    amount DOUBLE PRECISION NOT NULL,
    memo TEXT NOT NULL
);
INSERT INTO perf_ledger
SELECT g, 'tenant-' || (g % 10), g * 1.5, 'memo-' || g FROM generate_series(1, 50000) g;

CREATE FUNCTION public.rls_set_session(
    p_tenant_id text, p_subject_id uuid, p_organization_id uuid, p_session_id text,
    p_roles jsonb, p_permissions jsonb, p_user_type text, p_org_type text
) RETURNS void LANGUAGE plpgsql AS $fn$
BEGIN
    PERFORM set_config('perf.tenant_id', p_tenant_id, true);
    PERFORM set_config('perf.subject_id', p_subject_id::text, true);
    PERFORM set_config('perf.organization_id', p_organization_id::text, true);
    PERFORM set_config('perf.session_id', p_session_id, true);
    PERFORM set_config('perf.roles', p_roles::text, true);
    PERFORM set_config('perf.permissions', p_permissions::text, true);
    PERFORM set_config('perf.user_type', coalesce(p_user_type, ''), true);
    PERFORM set_config('perf.org_type', coalesce(p_org_type, ''), true);
END;
$fn$;

ALTER TABLE perf_ledger ENABLE ROW LEVEL SECURITY;
ALTER TABLE perf_ledger FORCE ROW LEVEL SECURITY;
CREATE POLICY perf_ledger_tenant ON perf_ledger
    USING (tenant_id = current_setting('perf.tenant_id', true));

DO $roles$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'perf_app') THEN
        CREATE ROLE perf_app LOGIN PASSWORD 'perf_app';
    END IF;
END
$roles$;
GRANT SELECT, INSERT, UPDATE, DELETE ON perf_items, perf_ledger TO perf_app;
GRANT EXECUTE ON FUNCTION public.rls_set_session(text, uuid, uuid, text, jsonb, jsonb, text, text) TO perf_app;

-- Reflector: same key-only payload shape `lifeguard-migrate` generates for #[notify]
-- ({"table","op","id"}), so the measured path is the one the ORM ships.
CREATE TABLE perf_reflect (
    id BIGINT PRIMARY KEY,
    version BIGINT NOT NULL,
    body TEXT NOT NULL
);

CREATE FUNCTION perf_reflect_notify() RETURNS trigger LANGUAGE plpgsql AS $fn$
BEGIN
    PERFORM pg_notify(
        'perf_reflect',
        json_build_object('table', TG_TABLE_NAME, 'op', TG_OP, 'id', NEW.id,
                          'v', (pg_current_wal_insert_lsn() - '0/0'::pg_lsn)::bigint,
                          'ts', (extract(epoch FROM clock_timestamp()) * 1000)::bigint)::text
    );
    RETURN NULL;
END;
$fn$;
CREATE TRIGGER perf_reflect_notify AFTER INSERT OR UPDATE ON perf_reflect
    FOR EACH ROW EXECUTE FUNCTION perf_reflect_notify();

ANALYZE;
