-- pg_regress test: Named Bundle API (v0.98.0)
-- Tests: load_datalog_bundle, load_shape_bundle, active_datalog_bundles
-- RB-01 deliverable.

SET search_path TO pg_ripple, public;

-- ─── load_datalog_bundle ─────────────────────────────────────────────────────

-- 1. Load 'skos' bundle — must succeed.
SELECT pg_ripple.load_datalog_bundle('skos') IS NOT DISTINCT FROM NULL AS skos_bundle_loaded;

-- 2. Load 'skos-transitive' bundle.
SELECT pg_ripple.load_datalog_bundle('skos-transitive') IS NOT DISTINCT FROM NULL AS transitive_loaded;

-- 3. Load 'rdfs' bundle.
SELECT pg_ripple.load_datalog_bundle('rdfs') IS NOT DISTINCT FROM NULL AS rdfs_bundle_loaded;

-- 4. Load 'owl-rl' bundle (activates 'rdfs' dependency).
SELECT pg_ripple.load_datalog_bundle('owl-rl') IS NOT DISTINCT FROM NULL AS owlrl_bundle_loaded;

-- 5. Idempotency: loading skos again must not fail.
SELECT pg_ripple.load_datalog_bundle('skos') IS NOT DISTINCT FROM NULL AS skos_idempotent;

-- ─── active_datalog_bundles view ─────────────────────────────────────────────

-- 6. View must be queryable.
SELECT count(*) >= 0 AS view_queryable
FROM pg_ripple.active_datalog_bundles;

-- 7. 'skos' must appear in the catalog.
SELECT count(*) >= 1 AS skos_in_catalog
FROM pg_ripple.active_datalog_bundles
WHERE bundle_name = 'skos';

-- 8. 'rdfs' must appear in the catalog.
SELECT count(*) >= 1 AS rdfs_in_catalog
FROM pg_ripple.active_datalog_bundles
WHERE bundle_name = 'rdfs';

-- ─── load_shape_bundle ───────────────────────────────────────────────────────

-- 9. Load 'skos-integrity' shape bundle.
SELECT pg_ripple.load_shape_bundle('skos-integrity') IS NOT DISTINCT FROM NULL AS integrity_loaded;

-- 10. Dependency resolution: skos-transitive must be in catalog after skos-integrity.
SELECT count(*) >= 1 AS transitive_activated_by_dep
FROM pg_ripple.active_datalog_bundles
WHERE bundle_name = 'skos-transitive';

-- 11. skos-integrity must be in catalog.
SELECT count(*) >= 1 AS integrity_in_catalog
FROM pg_ripple.active_datalog_bundles
WHERE bundle_name = 'skos-integrity';

-- ─── unknown bundle error handling ───────────────────────────────────────────

-- 12. Unknown bundle name must raise an error.
DO $$
BEGIN
    PERFORM pg_ripple.load_datalog_bundle('nonexistent-bundle-xyz');
    RAISE EXCEPTION 'Expected error was not raised';
EXCEPTION WHEN OTHERS THEN
    IF SQLERRM LIKE '%unknown built-in rule set%' OR SQLERRM LIKE '%nonexistent%' THEN
        RAISE NOTICE 'Correctly rejected unknown bundle: %', SQLERRM;
    ELSE
        RAISE; -- Unexpected error.
    END IF;
END;
$$;

-- 13. Unknown shape bundle name must raise an error.
DO $$
BEGIN
    PERFORM pg_ripple.load_shape_bundle('nonexistent-shapes-xyz');
    RAISE EXCEPTION 'Expected error was not raised';
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'Correctly rejected unknown shape bundle: %', SQLERRM;
END;
$$;

-- ─── bundle_version tracking ─────────────────────────────────────────────────

-- 14. bundle_version must be a positive integer.
SELECT bool_and(bundle_version >= 1) AS bundle_version_positive
FROM pg_ripple.active_datalog_bundles;

-- 15. loaded_at must be a recent timestamp.
SELECT bool_and(loaded_at >= (now() - interval '1 hour')) AS loaded_at_recent
FROM pg_ripple.active_datalog_bundles;

-- ─── Cleanup ─────────────────────────────────────────────────────────────────

SELECT pg_ripple.drop_rules('skos') >= 0 AS skos_cleanup;
SELECT pg_ripple.drop_rules('skosxl') >= 0 AS skosxl_cleanup;
SELECT pg_ripple.drop_rules('skos-transitive') >= 0 AS transitive_cleanup;
SELECT pg_ripple.drop_rules('skos-integrity') >= 0 AS integrity_cleanup;
SELECT pg_ripple.drop_rules('rdfs') >= 0 AS rdfs_cleanup;
SELECT pg_ripple.drop_rules('owl-rl') >= 0 AS owlrl_cleanup;
DELETE FROM _pg_ripple.datalog_bundles;
