-- Migration 0.127.0 → 0.128.0: JSON mapping relational writeback (JSON-LD reverse mapping)
--
-- New features:
--   JSON-WRITEBACK-01: relational write-back path for register_json_mapping()
--     - writeback_json_row(mapping, subject_iri)        → BIGINT
--     - writeback_json_row_delete(mapping, subject_iri) → BIGINT
--     - enable_json_writeback(mapping)                  → VOID
--     - disable_json_writeback(mapping)                 → VOID
--     - json_writeback_status()                         → TABLE(…)
--     - pg_ripple.json_writeback_batch_size GUC (default 100)
--   _pg_ripple.json_writeback_queue catalog table
--   Five new columns on _pg_ripple.json_mappings

-- Extend json_mappings with writeback configuration columns.
ALTER TABLE _pg_ripple.json_mappings
    ADD COLUMN IF NOT EXISTS writeback_table        TEXT,
    ADD COLUMN IF NOT EXISTS writeback_schema       TEXT    NOT NULL DEFAULT 'public',
    ADD COLUMN IF NOT EXISTS writeback_key_columns  TEXT[]  NOT NULL DEFAULT '{}',
    ADD COLUMN IF NOT EXISTS writeback_conflict_policy TEXT NOT NULL DEFAULT 'replace'
        CHECK (writeback_conflict_policy IN ('replace', 'skip', 'error')),
    ADD COLUMN IF NOT EXISTS writeback_enabled      BOOLEAN NOT NULL DEFAULT false;

-- Queue table for asynchronous VP-trigger-based writeback.
CREATE TABLE IF NOT EXISTS _pg_ripple.json_writeback_queue (
    id            BIGSERIAL PRIMARY KEY,
    mapping_name  TEXT      NOT NULL
        REFERENCES _pg_ripple.json_mappings(name) ON DELETE CASCADE,
    subject_id    BIGINT    NOT NULL,
    operation     TEXT      NOT NULL CHECK (operation IN ('upsert', 'delete')),
    queued_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    processed_at  TIMESTAMPTZ,
    error         TEXT
);

CREATE INDEX IF NOT EXISTS json_writeback_queue_pending_idx
    ON _pg_ripple.json_writeback_queue (mapping_name, queued_at)
    WHERE processed_at IS NULL;

COMMENT ON TABLE _pg_ripple.json_writeback_queue IS
    'Async queue for JSON-mapping relational writeback events (v0.128.0 JSON-WRITEBACK-01)';
