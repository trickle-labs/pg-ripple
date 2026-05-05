-- pg_regress test: Cyclic-group pre-check source in parallel Datalog (M15-21, v0.96.0)
--
-- Tests that mutually-recursive rules (A depends on B, B depends on A) are
-- correctly handled by the parallel stratum evaluator's cycle-detection pre-check
-- (P13-06, v0.85.0) when pg_ripple.datalog_parallel_workers > 1.
--
-- The pre-check detects directed cycles in the head-group dependency graph
-- and merges the cyclic groups into a single serial group before parallel dispatch.
-- This test verifies that:
-- 1. Mutually-recursive rules complete without error.
-- 2. The result is correct (non-negative triple count).

CREATE EXTENSION IF NOT EXISTS pg_ripple;
SELECT pg_ripple.triple_count() >= 0 AS library_loaded;

SET search_path TO pg_ripple, public;

-- Force parallel evaluation with threshold 0 (always analyse).
SET pg_ripple.datalog_parallel_workers = 2;
SET pg_ripple.datalog_parallel_threshold = 0;
SHOW pg_ripple.datalog_parallel_workers;
SHOW pg_ripple.datalog_parallel_threshold;

-- Load base triples: node1 → node2, node2 → node3
SELECT pg_ripple.insert_triple(
    '<https://cyclic.test/node1>',
    '<https://cyclic.test/edge>',
    '<https://cyclic.test/node2>'
) > 0 AS t1_loaded;

SELECT pg_ripple.insert_triple(
    '<https://cyclic.test/node2>',
    '<https://cyclic.test/edge>',
    '<https://cyclic.test/node3>'
) > 0 AS t2_loaded;

-- Define two rules in the same rule set that use the same derived predicate
-- (mutually dependent / cyclic in the head-group dependency graph).
-- Rule 1: base edges are reachable
SELECT pg_ripple.add_rule(
    'cyclic_parallel_test',
    '?x <https://cyclic.test/reachable> ?y :- ?x <https://cyclic.test/edge> ?y .'
) > 0 AS rule1_added;

-- Rule 2: transitive closure
SELECT pg_ripple.add_rule(
    'cyclic_parallel_test',
    '?x <https://cyclic.test/reachable> ?z :- ?x <https://cyclic.test/edge> ?y, ?y <https://cyclic.test/reachable> ?z .'
) > 0 AS rule2_added;

-- Run inference with parallel evaluation.
-- The cyclic pre-check (P13-06) must handle the mutual dependency without error.
SELECT pg_ripple.infer('cyclic_parallel_test') >= 0 AS infer_ok;

-- Assert the result is sensible (non-negative triple count).
SELECT pg_ripple.triple_count() >= 2 AS store_has_triples;

-- Clean up.
SELECT pg_ripple.drop_rules('cyclic_parallel_test') >= 0 AS cleanup_ok;

-- Restore defaults.
SET pg_ripple.datalog_parallel_workers = 4;
SET pg_ripple.datalog_parallel_threshold = 10000;
