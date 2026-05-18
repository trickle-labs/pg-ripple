-- v0.118.0 Feature Regression Tests
-- Tests for: Allen's Interval Relations (Feature 4)
--
-- Covers all 7 Allen's temporal interval relations:
--   ALLEN-01: pg:before — interval A entirely before interval B
--   ALLEN-02: pg:meets  — A ends exactly when B begins
--   ALLEN-03: pg:overlaps — A starts before B, they overlap, A ends first
--   ALLEN-04: pg:during — A is entirely contained within B
--   ALLEN-05: pg:finishes — A ends at the same time as B, A starts after
--   ALLEN-06: pg:starts — A starts at the same time as B, A ends first
--   ALLEN-07: pg:equals — A and B are identical intervals

SET client_min_messages = warning;
CREATE EXTENSION IF NOT EXISTS pg_ripple;
SET client_min_messages = DEFAULT;
SET search_path TO pg_ripple, public;

LOAD '$libdir/pg_ripple';

-- ALLEN-01: pg:before — a_end <= b_start

SELECT pg_ripple.allen_before(
    '2024-01-01'::timestamptz,
    '2024-01-05'::timestamptz,
    '2024-01-06'::timestamptz,
    '2024-01-10'::timestamptz
) AS allen01_before_true;

SELECT pg_ripple.allen_before(
    '2024-01-01'::timestamptz,
    '2024-01-07'::timestamptz,
    '2024-01-06'::timestamptz,
    '2024-01-10'::timestamptz
) AS allen01_before_false;

-- ALLEN-02: pg:meets — a_end = b_start

SELECT pg_ripple.allen_meets(
    '2024-01-01'::timestamptz,
    '2024-01-06'::timestamptz,
    '2024-01-06'::timestamptz,
    '2024-01-10'::timestamptz
) AS allen02_meets_true;

SELECT pg_ripple.allen_meets(
    '2024-01-01'::timestamptz,
    '2024-01-05'::timestamptz,
    '2024-01-06'::timestamptz,
    '2024-01-10'::timestamptz
) AS allen02_meets_false;

-- ALLEN-03: pg:overlaps — a_start < b_start AND a_end > b_start AND a_end < b_end

SELECT pg_ripple.allen_overlaps(
    '2024-01-01'::timestamptz,
    '2024-01-07'::timestamptz,
    '2024-01-05'::timestamptz,
    '2024-01-10'::timestamptz
) AS allen03_overlaps_true;

SELECT pg_ripple.allen_overlaps(
    '2024-01-01'::timestamptz,
    '2024-01-04'::timestamptz,
    '2024-01-05'::timestamptz,
    '2024-01-10'::timestamptz
) AS allen03_overlaps_false;

-- ALLEN-04: pg:during — a_start > b_start AND a_end < b_end

SELECT pg_ripple.allen_during(
    '2024-01-03'::timestamptz,
    '2024-01-08'::timestamptz,
    '2024-01-01'::timestamptz,
    '2024-01-10'::timestamptz
) AS allen04_during_true;

SELECT pg_ripple.allen_during(
    '2024-01-01'::timestamptz,
    '2024-01-10'::timestamptz,
    '2024-01-03'::timestamptz,
    '2024-01-08'::timestamptz
) AS allen04_during_false;

-- ALLEN-05: pg:finishes — a_end = b_end AND a_start > b_start

SELECT pg_ripple.allen_finishes(
    '2024-01-05'::timestamptz,
    '2024-01-10'::timestamptz,
    '2024-01-01'::timestamptz,
    '2024-01-10'::timestamptz
) AS allen05_finishes_true;

SELECT pg_ripple.allen_finishes(
    '2024-01-01'::timestamptz,
    '2024-01-10'::timestamptz,
    '2024-01-01'::timestamptz,
    '2024-01-10'::timestamptz
) AS allen05_finishes_false;

-- ALLEN-06: pg:starts — a_start = b_start AND a_end < b_end

SELECT pg_ripple.allen_starts(
    '2024-01-01'::timestamptz,
    '2024-01-05'::timestamptz,
    '2024-01-01'::timestamptz,
    '2024-01-10'::timestamptz
) AS allen06_starts_true;

SELECT pg_ripple.allen_starts(
    '2024-01-01'::timestamptz,
    '2024-01-10'::timestamptz,
    '2024-01-01'::timestamptz,
    '2024-01-10'::timestamptz
) AS allen06_starts_false;

-- ALLEN-07: pg:equals — a_start = b_start AND a_end = b_end

SELECT pg_ripple.allen_equals(
    '2024-01-01'::timestamptz,
    '2024-01-10'::timestamptz,
    '2024-01-01'::timestamptz,
    '2024-01-10'::timestamptz
) AS allen07_equals_true;

SELECT pg_ripple.allen_equals(
    '2024-01-01'::timestamptz,
    '2024-01-09'::timestamptz,
    '2024-01-01'::timestamptz,
    '2024-01-10'::timestamptz
) AS allen07_equals_false;
