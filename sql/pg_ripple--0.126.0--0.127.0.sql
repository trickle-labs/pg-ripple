-- pg_ripple upgrade: 0.126.0 -> 0.127.0
-- v0.127.0: pg_tide relay migration and CDC bridge canonicalization
--
-- This release moves the CDC bridge's relay transport model from the old
-- pg_trickle table-outbox assumption to pg_tide named outboxes.
--
-- New SQL-visible function provided by compiled Rust:
--   pg_ripple.relay_available() -> boolean
--
-- Compatibility retained:
--   pg_ripple.trickle_available() remains as a deprecated alias for relay
--   availability. pg_ripple.pg_trickle_available() remains the IVM check.

-- Add the canonical pg_tide outbox-name column while preserving the old
-- outbox_table compatibility column for existing callers and catalog queries.
ALTER TABLE _pg_ripple.cdc_bridge_triggers
    ADD COLUMN IF NOT EXISTS outbox_name TEXT;

UPDATE _pg_ripple.cdc_bridge_triggers
SET outbox_name = outbox_table
WHERE outbox_name IS NULL;

ALTER TABLE _pg_ripple.cdc_bridge_triggers
    ALTER COLUMN outbox_name SET NOT NULL;

-- Replace the old dynamic table INSERT trigger function with a pg_tide publish
-- function. PL/pgSQL resolves tide.outbox_publish at execution time, so the
-- extension can still be upgraded in databases where pg_tide is installed later.
CREATE OR REPLACE FUNCTION _pg_ripple.cdc_bridge_trigger_fn()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE
    pred_id     BIGINT := TG_ARGV[0]::bigint;
    outbox_name TEXT   := TG_ARGV[1];
    s_iri       TEXT;
    p_iri       TEXT;
    o_iri       TEXT;
    payload     JSONB;
    headers     JSONB;
    dedup_key   TEXT;
    sid         BIGINT;
BEGIN
    SELECT value INTO s_iri FROM _pg_ripple.dictionary WHERE id = NEW.s;
    SELECT value INTO p_iri FROM _pg_ripple.dictionary WHERE id = pred_id;
    SELECT value INTO o_iri FROM _pg_ripple.dictionary WHERE id = NEW.o;

    sid := NEW.i;
    dedup_key := 'ripple:' || sid::text;

    payload := jsonb_build_object(
        '@context',   'https://schema.org/',
        '@id',        COALESCE(s_iri, '_:' || NEW.s::text),
        p_iri,        COALESCE(o_iri, NEW.o::text)
    );

    headers := jsonb_build_object(
        'event_id',     dedup_key,
        'dedup_key',    dedup_key,
        'event_type',   'pg_ripple.triple.insert',
        'predicate_id', pred_id,
        'statement_id', sid,
        'graph_id',     NEW.g
    );

    PERFORM tide.outbox_publish(outbox_name, payload, headers);
    RETURN NEW;
END;
$$;

-- Update extension version metadata only; pgrx supplies Rust-backed SQL
-- functions during CREATE/ALTER EXTENSION.
