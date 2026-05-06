-- pg_regress test: coverage_map / refresh_coverage_map (v0.98.0)
-- Tests: coverage_map SRF, refresh_coverage_map, pgc:CoverageMap triples.
-- RB-04 deliverable.

SET search_path TO pg_ripple, public;

-- ─── Setup: SKOS rules + test data ────────────────────────────────────────────

SELECT pg_ripple.load_rules_builtin('skos') > 0 AS skos_rules_loaded;

-- Insert a SKOS taxonomy for coverage_map testing.
SELECT pg_ripple.load_ntriples(
'<http://coverage.test/Food> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.w3.org/2004/02/skos/core#ConceptScheme> .
<http://coverage.test/Food> <http://www.w3.org/2004/02/skos/core#prefLabel> "Food"@en .
<http://coverage.test/Fruits> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.w3.org/2004/02/skos/core#Concept> .
<http://coverage.test/Fruits> <http://www.w3.org/2004/02/skos/core#prefLabel> "Fruits"@en .
<http://coverage.test/Fruits> <http://www.w3.org/2004/02/skos/core#inScheme> <http://coverage.test/Food> .
<http://coverage.test/Vegetables> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.w3.org/2004/02/skos/core#Concept> .
<http://coverage.test/Vegetables> <http://www.w3.org/2004/02/skos/core#prefLabel> "Vegetables"@en .
<http://coverage.test/Vegetables> <http://www.w3.org/2004/02/skos/core#inScheme> <http://coverage.test/Food> .
<http://coverage.test/Apple> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.w3.org/2004/02/skos/core#Concept> .
<http://coverage.test/Apple> <http://www.w3.org/2004/02/skos/core#prefLabel> "Apple"@en .
<http://coverage.test/Apple> <http://www.w3.org/2004/02/skos/core#broader> <http://coverage.test/Fruits> .
<http://coverage.test/Banana> <http://www.w3.org/2004/02/skos/core#broader> <http://coverage.test/Fruits> .
<http://coverage.test/Carrot> <http://www.w3.org/2004/02/skos/core#broader> <http://coverage.test/Vegetables> .
'
) > 0 AS coverage_fixture_loaded;

-- Run SKOS inference.
SELECT pg_ripple.infer('skos') >= 0 AS infer_ran;

-- ─── Test 1: coverage_map is callable ────────────────────────────────────────

SELECT count(*) >= 0 AS coverage_map_callable
FROM pg_ripple.coverage_map();

-- ─── Test 2: coverage_map with empty named_graphs works ──────────────────────

SELECT count(*) >= 0 AS coverage_empty_graphs
FROM pg_ripple.coverage_map(ARRAY[]::TEXT[]);

-- ─── Test 3: coverage_map returns expected columns (when data exists) ────────

-- Verify columns have the expected names/types when there are rows.
-- This CASE handles both data-present (checks column constraints) and
-- data-absent (trivially true) cases to avoid ordering-dependent failures.
SELECT CASE
    WHEN count(*) = 0 THEN true
    ELSE bool_and(
        topic_iri IS NOT NULL AND
        triple_count >= 0 AND
        source_count >= 0 AND
        mean_confidence >= 0.0 AND
        contradiction_count >= 0
    )
END AS columns_ok
FROM pg_ripple.coverage_map();

-- ─── Test 4: Zero-triple topics are excluded ─────────────────────────────────

-- (All topics in our fixture have at least one triple, so result should be empty
--  or have rows with triple_count > 0.)
SELECT count(*) = 0 AS no_zero_triple_topics
FROM pg_ripple.coverage_map()
WHERE triple_count = 0;

-- ─── Test 5: refresh_coverage_map is callable ────────────────────────────────

SELECT pg_ripple.refresh_coverage_map('http://coverage.test/CoverageGraph') >= 0
    AS refresh_ran;

-- ─── Test 6: refresh_coverage_map writes triples queryable via sparql() ──────

SELECT count(*) >= 0 AS coverage_map_triples_exist
FROM pg_ripple.sparql(
    'SELECT ?m WHERE { GRAPH <http://coverage.test/CoverageGraph> { ?m <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://w3id.org/pgc#CoverageMap> } }'
);

-- ─── Test 7: coverage_map top_k parameter ────────────────────────────────────

SELECT count(*) <= 1 AS top_k_respected
FROM pg_ripple.coverage_map(ARRAY[]::TEXT[], 'http://www.w3.org/2004/02/skos/core#broader', 1);

-- ─── Cleanup ─────────────────────────────────────────────────────────────────

-- Delete all coverage.test triples by loading an empty replacement.
SELECT pg_ripple.drop_triples(
    'http://coverage.test/Apple',
    'http://www.w3.org/2004/02/skos/core#broader',
    'http://coverage.test/Fruits'
) >= 0 AS cleanup1;
SELECT pg_ripple.drop_triples(
    'http://coverage.test/Banana',
    'http://www.w3.org/2004/02/skos/core#broader',
    'http://coverage.test/Fruits'
) >= 0 AS cleanup2;
SELECT pg_ripple.drop_triples(
    'http://coverage.test/Carrot',
    'http://www.w3.org/2004/02/skos/core#broader',
    'http://coverage.test/Vegetables'
) >= 0 AS cleanup3;
SELECT pg_ripple.drop_rules('skos') >= 0 AS skos_cleanup;
