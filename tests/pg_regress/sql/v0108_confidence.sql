-- v0.108.0 Feature Regression Tests
-- Tests for: Bayesian Confidence Updates
--
-- Covers:
--   BAYES-01: update_confidence() with LR > 1.0 increases confidence
--   BAYES-02: update_confidence() with LR < 1.0 decreases confidence
--   BAYES-03: update_confidence() with LR = 1.0 leaves confidence unchanged
--   BAYES-04: each call creates one row in _pg_ripple.evidence_log
--   BAYES-05: PT0440 raised for LR <= 0.0
--   BAYES-06: PT0441 raised when strategy = 'manual'
--   BAYES-07: vacuum_evidence_log() returns a non-negative count
--   BAYES-08: bulk_update_confidence() CSV format returns count of updated facts
--   BAYES-09: bulk_update_confidence() JSON-L format
--   BAYES-10: evidence_log table has correct schema
--   BAYES-11: confidence_stale table has correct schema
--   BAYES-12: GUC pg_ripple.confidence_propagation_max_depth default is 10
--   BAYES-13: GUC pg_ripple.confidence_update_strategy default is NULL (bayesian)
--   BAYES-14: noisy-or strategy path

SET client_min_messages = warning;
CREATE EXTENSION IF NOT EXISTS pg_ripple;
SET client_min_messages = DEFAULT;
SET search_path TO pg_ripple, public;

LOAD '$libdir/pg_ripple';

-- ─── Setup: load a test triple with explicit confidence ───────────────────────

SELECT pg_ripple.load_triples_with_confidence(
    '<http://example.org/alice> <http://example.org/worksAt> <http://example.org/acme> .',
    0.5
) AS triples_loaded;

-- ─── BAYES-01: LR > 1.0 increases confidence ─────────────────────────────────

SELECT
    prior < posterior AS bayes01_lr_gt1_increases,
    prior BETWEEN 0.001 AND 0.999 AS bayes01_prior_in_range,
    posterior BETWEEN 0.001 AND 0.999 AS bayes01_posterior_in_range
FROM pg_ripple.update_confidence(
    'http://example.org/alice',
    'http://example.org/worksAt',
    'http://example.org/acme',
    '{"source":"doc1","likelihood_ratio":3.0}'
);

-- ─── BAYES-02: LR < 1.0 decreases confidence ─────────────────────────────────

SELECT
    prior > posterior AS bayes02_lr_lt1_decreases
FROM pg_ripple.update_confidence(
    'http://example.org/alice',
    'http://example.org/worksAt',
    'http://example.org/acme',
    '{"source":"doc2","likelihood_ratio":0.2}'
);

-- ─── BAYES-03: LR = 1.0 leaves confidence unchanged ──────────────────────────

SELECT
    abs(prior - posterior) < 1e-9 AS bayes03_neutral_lr
FROM pg_ripple.update_confidence(
    'http://example.org/alice',
    'http://example.org/worksAt',
    'http://example.org/acme',
    '{"source":"doc3","likelihood_ratio":1.0}'
);

-- ─── BAYES-04: each call creates a row in evidence_log ───────────────────────

SELECT COUNT(*) >= 3 AS bayes04_evidence_log_has_rows
FROM _pg_ripple.evidence_log
WHERE sid IN (
    SELECT i FROM _pg_ripple.vp_rare
    WHERE s = pg_ripple.encode_term('http://example.org/alice', 0::smallint)
      AND p = pg_ripple.encode_term('http://example.org/worksAt', 0::smallint)
      AND o = pg_ripple.encode_term('http://example.org/acme', 0::smallint)
    LIMIT 1
);

-- ─── BAYES-05: PT0440 raised for LR <= 0.0 ───────────────────────────────────

