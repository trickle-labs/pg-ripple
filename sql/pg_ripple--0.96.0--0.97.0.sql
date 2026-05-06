-- Migration: pg_ripple 0.96.0 → 0.97.0
-- v0.97.0 — A15 Low-severity Polish & Supply-Chain
--
-- Deliverables in this release (all Low-severity from Assessment 15):
--   L15-01: Fix CHANGELOG v0.90.0 date placeholder (no SQL)
--   L15-02: Add Arrow Flight, PageRank, and bidi relay example files (no SQL)
--   L15-03: Wire examples/test_all.sh --live in CI (no SQL)
--   L15-04: Enforce clippy::missing_safety_doc + undocumented_unsafe_blocks (no SQL)
--   L15-05: #[allow(...)] justification audit with // Q15-xx: convention (no SQL)
--   L15-06: gen_random_uuid() availability check at _PG_init (see DO block below)
--   M15-16: serde_cbor consumer audit; documented in Cargo.toml (no SQL)
--   L15-08: RDF-star <<>> position support matrix in docs (no SQL)
--   L15-09: cargo doc --no-deps gate in CI (no SQL)
--   L15-10: Auto-compute HIGHEST_CHECKPOINT in test_migration_chain.sh (no SQL)
--   L15-11: Document statement_id_seq exhaustion in docs/operations/scaling.md (no SQL)
--   L15-12: owl_sameas_cycle.sql regression test (no SQL schema changes)
--   L15-14: Conformance-suite pass-rate badges in README.md (no SQL)

-- GEN-UUID-01 (v0.97.0, L15-06): Verify gen_random_uuid() is available.
-- In PostgreSQL 14+, gen_random_uuid() is a built-in function.
-- On PostgreSQL 18 (the only supported target) it is always present.
-- This check is a defensive guard for unusual build configurations.
DO $$
DECLARE
    _uuid uuid;
BEGIN
    _uuid := gen_random_uuid();
    -- Function available; no action needed.
EXCEPTION WHEN undefined_function THEN
    RAISE WARNING
        'pg_ripple: gen_random_uuid() is not available. '
        'The bidi relay and SPARQL uuid() / struuid() functions will not work. '
        'HINT: run CREATE EXTENSION IF NOT EXISTS pgcrypto; to enable it.';
END;
$$;

-- All other v0.97.0 changes are documentation, example files, CI, and code quality.
-- No schema changes are required for this release.
