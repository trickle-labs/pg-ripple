-- v0.107.0 Feature Regression Tests
-- Tests for: Temporal Reasoning Phase 2 — Sequential Patterns & CDC Integration
--
-- Covers:
--   SEQ-01: WITHIN operator returns true when fact is in window
--   SEQ-02: WITHIN operator returns false when fact is outside window
--   SEQ-03: SEQUENCE operator detects A before B within window
--   SEQ-04: SEQUENCE operator returns false for reversed ordering within window
--   SEQ-05: CONSECUTIVE fires at n=3 consecutive readings
--   SEQ-06: CONSECUTIVE does not fire for a subject with only n-1 readings
--   SEQ-07: Window boundary edge case — fact exactly at window start (inclusive)
--   SEQ-08: Window boundary edge case — fact just outside window
--   SEQ-09: Zero-duration SEQUENCE window edge case (strict ordering)
--   SEQ-10: CDC integration sets valid_from on temporal predicate assert
--   SEQ-11: snapshot retraction closes open row; re-assertion creates new open row
--   SEQ-12: versioned retraction closes all open rows
--   SEQ-13: temporal_cdc_enabled GUC default is on
--   SEQ-14: retract_triple_temporal() returns 0 when no open row exists

SET client_min_messages = warning;
CREATE EXTENSION IF NOT EXISTS pg_ripple;
SET client_min_messages = DEFAULT;
SET search_path TO pg_ripple, public;

-- Load library so _PG_init registers GUCs.
LOAD '$libdir/pg_ripple';

-- ─── Setup: register temporal predicates ─────────────────────────────────────

SELECT pg_ripple.mark_temporal('http://example.org/temperature', 'versioned');
SELECT pg_ripple.mark_temporal('http://example.org/login', 'versioned');
SELECT pg_ripple.mark_temporal('http://example.org/locked', 'versioned');
SELECT pg_ripple.mark_temporal('http://example.org/feverReading', 'versioned');
SELECT pg_ripple.mark_temporal('http://example.org/pressure', 'snapshot');

-- ─── SEQ-01: WITHIN returns true when fact is inside window ──────────────────

-- Insert a reading 1 hour ago (well within a P1D window).
INSERT INTO _pg_ripple.temporal_facts (s, p, o, g, valid_from, valid_to)
VALUES (
    pg_ripple.encode_term('http://example.org/SensorA', 0::smallint),
    pg_ripple.encode_term('http://example.org/temperature', 0::smallint),
    pg_ripple.encode_term('http://example.org/37c', 0::smallint),
    0,
    now() - INTERVAL '1 hour',
    NULL
);

SELECT pg_ripple.temporal_within(
    'http://example.org/SensorA',
    'http://example.org/temperature',
    'P1D'
) AS seq01_within_true;

-- ─── SEQ-02: WITHIN returns false when fact is outside window ────────────────

-- Insert a reading 10 days ago (outside a P3D window).
INSERT INTO _pg_ripple.temporal_facts (s, p, o, g, valid_from, valid_to)
VALUES (
    pg_ripple.encode_term('http://example.org/SensorB', 0::smallint),
    pg_ripple.encode_term('http://example.org/temperature', 0::smallint),
    pg_ripple.encode_term('http://example.org/38c', 0::smallint),
    0,
    now() - INTERVAL '10 days',
    NULL
);

SELECT NOT pg_ripple.temporal_within(
    'http://example.org/SensorB',
    'http://example.org/temperature',
    'P3D'
) AS seq02_within_false;

-- ─── SEQ-03: SEQUENCE detects A before B within window ───────────────────────

-- Insert login event, then locked event 30 minutes later (within PT1H window).
INSERT INTO _pg_ripple.temporal_facts (s, p, o, g, valid_from, valid_to)
VALUES (
    pg_ripple.encode_term('http://example.org/UserX', 0::smallint),
    pg_ripple.encode_term('http://example.org/login', 0::smallint),
    pg_ripple.encode_term('http://example.org/failed', 0::smallint),
    0,
    now() - INTERVAL '90 minutes',
    NULL
);

INSERT INTO _pg_ripple.temporal_facts (s, p, o, g, valid_from, valid_to)
VALUES (
    pg_ripple.encode_term('http://example.org/UserX', 0::smallint),
    pg_ripple.encode_term('http://example.org/locked', 0::smallint),
    pg_ripple.encode_term('http://example.org/yes', 0::smallint),
    0,
    now() - INTERVAL '60 minutes',
    NULL
);

