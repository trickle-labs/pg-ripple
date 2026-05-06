-- v0.98.0 Feature Regression Tests
-- Tests for: SKOS support, named bundle API, explain_contradiction,
--            federation trust layer, and coverage_map
--
-- These tests require the pg_ripple extension to be installed.

SET client_min_messages = warning;
CREATE EXTENSION IF NOT EXISTS pg_ripple;
SET client_min_messages = DEFAULT;
SET search_path TO pg_ripple, public;

-- ─── SKOS-01: load_builtin_rules('skos') ─────────────────────────────────────

-- Loading the 'skos' rule set must return a positive rule count.
SELECT pg_ripple.load_rules_builtin('skos') > 0 AS skos_rules_loaded;

-- ─── SKOS-02: load_builtin_rules('skosxl') ───────────────────────────────────

SELECT pg_ripple.load_rules_builtin('skosxl') > 0 AS skosxl_rules_loaded;

-- ─── SKOS-03: load_builtin_rules('skos-transitive') ──────────────────────────

SELECT pg_ripple.load_rules_builtin('skos-transitive') > 0 AS skos_transitive_rules_loaded;

-- ─── SKOS-04: Prefix registration ────────────────────────────────────────────

-- skos: prefix must be registered after loading the rule set.
SELECT count(*) >= 1 AS skos_prefix_registered
FROM _pg_ripple.prefixes
WHERE prefix = 'skos';

-- skosxl: prefix must be registered.
SELECT count(*) >= 1 AS skosxl_prefix_registered
FROM _pg_ripple.prefixes
WHERE prefix = 'skosxl';

-- ─── RB-01: load_datalog_bundle ──────────────────────────────────────────────

-- load_datalog_bundle('skos') must succeed.
SELECT pg_ripple.load_datalog_bundle('skos') IS NOT DISTINCT FROM NULL AS bundle_loaded;

-- Bundle must appear in active_datalog_bundles view.
SELECT count(*) >= 1 AS bundle_in_catalog
FROM pg_ripple.active_datalog_bundles
WHERE bundle_name = 'skos';

-- ─── RB-01: load_shape_bundle ────────────────────────────────────────────────

-- load_shape_bundle('skos-integrity') must automatically activate skos-transitive.
SELECT pg_ripple.load_shape_bundle('skos-integrity') IS NOT DISTINCT FROM NULL AS shape_bundle_loaded;

-- skos-integrity and skos-transitive must both appear in the catalog.
SELECT count(*) >= 1 AS integrity_in_catalog
FROM pg_ripple.active_datalog_bundles
WHERE bundle_name = 'skos-integrity';

SELECT count(*) >= 1 AS transitive_in_catalog
FROM pg_ripple.active_datalog_bundles
WHERE bundle_name = 'skos-transitive';

-- ─── RB-01: active_datalog_bundles view ──────────────────────────────────────

-- View must be queryable.
SELECT count(*) >= 0 AS view_queryable
FROM pg_ripple.active_datalog_bundles;

-- ─── RB-03: federation_endpoints table ───────────────────────────────────────

-- Table must exist.
SELECT count(*) >= 0 AS federation_endpoints_exists
FROM pg_ripple.federation_endpoints;

-- Insert and select an endpoint.
INSERT INTO pg_ripple.federation_endpoints (name, endpoint_url, min_confidence)
VALUES ('test-endpoint', 'https://sparql.example.org/query', 0.7);

SELECT name, endpoint_url, min_confidence >= 0.0 AS conf_non_negative
FROM pg_ripple.federation_endpoints
WHERE name = 'test-endpoint';

-- Cleanup.
DELETE FROM pg_ripple.federation_endpoints WHERE name = 'test-endpoint';

-- ─── RB-03: allow_unregistered_service_endpoints GUC ─────────────────────────

-- GUC must exist with default value 'off'.
SHOW pg_ripple.allow_unregistered_service_endpoints;

-- Can be enabled.
SET pg_ripple.allow_unregistered_service_endpoints = on;
SHOW pg_ripple.allow_unregistered_service_endpoints;

-- Reset.
RESET pg_ripple.allow_unregistered_service_endpoints;
SHOW pg_ripple.allow_unregistered_service_endpoints;

-- ─── SKOS-05: SQL helpers callable ───────────────────────────────────────────

-- skos_ancestors must be callable (may return empty for unknown IRI).
SELECT count(*) >= 0 AS ancestors_callable
FROM pg_ripple.skos_ancestors('http://skos.test/unknown');

-- skos_descendants must be callable.
SELECT count(*) >= 0 AS descendants_callable
FROM pg_ripple.skos_descendants('http://skos.test/unknown');

-- skos_label must return NULL for unknown concept.
SELECT pg_ripple.skos_label('http://skos.test/unknown') IS NULL AS label_null_for_unknown;

-- skos_related must be callable.
SELECT count(*) >= 0 AS related_callable
FROM pg_ripple.skos_related('http://skos.test/unknown');

-- skos_siblings must be callable.
SELECT count(*) >= 0 AS siblings_callable
FROM pg_ripple.skos_siblings('http://skos.test/unknown');

-- ─── RB-02: explain_contradiction callable ────────────────────────────────────

-- explain_contradiction must return 0 rows for an unknown subject.
SELECT count(*) = 0 AS no_contradictions_for_unknown
FROM pg_ripple.explain_contradiction('http://skos.test/unknown');

-- explain_contradiction_json must return an empty array for unknown subject.
SELECT pg_ripple.explain_contradiction_json('http://skos.test/unknown') = '[]'::jsonb
    AS json_empty_for_unknown;

-- ─── RB-04: coverage_map callable ────────────────────────────────────────────

-- coverage_map must be callable (may return empty without SKOS data).
SELECT count(*) >= 0 AS coverage_map_callable
FROM pg_ripple.coverage_map();

-- ─── validate_skos callable ──────────────────────────────────────────────────

-- validate_skos must return 0 violations against empty graph.
SELECT count(*) = 0 AS no_violations_on_empty
FROM pg_ripple.validate_skos();

-- ─── Version check ───────────────────────────────────────────────────────────

-- compiled_version must be 0.98.0.
SELECT value = '0.98.0' AS version_is_0_98_0
FROM pg_ripple.diagnostic_report()
WHERE key = 'compiled_version';

-- ─── Cleanup ─────────────────────────────────────────────────────────────────

SELECT pg_ripple.drop_rules('skos') >= 0 AS skos_cleanup_ok;
SELECT pg_ripple.drop_rules('skosxl') >= 0 AS skosxl_cleanup_ok;
SELECT pg_ripple.drop_rules('skos-transitive') >= 0 AS transitive_cleanup_ok;
SELECT pg_ripple.drop_rules('skos-integrity') >= 0 AS integrity_cleanup_ok;
DELETE FROM _pg_ripple.datalog_bundles;
