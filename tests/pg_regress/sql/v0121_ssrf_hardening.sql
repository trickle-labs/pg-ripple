-- v0.121.0 Security Regression Tests: SSRF guard hardening
-- H17-01 / SEC-H-01: resolve_and_check_endpoint() replaces string-contains guard
-- SEC-M-03: CGNAT (100.64.0.0/10), multicast (224.0.0.0/4), this-network (0.0.0.0/8),
--           and IPv4-mapped IPv6 (::ffff:...) added to SSRF blocklist

SET client_min_messages = warning;
CREATE EXTENSION IF NOT EXISTS pg_ripple;
SET client_min_messages = DEFAULT;
SET search_path TO pg_ripple, public;
LOAD '$libdir/pg_ripple';

-- Cleanup any leftovers from previous runs
DELETE FROM _pg_ripple.rule_library_federation WHERE name LIKE 'test-v0121%';

-- SSRF-01: IPv6-mapped private address (::ffff:192.168.x.x) must be blocked
-- Previously bypassed the string-contains guard; now caught by is_blocked_host()
DO $do$
BEGIN
  PERFORM pg_ripple.subscribe_rule_library('http://[::ffff:192.168.1.1]/stream', 'test-v0121-ssrf-ipv6mapped');
  RAISE EXCEPTION 'expected SSRF error not raised';
EXCEPTION WHEN others THEN
  IF sqlerrm LIKE '%PT046%' OR sqlerrm LIKE '%SSRF%' OR sqlerrm LIKE '%blocked%' OR sqlerrm LIKE '%PT606%' THEN NULL;
  ELSE RAISE;
  END IF;
END;
$do$ LANGUAGE plpgsql;
SELECT 'SSRF-01 IPv6-mapped blocked' AS ssrf_01_ipv6_mapped_ok;

-- SSRF-02: CGNAT address (100.64.x.x, RFC 6598) must be blocked
-- SEC-M-03: was missing from the SSRF blocklist prior to v0.121.0
DO $do$
BEGIN
  PERFORM pg_ripple.subscribe_rule_library('http://100.64.1.1/stream', 'test-v0121-ssrf-cgnat');
  RAISE EXCEPTION 'expected SSRF error not raised';
EXCEPTION WHEN others THEN
  IF sqlerrm LIKE '%PT046%' OR sqlerrm LIKE '%SSRF%' OR sqlerrm LIKE '%blocked%' OR sqlerrm LIKE '%PT606%' THEN NULL;
  ELSE RAISE;
  END IF;
END;
$do$ LANGUAGE plpgsql;
SELECT 'SSRF-02 CGNAT blocked' AS ssrf_02_cgnat_ok;

-- SSRF-03: IPv4 multicast (224.0.0.1) must be blocked
-- SEC-M-03: was missing from the SSRF blocklist prior to v0.121.0
DO $do$
BEGIN
  PERFORM pg_ripple.subscribe_rule_library('http://224.0.0.1/stream', 'test-v0121-ssrf-multicast');
  RAISE EXCEPTION 'expected SSRF error not raised';
EXCEPTION WHEN others THEN
  IF sqlerrm LIKE '%PT046%' OR sqlerrm LIKE '%SSRF%' OR sqlerrm LIKE '%blocked%' OR sqlerrm LIKE '%PT606%' THEN NULL;
  ELSE RAISE;
  END IF;
END;
$do$ LANGUAGE plpgsql;
SELECT 'SSRF-03 multicast blocked' AS ssrf_03_multicast_ok;

-- SSRF-04: this-network (0.0.0.0/8) must be blocked
-- SEC-M-03: was missing from the SSRF blocklist prior to v0.121.0
DO $do$
BEGIN
  PERFORM pg_ripple.subscribe_rule_library('http://0.0.0.0/stream', 'test-v0121-ssrf-thisnet');
  RAISE EXCEPTION 'expected SSRF error not raised';
EXCEPTION WHEN others THEN
  IF sqlerrm LIKE '%PT046%' OR sqlerrm LIKE '%SSRF%' OR sqlerrm LIKE '%blocked%' OR sqlerrm LIKE '%PT606%' THEN NULL;
  ELSE RAISE;
  END IF;
END;
$do$ LANGUAGE plpgsql;
SELECT 'SSRF-04 this-network blocked' AS ssrf_04_thisnet_ok;

-- SSRF-05: loopback still blocked (regression guard)
DO $do$
BEGIN
  PERFORM pg_ripple.subscribe_rule_library('http://127.0.0.1/stream', 'test-v0121-ssrf-loop');
  RAISE EXCEPTION 'expected SSRF error not raised';
EXCEPTION WHEN others THEN
  IF sqlerrm LIKE '%PT046%' OR sqlerrm LIKE '%SSRF%' OR sqlerrm LIKE '%blocked%' OR sqlerrm LIKE '%PT606%' THEN NULL;
  ELSE RAISE;
  END IF;
END;
$do$ LANGUAGE plpgsql;
SELECT 'SSRF-05 loopback still blocked' AS ssrf_05_loopback_ok;

-- SSRF-06: legitimate public HTTPS URI passes validation without error
--  (catalog write may fail with PT0467 if network is unavailable, but no SSRF error)
DO $do$
BEGIN
  PERFORM pg_ripple.subscribe_rule_library('https://rules.example.com/stream', 'test-v0121-legit');
  -- Success: no exception raised
EXCEPTION WHEN others THEN
  -- PT0467 catalog errors are acceptable; SSRF errors are not
  IF sqlerrm LIKE '%PT046%' AND sqlerrm NOT LIKE '%PT0464%' AND sqlerrm NOT LIKE '%PT0465%' AND sqlerrm NOT LIKE '%PT0466%' AND sqlerrm NOT LIKE '%PT0467%' THEN
    RAISE EXCEPTION 'unexpected SSRF block for legitimate URI: %', sqlerrm;
  END IF;
  -- Any other error (PT0467, DNS failure surfaced as PT606) is acceptable
END;
$do$ LANGUAGE plpgsql;
SELECT 'SSRF-06 public URI passes SSRF check' AS ssrf_06_public_uri_ok;

-- SSRF-07: version check
SELECT (split_part(value,'.',1)::int*1000000+split_part(value,'.',2)::int*1000+split_part(value,'.',3)::int) >= 121000 AS version_ok
FROM pg_ripple.diagnostic_report() WHERE key = 'compiled_version';

-- Cleanup
DELETE FROM _pg_ripple.rule_library_federation WHERE name LIKE 'test-v0121%';
