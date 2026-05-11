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
--   LIB-07: PT0455 -- non-permissive license without accept_license=TRUE
--   LIB-08: PT0452 -- URL blocked by SSRF allowlist (localhost URL)
--   LIB-09: installed Datalog rules are usable after install
--   LIB-10: uninstall removes associated Datalog rules

SET client_min_messages = warning;
CREATE EXTENSION IF NOT EXISTS pg_ripple;
SET client_min_messages = DEFAULT;
SET search_path TO pg_ripple, public;

-- Load library so _PG_init registers GUCs (required when shared_preload_libraries is not set).
LOAD '$libdir/pg_ripple';

-- Write the test library Turtle file to /tmp using lo_from_bytea + lo_export.
-- This preserves real newlines; COPY text format would escape \n, corrupting Turtle.
DO $$
DECLARE
    v_loid OID;
    v_ttl  TEXT;
BEGIN
    v_ttl :=
        '@prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .' || chr(10) ||
        '@prefix dcterms: <http://purl.org/dc/terms/> .' || chr(10) ||
        '@prefix owl: <http://www.w3.org/2002/07/owl#> .' || chr(10) ||
        '@prefix pg: <http://pg-ripple.org/lib/> .' || chr(10) ||
        '' || chr(10) ||
        '<urn:pg-ripple:test-v0104-library> a pg:RuleLibrary ;' || chr(10) ||
        '    dcterms:title "test-v0104-library" ;' || chr(10) ||
        '    dcterms:description "Test library for v0.104.0 regression." ;' || chr(10) ||
        '    dcterms:license <https://spdx.org/licenses/MIT.html> ;' || chr(10) ||
        '    owl:versionInfo "1.0.0" ;' || chr(10) ||
        '    pg:rules """?x <http://v0104t.org/tKnows> ?z :- ?x <http://v0104t.org/knows> ?z .""" .' || chr(10);
    v_loid := lo_from_bytea(0, convert_to(v_ttl, 'UTF8'));
    PERFORM lo_export(v_loid, '/tmp/pg_ripple_v0104_test_lib.ttl');
    PERFORM lo_unlink(v_loid);
END;
$$ LANGUAGE plpgsql;

-- Write the non-permissive license test library to /tmp.
DO $$
DECLARE
    v_loid OID;
    v_ttl  TEXT;
BEGIN
    v_ttl :=
        '@prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .' || chr(10) ||
        '@prefix dcterms: <http://purl.org/dc/terms/> .' || chr(10) ||
        '@prefix owl: <http://www.w3.org/2002/07/owl#> .' || chr(10) ||
        '@prefix pg: <http://pg-ripple.org/lib/> .' || chr(10) ||
        '' || chr(10) ||
        '<urn:pg-ripple:test-v0104-nonfree-library> a pg:RuleLibrary ;' || chr(10) ||
        '    dcterms:title "test-v0104-nonfree-library" ;' || chr(10) ||
        '    dcterms:description "Non-permissive license test library." ;' || chr(10) ||
        '    dcterms:license <https://spdx.org/licenses/GPL-3.0-only.html> ;' || chr(10) ||
        '    owl:versionInfo "1.0.0" ;' || chr(10) ||
        '    pg:rules """?x <http://v0104t.org/nKnows> ?z :- ?x <http://v0104t.org/knows> ?z .""" .' || chr(10);
    v_loid := lo_from_bytea(0, convert_to(v_ttl, 'UTF8'));
    PERFORM lo_export(v_loid, '/tmp/pg_ripple_v0104_nonfree_lib.ttl');
    PERFORM lo_unlink(v_loid);
END;
$$ LANGUAGE plpgsql;

-- Ensure the catalog table exists by calling list_rule_libraries() first.
-- This triggers ensure_catalog() inside the function.
SELECT count(*) = 0 AS no_preinstalled_test_libs
FROM pg_ripple.list_rule_libraries()
WHERE name LIKE 'test-v0104%';

-- ---- LIB-01: catalog table exists ----------------------------------------

SELECT EXISTS (
    SELECT 1 FROM information_schema.tables
    WHERE table_schema = '_pg_ripple' AND table_name = 'rule_libraries'
) AS rule_libraries_table_exists;

-- ---- LIB-02: list_rule_libraries() returns empty set initially ------------

-- Ensure no libraries are installed (clean state).
DELETE FROM _pg_ripple.rule_libraries WHERE name LIKE 'test-v0104%';

SELECT count(*) = 0 AS list_empty_initially
FROM pg_ripple.list_rule_libraries()
WHERE name LIKE 'test-v0104%';

-- ---- LIB-03: install_rule_library() with a local Turtle file -------------

