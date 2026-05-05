-- sparql_optional_path_in_graph_rare.sql — OPTIONAL + property paths inside GRAPH with vp_rare predicates (M15-08 v0.95.0)
--
-- Explicitly tests the combination of:
--   1. OPTIONAL {} inside GRAPH <g> {}
--   2. Property path expressions (+ / * / | / /)
--   3. Predicates stored in vp_rare (low-frequency, below promotion threshold)
--
-- These combinations exercise the interaction between the rare-predicate view,
-- the graph-filter injection in property path CTEs, and the OPTIONAL LEFT JOIN
-- rewrite — all three of which have historically been sources of subtle bugs.

SET search_path TO pg_ripple, public;

CREATE EXTENSION IF NOT EXISTS pg_ripple;

-- ── Setup: three named graphs with rare predicates ───────────────────────────

-- Use unique enough predicate IRIs to stay in vp_rare (well below promotion threshold).
SELECT pg_ripple.load_ntriples_into_graph($$
<https://rare.path.test/a> <https://rare.path.test/step> <https://rare.path.test/b> .
<https://rare.path.test/b> <https://rare.path.test/step> <https://rare.path.test/c> .
<https://rare.path.test/c> <https://rare.path.test/step> <https://rare.path.test/d> .
<https://rare.path.test/a> <https://rare.path.test/label> "NodeA" .
<https://rare.path.test/b> <https://rare.path.test/label> "NodeB" .
$$, 'https://rare.path.test/g1') = 5 AS g1_loaded;

SELECT pg_ripple.load_ntriples_into_graph($$
<https://rare.path.test/x> <https://rare.path.test/step> <https://rare.path.test/y> .
<https://rare.path.test/y> <https://rare.path.test/step> <https://rare.path.test/z> .
<https://rare.path.test/x> <https://rare.path.test/label> "NodeX" .
$$, 'https://rare.path.test/g2') = 3 AS g2_loaded;

-- ── Test 1: Kleene-plus path with vp_rare predicate inside GRAPH ──────────────

-- From g1: a reaches b, c, d via a+ path starting from a
SELECT count(*) >= 3 AS plus_path_in_g1_works
FROM pg_ripple.sparql($$
    SELECT ?t WHERE {
        GRAPH <https://rare.path.test/g1> {
            <https://rare.path.test/a> <https://rare.path.test/step>+ ?t
        }
    }
$$);

-- From g2: x reaches y, z via a+ path
SELECT count(*) >= 2 AS plus_path_in_g2_works
FROM pg_ripple.sparql($$
    SELECT ?t WHERE {
        GRAPH <https://rare.path.test/g2> {
            <https://rare.path.test/x> <https://rare.path.test/step>+ ?t
        }
    }
$$);

-- ── Test 2: OPTIONAL with vp_rare property path inside GRAPH ─────────────────

-- All nodes in g1 that have a step path, with optional label.
-- a has a label, b has a label, c and d do not.
SELECT count(*) >= 1 AS optional_path_in_g1_rows_found
FROM pg_ripple.sparql($$
    SELECT ?node ?lbl WHERE {
        GRAPH <https://rare.path.test/g1> {
            <https://rare.path.test/a> <https://rare.path.test/step>+ ?node .
            OPTIONAL { ?node <https://rare.path.test/label> ?lbl }
        }
    }
$$);

-- c and d have no label — verify some rows have NULL ?lbl
SELECT bool_or((result->>'lbl') IS NULL) AS some_null_labels
FROM pg_ripple.sparql($$
    SELECT ?node ?lbl WHERE {
        GRAPH <https://rare.path.test/g1> {
            <https://rare.path.test/a> <https://rare.path.test/step>+ ?node .
            OPTIONAL { ?node <https://rare.path.test/label> ?lbl }
        }
    }
$$);

-- b has a label — verify at least one row has a non-NULL ?lbl
SELECT bool_or((result->>'lbl') IS NOT NULL) AS some_nonnull_labels
FROM pg_ripple.sparql($$
    SELECT ?node ?lbl WHERE {
        GRAPH <https://rare.path.test/g1> {
            <https://rare.path.test/a> <https://rare.path.test/step>+ ?node .
            OPTIONAL { ?node <https://rare.path.test/label> ?lbl }
        }
    }
$$);

-- ── Test 3: Property path must not leak across graph boundaries ───────────────

-- In g1: starting from x is impossible (x is only in g2)
SELECT count(*) = 0 AS path_confined_to_g1
FROM pg_ripple.sparql($$
    SELECT ?t WHERE {
        GRAPH <https://rare.path.test/g1> {
            <https://rare.path.test/x> <https://rare.path.test/step>* ?t
        }
    }
$$);

-- In g2: starting from a is impossible (a is only in g1)
SELECT count(*) = 0 AS path_confined_to_g2
FROM pg_ripple.sparql($$
    SELECT ?t WHERE {
        GRAPH <https://rare.path.test/g2> {
            <https://rare.path.test/a> <https://rare.path.test/step>* ?t
        }
    }
$$);

-- ── Test 4: Kleene-star (includes starting node) with vp_rare predicate ──────

-- From a in g1 via step*: should include a itself plus b, c, d → at least 4
SELECT count(*) >= 4 AS star_path_includes_start
FROM pg_ripple.sparql($$
    SELECT ?t WHERE {
        GRAPH <https://rare.path.test/g1> {
            <https://rare.path.test/a> <https://rare.path.test/step>* ?t
        }
    }
$$);

-- ── Test 5: Alternation path | with vp_rare predicates ───────────────────────

-- Using label | step alternation from a: should match b, c, d (via step) and "NodeA" (via label)
SELECT count(*) >= 1 AS alt_path_works
FROM pg_ripple.sparql($$
    SELECT ?o WHERE {
        GRAPH <https://rare.path.test/g1> {
            <https://rare.path.test/a> (<https://rare.path.test/step>|<https://rare.path.test/label>) ?o
        }
    }
$$);

-- ── Cleanup ───────────────────────────────────────────────────────────────────
SELECT pg_ripple.drop_graph('https://rare.path.test/g1') IS NOT NULL AS g1_dropped;
SELECT pg_ripple.drop_graph('https://rare.path.test/g2') IS NOT NULL AS g2_dropped;
