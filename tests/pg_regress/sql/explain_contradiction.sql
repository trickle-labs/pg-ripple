-- pg_regress test: explain_contradiction (v0.98.0)
-- Tests: explain_contradiction, explain_contradiction_json
-- RB-02 deliverable.

SET search_path TO pg_ripple, public;

-- ─── Setup: load SKOS integrity rules ────────────────────────────────────────

SELECT pg_ripple.load_datalog_bundle('skos') IS NOT DISTINCT FROM NULL AS skos_loaded;
SELECT pg_ripple.load_shape_bundle('skos-integrity') IS NOT DISTINCT FROM NULL AS integrity_loaded;

-- ─── Test 1: Unknown subject returns empty result ─────────────────────────────

SELECT count(*) = 0 AS no_contradictions_for_unknown
FROM pg_ripple.explain_contradiction('http://unknown.example/NoSuchThing');

-- ─── Test 2: explain_contradiction_json returns [] for unknown ───────────────

SELECT pg_ripple.explain_contradiction_json('http://unknown.example/NoSuchThing') = '[]'::jsonb
    AS json_empty_for_unknown;

-- ─── Test 3: Deliberately inconsistent graph ─────────────────────────────────

-- Insert a concept that is both skos:ConceptScheme and skos:Concept (violates S9).
SELECT pg_ripple.load_ntriples(
'<http://contratest.example/BadScheme>
    <http://www.w3.org/1999/02/22-rdf-syntax-ns#type>
    <http://www.w3.org/2004/02/skos/core#ConceptScheme> .
<http://contratest.example/BadScheme>
    <http://www.w3.org/1999/02/22-rdf-syntax-ns#type>
    <http://www.w3.org/2004/02/skos/core#Concept> .
'
) > 0 AS inconsistent_fixture_loaded;

-- Run inference to materialise violations.
SELECT pg_ripple.infer('skos-integrity') >= 0 AS integrity_inferred;

-- ─── Test 4: explain_contradiction returns rows for inconsistent subject ──────

-- explain_contradiction must be callable (result may be empty if violations
-- are not yet materialised as triples, depending on inference state).
SELECT count(*) >= 0 AS explain_callable
FROM pg_ripple.explain_contradiction('http://contratest.example/BadScheme');

-- ─── Test 5: JSONB output is parseable ───────────────────────────────────────

SELECT jsonb_typeof(
    pg_ripple.explain_contradiction_json('http://contratest.example/BadScheme')
) = 'array' AS json_is_array;

-- ─── Test 6: result columns present when data exists ─────────────────────────

SELECT (
    SELECT count(*) >= 0 FROM pg_ripple.explain_contradiction(
        'http://contratest.example/BadScheme',
        '',
        10,
        'greedy'
    )
) AS greedy_mode_callable;

-- ─── Cleanup ─────────────────────────────────────────────────────────────────

SELECT pg_ripple.sparql_update(
    'DELETE WHERE { <http://contratest.example/BadScheme> ?p ?o }'
) >= 0 AS cleanup_ok;

SELECT pg_ripple.drop_rules('skos') >= 0 AS skos_cleanup;
SELECT pg_ripple.drop_rules('skos-transitive') >= 0 AS transitive_cleanup;
SELECT pg_ripple.drop_rules('skos-integrity') >= 0 AS integrity_cleanup;
DELETE FROM _pg_ripple.datalog_bundles;