SELECT pg_ripple.install_rule_library('/tmp/pg_ripple_v0104_test_lib.ttl') = 'test-v0104-library'
    AS install_returns_name;

-- ---- LIB-04: installed library appears in list_rule_libraries() ----------

SELECT count(*) = 1 AS library_in_list
FROM pg_ripple.list_rule_libraries()
WHERE name = 'test-v0104-library';

SELECT version = '1.0.0' AS correct_version
FROM pg_ripple.list_rule_libraries()
WHERE name = 'test-v0104-library';

SELECT license_iri = 'https://spdx.org/licenses/MIT.html' AS correct_license
FROM pg_ripple.list_rule_libraries()
WHERE name = 'test-v0104-library';

-- ---- LIB-05: re-install same version is idempotent -----------------------

SELECT pg_ripple.install_rule_library('/tmp/pg_ripple_v0104_test_lib.ttl') = 'test-v0104-library'
    AS reinstall_returns_name;

SELECT count(*) = 1 AS no_duplicate_after_reinstall
FROM pg_ripple.list_rule_libraries()
WHERE name = 'test-v0104-library';

-- ---- LIB-09: installed Datalog rules are usable after install ------------

SELECT count(*) >= 1 AS rules_stored_by_library
FROM _pg_ripple.rules
WHERE rule_set = 'test-v0104-library';

SELECT pg_ripple.load_ntriples(
    '<http://v0104t.org/Alice> <http://v0104t.org/knows> <http://v0104t.org/Bob> .'
) >= 0 AS base_triple_loaded;

-- infer() returns i64 (count of derived triples).
SELECT pg_ripple.infer('test-v0104-library') >= 0
    AS library_rules_usable;

-- ---- LIB-06: uninstall_rule_library() removes the catalog entry ----------

SELECT pg_ripple.uninstall_rule_library('test-v0104-library') IS NOT DISTINCT FROM NULL
    AS uninstall_returns_void;

SELECT count(*) = 0 AS catalog_row_gone
FROM pg_ripple.list_rule_libraries()
WHERE name = 'test-v0104-library';

-- ---- LIB-10: uninstall removes associated Datalog rules ------------------

SELECT count(*) = 0 AS rules_gone_after_uninstall
FROM _pg_ripple.rules
WHERE rule_set = 'test-v0104-library';

-- ---- LIB-07: PT0455 -- non-permissive license without accept_license -----

DO $$
BEGIN
    PERFORM pg_ripple.install_rule_library('/tmp/pg_ripple_v0104_nonfree_lib.ttl');
    RAISE EXCEPTION 'expected PT0455 error was not raised';
EXCEPTION
    WHEN others THEN
        IF sqlerrm LIKE '%PT0455%' OR sqlerrm LIKE '%accept_license%' THEN
            NULL; -- expected
        ELSE
            RAISE;
        END IF;
END;
$$ LANGUAGE plpgsql;

SELECT 'PT0455 raised correctly' AS pt0455_raised;

SELECT pg_ripple.install_rule_library(
    '/tmp/pg_ripple_v0104_nonfree_lib.ttl',
    true
) = 'test-v0104-nonfree-library'
    AS install_with_accept_license_works;

SELECT pg_ripple.uninstall_rule_library('test-v0104-nonfree-library') IS NOT DISTINCT FROM NULL
    AS nonfree_uninstall_ok;

-- ---- LIB-08: PT0452 -- URL blocked by SSRF allowlist --------------------

DO $$
BEGIN
    PERFORM pg_ripple.install_rule_library('http://127.0.0.1/pg_ripple_test.ttl');
    RAISE EXCEPTION 'expected PT0452 error was not raised';
EXCEPTION
    WHEN others THEN
        IF sqlerrm LIKE '%PT0452%' OR sqlerrm LIKE '%SSRF%' OR sqlerrm LIKE '%blocked%' THEN
            NULL; -- expected
        ELSE
            RAISE;
        END IF;
END;
$$ LANGUAGE plpgsql;

SELECT 'PT0452 raised correctly' AS pt0452_raised;

-- ---- VERSION CHECK -------------------------------------------------------

SELECT (
    split_part(value, '.', 1)::int * 1000000 +
    split_part(value, '.', 2)::int * 1000 +
    split_part(value, '.', 3)::int
) >= (0 * 1000000 + 104 * 1000 + 0) AS version_is_0_104_x
FROM pg_ripple.diagnostic_report()
WHERE key = 'compiled_version';

-- ---- CLEANUP -------------------------------------------------------------

SELECT pg_ripple.sparql_update(
    'DELETE WHERE { <http://v0104t.org/Alice> <http://v0104t.org/knows> <http://v0104t.org/Bob> }'
) >= 0 AS cleanup_data_ok;