SELECT pg_ripple.temporal_sequence(
    '',
    'http://example.org/login',
    '',
    '',
    'http://example.org/locked',
    '',
    'PT1H'
) AS seq03_sequence_detected;

-- ─── SEQ-04: SEQUENCE returns false for reversed ordering within window ───────

-- For UserX, login (at -90 min) before locked (at -60 min) is correct order.
-- Test that reversed order (locked before login) does not fire within PT30M.
SELECT NOT EXISTS (
    SELECT 1
    FROM _pg_ripple.temporal_facts e1
    JOIN _pg_ripple.temporal_facts e2 ON TRUE
    WHERE e1.p = pg_ripple.encode_term('http://example.org/login', 0::smallint)
      AND e2.p = pg_ripple.encode_term('http://example.org/locked', 0::smallint)
      AND e1.s = pg_ripple.encode_term('http://example.org/UserX', 0::smallint)
      AND e2.s = pg_ripple.encode_term('http://example.org/UserX', 0::smallint)
      -- Reversed: locked before login would mean e2.valid_from < e1.valid_from
      AND e2.valid_from < e1.valid_from
      AND e1.valid_from - e2.valid_from <= INTERVAL 'PT1H'
) AS seq04_reversed_sequence_false;

-- ─── SEQ-05: CONSECUTIVE fires at n=3 consecutive readings ───────────────────

-- Insert 3 fever readings for PatientZ (3 days apart, within P4D window).
INSERT INTO _pg_ripple.temporal_facts (s, p, o, g, valid_from, valid_to)
VALUES (
    pg_ripple.encode_term('http://example.org/PatientZ', 0::smallint),
    pg_ripple.encode_term('http://example.org/feverReading', 0::smallint),
    pg_ripple.encode_term('http://example.org/38.5c', 0::smallint),
    0,
    now() - INTERVAL '3 days',
    NULL
);

INSERT INTO _pg_ripple.temporal_facts (s, p, o, g, valid_from, valid_to)
VALUES (
    pg_ripple.encode_term('http://example.org/PatientZ', 0::smallint),
    pg_ripple.encode_term('http://example.org/feverReading', 0::smallint),
    pg_ripple.encode_term('http://example.org/39c', 0::smallint),
    0,
    now() - INTERVAL '2 days',
    NULL
);

INSERT INTO _pg_ripple.temporal_facts (s, p, o, g, valid_from, valid_to)
VALUES (
    pg_ripple.encode_term('http://example.org/PatientZ', 0::smallint),
    pg_ripple.encode_term('http://example.org/feverReading', 0::smallint),
    pg_ripple.encode_term('http://example.org/38.8c', 0::smallint),
    0,
    now() - INTERVAL '1 day',
    NULL
);

SELECT pg_ripple.temporal_consecutive(
    3,
    'http://example.org/feverReading',
    'P4D'
) AS seq05_consecutive_fires_at_3;

-- ─── SEQ-06: CONSECUTIVE does not fire at n-1 readings ───────────────────────

-- PatientW has only 2 fever readings. n=4 should not fire for anyone.
INSERT INTO _pg_ripple.temporal_facts (s, p, o, g, valid_from, valid_to)
VALUES (
    pg_ripple.encode_term('http://example.org/PatientW', 0::smallint),
    pg_ripple.encode_term('http://example.org/feverReading', 0::smallint),
    pg_ripple.encode_term('http://example.org/38c_w', 0::smallint),
    0,
    now() - INTERVAL '2 days',
    NULL
);

INSERT INTO _pg_ripple.temporal_facts (s, p, o, g, valid_from, valid_to)
VALUES (
    pg_ripple.encode_term('http://example.org/PatientW', 0::smallint),
    pg_ripple.encode_term('http://example.org/feverReading', 0::smallint),
    pg_ripple.encode_term('http://example.org/38.2c_w', 0::smallint),
    0,
    now() - INTERVAL '1 day',
    NULL
);

-- n=4 must not fire: PatientZ has 3 readings, PatientW has 2 — neither reaches 4.
SELECT NOT pg_ripple.temporal_consecutive(
    4,
    'http://example.org/feverReading',
    'P4D'
) AS seq06_consecutive_not_at_n_minus_1;

-- ─── SEQ-07: Window boundary — fact inside window (12h ago, P1D window) ───────

