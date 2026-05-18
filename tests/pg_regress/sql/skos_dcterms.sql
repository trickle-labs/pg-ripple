-- pg_regress test: DCTERMS vocabulary bundle activation path (M16-08 v0.116.0)
-- Exercises the dcterms vocabulary bundle independently via load_datalog_bundle()
-- and load_shape_bundle('dcterms-integrity').

SET search_path TO pg_ripple, public;

-- 1. Load the 'dcterms' Datalog bundle
SELECT pg_ripple.load_datalog_bundle('dcterms')
    IS NOT DISTINCT FROM NULL AS dcterms_loaded;

-- 2. Bundle must be recorded in _pg_ripple.datalog_bundles
SELECT count(*) >= 1 AS dcterms_bundle_registered
FROM _pg_ripple.datalog_bundles
WHERE bundle_name = 'dcterms';

-- 3. 'dcterms:' prefix must be registered
SELECT count(*) >= 1 AS dcterms_prefix_registered
FROM _pg_ripple.prefixes
WHERE prefix = 'dcterms';

-- 4. At least one rule must be stored for the dcterms rule set
SELECT count(*) >= 1 AS dcterms_rules_stored
FROM _pg_ripple.rules
WHERE rule_set = 'dcterms';

-- 5. Load dcterms-integrity shape bundle
SELECT pg_ripple.load_shape_bundle('dcterms-integrity')
    IS NOT DISTINCT FROM NULL AS dcterms_integrity_loaded;

-- 6. dcterms-integrity rules must be stored
SELECT count(*) >= 1 AS dcterms_integrity_rules_stored
FROM _pg_ripple.rules
WHERE rule_set = 'dcterms-integrity';

-- 7. Re-loading is idempotent — count stays >= 1
SELECT pg_ripple.load_datalog_bundle('dcterms')
    IS NOT DISTINCT FROM NULL AS dcterms_reload_idempotent;

SELECT count(*) >= 1 AS dcterms_rules_still_stored
FROM _pg_ripple.rules
WHERE rule_set = 'dcterms';

-- ─── Cleanup ──────────────────────────────────────────────────────────────────

SELECT pg_ripple.drop_rules('dcterms') >= 0 AS dcterms_cleanup;
SELECT pg_ripple.drop_rules('dcterms-integrity') >= 0 AS dcterms_integrity_cleanup;
DELETE FROM _pg_ripple.datalog_bundles WHERE bundle_name IN ('dcterms', 'dcterms-integrity');
