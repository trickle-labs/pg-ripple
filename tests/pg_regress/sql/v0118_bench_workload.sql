-- v0.118.0 Feature Regression Tests
-- Tests for: Integrated Benchmark Runner (Feature 1)
--
-- Covers:
--   BENCH-01: bench_workload('bsbm') returns a positive run_id
--   BENCH-02: bench_history has a row after bench_workload()
--   BENCH-03: bench_workload('watdiv') runs without error
--   BENCH-04: bench_workload('pagerank') runs without error
--   BENCH-05: bench_workload('pprl') runs without error
--   BENCH-06: bench_workload() with default profile runs without error
--   BENCH-07: bench_history_recent() returns result set (not null)

SET client_min_messages = warning;
CREATE EXTENSION IF NOT EXISTS pg_ripple;
SET client_min_messages = DEFAULT;
SET search_path TO pg_ripple, public;

LOAD '$libdir/pg_ripple';

-- BENCH-01: bench_workload('bsbm') returns a positive run_id

SELECT pg_ripple.bench_workload('bsbm') > 0 AS bench01_bsbm_run_id_positive;

-- BENCH-02: bench_history has a row after bench_workload()

SELECT EXISTS(
    SELECT 1 FROM _pg_ripple.bench_history
    WHERE profile = 'bsbm'
) AS bench02_history_row_exists;

-- BENCH-03: bench_workload('watdiv') runs without error

SELECT pg_ripple.bench_workload('watdiv') > 0 AS bench03_watdiv_run_id_positive;

-- BENCH-04: bench_workload('pagerank') runs without error

SELECT pg_ripple.bench_workload('pagerank') > 0 AS bench04_pagerank_run_id_positive;

-- BENCH-05: bench_workload('pprl') runs without error

SELECT pg_ripple.bench_workload('pprl') > 0 AS bench05_pprl_run_id_positive;

-- BENCH-06: bench_workload() with default profile runs without error

SELECT pg_ripple.bench_workload() > 0 AS bench06_default_profile_ok;

-- BENCH-07: bench_history_recent() returns result set (not null)

SELECT COUNT(*) >= 0 AS bench07_history_recent_ok
FROM pg_ripple.bench_history_recent();
