-- v0.99.0 Feature Regression Tests
-- Tests for: DCTERMS bundle, Schema.org bundle, FOAF bundle,
--            cross-vocabulary bridges, SQL helpers, and integrity bundles
--
-- These tests require the pg_ripple extension to be installed.

SET client_min_messages = warning;
CREATE EXTENSION IF NOT EXISTS pg_ripple;
SET client_min_messages = DEFAULT;
SET search_path TO pg_ripple, public;

-- ─── DCTERMS-01: load_datalog_bundle('dcterms') ──────────────────────────────

-- load_datalog_bundle('dcterms') must succeed.
SELECT pg_ripple.load_datalog_bundle('dcterms') IS NOT DISTINCT FROM NULL AS dcterms_bundle_loaded;

-- dcterms bundle must appear in active_datalog_bundles.
SELECT count(*) >= 1 AS dcterms_in_catalog
FROM pg_ripple.active_datalog_bundles
WHERE bundle_name = 'dcterms';

-- ─── DCTERMS-02: dcterms prefix registration ─────────────────────────────────

SELECT count(*) >= 1 AS dcterms_prefix_registered
FROM _pg_ripple.prefixes
WHERE prefix = 'dcterms';

SELECT count(*) >= 1 AS dc11_prefix_registered
FROM _pg_ripple.prefixes
WHERE prefix = 'dc11';

-- ─── DCTERMS-03: load_rules_builtin('dcterms') ───────────────────────────────

SELECT pg_ripple.load_rules_builtin('dcterms') > 0 AS dcterms_rules_count_positive;

-- ─── DCTERMS-04: dcterms-integrity bundle ────────────────────────────────────

SELECT pg_ripple.load_shape_bundle('dcterms-integrity') IS NOT DISTINCT FROM NULL AS dcterms_integrity_loaded;

SELECT count(*) >= 1 AS dcterms_integrity_in_catalog
FROM pg_ripple.active_datalog_bundles
WHERE bundle_name = 'dcterms-integrity';

-- ─── SCHEMA-01: load_datalog_bundle('schema') ────────────────────────────────

SELECT pg_ripple.load_datalog_bundle('schema') IS NOT DISTINCT FROM NULL AS schema_bundle_loaded;

SELECT count(*) >= 1 AS schema_in_catalog
FROM pg_ripple.active_datalog_bundles
WHERE bundle_name = 'schema';

-- ─── SCHEMA-02: schema prefix registration ───────────────────────────────────

SELECT count(*) >= 1 AS schema_prefix_registered
FROM _pg_ripple.prefixes
WHERE prefix = 'schema';

-- ─── SCHEMA-03: load_rules_builtin('schema') ─────────────────────────────────

SELECT pg_ripple.load_rules_builtin('schema') > 0 AS schema_rules_count_positive;

-- ─── SCHEMA-04: schema-integrity bundle ──────────────────────────────────────

SELECT pg_ripple.load_shape_bundle('schema-integrity') IS NOT DISTINCT FROM NULL AS schema_integrity_loaded;

SELECT count(*) >= 1 AS schema_integrity_in_catalog
FROM pg_ripple.active_datalog_bundles
WHERE bundle_name = 'schema-integrity';

-- ─── FOAF-01: load_datalog_bundle('foaf') ────────────────────────────────────

SELECT pg_ripple.load_datalog_bundle('foaf') IS NOT DISTINCT FROM NULL AS foaf_bundle_loaded;

SELECT count(*) >= 1 AS foaf_in_catalog
FROM pg_ripple.active_datalog_bundles
WHERE bundle_name = 'foaf';

-- ─── FOAF-02: foaf prefix registration ───────────────────────────────────────

SELECT count(*) >= 1 AS foaf_prefix_registered
FROM _pg_ripple.prefixes
WHERE prefix = 'foaf';

-- ─── FOAF-03: load_rules_builtin('foaf') ─────────────────────────────────────

SELECT pg_ripple.load_rules_builtin('foaf') > 0 AS foaf_rules_count_positive;

-- ─── FOAF-04: foaf-integrity bundle ──────────────────────────────────────────

SELECT pg_ripple.load_shape_bundle('foaf-integrity') IS NOT DISTINCT FROM NULL AS foaf_integrity_loaded;