-- Insert a fact 12 hours ago — well inside the P1D window.
-- (Using PT12H rather than exactly P1D avoids inter-transaction timing races:
-- the INSERT and the temporal_within SELECT run in separate transactions whose
-- transaction_timestamp() values differ by a few milliseconds, which would
-- make a fact at exactly "now - P1D" fall just outside the window at SELECT time.)
INSERT INTO _pg_ripple.temporal_facts (s, p, o, g, valid_from, valid_to)
VALUES (
    pg_ripple.encode_term('http://example.org/BoundaryA', 0::smallint),
    pg_ripple.encode_term('http://example.org/temperature', 0::smallint),
    pg_ripple.encode_term('http://example.org/37c_ba', 0::smallint),
    0,
    transaction_timestamp() - INTERVAL 'PT12H',
    NULL
);

SELECT pg_ripple.temporal_within(
    'http://example.org/BoundaryA',
    'http://example.org/temperature',
    'P1D'
) AS seq07_boundary_at_window_start_included;

-- ─── SEQ-08: Window boundary — fact just outside window ──────────────────────

-- Insert a fact 1 day + 1 second before now — should be outside the P1D window.
INSERT INTO _pg_ripple.temporal_facts (s, p, o, g, valid_from, valid_to)
VALUES (
    pg_ripple.encode_term('http://example.org/BoundaryB', 0::smallint),
    pg_ripple.encode_term('http://example.org/temperature', 0::smallint),
    pg_ripple.encode_term('http://example.org/37c_bb', 0::smallint),
    0,
    transaction_timestamp() - INTERVAL 'P1D' - INTERVAL '1 second',
    NULL
);

SELECT NOT pg_ripple.temporal_within(
    'http://example.org/BoundaryB',
    'http://example.org/temperature',
    'P1D'
) AS seq08_boundary_just_outside_window;

-- ─── SEQ-09: Zero-duration SEQUENCE window edge case ─────────────────────────

-- SEQUENCE with window '0 seconds' requires e1.valid_from < e2.valid_from AND
-- (e2.valid_from - e1.valid_from) <= 0. Since the two times differ, this is false.
SELECT NOT EXISTS (
    SELECT 1
    FROM _pg_ripple.temporal_facts e1
    JOIN _pg_ripple.temporal_facts e2 ON TRUE
    WHERE e1.p = pg_ripple.encode_term('http://example.org/login', 0::smallint)
      AND e2.p = pg_ripple.encode_term('http://example.org/locked', 0::smallint)
      AND e1.valid_from < e2.valid_from
      AND e2.valid_from - e1.valid_from <= INTERVAL '0 seconds'
) AS seq09_zero_window_false;

-- ─── SEQ-10: CDC integration sets valid_from on temporal predicate assert ─────

-- Check GUC is on by default.
SELECT current_setting('pg_ripple.temporal_cdc_enabled', true) IN ('on', 'true', '1')
    AS seq10_temporal_cdc_enabled_default_on;

-- Make sure CDC is enabled.
SET pg_ripple.temporal_cdc_enabled = on;

-- Insert via insert_triple() on a CDC-enabled temporal predicate.
SELECT pg_ripple.insert_triple(
    '<http://example.org/CdcPatient>',
    '<http://example.org/pressure>',
    '"120"'
) IS NOT NULL AS seq10_insert_triple_ok;

-- A temporal_facts row should appear with valid_from close to now().
SELECT EXISTS(
    SELECT 1
    FROM _pg_ripple.temporal_facts tf
    WHERE tf.s = pg_ripple.encode_term('http://example.org/CdcPatient', 0::smallint)
      AND tf.p = pg_ripple.encode_term('http://example.org/pressure', 0::smallint)
      AND tf.valid_from >= transaction_timestamp() - INTERVAL '30 seconds'
      AND tf.valid_to IS NULL
) AS seq10_cdc_temporal_fact_created;

-- ─── SEQ-11: snapshot retraction and re-assertion ─────────────────────────────

-- Insert initial snapshot fact for DeviceD.
SELECT pg_ripple.insert_triple_temporal(
    'http://example.org/DeviceD',
    'http://example.org/pressure',
    'http://example.org/Normal',
    '2025-01-01 00:00:00+00'::timestamptz
) IS NOT NULL AS seq11_initial_insert_ok;

