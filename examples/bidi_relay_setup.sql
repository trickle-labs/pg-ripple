-- examples/bidi_relay_setup.sql
-- L15-02 (v0.97.0): Bidirectional CDC relay configuration demonstration.
--
-- This example demonstrates how to configure, monitor, and manage pg_ripple's
-- bidirectional Change Data Capture (CDC) relay for synchronising knowledge
-- graphs across PostgreSQL instances using pg_tide relay pipelines.
--
-- Prerequisites:
--   - pg_ripple extension installed on both source and target instances
--   - pg_tide relay setup (see docs/src/operations/bidi-production-checklist.md)
--   - Logical replication enabled: wal_level = logical
--
-- Usage:
--   psql -f examples/bidi_relay_setup.sql

-- ── 1. Check pg_tide relay prerequisites ─────────────────────────────────────

-- Relay transport requires pg_tide. pg_trickle is still used separately for
-- IVM-backed views, not for relay transport.
SELECT pg_ripple.relay_available() AS relay_available;

-- Verify logical replication is available
SHOW wal_level;  -- should be 'logical'

-- Check existing replication slots
SELECT slot_name, plugin, active, restart_lsn
FROM pg_replication_slots
WHERE plugin = 'pgoutput';

-- ── 2. Start the bidirectional relay ─────────────────────────────────────────
-- bidi_relay_start() configures CDC subscriptions and starts the relay worker.
-- Parameters:
--   source_dsn:  libpq connection string to the source database
--   target_dsn:  libpq connection string to the target database
--   graph_filter: optional named graph IRI to replicate (NULL = replicate all)

SELECT pg_ripple.bidi_relay_start(
  'host=source-db.example.com port=5432 dbname=knowledge user=ripple_repl',
  'host=target-db.example.com port=5432 dbname=knowledge user=ripple_repl',
  NULL  -- replicate all named graphs
);

-- ── 3. Monitor relay status ───────────────────────────────────────────────────

SELECT
  relay_id,
  source_host,
  target_host,
  status,
  triples_replicated,
  lag_bytes,
  last_checkpoint_lsn,
  last_heartbeat_at
FROM pg_ripple.bidi_relay_status();

-- ── 4. Monitor Prometheus metrics ────────────────────────────────────────────
-- The pg_ripple_http companion service exposes these metrics at /metrics:
--
--   pg_ripple_cdc_replication_slot_lag_bytes  — replication slot lag
--   pg_ripple_bidi_relay_triples_replicated   — cumulative triple count
--
-- Example Prometheus query:
--   rate(pg_ripple_bidi_relay_triples_replicated[5m])

-- ── 5. Pause and resume replication ──────────────────────────────────────────

-- Pause during maintenance window
SELECT pg_ripple.bidi_relay_pause('relay-id-here');

-- Resume after maintenance
SELECT pg_ripple.bidi_relay_resume('relay-id-here');

-- ── 6. Check for replication conflicts ───────────────────────────────────────

SELECT
  conflict_id,
  conflict_type,
  subject_iri,
  predicate_iri,
  occurred_at
FROM pg_ripple.bidi_relay_conflicts()
ORDER BY occurred_at DESC
LIMIT 20;

-- ── 7. Graceful shutdown ──────────────────────────────────────────────────────
-- bidi_relay_stop() drains pending events and closes the replication slot cleanly.

SELECT pg_ripple.bidi_relay_stop('relay-id-here');

-- Verify the relay stopped
SELECT status FROM pg_ripple.bidi_relay_status()
WHERE relay_id = 'relay-id-here';
