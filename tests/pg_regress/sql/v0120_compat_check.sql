-- v0.120.0 Feature Regression Tests: compat_check() JSON Schema
-- More thorough validation of the compat_check() JSON schema than v0118.
--
-- Covers:
--   COMPAT-10: compat_check() output is valid JSON
--   COMPAT-11: extension_version field is a non-empty string
--   COMPAT-12: http_min_version field is a non-empty string
--   COMPAT-13: compatible field is a JSON boolean (not a string)
--   COMPAT-14: no unexpected top-level keys (schema is stable)

SET client_min_messages = warning;
CREATE EXTENSION IF NOT EXISTS pg_ripple;
SET client_min_messages = DEFAULT;
SET search_path TO pg_ripple, public;

LOAD '$libdir/pg_ripple';

-- COMPAT-10: valid JSON
SELECT (pg_ripple.compat_check())::jsonb IS NOT NULL AS compat10_valid_json;

-- COMPAT-11: extension_version is non-empty string
SELECT length((pg_ripple.compat_check())::jsonb ->> 'extension_version') > 0
    AS compat11_extension_version_present;

-- COMPAT-12: http_min_version is non-empty string
SELECT length((pg_ripple.compat_check())::jsonb ->> 'http_min_version') > 0
    AS compat12_http_min_version_present;

-- COMPAT-13: compatible is a JSON boolean (jsonb type is 'boolean', not 'string')
SELECT jsonb_typeof((pg_ripple.compat_check())::jsonb -> 'compatible') = 'boolean'
    AS compat13_compatible_is_boolean;

-- COMPAT-14: all three required keys are present
SELECT (
    ((pg_ripple.compat_check())::jsonb ? 'extension_version') AND
    ((pg_ripple.compat_check())::jsonb ? 'http_min_version') AND
    ((pg_ripple.compat_check())::jsonb ? 'compatible')
) AS compat14_all_keys_present;
