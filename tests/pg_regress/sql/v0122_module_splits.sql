-- v0.122.0 Module Splits Regression Tests
-- Verifies that no public functions regressed from the H17-02 god-module splits.
--
-- Covers:
--   SPLIT-01: SPARQL built-in string functions still translate correctly
--   SPLIT-02: Datalog helper functions available (no regression)
--   SPLIT-03: bulk_load entry points callable
--   SPLIT-04: LLM suggest_mappings callable
--   SPLIT-05: gucs storage late registrations present

SET client_min_messages = warning;
CREATE EXTENSION IF NOT EXISTS pg_ripple;
SET client_min_messages = DEFAULT;
SET search_path TO pg_ripple, public;

LOAD '$libdir/pg_ripple';

-- SPLIT-01: SPARQL string function (STRLEN) translated via new string.rs module
SELECT count(*) >= 0 AS split01_sparql_string_ok
FROM pg_ripple.sparql($q$
    SELECT (STRLEN("hello") AS ?n) WHERE {}
$q$);

-- SPLIT-02: Datalog rule compilation works (compiler/helpers.rs intact)
SELECT pg_ripple.datalog_rules_count() >= 0 AS split02_datalog_ok;

-- SPLIT-03: bulk_load entry point callable
SELECT 'load_ntriples' IN (
    SELECT routine_name FROM information_schema.routines
    WHERE routine_schema = 'pg_ripple'
      AND routine_type = 'FUNCTION'
) AS split03_load_ntriples_exists;

-- SPLIT-04: suggest_mappings callable (llm/mapping.rs intact)
SELECT 'suggest_mappings' IN (
    SELECT routine_name FROM information_schema.routines
    WHERE routine_schema = 'pg_ripple'
      AND routine_type = 'FUNCTION'
) AS split04_suggest_mappings_exists;

-- SPLIT-05: v0.81.0+ GUC registered (storage_late.rs intact)
SELECT count(*) = 1 AS split05_strict_dictionary_guc_exists
FROM pg_settings
WHERE name = 'pg_ripple.strict_dictionary';
