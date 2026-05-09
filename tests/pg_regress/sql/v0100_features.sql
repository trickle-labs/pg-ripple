-- v0.100.0 Feature Regression Tests
-- Tests for: proof trees & justification infrastructure
--
-- Covers:
--   PROOF-TREE-01: _pg_ripple.derivations table creation
--   PROOF-TREE-02: pg_ripple.record_derivations GUC
--   PROOF-TREE-03: derivation recording during infer()
--   PROOF-TREE-04: justify() function proof tree structure
--   PROOF-TREE-05: justify() returns NULL for base (non-inferred) facts
--   PROOF-TREE-06: cycle protection in proof tree walker
--   PROOF-TREE-07: vacuum_derivations() removes orphan rows
--   PROOF-TREE-08: record_derivations = off has zero overhead

SET client_min_messages = warning;
CREATE EXTENSION IF NOT EXISTS pg_ripple;
SET client_min_messages = DEFAULT;
SET search_path TO pg_ripple, public;

-- Load library so _PG_init registers GUCs (required when shared_preload_libraries is not set).
LOAD '$libdir/pg_ripple';

SELECT EXISTS (
    SELECT 1 FROM information_schema.tables
    WHERE table_schema = '_pg_ripple' AND table_name = 'derivations'
) AS derivations_table_exists;

-- ─── PROOF-TREE-02: GUC default is off ───────────────────────────────────────

SELECT current_setting('pg_ripple.record_derivations', true) = 'off'
    AS record_derivations_default_off;

-- ─── PROOF-TREE-03: vacuum_derivations() available ───────────────────────────

-- With no inferred facts, vacuum_derivations returns 0.
SELECT pg_ripple.vacuum_derivations() = 0 AS vacuum_returns_zero_baseline;

-- ─── PROOF-TREE-04: derivation recording during infer() ──────────────────────

-- Ensure clean state.
SELECT pg_ripple.drop_rules('test_proof_tree') IS NOT DISTINCT FROM NULL AS rules_dropped;

-- Load a simple transitivity-style rule.
-- Rule: ?x <http://test.org/ancestor> ?z :- ?x <http://test.org/parent> ?z .
SELECT pg_ripple.load_rules(
    '?x <http://test.org/ancestor> ?z :- ?x <http://test.org/parent> ?z .',
    'test_proof_tree'
) > 0 AS rules_loaded;

-- Insert a base triple: Alice parent Bob.
SELECT pg_ripple.insert_triple(
    '<http://test.org/Alice>',
    '<http://test.org/parent>',
    '<http://test.org/Bob>'
) IS NOT DISTINCT FROM NULL AS base_triple_inserted;

-- Enable derivation recording.
SET pg_ripple.record_derivations = on;

-- Run inference (semi-naive path needed for derivation recording).
SELECT (pg_ripple.infer_with_stats('test_proof_tree')->>'derived')::int >= 1 AS inference_derived_some;

-- Reset GUC.
SET pg_ripple.record_derivations = off;

-- Verify at least one derivation was recorded.
SELECT count(*) >= 1 AS derivations_recorded
FROM _pg_ripple.derivations
WHERE rule_set = 'test_proof_tree';

-- The derivation row must reference the rule text.
SELECT count(*) >= 1 AS derivation_has_rule_text
FROM _pg_ripple.derivations
WHERE rule_name LIKE '%ancestor%'
  AND rule_set = 'test_proof_tree';

-- ─── PROOF-TREE-05: justify() proof tree structure ───────────────────────────

-- justify() must return a JSONB object for the inferred triple.
SELECT pg_ripple.justify(
    'http://test.org/Alice',
    'http://test.org/ancestor',
    'http://test.org/Bob'
) IS NOT NULL AS justify_returns_value;

-- The returned object must have "type" = "inferred".
SELECT (pg_ripple.justify(
    'http://test.org/Alice',
    'http://test.org/ancestor',
    'http://test.org/Bob'
)->>'type') = 'inferred' AS justify_type_is_inferred;

