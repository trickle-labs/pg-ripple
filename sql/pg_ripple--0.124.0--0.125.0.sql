-- Migration 0.124.0 → 0.125.0: Temporal graph snapshots (FEAT-02)
--
-- New SQL objects in this release:
--
-- 1. _pg_ripple.snapshot_id_seq (SEQUENCE)
--    Global sequence for snapshot_id primary keys.
--
-- 2. _pg_ripple.graph_snapshots (TABLE)
--    Catalog of registered point-in-time named-graph snapshots.
--    Columns:
--      snapshot_id  BIGINT DEFAULT nextval('_pg_ripple.snapshot_id_seq') PRIMARY KEY
--      graph_iri    TEXT NOT NULL
--      snapshot_iri TEXT NOT NULL UNIQUE
--      captured_at  TIMESTAMPTZ NOT NULL
--      triple_count BIGINT
--      expires_at   TIMESTAMPTZ
--
-- 3. pg_ripple.graph_at(graph_iri TEXT, snapshot_time TIMESTAMPTZ) → TEXT
--    Materialises a snapshot from _pg_ripple.temporal_facts and registers it
--    in _pg_ripple.graph_snapshots.  Returns the snapshot IRI.
--
-- 4. pg_ripple.graph_diff(graph_iri TEXT, from_ts TIMESTAMPTZ, to_ts TIMESTAMPTZ)
--      → TABLE(s BIGINT, p BIGINT, o BIGINT, change TEXT)
--    Returns 'added'/'removed' delta rows between two temporal snapshots.
--
-- 5. pg_ripple.graph_snapshots_count() → BIGINT
--    Returns the current live snapshot count.
--
-- 6. GUC pg_ripple.snapshot_retention_days (integer, default 30)
--    Controls automatic GC of expired snapshots via the merge background worker.
--    Set to 0 to keep snapshots indefinitely.
--
-- All objects are created idempotently by _PG_init / initialize_schema()
-- when the extension is loaded; no manual DDL is needed here.
--
-- The sequence and table are created automatically when the extension starts.
-- This script exists to satisfy ALTER EXTENSION pg_ripple UPDATE path.

CREATE SEQUENCE IF NOT EXISTS _pg_ripple.snapshot_id_seq;

CREATE TABLE IF NOT EXISTS _pg_ripple.graph_snapshots (
    snapshot_id  BIGINT      NOT NULL DEFAULT nextval('_pg_ripple.snapshot_id_seq')
                             PRIMARY KEY,
    graph_iri    TEXT        NOT NULL,
    snapshot_iri TEXT        NOT NULL UNIQUE,
    captured_at  TIMESTAMPTZ NOT NULL,
    triple_count BIGINT,
    expires_at   TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_graph_snapshots_graph_iri
    ON _pg_ripple.graph_snapshots (graph_iri, captured_at DESC);

CREATE INDEX IF NOT EXISTS idx_graph_snapshots_expires_at
    ON _pg_ripple.graph_snapshots (expires_at)
    WHERE expires_at IS NOT NULL;
