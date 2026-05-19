-- v0.120.0 Feature Regression Tests: Read-Replica Routing
-- Tests the read_replica_dsn GUC and routing behaviour.
--
-- Covers:
--   REPLICA-01: read_replica_dsn GUC exists and is accessible
--   REPLICA-02: default value is empty string (primary-only mode)
--   REPLICA-03: setting to empty does not error
--   REPLICA-04: triple insert/query round-trip works in primary-only mode

SET client_min_messages = warning;
CREATE EXTENSION IF NOT EXISTS pg_ripple;
SET client_min_messages = DEFAULT;
SET search_path TO pg_ripple, public;

LOAD '$libdir/pg_ripple';

-- REPLICA-01: GUC exists
SELECT count(*) = 1 AS replica01_guc_exists
FROM pg_settings
WHERE name = 'pg_ripple.read_replica_dsn';

-- REPLICA-02: default is empty string
SELECT length(COALESCE(setting, '')) = 0 AS replica02_default_empty
FROM pg_settings
WHERE name = 'pg_ripple.read_replica_dsn';

-- REPLICA-03: setting to empty is a no-op
SET pg_ripple.read_replica_dsn = '';
SELECT current_setting('pg_ripple.read_replica_dsn') = '' AS replica03_empty_ok;

-- REPLICA-04: basic query works in primary-only mode
SELECT pg_ripple.insert_triple(
    'https://example.org/replica-test/s',
    'https://example.org/replica-test/p',
    '"routing-test"'
) > 0 AS replica04_insert_ok;
SELECT count(*) >= 1 AS replica04_query_ok
FROM pg_ripple.sparql($q$
    SELECT ?o WHERE {
        <https://example.org/replica-test/s>
        <https://example.org/replica-test/p>
        ?o
    }
$q$);

-- Cleanup
SELECT pg_ripple.delete_triple(
    'https://example.org/replica-test/s',
    'https://example.org/replica-test/p',
    '"routing-test"'
);