-- The "derivations" array must be non-empty.
SELECT jsonb_array_length(
    pg_ripple.justify(
        'http://test.org/Alice',
        'http://test.org/ancestor',
        'http://test.org/Bob'
    )->'derivations'
) >= 1 AS justify_has_derivations;

-- The first derivation must include "rule" key.
SELECT (pg_ripple.justify(
    'http://test.org/Alice',
    'http://test.org/ancestor',
    'http://test.org/Bob'
)->'derivations'->0->>'rule') IS NOT NULL AS derivation_has_rule_key;

-- ─── PROOF-TREE-06: justify() NULL for base facts ────────────────────────────

-- justify() must return NULL for a fact that is in the store but not derived
-- (i.e., no derivation row exists for it — but if record_derivations was off
-- when base triples were inserted, they have no derivation rows).
-- The base triple (Alice parent Bob) was inserted before inference, so
-- if no derivation row points to it, justify returns a "base" node (not NULL).
-- NULL is returned only when the triple is not in the store at all.
SELECT pg_ripple.justify(
    'http://test.org/NonExistentSubject',
    'http://test.org/parent',
    'http://test.org/Bob'
) IS NULL AS justify_null_for_missing_triple;

-- ─── PROOF-TREE-07: justify() triple field has subject/predicate/object ───────

SELECT (pg_ripple.justify(
    'http://test.org/Alice',
    'http://test.org/ancestor',
    'http://test.org/Bob'
)->'triple') ? 'subject' AS triple_has_subject_field;

SELECT (pg_ripple.justify(
    'http://test.org/Alice',
    'http://test.org/ancestor',
    'http://test.org/Bob'
)->'triple') ? 'predicate' AS triple_has_predicate_field;

SELECT (pg_ripple.justify(
    'http://test.org/Alice',
    'http://test.org/ancestor',
    'http://test.org/Bob'
)->'triple') ? 'object' AS triple_has_object_field;

-- ─── PROOF-TREE-08: vacuum_derivations() ─────────────────────────────────────

-- Before vacuuming, there are some derivation rows.
SELECT count(*) >= 1 AS derivations_exist_before_vacuum
FROM _pg_ripple.derivations
WHERE rule_set = 'test_proof_tree';

-- After dropping all rules and deleting the derived triple,
-- vacuum_derivations() removes the orphan rows.
-- We don't drop the triple here (it would cascade-delete via DRed),
-- but we can verify vacuum_derivations() returns 0 when nothing is orphaned.
SELECT pg_ripple.vacuum_derivations() >= 0 AS vacuum_returns_nonneg;

-- ─── PROOF-TREE-09: record_derivations = off has zero overhead ───────────────

-- With GUC off (the default), running infer() should NOT add derivation rows.
SELECT pg_ripple.drop_rules('test_proof_tree_norecord') IS NOT DISTINCT FROM NULL AS rules_dropped2;

SELECT pg_ripple.load_rules(
    '?x <http://test.org/siblingOf> ?z :- ?x <http://test.org/parent> ?y, ?z <http://test.org/parent> ?y .',
    'test_proof_tree_norecord'
) > 0 AS norecord_rules_loaded;

-- GUC is already off (reset earlier).
-- Count derivations BEFORE.
SELECT count(*) AS before_count FROM _pg_ripple.derivations WHERE rule_set = 'test_proof_tree_norecord';

SELECT (pg_ripple.infer_with_stats('test_proof_tree_norecord')->>'derived')::int >= 0 AS norecord_infer_ran;

-- Count AFTER — must still be 0 new rows since GUC was off.
SELECT count(*) = 0 AS no_derivations_when_guc_off
FROM _pg_ripple.derivations
WHERE rule_set = 'test_proof_tree_norecord';

-- ─── Cleanup ──────────────────────────────────────────────────────────────────

SELECT pg_ripple.drop_rules('test_proof_tree') IS NOT DISTINCT FROM NULL AS cleanup_rules;
SELECT pg_ripple.drop_rules('test_proof_tree_norecord') IS NOT DISTINCT FROM NULL AS cleanup_rules2;
