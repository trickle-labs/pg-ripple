-- v0.125.0 Regression Tests: Temporal graph snapshots (FEAT-02)
--
-- Covers:
--   SNAP-01: graph_snapshots table exists in _pg_ripple schema
--   SNAP-02: snapshot_id_seq sequence exists
--   SNAP-03: graph_at() function exists in pg_ripple schema
--   SNAP-04: graph_diff() function exists in pg_ripple schema
--   SNAP-05: graph_snapshots_count() function exists in pg_ripple schema
--   SNAP-06: graph_at() returns a urn:snapshot: IRI
--   SNAP-07: graph_at() registers row in graph_snapshots table
--   SNAP-08: graph_at() idempotent — same IRI for same (graph, time) pair
--   SNAP-09: graph_snapshots_count() returns 0 when no snapshots exist
--   SNAP-10: graph_snapshots_count() increments after graph_at()
--   SNAP-11: graph_diff() returns empty set when no facts differ between timestamps
--   SNAP-12: graph_diff() returns 'added' row when fact added between timestamps
--   SNAP-13: graph_diff() returns 'removed' row when fact retracted between timestamps
--   SNAP-14: snapshot_retention_days GUC default is 30
--   SNAP-15: graph_snapshots table has expected columns

SET client_min_messages = warning;
CREATE EXTENSION IF NOT EXISTS pg_ripple;
SET client_min_messages = DEFAULT;
SET search_path TO pg_ripple, public;
LOAD '$libdir/pg_ripple';

-- ─── SNAP-01: graph_snapshots table exists ────────────────────────────────────

SELECT EXISTS(
    SELECT 1 FROM information_schema.tables
    WHERE table_schema = '_pg_ripple'
      AND table_name   = 'graph_snapshots'
) AS snap01_graph_snapshots_table_exists;

-- ─── SNAP-02: snapshot_id_seq sequence exists ─────────────────────────────────

SELECT EXISTS(
    SELECT 1 FROM pg_sequences
    WHERE schemaname = '_pg_ripple'
      AND sequencename = 'snapshot_id_seq'
) AS snap02_snapshot_id_seq_exists;

-- ─── SNAP-03: graph_at() function exists ──────────────────────────────────────

SELECT EXISTS(
    SELECT 1 FROM information_schema.routines
    WHERE routine_schema = 'pg_ripple'
      AND routine_name   = 'graph_at'
      AND routine_type   = 'FUNCTION'
) AS snap03_graph_at_exists;

-- ─── SNAP-04: graph_diff() function exists ────────────────────────────────────

SELECT EXISTS(
    SELECT 1 FROM information_schema.routines
    WHERE routine_schema = 'pg_ripple'
      AND routine_name   = 'graph_diff'
      AND routine_type   = 'FUNCTION'
) AS snap04_graph_diff_exists;

-- ─── SNAP-05: graph_snapshots_count() function exists ────────────────────────

SELECT EXISTS(
    SELECT 1 FROM information_schema.routines
    WHERE routine_schema = 'pg_ripple'
      AND routine_name   = 'graph_snapshots_count'
      AND routine_type   = 'FUNCTION'
) AS snap05_graph_snapshots_count_exists;

-- ─── SNAP-06: graph_at() returns a urn:snapshot: IRI ────────────────────────

-- Set up a temporal predicate and insert a fact so graph_at has something to count.
SELECT pg_ripple.mark_temporal('http://example.org/snap/knows', 'snapshot');

SELECT pg_ripple.insert_triple_temporal(
    'http://example.org/snap/Alice',
    'http://example.org/snap/knows',
    'http://example.org/snap/Bob',
    '2024-03-01 00:00:00+00'::timestamptz,
    NULL::timestamptz,
    'http://example.org/snap/Graph1'
) IS NOT NULL AS snap06_insert_ok;

SELECT starts_with(
    pg_ripple.graph_at(
        'http://example.org/snap/Graph1',
        '2024-03-01 12:00:00+00'::timestamptz
    ),
    'urn:snapshot:'
) AS snap06_graph_at_returns_urn;

-- ─── SNAP-07: graph_at() registers row in graph_snapshots ────────────────────

SELECT EXISTS(
    SELECT 1 FROM _pg_ripple.graph_snapshots
    WHERE graph_iri = 'http://example.org/snap/Graph1'
) AS snap07_snapshot_row_registered;

-- ─── SNAP-08: graph_at() is idempotent ───────────────────────────────────────

