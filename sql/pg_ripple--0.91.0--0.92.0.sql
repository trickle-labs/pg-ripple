-- Migration 0.91.0 → 0.92.0
-- Assessment 14 Low-severity polish & hardening
--
-- Behaviour changes (no DDL schema changes):
--   - pg_ripple._fuzzy_match_guard() and pg_ripple._token_set_ratio_guard() are now
--     declared STABLE (was: VOLATILE). Allows the planner to cache results within
--     a query and hoist calls out of inner loops. (PERF-08, v0.92.0)
--   - pg_ripple.pagerank_find_duplicates() is now declared STABLE (was: VOLATILE).
--     Allows the planner to hoist it out of joins for multi-tenant pruning. (SEC-08)
--   - pg_ripple.pagerank_partition GUC default changed from false to true. When
--     enabled, partition count is auto-tuned to min(num_cpus, named_graph_count).
--     (PERF-07, v0.92.0)
--   - EXPLAIN format 'algebra_optimized' (en_US) accepted as alias for
--     'algebra_optimised' (en_GB). Both spellings produce identical output. (OBS-04)
--   - pg_ripple.diagnostic_report() extended with v0.87/v0.88 catalog rows:
--     confidence_row_count, pagerank_last_computed, pagerank_queue_depth,
--     centrality_metrics. (OBS-05)
--   - pg_ripple_http: PG_RIPPLE_HTTP_SHUTDOWN_TIMEOUT_SECS env var configures
--     graceful shutdown drain timeout (default 30s). (HTTP-05)
--   - CDC notify: payloads > 8000 bytes now raise PT5001 WARNING instead of
--     silently failing. (CDC-03)
--   - cargo-audit CI now runs with --deny unmaintained. (SEC-09)
--
-- DDL changes:
--   - Add RLS policy on _pg_ripple.pagerank_dirty_edges for graph isolation. (SEC-07)

-- SEC-07: Enable RLS on pagerank_dirty_edges if not already enabled.
DO $$
BEGIN
    -- Enable RLS (idempotent — no error if already enabled).
    EXECUTE 'ALTER TABLE _pg_ripple.pagerank_dirty_edges ENABLE ROW LEVEL SECURITY';

    -- Create the graph isolation policy if it doesn't exist.
    -- Uses the same pattern as _pg_ripple.confidence (v0.87.0).
    IF NOT EXISTS (
        SELECT 1 FROM pg_policies
        WHERE schemaname = '_pg_ripple'
          AND tablename  = 'pagerank_dirty_edges'
          AND policyname = 'pagerank_dirty_edges_graph_isolation'
    ) THEN
        EXECUTE $policy$
            CREATE POLICY pagerank_dirty_edges_graph_isolation
              ON _pg_ripple.pagerank_dirty_edges
              USING (
                graph_id = current_setting('pg_ripple.current_graph', true)::bigint
                OR current_setting('pg_ripple.current_graph', true) IS NULL
                OR current_setting('pg_ripple.current_graph', true) = ''
              )
        $policy$;
    END IF;
EXCEPTION
    WHEN OTHERS THEN
        -- Warn but don't abort: RLS setup is best-effort during migration.
        RAISE WARNING 'v0.92.0 migration: could not enable RLS on pagerank_dirty_edges: %', SQLERRM;
END $$;
