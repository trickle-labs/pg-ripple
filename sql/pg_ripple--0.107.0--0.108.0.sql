-- Migration 0.107.0 → 0.108.0: Bayesian Confidence Updates
--
-- New SQL functions (compiled from Rust):
--   pg_ripple.update_confidence(subject, predicate, object, evidence TEXT) → TABLE(prior FLOAT8, posterior FLOAT8)
--   pg_ripple.bulk_update_confidence(data TEXT, format TEXT)               → BIGINT
--   pg_ripple.vacuum_evidence_log()                                         → BIGINT
--
-- New GUCs:
--   pg_ripple.confidence_update_strategy       (TEXT, default NULL = 'bayesian')
--   pg_ripple.confidence_propagation_max_depth (INT,  default 10)
--   pg_ripple.confidence_reprocessing_interval (TEXT, default NULL = '30 seconds')
--   pg_ripple.evidence_log_retention           (TEXT, default NULL = '1 year')
--   pg_ripple.confidence_batch_size            (INT,  default 1000)
--   pg_ripple.conflict_confidence_penalty      (FLOAT8, default 0.3)
--
-- New REST endpoints (pg_ripple_http):
--   POST /confidence/update       → {"prior": ..., "posterior": ...}
--   POST /confidence/bulk-update  → {"updated": N}
--
-- Schema changes:
--   CREATE TABLE _pg_ripple.evidence_log  — append-only Bayesian evidence log
--   CREATE TABLE _pg_ripple.confidence_stale  — overflow queue for deep propagation

CREATE TABLE IF NOT EXISTS _pg_ripple.evidence_log (
    id                   BIGSERIAL   PRIMARY KEY,
    sid                  BIGINT      NOT NULL,
    event_at             TIMESTAMPTZ NOT NULL DEFAULT now(),
    source_iri           BIGINT,
    likelihood_ratio     FLOAT8      NOT NULL,
    prior_confidence     FLOAT8      NOT NULL,
    posterior_confidence FLOAT8      NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_evidence_log_sid
    ON _pg_ripple.evidence_log (sid);

CREATE INDEX IF NOT EXISTS idx_evidence_log_event_at
    ON _pg_ripple.evidence_log (event_at);

CREATE TABLE IF NOT EXISTS _pg_ripple.confidence_stale (
    sid       BIGINT      NOT NULL PRIMARY KEY,
    marked_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
