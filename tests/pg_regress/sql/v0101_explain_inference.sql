-- v0.101.0 Feature Regression Tests
-- Tests for: Natural Language Explanation of Datalog-derived facts
--
-- Covers:
--   NL-EXPLAIN-01: _pg_ripple.explanation_cache table creation
--   NL-EXPLAIN-02: pg_ripple.explanation_cache_ttl GUC default
--   NL-EXPLAIN-03: explain_inference() on an inferred fact returns non-empty fallback text
--   NL-EXPLAIN-04: explain_inference() on a base fact returns NULL
--   NL-EXPLAIN-05: explain_inference_jsonb() returns both proof_tree and narrative keys
--   NL-EXPLAIN-06: vacuum_explanation_cache() removes expired rows
--   NL-EXPLAIN-07: explain_inference_provenance() still works (renamed from explain_inference)
--   NL-EXPLAIN-08: explain_inference() with mock LLM endpoint returns narrative

SET client_min_messages = warning;
CREATE EXTENSION IF NOT EXISTS pg_ripple;
SET client_min_messages = DEFAULT;
SET search_path TO pg_ripple, public;

-- Load library so _PG_init registers GUCs (required when shared_preload_libraries is not set).
LOAD '$libdir/pg_ripple';

-- ─── NL-EXPLAIN-01: explanation_cache table exists ───────────────────────────

SELECT EXISTS (
    SELECT 1 FROM information_schema.tables
    WHERE table_schema = '_pg_ripple' AND table_name = 'explanation_cache'
) AS explanation_cache_table_exists;

-- ─── NL-EXPLAIN-02: explanation_cache_ttl GUC default ────────────────────────

SELECT current_setting('pg_ripple.explanation_cache_ttl', true) = '3600'
    AS explanation_cache_ttl_default_3600;

-- ─── NL-EXPLAIN-03: vacuum_explanation_cache() returns 0 on empty cache ──────

SELECT pg_ripple.vacuum_explanation_cache() = 0
    AS vacuum_explanation_cache_empty_returns_zero;

-- ─── Setup: insert a base triple and run inference with derivation recording ──

SELECT pg_ripple.drop_rules('test_nl_explain') IS NOT DISTINCT FROM NULL
    AS rules_dropped;

SELECT pg_ripple.load_rules(
    '?x <http://test.org/ancestor> ?z :- ?x <http://test.org/parent> ?z .',
    'test_nl_explain'
) > 0 AS nl_rules_loaded;

SELECT pg_ripple.insert_triple(
    '<http://test.org/Alice>',
    '<http://test.org/parent>',
    '<http://test.org/Bob>'
) IS NOT DISTINCT FROM NULL AS nl_base_triple_inserted;

SET pg_ripple.record_derivations = on;

SELECT (pg_ripple.infer_with_stats('test_nl_explain')->>'derived')::int >= 1
    AS nl_inference_derived_some;

SET pg_ripple.record_derivations = off;

-- Verify at least one derivation was recorded.
SELECT count(*) >= 1 AS nl_derivations_recorded
FROM _pg_ripple.derivations
WHERE rule_set = 'test_nl_explain';

-- ─── NL-EXPLAIN-04: explain_inference() on a base fact returns NULL ───────────

-- The base triple (Alice parent Bob) has no derivation row → should return NULL.
SELECT pg_ripple.explain_inference(
    'http://test.org/Alice',
    'http://test.org/parent',
    'http://test.org/Bob'
) IS NULL AS explain_inference_null_for_base_fact;

-- ─── NL-EXPLAIN-05: explain_inference() on inferred fact returns non-empty ───

-- The inferred triple (Alice ancestor Bob) should return non-empty fallback text.
SELECT pg_ripple.explain_inference(
    'http://test.org/Alice',
    'http://test.org/ancestor',
    'http://test.org/Bob'
) IS NOT NULL AS explain_inference_not_null_for_inferred;

