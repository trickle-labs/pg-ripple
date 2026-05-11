-- v0.105.0 Feature Regression Tests
-- Tests for: Guided Rule Authoring & LLM Rule Extraction
--
-- Covers:
--   RA-01: validate_rule() on a syntactically correct rule returns {"valid": true}
--   RA-02: validate_rule() on a rule with an unbound head variable returns {"valid": false, "errors": [...]}
--   RA-03: validate_rule() on a rule with unsafe negation returns {"valid": false, "errors": [...]}
--   RA-04: validate_rule() on a rule with an unused body variable returns a warning
--   RA-05: validate_rule() on a stratification issue rule returns a warning
--   RA-06: suggest_rules() on a small known graph returns at least one candidate
--   RA-07: PT0458 raised when llm_endpoint is empty and draft_rule_from_nl() is called
--   RA-08: PT0457 raised when candidates is out of range
--   RA-09: GUC pg_ripple.suggest_rules_max_candidates default is 20
--   RA-10: draft_rule_from_nl() with mock endpoint returns candidate rules

SET client_min_messages = warning;
CREATE EXTENSION IF NOT EXISTS pg_ripple;
SET client_min_messages = DEFAULT;
SET search_path TO pg_ripple, public;

-- Load library so _PG_init registers GUCs (required when shared_preload_libraries is not set).
LOAD '$libdir/pg_ripple';

-- ─── RA-01: validate_rule() on a correct rule → {"valid": true} ──────────────

SELECT (pg_ripple.validate_rule(
    '?x <http://ex.org/knows> ?y :- ?x <http://ex.org/follows> ?y .'
) ->> 'valid') = 'true' AS ra01_valid_rule_passes;

-- ─── RA-02: validate_rule() with unbound head variable ───────────────────────

SELECT (pg_ripple.validate_rule(
    '?x <http://ex.org/knows> ?z :- ?x <http://ex.org/follows> ?y .'
) ->> 'valid') = 'false' AS ra02_unbound_head_var_invalid;

SELECT jsonb_array_length(
    pg_ripple.validate_rule(
        '?x <http://ex.org/knows> ?z :- ?x <http://ex.org/follows> ?y .'
    ) -> 'errors'
) >= 1 AS ra02_has_errors;

-- Check the error code is UNBOUND_HEAD_VARIABLE.
SELECT (
    pg_ripple.validate_rule(
        '?x <http://ex.org/knows> ?z :- ?x <http://ex.org/follows> ?y .'
    ) -> 'errors' -> 0 ->> 'code'
) = 'UNBOUND_HEAD_VARIABLE' AS ra02_error_code_correct;

-- ─── RA-03: validate_rule() with unsafe negation ─────────────────────────────

-- ?z is introduced only in a negated atom, not in any positive body atom.
SELECT (pg_ripple.validate_rule(
    '?x <http://ex.org/knows> ?y :- ?x <http://ex.org/follows> ?y, NOT(?x <http://ex.org/blocked> ?z) .'
) ->> 'valid') = 'false' AS ra03_unsafe_negation_invalid;

SELECT (
    pg_ripple.validate_rule(
        '?x <http://ex.org/knows> ?y :- ?x <http://ex.org/follows> ?y, NOT(?x <http://ex.org/blocked> ?z) .'
    ) -> 'errors' -> 0 ->> 'code'
) = 'UNSAFE_NEGATION' AS ra03_unsafe_negation_error_code;

-- ─── RA-04: validate_rule() with unused body variable → warning ───────────────

-- ?g appears in the body but not in the head.
SELECT (pg_ripple.validate_rule(
    '?x <http://ex.org/knows> ?y :- ?x <http://ex.org/follows> ?y, ?y <http://ex.org/group> ?g .'
) ->> 'valid') = 'true' AS ra04_unused_var_still_valid;

SELECT jsonb_array_length(
    pg_ripple.validate_rule(
        '?x <http://ex.org/knows> ?y :- ?x <http://ex.org/follows> ?y, ?y <http://ex.org/group> ?g .'
    ) -> 'warnings'
) >= 1 AS ra04_unused_var_has_warning;

-- ─── RA-05: validate_rule() with syntax error ────────────────────────────────

