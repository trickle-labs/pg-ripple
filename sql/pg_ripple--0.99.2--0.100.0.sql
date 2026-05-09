-- Migration 0.99.2 → 0.100.0: Proof trees & justification infrastructure
--
-- New in v0.100.0 (PROOF-TREE-01):
--
-- * _pg_ripple.derivations table — records (derived_sid, rule_name, rule_set,
--   antecedent_sids[]) for every Datalog-derived fact when
--   pg_ripple.record_derivations = on.
--
-- * pg_ripple.justify(subject, predicate, object) — backward-chaining proof
--   tree as JSONB.
--
-- * pg_ripple.vacuum_derivations() — removes orphan derivation rows.
--
-- * pg_ripple.record_derivations GUC (bool, default off) — gates overhead.
--
-- The Rust functions (justify, vacuum_derivations) are registered automatically
-- by the updated shared library; no manual SQL registration is needed.

-- Derivation provenance table
CREATE TABLE IF NOT EXISTS _pg_ripple.derivations (
    id              BIGINT      GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    derived_sid     BIGINT      NOT NULL,
    rule_name       TEXT        NOT NULL,
    rule_set        TEXT        NOT NULL DEFAULT '',
    antecedent_sids BIGINT[]    NOT NULL DEFAULT '{}',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT derivations_unique UNIQUE (derived_sid, rule_name)
);
CREATE INDEX IF NOT EXISTS idx_derivations_derived_sid
    ON _pg_ripple.derivations (derived_sid);
CREATE INDEX IF NOT EXISTS idx_derivations_antecedent
    ON _pg_ripple.derivations USING GIN (antecedent_sids);
COMMENT ON TABLE _pg_ripple.derivations IS
    'Proof provenance for Datalog-inferred facts. '
    'Populated when pg_ripple.record_derivations = on. '
    'Query with pg_ripple.justify(subject, predicate, object).';
