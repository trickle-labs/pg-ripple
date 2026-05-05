-- Migration 0.94.0 → 0.95.0: A15 Medium — Correctness, Security, Storage
--
-- Schema changes in this release:
--
-- M15-03: sql_drop event trigger for DROP EXTENSION pg_ripple replication-slot cleanup.
--         _pg_ripple.cleanup_on_drop() function + _pg_ripple_cleanup_on_drop event trigger.
--
-- M15-07: autovacuum_scale_factor reloptions on _pg_ripple.dictionary for more
--         aggressive autovacuum on this high-churn table.
--
-- M15-10: _pg_ripple.schema_generation_seq sequence for plan cache invalidation.
--         Included in SPARQL plan cache keys; bumped on every VP table creation
--         and predicate promotion so stale plans are never reused.
--
-- Pure Rust function changes (no SQL required):
--   M15-01: Replace unreachable!() in pagerank/export.rs and centrality.rs with pgrx::error!().
--   M15-02: Resolve-once DNS rebinding fix in federation/policy.rs.
--   M15-04: redacted_error() for SSE initialisation error paths in stream.rs.
--   M15-07: pg_ripple.dict_vacuum_threshold GUC (new GUC, no DDL required).
--   M15-08: New regression test sparql_optional_path_in_graph_rare.sql.
--   M15-09: Explicit NaN/Inf rejection in load_triples_with_confidence().
--   M15-12: ADD/COPY/MOVE SPARQL Update operations now flush mutation journal and write audit log.

-- M15-10: schema_generation_seq sequence.
CREATE SEQUENCE IF NOT EXISTS _pg_ripple.schema_generation_seq
    START 1 INCREMENT 1 NO CYCLE;
COMMENT ON SEQUENCE _pg_ripple.schema_generation_seq IS
    'Monotonic counter bumped on every VP table schema change (v0.95.0 M15-10).';

-- M15-07: tune autovacuum on the dictionary table for more aggressive vacuuming.
ALTER TABLE _pg_ripple.dictionary
    SET (
        autovacuum_scale_factor         = 0.01,
        autovacuum_analyze_scale_factor = 0.005
    );

-- M15-03: event trigger to clean up CDC replication slots on DROP EXTENSION.
CREATE OR REPLACE FUNCTION _pg_ripple.cleanup_on_drop()
    RETURNS event_trigger
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, _pg_ripple, public
AS $$
DECLARE
    _rec record;
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_event_trigger_dropped_objects()
        WHERE object_type = 'extension'
          AND object_name = 'pg_ripple'
    ) THEN
        RETURN;
    END IF;
    FOR _rec IN
        SELECT slot_name
        FROM pg_replication_slots
        WHERE plugin = 'pg_ripple'
           OR slot_name LIKE 'pg_ripple%'
    LOOP
        BEGIN
            PERFORM pg_drop_replication_slot(_rec.slot_name);
            RAISE NOTICE 'pg_ripple: dropped replication slot % on extension drop', _rec.slot_name;
        EXCEPTION WHEN OTHERS THEN
            RAISE WARNING 'pg_ripple: could not drop replication slot %: %',
                _rec.slot_name, SQLERRM;
        END;
    END LOOP;
END;
$$;

COMMENT ON FUNCTION _pg_ripple.cleanup_on_drop() IS
    'Event trigger: drops pg_ripple CDC replication slots when the extension is uninstalled (M15-03 v0.95.0).';

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_event_trigger WHERE evtname = '_pg_ripple_cleanup_on_drop'
    ) THEN
        EXECUTE $et$
            CREATE EVENT TRIGGER _pg_ripple_cleanup_on_drop
                ON sql_drop
                EXECUTE FUNCTION _pg_ripple.cleanup_on_drop()
        $et$;
    END IF;
END;
$$;
