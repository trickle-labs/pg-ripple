-- Migration 0.122.0 → 0.123.0: A17 observability, documentation & advisory management
--
-- New SQL objects:
--   pg_ripple.bench_workload_result(profile TEXT DEFAULT 'bsbm')
--     Convenience wrapper that returns the most recent benchmark run for the
--     given profile from _pg_ripple.bench_history.  Eliminates the need to
--     write raw SQL after calling pg_ripple.bench_workload().
--
-- Other changes (Rust compiled-in, no SQL DDL required):
--   OBS-M-01: pg_ripple_http_replica_pool_size / pg_ripple_http_replica_pool_available Prometheus gauges
--   OBS-M-02: pg_ripple_rule_library_stream_duration_seconds histogram + pg_ripple_rule_library_subscribe_errors_total counter
--   SEC-M-01: RSA advisory expiry extended to 2027-01-01 with Q3-2026 re-evaluation scheduled
--   SEC-M-02: RUSTSEC-2026-0104 paste advisory mitigation rationale documented in audit.toml

-- ERG-L-01: bench_workload_result convenience SQL wrapper
CREATE OR REPLACE FUNCTION pg_ripple.bench_workload_result(
    profile TEXT DEFAULT 'bsbm'
) RETURNS TABLE (
    run_id              BIGINT,
    profile             TEXT,
    started_at          TIMESTAMPTZ,
    duration_ms         BIGINT,
    queries_per_second  FLOAT8,
    triples_processed   BIGINT
)
LANGUAGE SQL
STABLE
SECURITY INVOKER
SET search_path = pg_ripple, _pg_ripple, public
AS $$
    SELECT
        h.run_id,
        h.profile,
        h.started_at,
        h.duration_ms,
        h.queries_per_second,
        h.triples_processed
    FROM _pg_ripple.bench_history h
    WHERE h.profile = bench_workload_result.profile
    ORDER BY h.started_at DESC
    LIMIT 1;
$$;

COMMENT ON FUNCTION pg_ripple.bench_workload_result(TEXT) IS
    'Returns the most recent benchmark run for the given profile from '
    '_pg_ripple.bench_history. Convenience wrapper for pg_ripple.bench_workload(). '
    'Added in v0.123.0 (ERG-L-01).';
