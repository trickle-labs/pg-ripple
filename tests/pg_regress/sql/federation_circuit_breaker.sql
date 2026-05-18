-- pg_regress test: federation circuit breaker state table (Feature 6, v0.119.0)

CREATE EXTENSION IF NOT EXISTS pg_ripple;
SELECT pg_ripple.triple_count() >= 0 AS extension_loaded;
SET search_path TO pg_ripple, public;

-- 1. Verify the federation_circuit_state table exists.
SELECT COUNT(*) >= 0 AS circuit_state_table_exists
FROM _pg_ripple.federation_circuit_state;

-- 2. Verify the table schema has the expected columns.
SELECT COUNT(*) = 4 AS circuit_state_columns_ok
FROM information_schema.columns
WHERE table_schema = '_pg_ripple'
  AND table_name = 'federation_circuit_state'
  AND column_name IN ('endpoint_iri', 'state', 'last_failure_at', 'failure_count');

-- 3. Insert a test circuit state row.
INSERT INTO _pg_ripple.federation_circuit_state (endpoint_iri, state, failure_count)
VALUES ('https://endpoint.test/sparql', 'closed', 0)
ON CONFLICT (endpoint_iri) DO UPDATE SET state = 'closed', failure_count = 0;

SELECT state = 'closed' AS circuit_starts_closed
FROM _pg_ripple.federation_circuit_state
WHERE endpoint_iri = 'https://endpoint.test/sparql';

-- 4. Simulate circuit tripping to 'open'.
UPDATE _pg_ripple.federation_circuit_state
SET state = 'open', failure_count = 5, last_failure_at = now()
WHERE endpoint_iri = 'https://endpoint.test/sparql';

SELECT state = 'open' AND failure_count = 5 AS circuit_tripped_to_open
FROM _pg_ripple.federation_circuit_state
WHERE endpoint_iri = 'https://endpoint.test/sparql';

-- 5. Simulate half-open recovery.
UPDATE _pg_ripple.federation_circuit_state
SET state = 'half_open'
WHERE endpoint_iri = 'https://endpoint.test/sparql';

SELECT state = 'half_open' AS circuit_in_half_open
FROM _pg_ripple.federation_circuit_state
WHERE endpoint_iri = 'https://endpoint.test/sparql';

-- 6. Recovery: close the circuit on success.
UPDATE _pg_ripple.federation_circuit_state
SET state = 'closed', failure_count = 0
WHERE endpoint_iri = 'https://endpoint.test/sparql';

SELECT state = 'closed' AND failure_count = 0 AS circuit_recovered
FROM _pg_ripple.federation_circuit_state
WHERE endpoint_iri = 'https://endpoint.test/sparql';

-- 7. Verify CHECK constraint rejects invalid state values.
DO $$
BEGIN
    BEGIN
        INSERT INTO _pg_ripple.federation_circuit_state (endpoint_iri, state, failure_count)
        VALUES ('https://bad.test/sparql', 'invalid_state', 0);
        RAISE EXCEPTION 'expected constraint violation not raised';
    EXCEPTION WHEN check_violation THEN
        -- expected
    END;
END $$;
SELECT true AS invalid_state_rejected;

-- 8. Multiple endpoints can coexist.
INSERT INTO _pg_ripple.federation_circuit_state (endpoint_iri, state, failure_count)
VALUES ('https://endpoint2.test/sparql', 'open', 3)
ON CONFLICT (endpoint_iri) DO UPDATE SET state = 'open', failure_count = 3;

SELECT COUNT(*) >= 2 AS multiple_endpoints_stored
FROM _pg_ripple.federation_circuit_state;

-- Cleanup.
DELETE FROM _pg_ripple.federation_circuit_state
WHERE endpoint_iri IN ('https://endpoint.test/sparql', 'https://endpoint2.test/sparql');
SELECT true AS cleanup_done;
