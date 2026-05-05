-- Migration 0.93.0 → 0.94.0: Assessment 15 High Remediation
--
-- New features in this release (v0.94.0):
--
--   H15-01: `just bump-version X.Y.Z COMPAT_MIN` recipe update — accepts an
--           optional COMPAT_MIN argument to set COMPATIBLE_EXTENSION_MIN
--           independently from the new version.  `check-version-sync` recipe
--           updated to allow COMPAT_MIN ≤ EXT_VER (matching the CI gate).
--           COMPATIBLE_EXTENSION_MIN bumped to v0.93.0.
--
--   H15-02: `SET search_path = pg_catalog, _pg_ripple, public` added to
--           `_pg_ripple.ddl_guard_vp_tables()` (SECDEF event trigger function)
--           to prevent search-path injection attacks.  New CI script
--           `scripts/check_security_definer_search_path.sh` verifies all
--           SECDEF functions in src/ have a pinned search_path.
--
--   H15-03/L15-13: Bounded bidirectional relay channel.
--           `pg_ripple.bidi_relay_max_inflight` GUC (default: 1000) limits
--           concurrent in-flight relay dispatch calls.  When the limit is
--           reached, new relay calls are dropped (drop-oldest policy) and the
--           `pg_ripple_bidi_relay_dropped_total` Prometheus counter is
--           incremented.  Counter exposed via `streaming_metrics()` and
--           `/metrics` endpoint.
--
--   H15-05/M15-20: Bulk loader COPY FROM STDIN BINARY path.
--           `pg_ripple.bulk_load_use_copy` GUC (default: off) switches bulk
--           loaders to use UNNEST-array based batch insertion (equivalent
--           performance to COPY FROM STDIN BINARY for encoded triples).
--           Shared `copy_into_vp()` helper extracted and used by bulk loader,
--           R2RML, and CDC paths when the GUC is on.

-- Schema changes for v0.94.0:
--
-- 1. Recreate _pg_ripple.ddl_guard_vp_tables() with SET search_path clause.
--    This is the only SQL-visible change in v0.94.0; the GUCs and counters
--    are Rust-only and require no SQL schema migration.

CREATE OR REPLACE FUNCTION _pg_ripple.ddl_guard_vp_tables()
    RETURNS event_trigger
    LANGUAGE plpgsql
    SECURITY DEFINER -- SECURITY-JUSTIFY: event trigger needs SECURITY DEFINER to call
    -- pg_event_trigger_dropped_objects(), which requires elevated privilege; the
    -- function only reads the event trigger context and raises a WARNING/ERROR
    -- to protect VP tables from accidental DDL drops outside maintenance mode.
    -- H15-02 (v0.94.0): SET search_path pins name resolution for this SECURITY
    -- DEFINER function to prevent search-path injection.
    SET search_path = pg_catalog, _pg_ripple, public
AS $$
DECLARE
    _obj record;
    _in_maintenance bool;
BEGIN
    -- Skip if we are inside a pg_ripple maintenance operation.
    _in_maintenance := coalesce(
        current_setting('pg_ripple.maintenance_mode', true) = 'on',
        false
    );
    IF _in_maintenance THEN
        RETURN;
    END IF;

    FOR _obj IN
        SELECT schema_name, object_name
        FROM pg_event_trigger_dropped_objects()
        WHERE object_type IN ('table', 'index')
          AND schema_name = '_pg_ripple'
          AND object_name LIKE 'vp_%'
    LOOP
        RAISE WARNING 'PT511: _pg_ripple relation % dropped outside pg_ripple maintenance function; '
                      'run pg_ripple.vacuum() to maintain consistent state', _obj.object_name;
        INSERT INTO _pg_ripple.catalog_events (op, objname, blocked_by_ripple)
        VALUES (tg_tag, _obj.schema_name || '.' || _obj.object_name, false);
    END LOOP;
END;
$$;

INSERT INTO _pg_ripple.schema_version (version, upgraded_from, installed_at)
VALUES ('0.94.0', '0.93.0', clock_timestamp());
