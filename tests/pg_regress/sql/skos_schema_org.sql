-- pg_regress test: Schema.org vocabulary bundle activation path (M16-08 v0.116.0)
-- Exercises the 'schema' vocabulary bundle independently via load_datalog_bundle().

SET search_path TO pg_ripple, public;

-- 1. Load the 'schema' Datalog bundle
SELECT pg_ripple.load_datalog_bundle('schema')
    IS NOT DISTINCT FROM NULL AS schema_loaded;

-- 2. Bundle must be recorded in _pg_ripple.datalog_bundles
SELECT count(*) >= 1 AS schema_bundle_registered
FROM _pg_ripple.datalog_bundles
WHERE bundle_name = 'schema';

-- 3. At least one rule must be stored for the schema rule set
SELECT count(*) >= 1 AS schema_rules_stored
FROM _pg_ripple.rules
WHERE rule_set = 'schema';

-- 4. Load schema-integrity shape bundle
SELECT pg_ripple.load_shape_bundle('schema-integrity')
    IS NOT DISTINCT FROM NULL AS schema_integrity_loaded;

-- 5. schema-integrity rules must be stored
SELECT count(*) >= 1 AS schema_integrity_rules_stored
FROM _pg_ripple.rules
WHERE rule_set = 'schema-integrity';

-- 6. Re-loading is idempotent — count stays >= 1
SELECT pg_ripple.load_datalog_bundle('schema')
    IS NOT DISTINCT FROM NULL AS schema_reload_idempotent;

SELECT count(*) >= 1 AS schema_rules_still_stored
FROM _pg_ripple.rules
WHERE rule_set = 'schema';

-- ─── Cleanup ──────────────────────────────────────────────────────────────────

SELECT pg_ripple.drop_rules('schema') >= 0 AS schema_cleanup;
SELECT pg_ripple.drop_rules('schema-integrity') >= 0 AS schema_integrity_cleanup;
DELETE FROM _pg_ripple.datalog_bundles WHERE bundle_name IN ('schema', 'schema-integrity');