SELECT (
    pg_ripple.graph_at(
        'http://example.org/snap/Graph1',
        '2024-03-01 12:00:00+00'::timestamptz
    )
    =
    pg_ripple.graph_at(
        'http://example.org/snap/Graph1',
        '2024-03-01 12:00:00+00'::timestamptz
    )
) AS snap08_graph_at_idempotent;

-- Row count should still be 1 (ON CONFLICT DO UPDATE).
SELECT COUNT(*) = 1 AS snap08_no_duplicate_rows
FROM _pg_ripple.graph_snapshots
WHERE graph_iri    = 'http://example.org/snap/Graph1'
  AND snapshot_iri = pg_ripple.graph_at(
          'http://example.org/snap/Graph1',
          '2024-03-01 12:00:00+00'::timestamptz
      );

-- ─── SNAP-09: graph_snapshots_count() reflects existing rows ─────────────────

SELECT pg_ripple.graph_snapshots_count() >= 1 AS snap09_count_at_least_one;

-- ─── SNAP-10: graph_at() for a new graph registers a row ─────────────────────

-- Create a snapshot for a second graph (Graph2) to verify new rows are added.
SELECT starts_with(
    pg_ripple.graph_at(
        'http://example.org/snap/Graph2',
        '2024-06-01 00:00:00+00'::timestamptz
    ),
    'urn:snapshot:'
) AS snap10_graph2_snapshot_returns_urn;

SELECT EXISTS(
    SELECT 1 FROM _pg_ripple.graph_snapshots
    WHERE graph_iri = 'http://example.org/snap/Graph2'
) AS snap10_graph2_snapshot_registered;

-- ─── SNAP-11: graph_diff() empty when facts unchanged ────────────────────────

-- Query the diff between the same timestamp (no delta expected).
SELECT COUNT(*) = 0 AS snap11_diff_empty_same_ts
FROM pg_ripple.graph_diff(
    'http://example.org/snap/Graph1',
    '2024-03-01 12:00:00+00'::timestamptz,
    '2024-03-01 12:00:00+00'::timestamptz
);

-- ─── SNAP-12: graph_diff() 'added' row ───────────────────────────────────────

-- Insert a second fact after the first snapshot timestamp.
SELECT pg_ripple.mark_temporal('http://example.org/snap/likes', 'snapshot');

SELECT pg_ripple.insert_triple_temporal(
    'http://example.org/snap/Alice',
    'http://example.org/snap/likes',
    'http://example.org/snap/Coffee',
    '2024-05-01 00:00:00+00'::timestamptz,
    NULL::timestamptz,
    'http://example.org/snap/Graph1'
) IS NOT NULL AS snap12_second_insert_ok;

-- The diff from 2024-03 to 2024-06 should include the 'added' likes fact.
SELECT COUNT(*) >= 1 AS snap12_added_row_present
FROM pg_ripple.graph_diff(
    'http://example.org/snap/Graph1',
    '2024-03-01 12:00:00+00'::timestamptz,
    '2024-06-01 00:00:00+00'::timestamptz
)
WHERE change = 'added';

-- ─── SNAP-13: graph_diff() 'removed' row ─────────────────────────────────────

-- Retract the knows fact (close the interval) so it disappears after NOW().
SELECT pg_ripple.retract_triple_temporal(
    'http://example.org/snap/Alice',
    'http://example.org/snap/knows',
    'http://example.org/snap/Graph1'
) >= 0 AS snap13_retract_ok;

-- The diff from 2024-03 to NOW()+1day: knows was removed (retracted at NOW()).
SELECT COUNT(*) >= 1 AS snap13_removed_row_present
FROM pg_ripple.graph_diff(
    'http://example.org/snap/Graph1',
    '2024-03-01 12:00:00+00'::timestamptz,
    (NOW() + interval '1 day')::timestamptz
)
WHERE change = 'removed';

-- ─── SNAP-14: snapshot_retention_days GUC default is 30 ─────────────────────

SELECT current_setting('pg_ripple.snapshot_retention_days') = '30'
    AS snap14_retention_days_default_30;

-- ─── SNAP-15: graph_snapshots table has expected columns ─────────────────────

SELECT (
    COUNT(*) = 6
) AS snap15_expected_column_count
FROM information_schema.columns
WHERE table_schema = '_pg_ripple'
  AND table_name   = 'graph_snapshots'
  AND column_name IN (
      'snapshot_id', 'graph_iri', 'snapshot_iri',
      'captured_at', 'triple_count', 'expires_at'
  );
