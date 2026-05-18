-- v0.118.0 Feature Regression Tests
-- Tests for: AT TIME ZONE Gap Fix for Temporal Queries
--
-- Covers:
--   TZ-01: mark_temporal() with time_zone parameter runs without error
--   TZ-02: point_in_time() with time_zone parameter runs without error
--   TZ-03: default_tz is NULL when mark_temporal() called without time_zone
--   TZ-04: mark_temporal() with time_zone stores the value in temporal_predicates

SET client_min_messages = warning;
CREATE EXTENSION IF NOT EXISTS pg_ripple;
SET client_min_messages = DEFAULT;
SET search_path TO pg_ripple, public;

LOAD '$libdir/pg_ripple';

-- ─── Setup ───────────────────────────────────────────────────────────────────

-- Register a temporal predicate without time zone (for TZ-03)
SELECT pg_ripple.mark_temporal(
    'http://tz.test/plain',
    'snapshot'
);

-- ─── TZ-01: mark_temporal() with time_zone parameter runs without error ──────

SELECT pg_ripple.mark_temporal(
    'http://tz.test/aware',
    'snapshot',
    'UTC'
);

-- ─── TZ-02: point_in_time() with time_zone parameter runs without error ──────

SELECT pg_ripple.point_in_time(
    '2024-06-01 12:00:00+00'::timestamptz,
    'UTC'
);

SELECT pg_ripple.clear_point_in_time();

-- ─── TZ-03: default_tz is NULL when mark_temporal() called without time zone ─

SELECT EXISTS(
    SELECT 1 FROM _pg_ripple.temporal_predicates tp
    JOIN _pg_ripple.dictionary d ON d.id = tp.predicate_id
    WHERE d.value = 'http://tz.test/plain'
      AND tp.default_tz IS NULL
) AS tz03_no_tz_is_null;

-- ─── TZ-04: mark_temporal() with time_zone stores the value ──────────────────

SELECT EXISTS(
    SELECT 1 FROM _pg_ripple.temporal_predicates tp
    JOIN _pg_ripple.dictionary d ON d.id = tp.predicate_id
    WHERE d.value = 'http://tz.test/aware'
      AND tp.default_tz = 'UTC'
) AS tz04_tz_stored;

-- ─── Cleanup ─────────────────────────────────────────────────────────────────

SELECT pg_ripple.unmark_temporal('http://tz.test/plain');
SELECT pg_ripple.unmark_temporal('http://tz.test/aware');
