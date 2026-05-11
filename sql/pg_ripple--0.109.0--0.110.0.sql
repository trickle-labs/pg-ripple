-- Migration 0.109.0 → 0.110.0: NS-RL Evaluation Harness, Continuous Monitoring & Rule Explainability
--
-- New schema objects:
--   _pg_ripple.rule_explanations   — explanation cache for Datalog rules
--   _pg_ripple.sameas_anomaly_log  — append-only audit log for PT550 events
--
-- New SQL-callable functions (provided via compiled Rust):
--   pg_ripple.evaluate_resolution(gold_graph TEXT,
--                                  pipeline_options JSONB DEFAULT '{}') → JSONB
--     Runs the NS-RL pipeline against a gold-standard graph and returns pairwise,
--     blocking, and B³ cluster precision/recall/F1 metrics.
--
--   pg_ripple.enable_er_monitoring() → VOID
--     Creates three live monitoring tables:
--       _pg_ripple.er_unresolved_entities
--       _pg_ripple.er_cluster_sizes
--       _pg_ripple.er_resolution_dashboard
--
--   pg_ripple.disable_er_monitoring() → VOID
--     Drops the three monitoring tables (idempotent).
--
--   pg_ripple.explain_rule(rule_id BIGINT,
--                          language TEXT DEFAULT 'en',
--                          format   TEXT DEFAULT 'text') → TEXT
--     Returns a plain-English explanation of the Datalog rule.
--     LLM-driven when pg_ripple.llm_endpoint is configured; otherwise
--     uses a template-driven structural description.
--
--   pg_ripple.explain_rule_batch(rule_ids BIGINT[]) → TABLE(rule_id BIGINT, explanation TEXT)
--     Batch variant of explain_rule().
--
-- New GUC parameters:
--   pg_ripple.record_sameas_anomalies   (BOOL, default on)
--     When on, PT550-triggering merges are logged to sameas_anomaly_log.
--   pg_ripple.sameas_anomaly_log_retention (TEXT, default '90 days')
--     Retention period for sameas_anomaly_log rows.
--   pg_ripple.rule_explanation_cache_ttl (TEXT, default '24 hours')
--     TTL for cached explain_rule() results.

-- ── Rule explanation cache ────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS _pg_ripple.rule_explanations (
    rule_id       BIGINT      NOT NULL,
    language      TEXT        NOT NULL DEFAULT 'en',
    format        TEXT        NOT NULL DEFAULT 'text',
    explanation   TEXT        NOT NULL,
    generated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (rule_id, language, format)
);
COMMENT ON TABLE _pg_ripple.rule_explanations IS
    'Plain-English explanation cache for Datalog rules (v0.110.0). '
    'One row per (rule_id, language, format). '
    'TTL controlled by pg_ripple.rule_explanation_cache_ttl (default 24 hours).';

-- ── owl:sameAs anomaly log ────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS _pg_ripple.sameas_anomaly_log (
    id                   BIGSERIAL   PRIMARY KEY,
    detected_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    entity1              BIGINT      NOT NULL,
    entity2              BIGINT      NOT NULL,
    cluster_size_before  INT         NOT NULL DEFAULT 0,
    cluster_size_after   INT         NOT NULL DEFAULT 0,
    trigger              TEXT        NOT NULL DEFAULT '',
    transaction_xid      XID8        NOT NULL DEFAULT pg_current_xact_id()
);
CREATE INDEX IF NOT EXISTS idx_sameas_anomaly_log_detected_at
    ON _pg_ripple.sameas_anomaly_log (detected_at);
ALTER TABLE _pg_ripple.sameas_anomaly_log ENABLE ROW LEVEL SECURITY;
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_policies
        WHERE schemaname = '_pg_ripple'
          AND tablename  = 'sameas_anomaly_log'
          AND policyname = 'insert_only'
    ) THEN
        CREATE POLICY insert_only ON _pg_ripple.sameas_anomaly_log
            FOR INSERT WITH CHECK (true);
    END IF;
END;
$$;
COMMENT ON TABLE _pg_ripple.sameas_anomaly_log IS
    'Append-only audit log of owl:sameAs assertions that would exceed '
    'pg_ripple.sameas_max_cluster_size (PT550). '
    'Rows older than pg_ripple.sameas_anomaly_log_retention (default 90 days) '
    'are pruned by the background worker.';
