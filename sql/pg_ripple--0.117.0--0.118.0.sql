-- Migration 0.117.0 → 0.118.0
-- Theme: Temporal Allen's Relations, compat_check() and Privacy Budget Registry
--
-- New SQL objects and schema changes in this release:
--
-- Feature 1 — Integrated Benchmark Runner:
--   CREATE TABLE _pg_ripple.bench_history (run_id, profile, started_at,
--       duration_ms, triples_processed, queries_per_second)
--   New SQL functions: pg_ripple.bench_workload(profile), bench_history_recent()
--
-- Feature 2 — Privacy Budget Registry:
--   CREATE TABLE _pg_ripple.privacy_budget (dataset_id, principal,
--       budget_total, budget_spent, last_reset_at)
--   New GUC: pg_ripple.privacy_budget_reset_interval (default '1 day')
--   dp_noisy_count() and dp_noisy_histogram() now accept optional
--   dataset_id BIGINT and principal TEXT parameters for budget tracking.
--   PT0490 raised when budget_spent + epsilon > budget_total.
--
-- Feature 3 — compat_check() SQL Function:
--   New SQL function: pg_ripple.compat_check() → TEXT
--   Returns {"extension_version":"...","http_min_version":"...","compatible":true}
--
-- Feature 4 — Allen's Interval Relations:
--   Seven new SPARQL FILTER functions: pg:before, pg:meets, pg:overlaps,
--   pg:during, pg:finishes, pg:starts, pg:equals
--   Available as Datalog temporal operators: ALLEN_BEFORE, ALLEN_MEETS,
--   ALLEN_OVERLAPS, ALLEN_DURING, ALLEN_FINISHES, ALLEN_STARTS, ALLEN_EQUALS
--
-- AT TIME ZONE Gap Fix:
--   point_in_time() now accepts optional time_zone TEXT parameter.
--   mark_temporal() now accepts optional time_zone TEXT parameter.
--   _pg_ripple.temporal_predicates gains a default_tz TEXT column.

-- Privacy budget registry table
CREATE TABLE IF NOT EXISTS _pg_ripple.privacy_budget (
    dataset_id    BIGINT      NOT NULL,
    principal     TEXT        NOT NULL,
    budget_total  FLOAT8      NOT NULL,
    budget_spent  FLOAT8      NOT NULL DEFAULT 0,
    last_reset_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT privacy_budget_pk PRIMARY KEY (dataset_id, principal),
    CONSTRAINT privacy_budget_total_pos CHECK (budget_total > 0),
    CONSTRAINT privacy_budget_spent_nonneg CHECK (budget_spent >= 0)
);
COMMENT ON TABLE _pg_ripple.privacy_budget IS
    'Per-dataset per-principal differential-privacy epsilon budget registry (v0.118.0).';

-- Benchmark history table
CREATE TABLE IF NOT EXISTS _pg_ripple.bench_history (
    run_id              BIGSERIAL   PRIMARY KEY,
    profile             TEXT        NOT NULL,
    started_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    duration_ms         BIGINT,
    triples_processed   BIGINT,
    queries_per_second  FLOAT8
);
CREATE INDEX IF NOT EXISTS idx_bench_history_started_at
    ON _pg_ripple.bench_history (started_at DESC);
COMMENT ON TABLE _pg_ripple.bench_history IS
    'Benchmark run history for pg_ripple.bench_workload() (v0.118.0).';

-- AT TIME ZONE gap: add default_tz column to temporal_predicates
ALTER TABLE _pg_ripple.temporal_predicates
    ADD COLUMN IF NOT EXISTS default_tz TEXT;
COMMENT ON COLUMN _pg_ripple.temporal_predicates.default_tz IS
    'Optional default time zone for temporal queries against this predicate (v0.118.0).';
