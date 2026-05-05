-- v0.94.0 Feature Regression Tests
-- Tests for: H15-01 (bump-version / COMPATIBLE_EXTENSION_MIN),
--            H15-02 (SECURITY DEFINER search_path),
--            H15-03 / L15-13 (bounded bidi relay channel + Prometheus counter),
--            H15-05 / M15-20 (BULK_LOAD_USE_COPY GUC + copy_into_vp helper)
--
-- These tests require the pg_ripple extension to be installed.

SET client_min_messages = warning;
CREATE EXTENSION IF NOT EXISTS pg_ripple;
SET client_min_messages = DEFAULT;
SET search_path TO pg_ripple, public;

-- ─── H15-01: Version bump ────────────────────────────────────────────────────

-- compiled_version should be 0.94.0 or later (extension bumped to 0.95.0)
SELECT value >= '0.94.0' AS version_ok
FROM pg_ripple.diagnostic_report()
WHERE key = 'compiled_version';

-- ─── H15-02: ddl_guard_vp_tables search_path ─────────────────────────────────

-- The SECURITY DEFINER event trigger function must exist and have a
-- SET search_path attribute (prosecdef = true, proconfig contains search_path).
SELECT p.prosecdef = true AS is_secdef,
       (
           SELECT bool_or(cfg LIKE 'search_path%')
           FROM unnest(p.proconfig) AS cfg
       ) AS has_search_path
FROM   pg_catalog.pg_proc p
JOIN   pg_catalog.pg_namespace n ON n.oid = p.pronamespace
WHERE  n.nspname = '_pg_ripple'
AND    p.proname = 'ddl_guard_vp_tables';

-- ─── H15-03: pg_ripple.bidi_relay_max_inflight GUC ───────────────────────────

-- GUC must exist and return the default value of 1000.
SHOW pg_ripple.bidi_relay_max_inflight;

-- Setting to a custom value must work.
SET pg_ripple.bidi_relay_max_inflight = 500;
SHOW pg_ripple.bidi_relay_max_inflight;

-- Reset to default.
RESET pg_ripple.bidi_relay_max_inflight;
SHOW pg_ripple.bidi_relay_max_inflight;

-- ─── L15-13: bidi_relay_dropped_total in streaming_metrics() ─────────────────

-- streaming_metrics() must expose the bidi_relay_dropped_total counter key.
SELECT (
    SELECT count(*) >= 1
    FROM jsonb_each_text(pg_ripple.streaming_metrics())
    WHERE key = 'bidi_relay_dropped_total'
) AS has_dropped_total_key;

-- bidi_relay_inflight counter must also be present.
SELECT (
    SELECT count(*) >= 1
    FROM jsonb_each_text(pg_ripple.streaming_metrics())
    WHERE key = 'bidi_relay_inflight'
) AS has_inflight_key;

-- ─── H15-05: pg_ripple.bulk_load_use_copy GUC ────────────────────────────────

-- GUC must exist with default value 'off'.
SHOW pg_ripple.bulk_load_use_copy;

-- Enabling the GUC must work.
SET pg_ripple.bulk_load_use_copy = on;
SHOW pg_ripple.bulk_load_use_copy;

-- Insert a triple via the bulk loader path to exercise copy_into_vp().
SELECT pg_ripple.load_ntriples(
    '<http://v094test.example/Alice> <http://v094test.example/knows> <http://v094test.example/Bob> .'
) > 0 AS bulk_copy_insert_ok;

-- Reset GUC.
RESET pg_ripple.bulk_load_use_copy;
SHOW pg_ripple.bulk_load_use_copy;

-- ─── Cleanup ─────────────────────────────────────────────────────────────────

SELECT pg_ripple.sparql_update(
    'DELETE DATA { <http://v094test.example/Alice> <http://v094test.example/knows> <http://v094test.example/Bob> }'
) >= 0 AS cleanup_ok;
