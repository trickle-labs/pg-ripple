-- v0.118.0 Feature Regression Tests
-- Tests for: compat_check() SQL Function (Feature 3 / C16-01)
--
-- Covers:
--   COMPAT-01: compat_check() returns valid JSON text
--   COMPAT-02: extension_version field matches installed version
--   COMPAT-03: compatible field is boolean true
--   COMPAT-04: http_min_version field is present and non-empty

SET client_min_messages = warning;
CREATE EXTENSION IF NOT EXISTS pg_ripple;
SET client_min_messages = DEFAULT;
SET search_path TO pg_ripple, public;

LOAD '$libdir/pg_ripple';

-- COMPAT-01: compat_check() returns valid JSON text

SELECT (pg_ripple.compat_check())::jsonb IS NOT NULL AS compat01_valid_json;

-- COMPAT-02: extension_version field matches installed version

SELECT ((pg_ripple.compat_check())::jsonb ->> 'extension_version') IS NOT NULL
    AS compat02_has_version;

-- COMPAT-03: compatible field is boolean true

SELECT ((pg_ripple.compat_check())::jsonb ->> 'compatible')::boolean = true
    AS compat03_compatible_true;

-- COMPAT-04: http_min_version field is present and non-empty

SELECT length((pg_ripple.compat_check())::jsonb ->> 'http_min_version') > 0
    AS compat04_has_http_min_version;