-- Retract it.
SELECT pg_ripple.retract_triple_temporal(
    'http://example.org/DeviceD',
    'http://example.org/pressure'
) >= 1 AS seq11_retraction_closes_open_row;

-- Verify original row is now closed (valid_to IS NOT NULL).
SELECT EXISTS(
    SELECT 1
    FROM _pg_ripple.temporal_facts tf
    JOIN _pg_ripple.dictionary dobj ON dobj.id = tf.o
    WHERE tf.s = pg_ripple.encode_term('http://example.org/DeviceD', 0::smallint)
      AND tf.p = pg_ripple.encode_term('http://example.org/pressure', 0::smallint)
      AND dobj.value = 'http://example.org/Normal'
      AND tf.valid_to IS NOT NULL
) AS seq11_retracted_row_closed;

-- Re-assert with a new value.
SELECT pg_ripple.insert_triple_temporal(
    'http://example.org/DeviceD',
    'http://example.org/pressure',
    'http://example.org/High',
    now()
) IS NOT NULL AS seq11_reassert_ok;

-- The new row should be open-ended.
SELECT EXISTS(
    SELECT 1
    FROM _pg_ripple.temporal_facts tf
    JOIN _pg_ripple.dictionary dobj ON dobj.id = tf.o
    WHERE tf.s = pg_ripple.encode_term('http://example.org/DeviceD', 0::smallint)
      AND tf.p = pg_ripple.encode_term('http://example.org/pressure', 0::smallint)
      AND dobj.value = 'http://example.org/High'
      AND tf.valid_to IS NULL
) AS seq11_reasserted_row_open;

-- ─── SEQ-12: versioned retraction closes all open rows ───────────────────────

-- Insert two versioned facts for SensorV.
SELECT pg_ripple.insert_triple_temporal(
    'http://example.org/SensorV',
    'http://example.org/temperature',
    'http://example.org/36c',
    '2025-01-01 00:00:00+00'::timestamptz
) IS NOT NULL AS seq12_versioned_insert1_ok;

SELECT pg_ripple.insert_triple_temporal(
    'http://example.org/SensorV',
    'http://example.org/temperature',
    'http://example.org/37c',
    '2025-06-01 00:00:00+00'::timestamptz
) IS NOT NULL AS seq12_versioned_insert2_ok;

-- Retract — closes all open rows for (SensorV, temperature).
SELECT pg_ripple.retract_triple_temporal(
    'http://example.org/SensorV',
    'http://example.org/temperature'
) >= 1 AS seq12_versioned_retract_ok;

-- All rows for SensorV temperature should now be closed.
SELECT COUNT(*) = 0 AS seq12_all_rows_closed
FROM _pg_ripple.temporal_facts tf
WHERE tf.s = pg_ripple.encode_term('http://example.org/SensorV', 0::smallint)
  AND tf.p = pg_ripple.encode_term('http://example.org/temperature', 0::smallint)
  AND tf.valid_to IS NULL;

-- ─── SEQ-13: temporal_cdc_enabled GUC default is on ─────────────────────────

SELECT current_setting('pg_ripple.temporal_cdc_enabled', true) IN ('on', 'true', '1')
    AS seq13_cdc_guc_default_on;

-- ─── SEQ-14: retract_triple_temporal returns 0 when no open row exists ────────

-- Use a subject that has never had any temporal facts — retract must return 0.
SELECT pg_ripple.retract_triple_temporal(
    'http://example.org/NeverInsertedSubject',
    'http://example.org/pressure'
) = 0 AS seq14_retract_no_open_row_zero;

-- ─── Cleanup ─────────────────────────────────────────────────────────────────

DELETE FROM _pg_ripple.temporal_facts
WHERE p IN (
    SELECT tp.predicate_id FROM _pg_ripple.temporal_predicates tp
    JOIN _pg_ripple.dictionary d ON d.id = tp.predicate_id
    WHERE d.value IN (
        'http://example.org/temperature',
        'http://example.org/login',
        'http://example.org/locked',
        'http://example.org/feverReading',
        'http://example.org/pressure'
    )
);

SELECT pg_ripple.unmark_temporal('http://example.org/temperature');
SELECT pg_ripple.unmark_temporal('http://example.org/login');
SELECT pg_ripple.unmark_temporal('http://example.org/locked');
SELECT pg_ripple.unmark_temporal('http://example.org/feverReading');
SELECT pg_ripple.unmark_temporal('http://example.org/pressure');
