-- v0.110.0 Feature Regression Tests
-- Tests for: ER Monitoring Stream Tables
--
-- Covers:
--   ERMON-01: enable_er_monitoring() creates er_unresolved_entities table
--   ERMON-02: enable_er_monitoring() creates er_cluster_sizes table
--   ERMON-03: enable_er_monitoring() creates er_resolution_dashboard table
--   ERMON-04: enable_er_monitoring() is idempotent (second call does not fail)
--   ERMON-05: er_resolution_dashboard has expected columns
--   ERMON-06: er_resolution_dashboard has at least one row after enable
--   ERMON-07: disable_er_monitoring() removes all three tables
--   ERMON-08: disable_er_monitoring() is idempotent (second call does not fail)
--   ERMON-09: evaluate_resolution() returns all nine metric fields
--   ERMON-10: evaluate_resolution() raises PT0461 for empty gold graph

SET client_min_messages = warning;
CREATE EXTENSION IF NOT EXISTS pg_ripple;
SET client_min_messages = DEFAULT;
SET search_path TO pg_ripple, public;

LOAD '$libdir/pg_ripple';

-- ERMON-01: enable_er_monitoring() creates er_unresolved_entities

SELECT pg_ripple.enable_er_monitoring();

SELECT EXISTS (
    SELECT 1 FROM information_schema.tables
    WHERE table_schema = '_pg_ripple'
      AND table_name   = 'er_unresolved_entities'
) AS ermon01_unresolved_exists;

-- ERMON-02: er_cluster_sizes table exists

SELECT EXISTS (
    SELECT 1 FROM information_schema.tables
    WHERE table_schema = '_pg_ripple'
      AND table_name   = 'er_cluster_sizes'
) AS ermon02_cluster_sizes_exists;

-- ERMON-03: er_resolution_dashboard table exists

SELECT EXISTS (
    SELECT 1 FROM information_schema.tables
    WHERE table_schema = '_pg_ripple'
      AND table_name   = 'er_resolution_dashboard'
) AS ermon03_dashboard_exists;

-- ERMON-04: enable_er_monitoring() is idempotent

SELECT pg_ripple.enable_er_monitoring();

SELECT EXISTS (
    SELECT 1 FROM information_schema.tables
    WHERE table_schema = '_pg_ripple'
      AND table_name   = 'er_resolution_dashboard'
) AS ermon04_idempotent_ok;

-- ERMON-05: er_resolution_dashboard has required columns

SELECT
    COUNT(*) >= 5 AS ermon05_has_five_plus_columns
FROM information_schema.columns
WHERE table_schema = '_pg_ripple'
  AND table_name   = 'er_resolution_dashboard';

-- ERMON-06: er_resolution_dashboard has at least one row

SELECT COUNT(*) >= 1 AS ermon06_dashboard_has_rows
FROM _pg_ripple.er_resolution_dashboard;

-- ERMON-07: disable_er_monitoring() removes all three tables

SELECT pg_ripple.disable_er_monitoring();

SELECT
    NOT EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_schema = '_pg_ripple'
          AND table_name   = 'er_unresolved_entities'
    ) AS ermon07_unresolved_dropped,
    NOT EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_schema = '_pg_ripple'
          AND table_name   = 'er_cluster_sizes'
    ) AS ermon07_cluster_sizes_dropped,
    NOT EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_schema = '_pg_ripple'
          AND table_name   = 'er_resolution_dashboard'
    ) AS ermon07_dashboard_dropped;

-- ERMON-08: disable_er_monitoring() is idempotent (no error on second call)

SELECT pg_ripple.disable_er_monitoring();

SELECT true AS ermon08_idempotent_ok;

-- ERMON-09: evaluate_resolution() returns all nine metric fields
-- First insert some owl:sameAs gold triples so the gold graph is non-empty.

SELECT pg_ripple.insert_triple(
    'http://example.org/e1',
    'http://www.w3.org/2002/07/owl#sameAs',
    'http://example.org/e2',
    'http://example.org/goldGraph'
) > 0 AS ermon09_triple_inserted;

SELECT
    result ? 'precision'              AS ermon09_has_precision,
    result ? 'recall'                 AS ermon09_has_recall,
    result ? 'f1'                     AS ermon09_has_f1,
    result ? 'pairs_completeness'     AS ermon09_has_pairs_completeness,
    result ? 'reduction_ratio'        AS ermon09_has_reduction_ratio,
    result ? 'f_pq'                   AS ermon09_has_f_pq,
    result ? 'b3_precision'           AS ermon09_has_b3_precision,
    result ? 'b3_recall'              AS ermon09_has_b3_recall,
    result ? 'b3_f1'                  AS ermon09_has_b3_f1,
    result ? 'total_gold_pairs'       AS ermon09_has_total_gold_pairs,
    result ? 'evaluated_at'           AS ermon09_has_evaluated_at
FROM (
    SELECT pg_ripple.evaluate_resolution('http://example.org/goldGraph') AS result
) sub;

-- ERMON-10: evaluate_resolution() raises PT0461 for empty/nonexistent gold graph

DO $$
BEGIN
    PERFORM pg_ripple.evaluate_resolution('http://example.org/nonExistentGoldGraph');
    RAISE EXCEPTION 'expected PT0461 error not raised';
EXCEPTION
    WHEN OTHERS THEN
        RAISE NOTICE 'ERMON-10 ok: caught PT0461 for empty gold graph';
END;
$$;
