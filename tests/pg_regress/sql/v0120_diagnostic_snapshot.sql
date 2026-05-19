-- v0.120.0 Feature Regression Tests: Diagnostic Snapshot
-- Tests the SQL layer backing the HTTP /admin/diagnostic-snapshot endpoint.
--
-- Covers:
--   DIAG-01: diagnostic_report() includes the five snapshot-required keys
--   DIAG-02: compiled_version is >= 0.120.0
--   DIAG-03: no NULL values in diagnostic_report()
--   DIAG-04: schema_version key is present and non-empty

SET client_min_messages = warning;
CREATE EXTENSION IF NOT EXISTS pg_ripple;
SET client_min_messages = DEFAULT;
SET search_path TO pg_ripple, public;

LOAD '$libdir/pg_ripple';

-- DIAG-01: snapshot-required keys are present
SELECT count(*) >= 5 AS diag01_required_keys_present
FROM pg_ripple.diagnostic_report()
WHERE key IN (
    'compiled_version',
    'schema_version',
    'total_triple_count',
    'predicate_count',
    'dictionary_size'
);

-- DIAG-02: compiled_version reflects v0.120.0 or later
SELECT (
    split_part(value,'.',1)::int * 1000000
  + split_part(value,'.',2)::int * 1000
  + split_part(value,'.',3)::int
) >= 120000 AS diag02_version_ge_0120
FROM pg_ripple.diagnostic_report()
WHERE key = 'compiled_version';

-- DIAG-03: no NULL values
SELECT count(*) AS diag03_null_values
FROM pg_ripple.diagnostic_report()
WHERE value IS NULL;

-- DIAG-04: schema_version is non-empty
SELECT length(value) > 0 AS diag04_schema_version_present
FROM pg_ripple.diagnostic_report()
WHERE key = 'schema_version';
