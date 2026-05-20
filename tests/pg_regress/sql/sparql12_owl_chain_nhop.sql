-- pg_regress test: OWL 2 RL owl:propertyChainAxiom n-hop chains via SPARQL
-- property path engine (v0.124.0, FEAT-01).
-- Tests n=4 and n=5 hop chains exercising the new path algebra execution.

CREATE EXTENSION IF NOT EXISTS pg_ripple;
SELECT pg_ripple.triple_count() >= 0 AS extension_loaded;
SET search_path TO pg_ripple, public;

SELECT pg_ripple.load_rules_builtin('owl-rl') >= 0 AS owl_rl_loaded;

-- ────────────────────────────────────────────────────────────────────────────
-- Test 1: 4-hop owl:propertyChainAxiom (p1/p2/p3/p4)
-- Chain: n0 -step-> n1 -step-> n2 -step-> n3 -step-> n4
-- owl:propertyChainAxiom: derived4 := step/step/step/step
-- ────────────────────────────────────────────────────────────────────────────
SELECT pg_ripple.load_ntriples(
    '<https://nhop.test/n0> <https://nhop.test/step> <https://nhop.test/n1> .' || E'\n' ||
    '<https://nhop.test/n1> <https://nhop.test/step> <https://nhop.test/n2> .' || E'\n' ||
    '<https://nhop.test/n2> <https://nhop.test/step> <https://nhop.test/n3> .' || E'\n' ||
    '<https://nhop.test/n3> <https://nhop.test/step> <https://nhop.test/n4> .' || E'\n' ||
    '<https://nhop.test/derived4> <http://www.w3.org/2002/07/owl#propertyChainAxiom> <https://nhop.test/c4a> .' || E'\n' ||
    '<https://nhop.test/c4a> <http://www.w3.org/1999/02/22-rdf-syntax-ns#first> <https://nhop.test/step> .' || E'\n' ||
    '<https://nhop.test/c4a> <http://www.w3.org/1999/02/22-rdf-syntax-ns#rest>  <https://nhop.test/c4b> .' || E'\n' ||
    '<https://nhop.test/c4b> <http://www.w3.org/1999/02/22-rdf-syntax-ns#first> <https://nhop.test/step> .' || E'\n' ||
    '<https://nhop.test/c4b> <http://www.w3.org/1999/02/22-rdf-syntax-ns#rest>  <https://nhop.test/c4c> .' || E'\n' ||
    '<https://nhop.test/c4c> <http://www.w3.org/1999/02/22-rdf-syntax-ns#first> <https://nhop.test/step> .' || E'\n' ||
    '<https://nhop.test/c4c> <http://www.w3.org/1999/02/22-rdf-syntax-ns#rest>  <https://nhop.test/c4d> .' || E'\n' ||
    '<https://nhop.test/c4d> <http://www.w3.org/1999/02/22-rdf-syntax-ns#first> <https://nhop.test/step> .' || E'\n' ||
    '<https://nhop.test/c4d> <http://www.w3.org/1999/02/22-rdf-syntax-ns#rest>  <http://www.w3.org/1999/02/22-rdf-syntax-ns#nil> .'
) = 13 AS nhop4_triples_loaded;

SELECT pg_ripple.infer('owl-rl') >= 0 AS nhop4_inferred;

