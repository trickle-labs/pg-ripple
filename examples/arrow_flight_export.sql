-- examples/arrow_flight_export.sql
-- L15-02 (v0.97.0): Arrow Flight bulk-export demonstration.
--
-- This example demonstrates how to export pg_ripple triple data in bulk using
-- the Arrow Flight IPC protocol via pg_ripple_http. The Arrow endpoint streams
-- rows as Apache Arrow record batches, which is significantly faster than JSON
-- for large datasets (typically 3-5× throughput improvement).
--
-- Prerequisites:
--   - pg_ripple extension installed and initialised
--   - pg_ripple_http companion service running (see docs/src/operations/deployment.md)
--   - Arrow Flight client library (e.g. pyarrow, Apache Arrow Java)
--
-- Usage:
--   psql -f examples/arrow_flight_export.sql

-- ── 1. Load some sample data ──────────────────────────────────────────────────

SELECT pg_ripple.load_turtle($ttl$
  @prefix ex: <https://example.org/> .
  @prefix foaf: <http://xmlns.com/foaf/0.1/> .

  ex:Alice foaf:name "Alice Smith" ;
           foaf:knows ex:Bob, ex:Carol .
  ex:Bob   foaf:name "Bob Jones" ;
           foaf:age  42 .
  ex:Carol foaf:name "Carol Davis" .
$ttl$);

-- ── 2. Preview the data with a SPARQL query ───────────────────────────────────

SELECT * FROM pg_ripple.sparql($q$
  SELECT ?person ?name WHERE {
    ?person <http://xmlns.com/foaf/0.1/name> ?name .
  }
  ORDER BY ?name
$q$);

-- ── 3. Estimate Arrow Flight export size ─────────────────────────────────────
-- The Arrow Flight endpoint uses EXPLAIN (FORMAT JSON) to estimate row count
-- before streaming, enabling clients to pre-allocate buffers (M15-22, v0.96.0).

EXPLAIN (FORMAT JSON)
SELECT s, p, o, g
FROM _pg_ripple.vp_rare
LIMIT 1000;

-- ── 4. Arrow Flight authentication token ─────────────────────────────────────
-- To authenticate with pg_ripple_http, generate a token via:
--
--   curl -X POST http://localhost:8080/api/v1/token \
--     -H 'Content-Type: application/json' \
--     -d '{"username":"ripple_admin","password":"<password>"}'
--
-- Then use the token as a Bearer header in Arrow Flight requests:
--
--   curl -H 'Authorization: Bearer <token>' \
--     'http://localhost:8080/api/v1/arrow/export?graph=default&format=ipc'
--
-- The export endpoint streams Arrow IPC format (content-type: application/vnd.apache.arrow.stream).

-- ── 5. Export statistics ─────────────────────────────────────────────────────

SELECT
  (SELECT count(*) FROM _pg_ripple.vp_rare)          AS rare_triples,
  (SELECT count(*) FROM _pg_ripple.predicates)        AS predicate_count,
  pg_ripple.triple_count()                            AS total_triples;