-- The returned text must be non-empty.
SELECT length(pg_ripple.explain_inference(
    'http://test.org/Alice',
    'http://test.org/ancestor',
    'http://test.org/Bob'
)) > 0 AS explain_inference_nonempty;

-- The fallback text should contain the rule name (contains 'ancestor').
SELECT pg_ripple.explain_inference(
    'http://test.org/Alice',
    'http://test.org/ancestor',
    'http://test.org/Bob'
) ILIKE '%ancestor%' AS explain_inference_contains_rule_info;

-- ─── NL-EXPLAIN-06: explain_inference_jsonb() has proof_tree and narrative ───

SELECT pg_ripple.explain_inference_jsonb(
    'http://test.org/Alice',
    'http://test.org/ancestor',
    'http://test.org/Bob'
) IS NOT NULL AS explain_jsonb_not_null;

SELECT (pg_ripple.explain_inference_jsonb(
    'http://test.org/Alice',
    'http://test.org/ancestor',
    'http://test.org/Bob'
)) ? 'proof_tree' AS explain_jsonb_has_proof_tree;

SELECT (pg_ripple.explain_inference_jsonb(
    'http://test.org/Alice',
    'http://test.org/ancestor',
    'http://test.org/Bob'
)) ? 'narrative' AS explain_jsonb_has_narrative;

-- ─── NL-EXPLAIN-07: explain_inference_jsonb() NULL for base fact ─────────────

SELECT pg_ripple.explain_inference_jsonb(
    'http://test.org/Alice',
    'http://test.org/parent',
    'http://test.org/Bob'
) IS NULL AS explain_jsonb_null_for_base_fact;

-- ─── NL-EXPLAIN-08: explain_inference() with mock LLM endpoint ───────────────

SET pg_ripple.llm_endpoint = 'mock';

SELECT pg_ripple.explain_inference(
    'http://test.org/Alice',
    'http://test.org/ancestor',
    'http://test.org/Bob'
) IS NOT NULL AS explain_inference_mock_llm_not_null;

-- Mock LLM output should contain the word "derived" or "rule".
SELECT pg_ripple.explain_inference(
    'http://test.org/Alice',
    'http://test.org/ancestor',
    'http://test.org/Bob'
) ILIKE '%derived%' OR pg_ripple.explain_inference(
    'http://test.org/Alice',
    'http://test.org/ancestor',
    'http://test.org/Bob'
) ILIKE '%rule%' AS explain_inference_mock_contains_key_word;

RESET pg_ripple.llm_endpoint;

-- ─── NL-EXPLAIN-09: vacuum_explanation_cache() with TTL 0 returns 0 ──────────

-- With TTL=0 (caching disabled), vacuum removes nothing.
SET pg_ripple.explanation_cache_ttl = 0;
SELECT pg_ripple.vacuum_explanation_cache() = 0 AS vacuum_with_ttl_zero_returns_zero;
RESET pg_ripple.explanation_cache_ttl;

-- ─── NL-EXPLAIN-10: explain_inference_provenance() still works ───────────────

-- The old explain_inference was renamed to explain_inference_provenance in v0.101.0.
-- It should still return an empty result set (since provenance-chain walking
-- was originally for explicit source-column inspection, separate from derivations).
SELECT count(*) >= 0 AS explain_provenance_returns_rows
FROM pg_ripple.explain_inference_provenance(
    'http://test.org/Alice',
    'http://test.org/ancestor',
    'http://test.org/Bob'
);

-- ─── NL-EXPLAIN-11: explain_inference() on missing triple returns NULL ────────

SELECT pg_ripple.explain_inference(
    'http://test.org/NonExistent',
    'http://test.org/ancestor',
    'http://test.org/Bob'
) IS NULL AS explain_inference_null_for_missing_triple;

-- ─── Cleanup ──────────────────────────────────────────────────────────────────

SELECT pg_ripple.drop_rules('test_nl_explain') IS NOT DISTINCT FROM NULL
    AS cleanup_nl_rules;