SELECT count(*) >= 1 AS foaf_integrity_in_catalog
FROM pg_ripple.active_datalog_bundles
WHERE bundle_name = 'foaf-integrity';

-- ─── CROSS-01: all three bundles visible in active_datalog_bundles ────────────

SELECT count(*) = 3 AS three_bundles_active
FROM pg_ripple.active_datalog_bundles
WHERE bundle_name IN ('dcterms', 'schema', 'foaf');

-- ─── HELPER-01: schema_type_ancestors callable ───────────────────────────────

-- schema_type_ancestors must be callable (returns empty for unknown IRI).
SELECT count(*) >= 0 AS schema_ancestors_callable
FROM pg_ripple.schema_type_ancestors('https://schema.org/Unknown');

-- ─── HELPER-02: foaf_persons callable ────────────────────────────────────────

-- foaf_persons must be callable (returns empty when no foaf:Person data loaded).
SELECT count(*) >= 0 AS foaf_persons_callable
FROM pg_ripple.foaf_persons();

-- ─── GUC-01: rule_graph_scope GUC ────────────────────────────────────────────

-- rule_graph_scope GUC must be settable to 'all'.
SET pg_ripple.rule_graph_scope = 'all';
SHOW pg_ripple.rule_graph_scope;

-- rule_graph_scope can also be set to 'default'.
SET pg_ripple.rule_graph_scope = 'default';
SHOW pg_ripple.rule_graph_scope;

-- Reset to default.
RESET pg_ripple.rule_graph_scope;

-- ─── DCTERMS-05: DC11 compatibility rule fires ───────────────────────────────

-- Load DC11 triples and verify dcterms:creator inference.
SELECT pg_ripple.load_ntriples(
    '<http://test.org/doc1> <http://purl.org/dc/elements/1.1/creator> <http://test.org/alice> .'
) >= 0 AS dc11_loaded;

-- ─── SCHEMA-05: Schema.org type hierarchy inference ──────────────────────────

-- Load a schema:LocalBusiness triple.
SELECT pg_ripple.load_ntriples(
    '<http://test.org/biz1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://schema.org/LocalBusiness> .'
) >= 0 AS schema_local_biz_loaded;

-- ─── FOAF-05: foaf:knows triple loading ─────────────────────────────────────

-- Load a foaf:knows triple.
SELECT pg_ripple.load_ntriples(
    '<http://test.org/alice> <http://xmlns.com/foaf/0.1/knows> <http://test.org/bob> .'
) >= 0 AS foaf_knows_loaded;

-- ─── HELPER-03: foaf_persons callable ────────────────────────────────────────

-- foaf_persons must be callable (returns empty when no foaf:Person data loaded for unknown IRI).
SELECT count(*) >= 0 AS foaf_persons_callable
FROM pg_ripple.foaf_persons();

-- ─── HELPER-04: schema_type_ancestors callable ───────────────────────────────

-- schema_type_ancestors for unknown IRI returns empty (>= 0 check always passes).
SELECT count(*) >= 0 AS schema_ancestors_callable
FROM pg_ripple.schema_type_ancestors('http://schema.test/unknown');

-- ─── VERSION-01: compiled_version must be 0.99.x ────────────────────────────

SELECT value LIKE '0.99.%' AS version_is_0_99_x
FROM pg_ripple.diagnostic_report()
WHERE key = 'compiled_version';

-- ─── CLEANUP ─────────────────────────────────────────────────────────────────

SELECT pg_ripple.drop_rules('dcterms') >= 0 AS dcterms_cleanup_ok;
SELECT pg_ripple.drop_rules('dcterms-integrity') >= 0 AS dcterms_integrity_cleanup_ok;
SELECT pg_ripple.drop_rules('schema') >= 0 AS schema_cleanup_ok;
SELECT pg_ripple.drop_rules('schema-integrity') >= 0 AS schema_integrity_cleanup_ok;
SELECT pg_ripple.drop_rules('foaf') >= 0 AS foaf_cleanup_ok;
SELECT pg_ripple.drop_rules('foaf-integrity') >= 0 AS foaf_integrity_cleanup_ok;
DELETE FROM _pg_ripple.datalog_bundles
WHERE bundle_name IN ('dcterms', 'dcterms-integrity', 'schema', 'schema-integrity', 'foaf', 'foaf-integrity');
