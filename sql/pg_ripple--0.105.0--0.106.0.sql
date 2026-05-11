-- Migration 0.105.0 → 0.106.0: Temporal Reasoning Phase 1
--
-- New features:
--   - _pg_ripple.temporal_facts table with three indexes
--   - _pg_ripple.temporal_predicates registry table
--   - pg_ripple.mark_temporal(predicate_iri, data_model) SQL function
--   - pg_ripple.unmark_temporal(predicate_iri) SQL function
--   - pg_ripple.insert_triple_temporal(...) SQL function
--   - pg_ripple.temporal_window(...) SQL function
--   - pg_ripple.temporal_data_model GUC
--   - pg_ripple.enable_temporal_operators GUC
--   - Datalog temporal operators: AFTER, BEFORE, DURING
--   - SPARQL pg:temporal_window() filter function
--   - SHACL sh:validFor duration constraint
--   - Error codes PT0430, PT0431, PT0432
--
-- Schema changes: creates _pg_ripple.temporal_predicates and
--   _pg_ripple.temporal_facts with their indexes.

-- ─── temporal_predicates registry ────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS _pg_ripple.temporal_predicates (
    predicate_id  BIGINT NOT NULL PRIMARY KEY,
    data_model    TEXT   NOT NULL
                  CHECK (data_model IN ('snapshot', 'versioned')),
    registered_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- ─── temporal_facts table ─────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS _pg_ripple.temporal_facts (
    s          BIGINT      NOT NULL,
    p          BIGINT      NOT NULL,
    o          BIGINT      NOT NULL,
    g          BIGINT      NOT NULL DEFAULT 0,
    valid_from TIMESTAMPTZ NOT NULL,
    valid_to   TIMESTAMPTZ
);

-- B-tree on (s, p, valid_from, valid_to) for subject-scoped temporal queries.
CREATE INDEX IF NOT EXISTS idx_temporal_facts_s_p_vf_vt
    ON _pg_ripple.temporal_facts (s, p, valid_from, valid_to);

-- B-tree on (p, valid_from, valid_to) for predicate-scoped temporal scans.
CREATE INDEX IF NOT EXISTS idx_temporal_facts_p_vf_vt
    ON _pg_ripple.temporal_facts (p, valid_from, valid_to);

-- Partial B-tree on (valid_from, valid_to) WHERE valid_to IS NULL for
-- currently-valid (open-ended interval) facts.
CREATE INDEX IF NOT EXISTS idx_temporal_facts_open
    ON _pg_ripple.temporal_facts (valid_from, valid_to)
    WHERE valid_to IS NULL;
