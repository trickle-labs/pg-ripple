-- pg_regress test: FOAF vocabulary bundle activation path (M16-08 v0.116.0)
-- Exercises the 'foaf' vocabulary bundle independently via load_datalog_bundle()
-- and load_shape_bundle('foaf-integrity').

SET search_path TO pg_ripple, public;

-- 1. Load the 'foaf' Datalog bundle
SELECT pg_ripple.load_datalog_bundle('foaf')
    IS NOT DISTINCT FROM NULL AS foaf_loaded;

-- 2. Bundle must be recorded in _pg_ripple.datalog_bundles
SELECT count(*) >= 1 AS foaf_bundle_registered
FROM _pg_ripple.datalog_bundles
WHERE bundle_name = 'foaf';

-- 3. 'foaf:' prefix must be registered
SELECT count(*) >= 1 AS foaf_prefix_registered
FROM _pg_ripple.prefixes
WHERE prefix = 'foaf';

-- 4. At least one rule must be stored for the foaf rule set
SELECT count(*) >= 1 AS foaf_rules_stored
FROM _pg_ripple.rules
WHERE rule_set = 'foaf';

-- 5. Load foaf-integrity shape bundle
SELECT pg_ripple.load_shape_bundle('foaf-integrity')
    IS NOT DISTINCT FROM NULL AS foaf_integrity_loaded;

-- 6. foaf-integrity rules must be stored
SELECT count(*) >= 1 AS foaf_integrity_rules_stored
FROM _pg_ripple.rules
WHERE rule_set = 'foaf-integrity';

-- 7. Re-loading is idempotent — count stays >= 1
SELECT pg_ripple.load_datalog_bundle('foaf')
    IS NOT DISTINCT FROM NULL AS foaf_reload_idempotent;

SELECT count(*) >= 1 AS foaf_rules_still_stored
FROM _pg_ripple.rules
WHERE rule_set = 'foaf';

-- ─── Cleanup ──────────────────────────────────────────────────────────────────

SELECT pg_ripple.drop_rules('foaf') >= 0 AS foaf_cleanup;
SELECT pg_ripple.drop_rules('foaf-integrity') >= 0 AS foaf_integrity_cleanup;
DELETE FROM _pg_ripple.datalog_bundles WHERE bundle_name IN ('foaf', 'foaf-integrity');
