-- Migration 0.118.0 → 0.119.0
-- Features: owl:propertyChainAxiom inference, SERVICE circuit breaker table,
--           schema-aware NL→SPARQL (nl_sparql_include_bundles GUC),
--           property paths over RDF-star gap fix.

-- Feature 6: Persistent federation circuit breaker state table.
-- Tracks per-endpoint circuit state across backend restarts for observability.
CREATE TABLE IF NOT EXISTS _pg_ripple.federation_circuit_state (
    endpoint_iri  TEXT PRIMARY KEY,
    state         TEXT NOT NULL DEFAULT 'closed'
                      CHECK (state IN ('closed', 'open', 'half_open')),
    last_failure_at TIMESTAMPTZ,
    failure_count   INT NOT NULL DEFAULT 0
);

COMMENT ON TABLE _pg_ripple.federation_circuit_state IS
    'Per-endpoint federation circuit breaker state (Feature 6, v0.119.0). '
    'Populated/updated by pg_ripple_federation_circuit_sync() on each SPI '
    'connection. Used for Prometheus gauge pg_ripple_federation_circuit_state.';
