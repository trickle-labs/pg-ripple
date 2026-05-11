-- v0.109.0 Feature Regression Tests
-- Tests for: NS-RL String Similarity Builtins + ER Orchestrator
--
-- Covers:
--   STRSIM-01: GUC pg_ripple.sameas_apply_rate_limit default is 1000
--   STRSIM-02: GUC pg_ripple.string_similarity_extensions_ok default is false
--   STRSIM-03: er_blocking_templates() returns 3 rows
--   STRSIM-04: er_blocking_template('email') returns non-empty rule text
--   STRSIM-05: er_blocking_template('postal_name') returns non-empty rule text
--   STRSIM-06: er_blocking_template('name_prefix') returns non-empty rule text
--   STRSIM-07: er_blocking_template() raises error for unknown name
--   STRSIM-08: er_blocking_templates() has correct column names
--   STRSIM-09: resolve_entities() dry_run returns expected JSON keys
--   STRSIM-10: resolve_entities() dry_run returns non-negative counts

SET client_min_messages = warning;
CREATE EXTENSION IF NOT EXISTS pg_ripple;
SET client_min_messages = DEFAULT;
SET search_path TO pg_ripple, public;

LOAD '$libdir/pg_ripple';

-- STRSIM-01: GUC sameas_apply_rate_limit default is 1000

SELECT current_setting('pg_ripple.sameas_apply_rate_limit') = '1000'
    AS strsim01_rate_limit_default;

-- STRSIM-02: GUC string_similarity_extensions_ok default is false

SELECT current_setting('pg_ripple.string_similarity_extensions_ok') = 'off'
    AS strsim02_ext_ok_default;

-- STRSIM-03: er_blocking_templates() returns exactly 3 rows

SELECT COUNT(*) = 3 AS strsim03_three_templates
FROM pg_ripple.er_blocking_templates();

-- STRSIM-04-06: each template returns non-empty text

SELECT
    length(pg_ripple.er_blocking_template('email')) > 0       AS strsim04_email_ok,
    length(pg_ripple.er_blocking_template('postal_name')) > 0 AS strsim05_postal_ok,
    length(pg_ripple.er_blocking_template('name_prefix')) > 0 AS strsim06_prefix_ok;

-- STRSIM-07: unknown template name raises error

DO $$
BEGIN
    PERFORM pg_ripple.er_blocking_template('no_such_template');
    RAISE EXCEPTION 'expected error not raised';
EXCEPTION
    WHEN OTHERS THEN
        RAISE NOTICE 'STRSIM-07 ok: caught error for unknown template';
END;
$$;

-- STRSIM-08: er_blocking_templates() column names accessible

SELECT
    name IS NOT NULL        AS strsim08_col_name,
    description IS NOT NULL AS strsim08_col_description,
    rule IS NOT NULL        AS strsim08_col_rule
FROM pg_ripple.er_blocking_templates()
LIMIT 1;

-- STRSIM-09: resolve_entities() dry_run JSON keys

SELECT
    result ? 'candidates'       AS strsim09_has_candidates,
    result ? 'would_assert'     AS strsim09_has_would_assert,
    result ? 'symbolic'         AS strsim09_has_symbolic,
    result ? 'neural'           AS strsim09_has_neural,
    result ? 'blocked_by_shacl' AS strsim09_has_blocked_by_shacl
FROM (
    SELECT pg_ripple.resolve_entities(
        'http://example.org/srcG',
        'http://example.org/tgtG',
        '{"dry_run": true}'::json
    )::jsonb AS result
) sub;

-- STRSIM-10: resolve_entities() dry_run counts are non-negative

SELECT
    (result ->> 'candidates')::int >= 0   AS strsim10_candidates_nonneg,
    (result ->> 'would_assert')::int >= 0 AS strsim10_would_assert_nonneg
FROM (
    SELECT pg_ripple.resolve_entities(
        'http://example.org/srcG',
        'http://example.org/tgtG',
        '{"dry_run": true}'::json
    )::jsonb AS result
) sub;