-- After inference: n0 derived4 n4 should be asserted
SELECT pg_ripple.sparql_ask($$
    ASK { <https://nhop.test/n0> <https://nhop.test/derived4> <https://nhop.test/n4> }
$$) AS t01_four_hop_chain_inferred;

-- ────────────────────────────────────────────────────────────────────────────
-- Test 2: 4-hop SPARQL property path cross-validates OWL inference result
-- The SPARQL path step/step/step/step from n0 should also reach n4
-- ────────────────────────────────────────────────────────────────────────────
SELECT pg_ripple.sparql_ask($$
    ASK {
        <https://nhop.test/n0>
            <https://nhop.test/step>/<https://nhop.test/step>/
            <https://nhop.test/step>/<https://nhop.test/step>
            <https://nhop.test/n4> .
    }
$$) AS t02_four_hop_path_query;

-- ────────────────────────────────────────────────────────────────────────────
-- Test 3: 5-hop owl:propertyChainAxiom (p1/p2/p3/p4/p5)
-- Chain: p0 -step-> p1 -step-> p2 -step-> p3 -step-> p4 -step-> p5
-- owl:propertyChainAxiom: derived5 := step/step/step/step/step
-- ────────────────────────────────────────────────────────────────────────────
SELECT pg_ripple.load_ntriples(
    '<https://nhop.test/p0> <https://nhop.test/step> <https://nhop.test/p1> .' || E'\n' ||
    '<https://nhop.test/p1> <https://nhop.test/step> <https://nhop.test/p2> .' || E'\n' ||
    '<https://nhop.test/p2> <https://nhop.test/step> <https://nhop.test/p3> .' || E'\n' ||
    '<https://nhop.test/p3> <https://nhop.test/step> <https://nhop.test/p4> .' || E'\n' ||
    '<https://nhop.test/p4> <https://nhop.test/step> <https://nhop.test/p5> .' || E'\n' ||
    '<https://nhop.test/derived5> <http://www.w3.org/2002/07/owl#propertyChainAxiom> <https://nhop.test/c5a> .' || E'\n' ||
    '<https://nhop.test/c5a> <http://www.w3.org/1999/02/22-rdf-syntax-ns#first> <https://nhop.test/step> .' || E'\n' ||
    '<https://nhop.test/c5a> <http://www.w3.org/1999/02/22-rdf-syntax-ns#rest>  <https://nhop.test/c5b> .' || E'\n' ||
    '<https://nhop.test/c5b> <http://www.w3.org/1999/02/22-rdf-syntax-ns#first> <https://nhop.test/step> .' || E'\n' ||
    '<https://nhop.test/c5b> <http://www.w3.org/1999/02/22-rdf-syntax-ns#rest>  <https://nhop.test/c5c> .' || E'\n' ||
    '<https://nhop.test/c5c> <http://www.w3.org/1999/02/22-rdf-syntax-ns#first> <https://nhop.test/step> .' || E'\n' ||
    '<https://nhop.test/c5c> <http://www.w3.org/1999/02/22-rdf-syntax-ns#rest>  <https://nhop.test/c5d> .' || E'\n' ||
    '<https://nhop.test/c5d> <http://www.w3.org/1999/02/22-rdf-syntax-ns#first> <https://nhop.test/step> .' || E'\n' ||
    '<https://nhop.test/c5d> <http://www.w3.org/1999/02/22-rdf-syntax-ns#rest>  <https://nhop.test/c5e> .' || E'\n' ||
    '<https://nhop.test/c5e> <http://www.w3.org/1999/02/22-rdf-syntax-ns#first> <https://nhop.test/step> .' || E'\n' ||
    '<https://nhop.test/c5e> <http://www.w3.org/1999/02/22-rdf-syntax-ns#rest>  <http://www.w3.org/1999/02/22-rdf-syntax-ns#nil> .'
) = 16 AS nhop5_triples_loaded;

SELECT pg_ripple.infer('owl-rl') >= 0 AS nhop5_inferred;

-- After inference: p0 derived5 p5 should be asserted
SELECT pg_ripple.sparql_ask($$
    ASK { <https://nhop.test/p0> <https://nhop.test/derived5> <https://nhop.test/p5> }
$$) AS t03_five_hop_chain_inferred;

-- ────────────────────────────────────────────────────────────────────────────
-- Test 4: 5-hop SPARQL property path cross-validates OWL inference
-- The SPARQL path step/step/step/step/step from p0 should reach p5
-- ────────────────────────────────────────────────────────────────────────────
SELECT pg_ripple.sparql_ask($$
    ASK {
        <https://nhop.test/p0>
            <https://nhop.test/step>/<https://nhop.test/step>/
            <https://nhop.test/step>/<https://nhop.test/step>/
            <https://nhop.test/step>
            <https://nhop.test/p5> .
    }
$$) AS t04_five_hop_path_query;

-- ────────────────────────────────────────────────────────────────────────────
-- Test 5: OWL inference + SPARQL path yield the same result set
-- For the 4-hop chain, OWL-derived derived4 and property path step/step/step/step
-- should agree: count of (n0, derived4, ?) == count of step/step/step/step from n0
-- Both should return exactly 1 result (n0 → n4)
-- ────────────────────────────────────────────────────────────────────────────
SELECT
    (SELECT COUNT(*) FROM pg_ripple.sparql($$
        SELECT ?o WHERE { <https://nhop.test/n0> <https://nhop.test/derived4> ?o }
    $$)) =
    (SELECT COUNT(*) FROM pg_ripple.sparql($$
        SELECT ?o WHERE {
            <https://nhop.test/n0>
                <https://nhop.test/step>/<https://nhop.test/step>/
                <https://nhop.test/step>/<https://nhop.test/step> ?o .
        }
    $$))
    AS t05_owl_matches_sparql_path;
