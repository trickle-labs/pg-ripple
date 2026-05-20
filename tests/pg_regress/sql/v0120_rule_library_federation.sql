-- v0.120.0 Feature Regression Tests: Rule-Library Federation

SET client_min_messages = warning;
CREATE EXTENSION IF NOT EXISTS pg_ripple;
SET client_min_messages = DEFAULT;
SET search_path TO pg_ripple, public;
LOAD '$libdir/pg_ripple';

-- FED-01: table exists
SELECT EXISTS (
    SELECT 1 FROM information_schema.tables
    WHERE table_schema = '_pg_ripple'
      AND table_name = 'rule_library_federation'
) AS rule_library_federation_table_exists;

-- Setup
DELETE FROM _pg_ripple.rule_library_federation WHERE name LIKE 'test-v0120%';
DELETE FROM _pg_ripple.rule_libraries WHERE name LIKE 'test-v0120%';
INSERT INTO _pg_ripple.rule_libraries (name, version, license_iri)
VALUES ('test-v0120-fed-lib', '1.0.0', 'https://spdx.org/licenses/MIT.html')
ON CONFLICT (name) DO NOTHING;
SELECT count(*) = 1 AS library_inserted
FROM _pg_ripple.rule_libraries WHERE name = 'test-v0120-fed-lib';

-- FED-02: publish_rule_library (returns void)
SELECT pg_ripple.publish_rule_library('test-v0120-fed-lib', 'https://hub.example.com');
SELECT published = TRUE AND endpoint_uri = 'https://hub.example.com' AS publish_recorded
FROM _pg_ripple.rule_library_federation WHERE name = 'test-v0120-fed-lib';

-- FED-03: subscribe_rule_library — PT0466 SSRF/DNS blocked
DO $do$
BEGIN
  PERFORM pg_ripple.subscribe_rule_library('https://hub.example.com/stream', 'test-v0120-sub-lib');
  RAISE EXCEPTION 'expected error not raised';
EXCEPTION WHEN others THEN
  IF sqlerrm LIKE '%PT0466%' OR sqlerrm LIKE '%DNS%' OR sqlerrm LIKE '%SSRF%' OR sqlerrm LIKE '%blocked%' THEN NULL;
  ELSE RAISE;
  END IF;
END;
$do$ LANGUAGE plpgsql;
SELECT 'PT0466 ok' AS fed03_raised;
SELECT subscribed = TRUE AS subscribe_recorded
FROM _pg_ripple.rule_library_federation WHERE name = 'test-v0120-sub-lib';

-- FED-04: PT0462 library not installed
DO $do$
BEGIN
  PERFORM pg_ripple.publish_rule_library('no-such-lib', 'https://x.example.com');
  RAISE EXCEPTION 'expected error not raised';
EXCEPTION WHEN others THEN
  IF sqlerrm LIKE '%PT0462%' OR sqlerrm LIKE '%not installed%' THEN NULL;
  ELSE RAISE;
  END IF;
END;
$do$ LANGUAGE plpgsql;
SELECT 'PT0462 ok' AS pt0462_raised;

-- FED-05: PT0464 empty source_uri
DO $do$
BEGIN
  PERFORM pg_ripple.subscribe_rule_library('', 'test-v0120-x');
  RAISE EXCEPTION 'expected error not raised';
EXCEPTION WHEN others THEN
  IF sqlerrm LIKE '%PT0464%' OR sqlerrm LIKE '%empty%' THEN NULL;
  ELSE RAISE;
  END IF;
END;
$do$ LANGUAGE plpgsql;
SELECT 'PT0464 ok' AS pt0464_raised;

-- FED-06: PT0466 SSRF blocked
DO $do$
BEGIN
  PERFORM pg_ripple.subscribe_rule_library('http://127.0.0.1/stream', 'test-v0120-ssrf');
  RAISE EXCEPTION 'expected error not raised';
EXCEPTION WHEN others THEN
  IF sqlerrm LIKE '%PT046%' OR sqlerrm LIKE '%SSRF%' OR sqlerrm LIKE '%blocked%' THEN NULL;
  ELSE RAISE;
  END IF;
END;
$do$ LANGUAGE plpgsql;
SELECT 'PT0466 ok' AS pt0466_raised;

-- FED-07: version >= 0.120.0
SELECT (split_part(value,'.',1)::int*1000000+split_part(value,'.',2)::int*1000+split_part(value,'.',3)::int) >= 120000 AS version_ok
FROM pg_ripple.diagnostic_report() WHERE key = 'compiled_version';

-- Cleanup
DELETE FROM _pg_ripple.rule_library_federation WHERE name LIKE 'test-v0120%';
DELETE FROM _pg_ripple.rule_libraries WHERE name LIKE 'test-v0120%';