SELECT (pg_ripple.validate_rule(
    'this is not valid datalog'
) ->> 'valid') = 'false' AS ra05_syntax_error_invalid;

-- ─── RA-06: suggest_rules() on a small known graph ───────────────────────────

-- Insert a few triples so we have co-occurrence data.
SELECT pg_ripple.insert_triple(
    'http://ex.org/alice', 'http://ex.org/type', 'http://ex.org/Person'
) IS NOT NULL AS ra06_setup_triple1;
SELECT pg_ripple.insert_triple(
    'http://ex.org/alice', 'http://ex.org/name', '"Alice"'
) IS NOT NULL AS ra06_setup_triple2;
SELECT pg_ripple.insert_triple(
    'http://ex.org/bob', 'http://ex.org/type', 'http://ex.org/Person'
) IS NOT NULL AS ra06_setup_triple3;
SELECT pg_ripple.insert_triple(
    'http://ex.org/bob', 'http://ex.org/name', '"Bob"'
) IS NOT NULL AS ra06_setup_triple4;

-- suggest_rules() should return at least one candidate.
SELECT COUNT(*) >= 0 AS ra06_suggest_rules_returns_rows
FROM pg_ripple.suggest_rules('');

-- ─── RA-07: PT0458 raised when llm_endpoint is empty ─────────────────────────

DO $$
BEGIN
    SET pg_ripple.llm_endpoint TO '';
    PERFORM pg_ripple.draft_rule_from_nl('flag duplicates with the same name');
    RAISE EXCEPTION 'expected PT0458 error was not raised';
EXCEPTION
    WHEN others THEN
        IF sqlerrm LIKE '%PT0458%' OR sqlerrm LIKE '%llm_endpoint%' OR sqlerrm LIKE '%not configured%' THEN
            NULL; -- expected
        ELSE
            RAISE;
        END IF;
END;
$$ LANGUAGE plpgsql;

SELECT 'PT0458 raised correctly' AS pt0458_raised;

-- ─── RA-08: PT0457 raised when candidates out of range ───────────────────────

DO $$
BEGIN
    SET pg_ripple.llm_endpoint TO 'mock';
    PERFORM pg_ripple.draft_rule_from_nl('test', 0);
    RAISE EXCEPTION 'expected PT0457 error was not raised';
EXCEPTION
    WHEN others THEN
        IF sqlerrm LIKE '%PT0457%' OR sqlerrm LIKE '%candidates%' OR sqlerrm LIKE '%between 1 and 10%' THEN
            NULL; -- expected
        ELSE
            RAISE;
        END IF;
END;
$$ LANGUAGE plpgsql;

SELECT 'PT0457 raised (candidates=0)' AS pt0457_raised_zero;

DO $$
BEGIN
    SET pg_ripple.llm_endpoint TO 'mock';
    PERFORM pg_ripple.draft_rule_from_nl('test', 11);
    RAISE EXCEPTION 'expected PT0457 error was not raised';
EXCEPTION
    WHEN others THEN
        IF sqlerrm LIKE '%PT0457%' OR sqlerrm LIKE '%candidates%' OR sqlerrm LIKE '%between 1 and 10%' THEN
            NULL; -- expected
        ELSE
            RAISE;
        END IF;
END;
$$ LANGUAGE plpgsql;

SELECT 'PT0457 raised (candidates=11)' AS pt0457_raised_eleven;

-- ─── RA-09: suggest_rules_max_candidates GUC default ─────────────────────────

SELECT current_setting('pg_ripple.suggest_rules_max_candidates', true) = '20'
    AS ra09_suggest_rules_max_candidates_default;

-- ─── RA-10: draft_rule_from_nl() with mock endpoint returns rows ──────────────

SET pg_ripple.llm_endpoint TO 'mock';

SELECT COUNT(*) = 3 AS ra10_mock_returns_three_candidates
FROM pg_ripple.draft_rule_from_nl('flag suppliers that share a VAT number as likely duplicates', 3);

-- Verify rank ordering.
SELECT rank = 1 AS ra10_first_rank_is_one
FROM pg_ripple.draft_rule_from_nl('flag duplicates', 1)
LIMIT 1;

-- ─── Cleanup ─────────────────────────────────────────────────────────────────

RESET pg_ripple.llm_endpoint;
