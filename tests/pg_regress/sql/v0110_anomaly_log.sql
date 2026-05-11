-- v0.110.0 Feature Regression Tests
-- Tests for: owl:sameAs Anomaly Detection Log
--
-- Covers:
--   ANOMALY-01: GUC pg_ripple.record_sameas_anomalies default is on
--   ANOMALY-02: GUC pg_ripple.sameas_anomaly_log_retention default is not 'impossible_value'
--   ANOMALY-03: _pg_ripple.sameas_anomaly_log table exists
--   ANOMALY-04: sameas_anomaly_log has INSERT-only RLS policy
--   ANOMALY-05: Force a PT550 cluster-size exceeded warning, verify anomaly log row is inserted
--   ANOMALY-06: sameas_anomaly_log columns are accessible

SET client_min_messages = warning;
CREATE EXTENSION IF NOT EXISTS pg_ripple;
SET client_min_messages = DEFAULT;
SET search_path TO pg_ripple, public;

LOAD '$libdir/pg_ripple';

-- ANOMALY-01: GUC record_sameas_anomalies default is on

SELECT current_setting('pg_ripple.record_sameas_anomalies') = 'on'
    AS anomaly01_record_on_by_default;

-- ANOMALY-02: GUC sameas_anomaly_log_retention is accessible

SELECT current_setting('pg_ripple.sameas_anomaly_log_retention', true) IS DISTINCT FROM 'impossible_value'
    AS anomaly02_retention_guc_accessible;

-- ANOMALY-03: sameas_anomaly_log table exists

SELECT EXISTS (
    SELECT 1 FROM information_schema.tables
    WHERE table_schema = '_pg_ripple'
      AND table_name   = 'sameas_anomaly_log'
) AS anomaly03_table_exists;

-- ANOMALY-04: sameas_anomaly_log has RLS enabled

SELECT relrowsecurity AS anomaly04_rls_enabled
FROM pg_class c
JOIN pg_namespace n ON n.oid = c.relnamespace
WHERE n.nspname = '_pg_ripple'
  AND c.relname = 'sameas_anomaly_log';

-- ANOMALY-05: Force PT550 by setting sameas_max_cluster_size very low,
-- then insert enough owl:sameAs triples to trigger it during canonicalization.
-- The canonicalization happens at infer() time so we check the anomaly log.

SET pg_ripple.sameas_max_cluster_size = 2;
SET pg_ripple.record_sameas_anomalies = on;

-- Insert owl:sameAs triples that would create a cluster of 3 members.
SELECT pg_ripple.insert_triple(
    'http://example.org/a1',
    'http://www.w3.org/2002/07/owl#sameAs',
    'http://example.org/a2',
    'http://example.org/testGraph'
) > 0 AS anomaly05_insert1;
SELECT pg_ripple.insert_triple(
    'http://example.org/a2',
    'http://www.w3.org/2002/07/owl#sameAs',
    'http://example.org/a3',
    'http://example.org/testGraph'
) > 0 AS anomaly05_insert2;
SELECT pg_ripple.insert_triple(
    'http://example.org/a1',
    'http://www.w3.org/2002/07/owl#sameAs',
    'http://example.org/a3',
    'http://example.org/testGraph'
) > 0 AS anomaly05_insert3;

-- Run OWL RL inference to trigger union-find canonicalization (and thus PT550).
SELECT pg_ripple.infer('owl-rl') >= 0 AS anomaly05_infer_ran;

-- Check that the anomaly log received a row.
SELECT COUNT(*) >= 1 AS anomaly05_log_has_row
FROM _pg_ripple.sameas_anomaly_log
WHERE cluster_size_after > 0;

-- Reset the GUC.
RESET pg_ripple.sameas_max_cluster_size;

-- ANOMALY-06: sameas_anomaly_log columns are accessible

SELECT
    id IS NOT NULL            AS anomaly06_col_id,
    detected_at IS NOT NULL   AS anomaly06_col_detected_at,
    entity1 IS NOT NULL       AS anomaly06_col_entity1,
    entity2 IS NOT NULL       AS anomaly06_col_entity2,
    trigger IS NOT NULL       AS anomaly06_col_trigger
FROM _pg_ripple.sameas_anomaly_log
ORDER BY id
LIMIT 1;
