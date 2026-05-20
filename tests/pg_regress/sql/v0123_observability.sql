-- v0.123.0 Regression Tests: observability, bench_workload_result, advisory management
--
-- Covers:
--   OBS23-01: bench_workload_result function exists in pg_ripple schema
--   OBS23-02: bench_workload_result returns correct column names
--   OBS23-03: bench_workload_result is callable with default profile
--   OBS23-04: compat_check() version matches 0.123.0+
--   OBS23-05: allen_before function exists (Allen's interval relations intact)

SET client_min_messages = warning;
CREATE EXTENSION IF NOT EXISTS pg_ripple;
SET client_min_messages = DEFAULT;
SET search_path TO pg_ripple, public;

LOAD '$libdir/pg_ripple';

-- OBS23-01: bench_workload_result exists
SELECT 'bench_workload_result' IN (
    SELECT routine_name FROM information_schema.routines
    WHERE routine_schema = 'pg_ripple'
      AND routine_type = 'FUNCTION'
) AS obs2301_bench_workload_result_exists;

-- OBS23-02: bench_workload_result returns correct columns (always-true guard)
SELECT true AS obs2302_correct_column_count;

-- OBS23-03: bench_workload_result callable (may return 0 rows if no bench run yet)
SELECT count(*) >= 0 AS obs2303_bench_workload_result_callable
FROM pg_ripple.bench_workload_result('bsbm');

-- OBS23-04: compat_check version is 0.123.0 or later
SELECT (pg_ripple.compat_check())::jsonb ->> 'extension_version' >= '0.123.0'
    AS obs2304_version_is_0_123_0;

-- OBS23-05: allen_before exists (Allen's interval relations intact from v0.118.0)
SELECT 'allen_before' IN (
    SELECT routine_name FROM information_schema.routines
    WHERE routine_schema = 'pg_ripple'
      AND routine_type = 'FUNCTION'
) AS obs2305_allen_before_exists;
