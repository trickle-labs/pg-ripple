-- v0.103.0 Feature Regression Tests
-- Tests for: Conflict detection — static analysis mode
--
-- Covers:
--   CONFLICT-S0: GUC defaults for rule_conflict_check_on_load and block_on_conflict
--   CONFLICT-S1: same-head opposing-value conflict detected in static mode
--   CONFLICT-S2: rule-vs-SHACL conflict detected in static mode
--   CONFLICT-S3: clean rule set returns [] in static mode

SET client_min_messages = warning;
CREATE EXTENSION IF NOT EXISTS pg_ripple;
SET client_min_messages = DEFAULT;
SET search_path TO pg_ripple, public;

-- Load library so _PG_init registers GUCs (required when shared_preload_libraries is not set).
LOAD '$libdir/pg_ripple';

-- ─── CONFLICT-S0: GUC defaults ───────────────────────────────────────────────

SELECT current_setting('pg_ripple.rule_conflict_check_on_load', true) = 'off'
    AS rule_conflict_check_on_load_default;

SELECT current_setting('pg_ripple.block_on_conflict', true) = 'off'
    AS block_on_conflict_default;

-- ─── Setup: clean up any leftover rule sets ───────────────────────────────────

SELECT pg_ripple.drop_rules('conflict_test_s') IS NOT DISTINCT FROM NULL
    AS conflict_s_rules_dropped;

-- ─── CONFLICT-S1: same-head opposing-value conflict ──────────────────────────
--
-- Load two rules that both derive the same predicate but with different
-- constant object values: one derives eligible = "true", the other eligible = "false".

SELECT pg_ripple.load_rules(
    '?x <http://ex.org/eligible> "true" :- ?x <http://ex.org/adult> "yes" .
     ?x <http://ex.org/eligible> "false" :- ?x <http://ex.org/minor> "yes" .',
    'conflict_test_s'
) = 2 AS conflict_s_two_rules_loaded;

-- rule_conflicts should detect the opposing-value conflict.
SELECT jsonb_array_length(
    pg_ripple.rule_conflicts('conflict_test_s', 'static')
) >= 1 AS conflict_s_opposing_values_found;

-- The conflict type should be same_head_opposing_values.
SELECT (
    SELECT count(*) FROM jsonb_array_elements(
        pg_ripple.rule_conflicts('conflict_test_s', 'static')
    ) AS c
    WHERE c->>'conflict_type' = 'same_head_opposing_values'
) >= 1 AS conflict_s_correct_type;

-- ─── CONFLICT-S2: rule-vs-SHACL conflict ─────────────────────────────────────
--
-- Load a SHACL shape with sh:not on the same predicate that the rule derives.

SELECT pg_ripple.drop_rules('conflict_test_shacl') IS NOT DISTINCT FROM NULL
    AS conflict_shacl_rules_dropped;

SELECT pg_ripple.load_rules(
    '?x <http://ex.org/approved> "true" :- ?x <http://ex.org/status> "ok" .',
    'conflict_test_shacl'
) = 1 AS conflict_shacl_one_rule_loaded;

-- Load a SHACL shape that has sh:not on the approved predicate.
SELECT pg_ripple.load_shacl($SHACL$
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix ex: <http://ex.org/> .

ex:ApprovalShape
    a sh:NodeShape ;
    sh:targetClass ex:Person ;
    sh:property [
        sh:path ex:approved ;
        sh:not [
            sh:datatype <http://www.w3.org/2001/XMLSchema#string>
        ]
    ] .
$SHACL$) >= 0 AS shacl_shape_loaded;

-- rule_conflicts in static mode should detect the rule-vs-shacl conflict.
SELECT jsonb_array_length(
    pg_ripple.rule_conflicts('conflict_test_shacl', 'static')
) >= 1 AS conflict_shacl_found;

-- ─── CONFLICT-S3: clean rule set returns [] ───────────────────────────────────

SELECT pg_ripple.drop_rules('conflict_test_clean') IS NOT DISTINCT FROM NULL
    AS conflict_clean_rules_dropped;

-- Load rules that don't conflict — different head predicates, no SHACL violations.
SELECT pg_ripple.load_rules(
    '?x <http://ex.org/knows> ?y :- ?y <http://ex.org/knows> ?x .
     ?x <http://ex.org/ancestor> ?z :- ?x <http://ex.org/parent> ?z .',
    'conflict_test_clean'
) = 2 AS conflict_clean_two_rules_loaded;

SELECT jsonb_array_length(
    pg_ripple.rule_conflicts('conflict_test_clean', 'static')
) = 0 AS conflict_clean_empty_array;

-- ─── Cleanup ─────────────────────────────────────────────────────────────────

SELECT pg_ripple.drop_rules('conflict_test_s') IS NOT DISTINCT FROM NULL
    AS conflict_s_cleanup;
SELECT pg_ripple.drop_rules('conflict_test_shacl') IS NOT DISTINCT FROM NULL
    AS conflict_shacl_cleanup;
SELECT pg_ripple.drop_rules('conflict_test_clean') IS NOT DISTINCT FROM NULL
    AS conflict_clean_cleanup;
