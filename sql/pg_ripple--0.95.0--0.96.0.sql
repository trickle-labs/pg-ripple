-- Migration 0.95.0 → 0.96.0: A15 Medium — Performance, Code Quality, Test Coverage
-- v0.96.0 deliverables:
--   M15-05: HTAP tombstone-skip optimisation (adds tombstone_count to predicates)
--   M15-06: Star-join collapse GUC (pg_ripple.star_join_collapse, pure Rust)
--   M15-11: Federation connect-timeout GUC (pg_ripple.federation_connect_timeout_secs, pure Rust)
--   M15-13: Sub-split five large source files into sub-modules (pure Rust, no SQL)
--   M15-14: Sub-split routing/datalog_handlers.rs (HTTP companion, no extension SQL)
--   M15-15: Zero missing-docs warnings (pure Rust, no SQL)
--   M15-17: pagerank_with_writes.sh concurrent-load benchmark (no SQL)
--   M15-18: shacl_report_scored column-order regression test (no SQL)
--   M15-19: Four new Prometheus metrics (HTTP companion, no extension SQL)
--   M15-21: datalog_cyclic_parallel regression test (no SQL schema changes)
--   M15-22: Arrow Flight EXPLAIN-only path (HTTP companion, no extension SQL)

-- Schema change required: M15-05 tombstone-skip
-- Add tombstone_count column to the predicate catalog so the HTAP view switcher
-- can skip the LEFT JOIN tombstone filter when there are no pending tombstones.
ALTER TABLE _pg_ripple.predicates
    ADD COLUMN IF NOT EXISTS tombstone_count BIGINT NOT NULL DEFAULT 0;

COMMENT ON COLUMN _pg_ripple.predicates.tombstone_count IS
    'Number of pending tombstone rows for this predicate. '
    'When 0, the HTAP view uses the faster tombstone-skip form (no LEFT JOIN). '
    '(M15-05, v0.96.0)';

-- All other v0.96.0 changes are pure Rust function / GUC additions.
-- New GUCs available after extension upgrade:
--   pg_ripple.star_join_collapse          BOOL    default true
--   pg_ripple.federation_connect_timeout_secs  INT   default 10
