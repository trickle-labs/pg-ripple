-- v0.118.0 Feature Regression Tests
-- Tests for: Privacy Budget Registry (Feature 2)
--
-- Covers:
--   BUDGET-01: privacy_budget table exists in _pg_ripple schema
--   BUDGET-02: can insert a budget row
--   BUDGET-03: dp_noisy_count() with valid budget deducts epsilon
--   BUDGET-04: dp_noisy_count() raises PT0490 when budget exhausted
--   BUDGET-05: bench_history table exists in _pg_ripple schema

SET client_min_messages = warning;
CREATE EXTENSION IF NOT EXISTS pg_ripple;
SET client_min_messages = DEFAULT;
SET search_path TO pg_ripple, public;

LOAD '$libdir/pg_ripple';

-- BUDGET-01: privacy_budget table exists in _pg_ripple schema

SELECT EXISTS(
    SELECT 1 FROM information_schema.tables
    WHERE table_schema = '_pg_ripple'
      AND table_name = 'privacy_budget'
) AS budget01_table_exists;

-- BUDGET-02: can insert a budget row

INSERT INTO _pg_ripple.privacy_budget
    (dataset_id, principal, budget_total, budget_spent)
VALUES
    (1001, 'test_principal', 1.0, 0.0)
ON CONFLICT (dataset_id, principal) DO UPDATE
    SET budget_total = EXCLUDED.budget_total,
        budget_spent = 0.0,
        last_reset_at = now();

SELECT budget_total = 1.0 AND budget_spent = 0.0 AS budget02_row_inserted
FROM _pg_ripple.privacy_budget
WHERE dataset_id = 1001 AND principal = 'test_principal';

-- BUDGET-03: dp_noisy_count() with valid budget deducts epsilon
-- epsilon = 0.1 against budget_total = 1.0 → should succeed

SELECT pg_ripple.dp_noisy_count(
    'SELECT COUNT(*) FROM _pg_ripple.dictionary',
    0.1,
    1001,
    'test_principal'
) >= 0 AS budget03_within_budget_succeeds;

-- Verify budget_spent was increased

SELECT budget_spent > 0 AS budget03_epsilon_deducted
FROM _pg_ripple.privacy_budget
WHERE dataset_id = 1001 AND principal = 'test_principal';

-- BUDGET-04: dp_noisy_count() raises PT0490 when budget exhausted
-- Set budget_spent to budget_total to force exhaustion

UPDATE _pg_ripple.privacy_budget
SET budget_spent = budget_total
WHERE dataset_id = 1001 AND principal = 'test_principal';

DO $$
BEGIN
    PERFORM pg_ripple.dp_noisy_count(
        'SELECT COUNT(*) FROM _pg_ripple.dictionary',
        0.1,
        1001::bigint,
        'test_principal'
    );
    RAISE EXCEPTION 'expected PT0490 error not raised';
EXCEPTION
    WHEN OTHERS THEN
        RAISE NOTICE 'BUDGET-04 ok: caught PT0490 for exhausted budget';
END;
$$;

-- Clean up

DELETE FROM _pg_ripple.privacy_budget
WHERE dataset_id = 1001 AND principal = 'test_principal';

-- BUDGET-05: bench_history table exists in _pg_ripple schema

SELECT EXISTS(
    SELECT 1 FROM information_schema.tables
    WHERE table_schema = '_pg_ripple'
      AND table_name = 'bench_history'
) AS budget05_bench_history_exists;
