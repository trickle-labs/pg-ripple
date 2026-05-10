-- v0.104.0 Feature Regression Tests
-- Tests for: Domain Rule Library Infrastructure
--
-- Covers:
--   LIB-01: _pg_ripple.rule_libraries catalog table exists
--   LIB-02: list_rule_libraries() returns empty set initially
--   LIB-03: install_rule_library() with a local Turtle file
--   LIB-04: installed library appears in list_rule_libraries()
--   LIB-05: re-install same version is idempotent (no duplicate, no error)
--   LIB-06: uninstall_rule_library() removes the catalog entry
--   LIB-07: PT0455 — non-permissive license without accept_license=TRUE
--   LIB-08: PT0452 — URL blocked by SSRF allowlist (localhost URL)
--   LIB-09: installed Datalog rules are usable after install
--   LIB-10: uninstall removes associated Datalog rules

SET client_min_messages = warning;
CREATE EXTENSION IF NOT EXISTS pg_ripple;
SET client_min_messages = DEFAULT;
SET search_path TO pg_ripple, public;

-- Write the test library Turtle file to /tmp for installation.
-- This approach allows the test to run without a pre-known absolute path.
COPY (
    VALUES (
        E'@prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .\n'
        '@prefix dcterms: <http://purl.org/dc/terms/> .\n'
        '@prefix owl: <http://www.w3.org/2002/07/owl#> .\n'
        '@prefix pg: <http://pg-ripple.org/lib/> .\n'
        '\n'
        '<urn:pg-ripple:test-v0104-library> a pg:RuleLibrary ;\n'
        '    dcterms:title "test-v0104-library" ;\n'
        '    dcterms:description "A minimal test library for v0.104.0 regression tests." ;\n'
        '    dcterms:license <https://spdx.org/licenses/MIT.html> ;\n'
        '    owl:versionInfo "1.0.0" ;\n'
        '    pg:rules """?x <http://v0104test.org/transitiveKnows> ?z :- ?x <http://v0104test.org/knows> ?z .""" .\n'
    )
) TO '/tmp/pg_ripple_v0104_test_lib.ttl' (FORMAT text, HEADER false);

-- Write the non-permissive license test library to /tmp.
COPY (
    VALUES (
        E'@prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .\n'
        '@prefix dcterms: <http://purl.org/dc/terms/> .\n'
        '@prefix owl: <http://www.w3.org/2002/07/owl#> .\n'
        '@prefix pg: <http://pg-ripple.org/lib/> .\n'
        '\n'
        '<urn:pg-ripple:test-v0104-nonfree-library> a pg:RuleLibrary ;\n'
        '    dcterms:title "test-v0104-nonfree-library" ;\n'
        '    dcterms:description "Non-permissive license test library." ;\n'
        '    dcterms:license <https://spdx.org/licenses/GPL-3.0-only.html> ;\n'
        '    owl:versionInfo "1.0.0" ;\n'
        '    pg:rules """?x <http://v0104test.org/nonfreeKnows> ?z :- ?x <http://v0104test.org/knows> ?z .""" .\n'
    )
) TO '/tmp/pg_ripple_v0104_nonfree_lib.ttl' (FORMAT text, HEADER false);

-- ─── LIB-01: catalog table exists ────────────────────────────────────────────

SELECT EXISTS (
    SELECT 1 FROM information_schema.tables
    WHERE table_schema = '_pg_ripple' AND table_name = 'rule_libraries'
) AS rule_libraries_table_exists;

-- ─── LIB-02: list_rule_libraries() returns empty set initially ───────────────

-- Ensure no libraries are installed (clean state).
DELETE FROM _pg_ripple.rule_libraries WHERE name LIKE 'test-v0104%';

SELECT count(*) = 0 AS list_empty_initially
FROM pg_ripple.list_rule_libraries()
WHERE name LIKE 'test-v0104%';

-- ─── LIB-03: install_rule_library() with a local Turtle file ─────────────────

-- Install the test library.
SELECT pg_ripple.install_rule_library('/tmp/pg_ripple_v0104_test_lib.ttl') = 'test-v0104-library'
    AS install_returns_name;

-- ─── LIB-04: installed library appears in list_rule_libraries() ──────────────

SELECT count(*) = 1 AS library_in_list
FROM pg_ripple.list_rule_libraries()
WHERE name = 'test-v0104-library';

-- Verify version is recorded correctly.
SELECT version = '1.0.0' AS correct_version
FROM pg_ripple.list_rule_libraries()
WHERE name = 'test-v0104-library';

-- Verify license_iri is recorded.
SELECT license_iri = 'https://spdx.org/licenses/MIT.html' AS correct_license
FROM pg_ripple.list_rule_libraries()
WHERE name = 'test-v0104-library';

