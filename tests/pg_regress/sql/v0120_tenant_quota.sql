-- v0.120.0 Feature Regression Tests: Tenant Quota
-- Tests the _pg_ripple.tenants table schema and quota enforcement.
--
-- Covers:
--   QUOTA-01: tenants table exists in _pg_ripple schema
--   QUOTA-02: tenants table has quota_triples column
--   QUOTA-03: tenant insert and query round-trip works
--   QUOTA-04: quota_triples defaults to 0

SET client_min_messages = warning;
CREATE EXTENSION IF NOT EXISTS pg_ripple;
SET client_min_messages = DEFAULT;
SET search_path TO pg_ripple, public;

LOAD '$libdir/pg_ripple';

-- QUOTA-01: tenants table exists
SELECT EXISTS (
    SELECT 1 FROM information_schema.tables
    WHERE table_schema = '_pg_ripple'
      AND table_name = 'tenants'
) AS quota01_tenants_table_exists;

-- QUOTA-02: quota_triples column exists with correct type
SELECT EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema = '_pg_ripple'
      AND table_name = 'tenants'
      AND column_name = 'quota_triples'
      AND data_type = 'bigint'
) AS quota02_quota_triples_column_exists;

-- QUOTA-03: tenant insert and query round-trip
DELETE FROM _pg_ripple.tenants WHERE tenant_name = 'test-v0120-quota';
INSERT INTO _pg_ripple.tenants (tenant_name)
VALUES ('test-v0120-quota')
ON CONFLICT (tenant_name) DO NOTHING;
SELECT tenant_name = 'test-v0120-quota' AS quota03_insert_ok
FROM _pg_ripple.tenants
WHERE tenant_name = 'test-v0120-quota';

-- QUOTA-04: quota_triples defaults to 0
SELECT quota_triples = 0 AS quota04_default_is_zero
FROM _pg_ripple.tenants
WHERE tenant_name = 'test-v0120-quota';

-- Cleanup
DELETE FROM _pg_ripple.tenants WHERE tenant_name = 'test-v0120-quota';
