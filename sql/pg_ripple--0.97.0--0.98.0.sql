-- Migration: pg_ripple 0.97.0 → 0.98.0
-- v0.98.0 — SKOS Support, Named Bundle API & Graph Intelligence
--
-- Schema changes:
--   CREATE TABLE _pg_ripple.datalog_bundles (named Datalog bundle catalog)
--   CREATE TABLE pg_ripple.federation_endpoints (user-facing trust registry)
--   CREATE VIEW  pg_ripple.active_datalog_bundles
--
-- New Rust-side features:
--   pg_ripple.load_builtin_rules('skos')           — 28 SKOS entailment rules (S7–S45)
--   pg_ripple.load_builtin_rules('skos-transitive') — 7-rule transitive-closure subset
--   pg_ripple.load_builtin_rules('skosxl')          — 3 SKOS-XL dumb-down chains (S55–S57)
--   pg_ripple.load_datalog_bundle(name, graph)      — named versioned bundle activation
--   pg_ripple.load_shape_bundle(name)               — named SHACL shape bundle (implicit deps)
--   pg_ripple.active_datalog_bundles                — view over _pg_ripple.datalog_bundles
--   pg_ripple.validate_skos()                       — integrity report wrapper
--   pg_ripple.skos_ancestors(iri, scheme)           — broaderTransitive closure
--   pg_ripple.skos_descendants(iri, scheme)         — narrowerTransitive closure
--   pg_ripple.skos_label(iri, lang)                 — prefLabel lookup
--   pg_ripple.skos_related(iri)                     — semanticRelation sub-property links
--   pg_ripple.skos_siblings(iri)                    — co-narrower sibling concepts
--   pg_ripple.explain_contradiction(iri, ...)       — greedy/exact contradiction explainer
--   pg_ripple.explain_contradiction_json(iri, ...)  — JSONB variant
--   pg_ripple.coverage_map(graphs, pred, top_k)     — per-topic coverage metrics
--   pg_ripple.refresh_coverage_map(target, graphs)  — write pgc:CoverageMap triples
--   GUC pg_ripple.allow_unregistered_service_endpoints (bool, default off)
--   'skos' and 'skosxl' prefixes auto-registered by load_builtin_rules / load_datalog_bundle

-- ── BUNDLE-01: Named Datalog bundle catalog ───────────────────────────────────
CREATE TABLE IF NOT EXISTS _pg_ripple.datalog_bundles (
    bundle_name    TEXT        NOT NULL,
    bundle_version INT         NOT NULL DEFAULT 1,
    loaded_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    named_graph    TEXT        NOT NULL DEFAULT '',
    PRIMARY KEY (bundle_name, named_graph)
);

-- ── FED-TRUST-01: User-facing federation endpoint trust registry ──────────────
CREATE TABLE IF NOT EXISTS pg_ripple.federation_endpoints (
    name            TEXT        PRIMARY KEY,
    endpoint_url    TEXT        NOT NULL,
    auth_token      TEXT,
    min_confidence  FLOAT4      NOT NULL DEFAULT 0.0
                    CHECK (min_confidence >= 0.0 AND min_confidence <= 1.0),
    timeout_ms      INT         NOT NULL DEFAULT 5000
                    CHECK (timeout_ms > 0),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- ── BUNDLE-02: active_datalog_bundles view ────────────────────────────────────
CREATE OR REPLACE VIEW pg_ripple.active_datalog_bundles AS
SELECT bundle_name, bundle_version, loaded_at, named_graph
FROM _pg_ripple.datalog_bundles
ORDER BY bundle_name, named_graph;