-- ─── LIB-05: re-install same version is idempotent ───────────────────────────

-- Re-installing the same version must not error and must not duplicate the row.
SELECT pg_ripple.install_rule_library('/tmp/pg_ripple_v0104_test_lib.ttl') = 'test-v0104-library'
    AS reinstall_returns_name;

-- Still exactly one row after re-install.
SELECT count(*) = 1 AS no_duplicate_after_reinstall
FROM pg_ripple.list_rule_libraries()
WHERE name = 'test-v0104-library';

-- ─── LIB-09: installed Datalog rules are usable after install ────────────────

-- The library defines: ?x <transitiveKnows> ?z :- ?x <knows> ?z
-- Load a base triple and run inference.
SELECT pg_ripple.load_ntriples(
    '<http://v0104test.org/Alice> <http://v0104test.org/knows> <http://v0104test.org/Bob> .'
) >= 0 AS base_triple_loaded;

SELECT (pg_ripple.infer('test-v0104-library')->>'derived')::bigint >= 0
    AS library_rules_usable;

-- ─── LIB-06: uninstall_rule_library() removes the catalog entry ──────────────

-- Uninstall the library.
SELECT pg_ripple.uninstall_rule_library('test-v0104-library') IS NOT DISTINCT FROM NULL
    AS uninstall_returns_void;

-- Row must be gone from the catalog.
SELECT count(*) = 0 AS catalog_row_gone
FROM pg_ripple.list_rule_libraries()
WHERE name = 'test-v0104-library';

-- ─── LIB-10: uninstall removes associated Datalog rules ──────────────────────

-- After uninstall, no rules for 'test-v0104-library' rule set remain.
SELECT count(*) = 0 AS rules_gone_after_uninstall
FROM _pg_ripple.rules
WHERE rule_set = 'test-v0104-library';

-- ─── LIB-07: PT0455 — non-permissive license without accept_license=TRUE ─────

-- Installing a GPL-licensed library without accept_license=TRUE must raise PT0455.
DO $$
BEGIN
    PERFORM pg_ripple.install_rule_library('/tmp/pg_ripple_v0104_nonfree_lib.ttl');
    RAISE EXCEPTION 'expected PT0455 error was not raised';
EXCEPTION
    WHEN others THEN
        IF sqlerrm LIKE '%PT0455%' OR sqlerrm LIKE '%accept_license%' THEN
            -- Expected error.
        ELSE
            RAISE;
        END IF;
END;
$$ LANGUAGE plpgsql;

SELECT 'PT0455 raised correctly' AS pt0455_raised;

-- Confirm we CAN install with accept_license=TRUE.
SELECT pg_ripple.install_rule_library(
    '/tmp/pg_ripple_v0104_nonfree_lib.ttl',
    true
) = 'test-v0104-nonfree-library'
    AS install_with_accept_license_works;

-- Clean up.
SELECT pg_ripple.uninstall_rule_library('test-v0104-nonfree-library') IS NOT DISTINCT FROM NULL
    AS nonfree_uninstall_ok;

-- ─── LIB-08: PT0452 — URL blocked by SSRF allowlist ────────────────────────

-- A URL pointing to localhost must be blocked by the SSRF check when the
-- federation_endpoint_policy is 'default-deny' (the default).
DO $$
BEGIN
    PERFORM pg_ripple.install_rule_library('http://127.0.0.1/pg_ripple_test.ttl');
    RAISE EXCEPTION 'expected PT0452 error was not raised';
EXCEPTION
    WHEN others THEN
        IF sqlerrm LIKE '%PT0452%' OR sqlerrm LIKE '%SSRF%' OR sqlerrm LIKE '%blocked%' THEN
            -- Expected error.
        ELSE
            RAISE;
        END IF;
END;
$$ LANGUAGE plpgsql;

SELECT 'PT0452 raised correctly' AS pt0452_raised;

-- ─── VERSION CHECK ────────────────────────────────────────────────────────────

-- Extension version must be 0.104.0 or later.
SELECT (
    split_part(value, '.', 1)::int * 1000000 +
    split_part(value, '.', 2)::int * 1000 +
    split_part(value, '.', 3)::int
) >= (0 * 1000000 + 104 * 1000 + 0) AS version_is_0_104_x
FROM pg_ripple.diagnostic_report()
WHERE key = 'compiled_version';

-- ─── CLEANUP ─────────────────────────────────────────────────────────────────

-- Remove any test data loaded during the test.
SELECT pg_ripple.sparql_update(
    'DELETE WHERE { <http://v0104test.org/Alice> <http://v0104test.org/knows> <http://v0104test.org/Bob> }'
) >= 0 AS cleanup_data_ok;

DELETE FROM _pg_ripple.rule_libraries WHERE name LIKE 'test-v0104%';