DO $$
BEGIN
    BEGIN
        PERFORM pg_ripple.update_confidence(
            'http://example.org/alice',
            'http://example.org/worksAt',
            'http://example.org/acme',
            '{"source":"bad","likelihood_ratio":-1.0}'
        );
        RAISE EXCEPTION 'expected error not raised';
    EXCEPTION WHEN OTHERS THEN
        IF sqlerrm LIKE '%PT0440%' OR sqlerrm LIKE '%likelihood_ratio must be positive%' THEN
            RAISE NOTICE 'BAYES-05 OK: PT0440 raised as expected';
        ELSE
            RAISE EXCEPTION 'BAYES-05 FAIL: unexpected error: %', sqlerrm;
        END IF;
    END;
END;
$$;

-- ─── BAYES-06: PT0441 raised when strategy = 'manual' ────────────────────────

SET pg_ripple.confidence_update_strategy = 'manual';

DO $$
BEGIN
    BEGIN
        PERFORM pg_ripple.update_confidence(
            'http://example.org/alice',
            'http://example.org/worksAt',
            'http://example.org/acme',
            '{"source":"doc5","likelihood_ratio":2.0}'
        );
        RAISE EXCEPTION 'expected error not raised';
    EXCEPTION WHEN OTHERS THEN
        IF sqlerrm LIKE '%PT0441%' OR sqlerrm LIKE '%manual%' THEN
            RAISE NOTICE 'BAYES-06 OK: PT0441 raised as expected';
        ELSE
            RAISE EXCEPTION 'BAYES-06 FAIL: unexpected error: %', sqlerrm;
        END IF;
    END;
END;
$$;

RESET pg_ripple.confidence_update_strategy;

-- ─── BAYES-07: vacuum_evidence_log() returns non-negative count ───────────────

SELECT pg_ripple.vacuum_evidence_log() >= 0 AS bayes07_vacuum_ok;

-- ─── BAYES-08: bulk_update_confidence() CSV format ───────────────────────────

-- Load a second triple to use for bulk update.
SELECT pg_ripple.load_triples_with_confidence(
    '<http://example.org/bob> <http://example.org/worksAt> <http://example.org/acme> .',
    0.6
) AS triples_loaded_bob;

SELECT pg_ripple.bulk_update_confidence(
    E'http://example.org/bob,http://example.org/worksAt,http://example.org/acme,src_bulk,2.0\n',
    'csv'
) >= 1 AS bayes08_bulk_csv_ok;

-- ─── BAYES-09: bulk_update_confidence() JSON-L format ─────────────────────────

SELECT pg_ripple.bulk_update_confidence(
    '{"subject":"http://example.org/bob","predicate":"http://example.org/worksAt","object":"http://example.org/acme","source":"src_jsonl","likelihood_ratio":1.5}',
    'json'
) >= 1 AS bayes09_bulk_jsonl_ok;

-- ─── BAYES-10: evidence_log table schema check ───────────────────────────────

SELECT
    COUNT(*) = 6 AS bayes10_evidence_log_columns
FROM information_schema.columns
WHERE table_schema = '_pg_ripple'
  AND table_name = 'evidence_log'
  AND column_name IN (
      'id', 'sid', 'event_at', 'source_iri',
      'likelihood_ratio', 'prior_confidence', 'posterior_confidence'
  );

-- ─── BAYES-11: confidence_stale table exists ─────────────────────────────────

SELECT EXISTS (
    SELECT 1 FROM information_schema.tables
    WHERE table_schema = '_pg_ripple' AND table_name = 'confidence_stale'
) AS bayes11_confidence_stale_exists;

-- ─── BAYES-12: GUC default for confidence_propagation_max_depth ──────────────

SHOW pg_ripple.confidence_propagation_max_depth;

-- ─── BAYES-13: GUC default for confidence_update_strategy ────────────────────

SHOW pg_ripple.confidence_update_strategy;

-- ─── BAYES-14: noisy-or strategy path ────────────────────────────────────────

SET pg_ripple.confidence_update_strategy = 'noisy-or';

SELECT
    prior < posterior AS bayes14_noisy_or_increases_with_lr_gt1
FROM pg_ripple.update_confidence(
    'http://example.org/alice',
    'http://example.org/worksAt',
    'http://example.org/acme',
    '{"source":"noisy_or_test","likelihood_ratio":4.0}'
);

RESET pg_ripple.confidence_update_strategy;
