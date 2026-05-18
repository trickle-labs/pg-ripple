-- Migration 0.115.0 → 0.116.0: A16 medium correctness, security GUCs, and CHANGELOG hygiene
--
-- Schema changes:
--   • rule_explanations: add rule_version_stamp column for stale-cache detection (M16-05)
--   • foaf-integrity builtin rules: fix triple-pattern syntax (functional notation → SPO) (M16-08)
--
-- New GUCs (registered at extension load time, no DDL required):
--   • pg_ripple.er_monitoring_retention_days   (M16-01)
--   • pg_ripple.proof_tree_max_depth           (M16-07)
--   • pg_ripple.proof_tree_max_nodes           (M16-07)
--   • pg_ripple.rule_explanation_cache_max_entries (M16-19)
--   • pg_ripple.bayesian_propagation_max_depth (M16-20)
--   • pg_ripple.bidi_relay_drop_policy         (M16-11)
--
-- No SQL changes are required for: M16-06 (audit.toml lifecycle policy),
--   M16-21 (audit.toml advisory header), M16-23 (docs/gucs.md).

ALTER TABLE _pg_ripple.rule_explanations
    ADD COLUMN IF NOT EXISTS rule_version_stamp BIGINT NOT NULL DEFAULT 0;
