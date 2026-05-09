-- Migration 0.100.0 → 0.101.0: Natural Language Explanation of Derived Facts
--
-- New in v0.101.0 (NL-EXPLAIN-01):
--
-- * _pg_ripple.explanation_cache — caches NL explanations keyed by
--   (sid, format, model) with TTL-based expiry.
--
-- * pg_ripple.explain_inference(subject TEXT, predicate TEXT, object TEXT,
--     format TEXT DEFAULT 'text') → TEXT
--   — returns a plain-English explanation of why a Datalog fact was derived.
--   Uses the configured LLM endpoint (pg_ripple.llm_endpoint) when available;
--   falls back to a deterministic indented-text renderer when not configured
--   or when the endpoint is unreachable.  Returns NULL for base facts.
--
-- * pg_ripple.explain_inference_jsonb(subject, predicate, object) → JSONB
--   — returns {"proof_tree": ..., "narrative": "..."}.
--
-- * pg_ripple.vacuum_explanation_cache() → BIGINT
--   — removes expired explanation_cache rows; returns the count deleted.
--
-- * pg_ripple.explanation_cache_ttl GUC (INT, default 3600 seconds)
--   — controls cache TTL; 0 disables caching.
--
-- * Function rename: explain_inference(text, text, text, text) RETURNS SETOF
--   was renamed to explain_inference_provenance in v0.101.0 to free the name
--   for the new NL explanation function.
--
-- Note: The new Rust functions (explain_inference, explain_inference_jsonb,
-- vacuum_explanation_cache, explain_inference_provenance) are registered
-- automatically by the updated shared library on CREATE EXTENSION or when
-- the library is reloaded after ALTER EXTENSION UPDATE.

-- 1. Drop the old explain_inference (v0.61.0 TABLE-returning) to make room
--    for the new NL scalar version.
DROP FUNCTION IF EXISTS pg_ripple.explain_inference(text, text, text, text);

-- 2. Create the NL explanation cache table.
CREATE TABLE IF NOT EXISTS _pg_ripple.explanation_cache (
    sid         BIGINT      NOT NULL,
    format      TEXT        NOT NULL DEFAULT 'text',
    model       TEXT        NOT NULL DEFAULT '',
    explanation TEXT        NOT NULL,
    cached_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (sid, format, model)
);
CREATE INDEX IF NOT EXISTS idx_explanation_cache_cached_at
    ON _pg_ripple.explanation_cache (cached_at);
COMMENT ON TABLE _pg_ripple.explanation_cache IS
    'NL explanation cache for Datalog-inferred facts (v0.101.0). '
    'One row per (sid, format, model). TTL controlled by '
    'pg_ripple.explanation_cache_ttl (default 3600 seconds). '
    'Vacuum with pg_ripple.vacuum_explanation_cache().';
