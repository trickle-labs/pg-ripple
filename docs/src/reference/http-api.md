# HTTP API Reference

`pg_ripple_http` is a standalone Rust binary that exposes the full pg_ripple feature set over HTTP. It supports the W3C SPARQL 1.1 Protocol, Server-Sent Events streaming, Datalog REST operations, PageRank, probabilistic reasoning, Arrow Flight bulk export, and administrative endpoints.

Start the service:

```bash
PG_RIPPLE_HTTP_PG_URL=postgresql://localhost/mydb \
PG_RIPPLE_HTTP_PORT=7878 \
PG_RIPPLE_HTTP_AUTH_TOKEN=mysecret \
pg_ripple_http
```

---

## Authentication

All endpoints that mutate state require a Bearer token when `PG_RIPPLE_HTTP_AUTH_TOKEN` is set.

```
Authorization: Bearer <token>
```

Read-only endpoints (`GET /health`, `GET /metrics`, `GET /openapi.yaml`) do not require authentication.

---

## Configuration

| Environment Variable | Default | Description |
|---|---|---|
| `PG_RIPPLE_HTTP_PG_URL` | `postgresql://localhost/postgres` | PostgreSQL connection URL |
| `PG_RIPPLE_HTTP_PORT` | `7878` | TCP listening port |
| `PG_RIPPLE_HTTP_POOL_SIZE` | `16` | PostgreSQL connection pool size |
| `PG_RIPPLE_HTTP_AUTH_TOKEN` | *(none)* | Bearer token; set to enable auth |
| `PG_RIPPLE_HTTP_RATE_LIMIT` | `0` | Per-IP rate limit (req/s; 0 = unlimited) |
| `PG_RIPPLE_HTTP_CORS_ORIGINS` | `*` | Comma-separated allowed CORS origins |
| `PG_RIPPLE_HTTP_MAX_BODY_BYTES` | `10485760` | Max request body size (10 MiB) |
| `PG_RIPPLE_HTTP_SKIP_COMPAT_CHECK` | *(unset)* | Set to `1` to skip extension version check |

---

## Complete Endpoint Reference

| Method | Path | Auth | Description |
|---|---|---|---|
| `GET` | `/sparql` | Optional | SPARQL 1.1 query via URL parameters |
| `POST` | `/sparql` | Optional | SPARQL 1.1 query/update via request body |
| `POST` | `/sparql/stream` | Optional | Streaming SPARQL SELECT (SSE) |
| `POST` | `/rag` | Optional | RAG retrieval / NL-to-SPARQL |
| `GET` | `/health` | None | Liveness probe |
| `GET` | `/ready` | None | Readiness probe (alias) |
| `GET` | `/health/ready` | None | Readiness probe |
| `GET` | `/metrics` | None | Prometheus metrics |
| `GET` | `/metrics/extension` | None | Extension-internal Prometheus metrics |
| `GET` | `/void` | None | VoID dataset description |
| `GET` | `/service` | None | SPARQL service description |
| `GET` | `/openapi.yaml` | None | OpenAPI 3.1 specification |
| `GET` | `/explorer` | None | Web-based SPARQL explorer UI |
| `POST` | `/flight/do_get` | Required | Arrow Flight bulk export |
| `GET` | `/subscribe/{id}` | Optional | SSE subscription stream |
| `GET` | `/datalog/rules` | Optional | List Datalog rule sets |
| `GET/DELETE` | `/datalog/rules/{rule_set}` | Required | Get or drop a rule set |
| `GET` | `/datalog/rules/{rule_set}/builtin` | Optional | Get built-in rules for a rule set |
| `POST` | `/datalog/rules/{rule_set}/add` | Required | Add a rule to a rule set |
| `POST` | `/datalog/rules/{rule_set}/enable` | Required | Enable a rule set |
| `POST` | `/datalog/rules/{rule_set}/disable` | Required | Disable a rule set |
| `DELETE` | `/datalog/rules/{rule_set}/{rule_id}` | Required | Delete a specific rule |
| `POST` | `/datalog/infer/{rule_set}` | Required | Run forward-chaining inference |
| `POST` | `/datalog/infer/{rule_set}/agg` | Required | Run inference with aggregation |
| `POST` | `/datalog/infer/{rule_set}/wfs` | Required | Run well-founded semantics inference |
| `POST` | `/datalog/infer/{rule_set}/demand` | Required | Goal-directed demand inference |
| `POST` | `/datalog/infer/{rule_set}/lattice` | Required | Lattice-based inference |
| `GET` | `/datalog/infer/{rule_set}/stats` | Optional | Inference stats for rule set |
| `POST` | `/datalog/query/{rule_set}` | Optional | Goal-directed Datalog query |
| `GET` | `/datalog/constraints` | Optional | Check all constraint rules |
| `GET` | `/datalog/constraints/{rule_set}` | Optional | Check constraints for one rule set |
| `GET` | `/datalog/stats/cache` | Optional | Rule plan cache statistics |
| `GET` | `/datalog/stats/tabling` | Optional | Tabling cache statistics |
| `GET` | `/datalog/lattices` | Optional | List active lattice structures |
| `GET` | `/datalog/views` | Optional | List Datalog-backed views |
| `DELETE` | `/datalog/views/{name}` | Required | Drop a Datalog-backed view |
| `POST` | `/pagerank/run` | Required | Start PageRank computation |
| `GET` | `/pagerank/status` | Optional | PageRank computation status |
| `GET` | `/pagerank/results` | Optional | Retrieve PageRank scores |
| `GET` | `/pagerank/export` | Optional | Export PageRank scores |
| `GET` | `/pagerank/explain/{node_iri}` | Optional | Explain PageRank score for a node |
| `GET` | `/pagerank/queue-stats` | Optional | PageRank queue statistics |
| `POST` | `/pagerank/vacuum-dirty` | Required | Vacuum stale PageRank rows |
| `POST` | `/centrality/run` | Required | Compute centrality metrics |
| `GET` | `/centrality/results` | Optional | Retrieve centrality results |
| `POST` | `/pagerank/find-duplicates` | Optional | Find near-duplicate nodes |
| `POST` | `/confidence/load` | Required | Load triples with confidence scores |
| `GET` | `/confidence/shacl-score` | Optional | Get SHACL soft-validation scores |
| `GET` | `/confidence/shacl-report` | Optional | Get scored SHACL validation report |
| `POST` | `/confidence/vacuum` | Required | Vacuum stale confidence rows |

---

## SPARQL Endpoints

### `GET /sparql`

W3C SPARQL 1.1 Protocol query endpoint.

**Query Parameters:**

| Parameter | Required | Description |
|---|---|---|
| `query` | Yes (query) | URL-encoded SPARQL SELECT/CONSTRUCT/ASK/DESCRIBE query |
| `update` | Yes (update) | URL-encoded SPARQL UPDATE |
| `default-graph-uri` | No | Default graph URI |
| `named-graph-uri` | No | Named graph URI (repeatable) |

**Accept header** controls the result format:

| Accept | Format |
|---|---|
| `application/sparql-results+json` | SPARQL JSON (default) |
| `application/sparql-results+xml` | SPARQL XML |
| `text/turtle` | Turtle (for CONSTRUCT/DESCRIBE) |
| `application/n-triples` | N-Triples (for CONSTRUCT/DESCRIBE) |
| `application/ld+json` | JSON-LD (for CONSTRUCT/DESCRIBE) |

**Example:**

```bash
curl -G "http://localhost:7878/sparql" \
  --data-urlencode 'query=SELECT ?s ?p ?o WHERE { ?s ?p ?o } LIMIT 10' \
  -H "Accept: application/sparql-results+json"
```

---

### `POST /sparql`

Accepts the query either as an `application/x-www-form-urlencoded` body (form post) or as a raw `application/sparql-query` / `application/sparql-update` body.

**Form body parameters:** same as `GET /sparql` query parameters.

**Example (SELECT):**

```bash
curl -X POST http://localhost:7878/sparql \
  -H "Content-Type: application/sparql-query" \
  -d 'SELECT ?label WHERE { <https://example.org/Alice> rdfs:label ?label }'
```

**Example (UPDATE, requires auth):**

```bash
curl -X POST http://localhost:7878/sparql \
  -H "Content-Type: application/sparql-update" \
  -H "Authorization: Bearer $TOKEN" \
  -d 'INSERT DATA { <:Alice> rdfs:label "Alice" }'
```

---

### `POST /sparql/stream`

Streaming SPARQL SELECT using Server-Sent Events. Results are delivered as SSE `data:` events, one result row per event, as the query executes.

**Request:** Same as `POST /sparql`.

**Response Content-Type:** `text/event-stream`

**Event format:**

```
event: row
data: {"?name": "\"Alice\"^^xsd:string", "?age": "\"30\"^^xsd:integer"}

event: done
data: {"total_rows": 42}
```

**Example:**

```bash
curl -X POST http://localhost:7878/sparql/stream \
  -H "Content-Type: application/sparql-query" \
  -H "Accept: text/event-stream" \
  -d 'SELECT ?s ?p ?o WHERE { ?s ?p ?o }' \
  --no-buffer
```

---

## RAG Endpoint

### `POST /rag`

Execute a hybrid SPARQL + vector-similarity RAG retrieval query. The endpoint embeds the question, finds the nearest RDF entities by cosine similarity, and returns structured results with a plain-text context string suitable for use as an LLM prompt.

**Content-Type:** `application/json`

**Request body:**

```json
{
  "question": "what treats headaches?",
  "sparql_filter": "?entity a <https://pharma.example/Drug>",
  "k": 5,
  "model": "text-embedding-3-small",
  "output_format": "jsonb"
}
```

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `question` | string | **yes** | — | Natural-language question |
| `sparql_filter` | string | no | `null` | SPARQL WHERE fragment to filter candidates |
| `k` | integer | no | `5` | Number of nearest neighbors |
| `model` | string | no | *(GUC)* | Override `pg_ripple.embedding_model` |
| `output_format` | string | no | `"jsonb"` | `"jsonb"` or `"jsonld"` |

**Response (200 OK):**

```json
{
  "results": [
    {
      "entity_iri": "https://pharma.example/aspirin",
      "label": "aspirin",
      "context_json": {
        "label": "aspirin",
        "types": ["Drug", "NSAID"],
        "properties": [{"predicate": "treats", "object": "headache"}],
        "contextText": "aspirin. Type: NSAID, Drug."
      },
      "distance": 0.12
    }
  ],
  "context": "aspirin. Type: NSAID, Drug.\n\nibuprofen. Type: Drug."
}
```

The `context` field is a concatenated plain-text summary ready for use as an LLM system prompt.

---

## Arrow Flight Bulk Export

### `POST /flight/do_get`

Bulk-export SPARQL query results as an [Apache Arrow IPC](https://arrow.apache.org/docs/format/Columnar.html) record stream. Suitable for high-throughput extract pipelines (DuckDB, pandas, Spark).

**Content-Type:** `application/json`

**Request body:**

```json
{
  "ticket": "eyJhbGciOiJIUzI1NiJ9...",
  "query": "SELECT ?s ?p ?o WHERE { ?s ?p ?o }"
}
```

| Field | Type | Required | Description |
|---|---|---|---|
| `ticket` | string | **yes** | HMAC-signed JWT ticket issued by `pg_ripple.arrow_flight_ticket(query)` |
| `query` | string | no | Inline query (only when `pg_ripple.arrow_unsigned_tickets_allowed = on`) |

**Response:** `application/vnd.apache.arrow.stream` — Arrow IPC stream.

**Example (Python):**

```python
import requests, pyarrow as pa

# 1. Get a signed ticket from PostgreSQL
ticket = pg.execute("SELECT pg_ripple.arrow_flight_ticket('SELECT ?s ?p ?o WHERE { ?s ?p ?o }')")

# 2. Stream the Arrow IPC export
r = requests.post("http://localhost:7878/flight/do_get",
    json={"ticket": ticket},
    headers={"Authorization": f"Bearer {token}"},
    stream=True)

reader = pa.ipc.open_stream(r.raw)
table = reader.read_all()
print(table.to_pandas())
```

---

## Streaming Subscriptions

### `GET /subscribe/{subscription_id}`

Subscribe to a live SPARQL result stream via Server-Sent Events. The `subscription_id` is returned by `pg_ripple.subscribe(query)`.

**Response Content-Type:** `text/event-stream`

**Event types:**

| Event | Payload | Description |
|---|---|---|
| `row` | JSON object | New result row |
| `retract` | JSON object | Retracted row (triple deleted) |
| `heartbeat` | `{}` | Keep-alive every 30 s |
| `error` | `{"message": "..."}` | Subscription error |

**Example:**

```bash
# Create subscription in PostgreSQL
psql -c "SELECT pg_ripple.subscribe('SELECT ?s ?p ?o WHERE { ?s ?p ?o }')"
-- Returns: sub_abc123

# Connect SSE stream
curl -N "http://localhost:7878/subscribe/sub_abc123" \
  -H "Accept: text/event-stream"
```

---

## Datalog REST API

### `GET /datalog/rules`

List all registered Datalog rule sets.

**Response (200 OK):**

```json
[
  {"rule_set": "rdfs_closure", "rule_count": 13, "enabled": true},
  {"rule_set": "custom_rules", "rule_count": 5, "enabled": true}
]
```

---

### `POST /datalog/rules/{rule_set}/add`

Add a Datalog rule to a rule set.

**Content-Type:** `application/json`

```json
{
  "rule": "ancestor(?x, ?z) :- ancestor(?x, ?y), parent(?y, ?z).",
  "enabled": true
}
```

**Response (200 OK):**

```json
{"rule_id": "rule_42", "rule_set": "custom_rules"}
```

---

### `POST /datalog/infer/{rule_set}`

Run forward-chaining inference for a rule set.

**Content-Type:** `application/json`

```json
{
  "graph": "https://example.org/graph1",
  "max_iterations": 100,
  "dry_run": false
}
```

**Response (200 OK):**

```json
{
  "derived_count": 847,
  "iterations": 3,
  "elapsed_ms": 42,
  "stratification_order": ["base_rules", "closure_rules"]
}
```

---

### `POST /datalog/infer/{rule_set}/wfs`

Run well-founded semantics inference. Returns three-valued results (true / false / undefined) for recursive rules with negation.

---

### `POST /datalog/infer/{rule_set}/demand`

Goal-directed magic-sets inference. Only derives facts relevant to a specific query goal.

**Content-Type:** `application/json`

```json
{
  "goal": "ancestor(<:Alice>, ?z)",
  "graph": "https://example.org/graph1"
}
```

---

### `POST /datalog/query/{rule_set}`

Run a single Datalog goal query against a materialized rule set without modifying the store.

**Content-Type:** `application/json`

```json
{
  "goal": "ancestor(<:Alice>, ?z)",
  "limit": 100
}
```

**Response (200 OK):**

```json
{
  "results": [
    {"?z": "<https://example.org/Bob>"},
    {"?z": "<https://example.org/Carol>"}
  ],
  "total": 2
}
```

---

### `GET /datalog/constraints`

Check all constraint rules across all rule sets.

**Response (200 OK):**

```json
{
  "violations": [
    {
      "rule_set": "integrity_rules",
      "constraint": "uniqueness_violation",
      "violating_bindings": [{"?x": "<:entity1>"}]
    }
  ],
  "total_violations": 1
}
```

---

### `GET /datalog/stats/cache`

Returns rule plan cache hit/miss statistics.

---

### `GET /datalog/stats/tabling`

Returns tabling cache occupancy and eviction statistics.

---

## PageRank API

### `POST /pagerank/run`

Start a PageRank computation job.

**Content-Type:** `application/json`

```json
{
  "graph": "https://example.org/graph1",
  "edge_predicates": ["schema:knows", "schema:worksFor"],
  "damping": 0.85,
  "max_iterations": 100,
  "convergence_delta": 0.0001
}
```

**Response (200 OK):**

```json
{
  "job_id": "pr_20250503_001",
  "status": "running",
  "started_at": "2025-05-03T12:00:00Z"
}
```

---

### `GET /pagerank/status`

Poll the status of the most recent PageRank job.

**Response:**

```json
{
  "job_id": "pr_20250503_001",
  "status": "completed",
  "iterations": 47,
  "elapsed_ms": 1204,
  "node_count": 50000,
  "edge_count": 250000
}
```

---

### `GET /pagerank/export`

Export PageRank scores as JSON or CSV.

**Query parameters:**

| Parameter | Default | Description |
|---|---|---|
| `format` | `json` | `json` or `csv` |
| `limit` | `1000` | Maximum nodes |
| `offset` | `0` | Pagination offset |
| `graph` | *(all)* | Filter by named graph |

**Response (200 OK, JSON):**

```json
{
  "scores": [
    {"iri": "https://example.org/Alice", "score": 0.0423, "rank": 1},
    {"iri": "https://example.org/Bob", "score": 0.0381, "rank": 2}
  ],
  "total": 50000
}
```

---

### `GET /pagerank/explain/{node_iri}`

Explain the PageRank score for a specific node, showing top contributing in-edges.

**Path parameter:** `node_iri` — URL-encoded IRI.

**Response:**

```json
{
  "iri": "https://example.org/Alice",
  "score": 0.0423,
  "rank": 1,
  "top_contributors": [
    {"from_iri": "https://example.org/Bob", "edge_weight": 0.012},
    {"from_iri": "https://example.org/Carol", "edge_weight": 0.009}
  ]
}
```

---

### `POST /centrality/run`

Compute centrality metrics (degree, betweenness, Katz).

**Content-Type:** `application/json`

```json
{
  "graph": "https://example.org/graph1",
  "metrics": ["degree", "katz"],
  "katz_alpha": 0.01
}
```

---

### `POST /pagerank/find-duplicates`

Find near-duplicate nodes by PageRank score similarity and graph structure.

---

## Confidence / Probabilistic API

### `POST /confidence/load`

Load triples with associated confidence scores.

**Content-Type:** `application/json`

```json
{
  "triples": [
    {
      "subject": "https://example.org/Alice",
      "predicate": "rdf:type",
      "object": "schema:Person",
      "confidence": 0.95,
      "graph": "https://example.org/g1"
    }
  ]
}
```

**Response (200 OK):**

```json
{"inserted": 1, "updated": 0}
```

---

### `GET /confidence/shacl-score`

Get SHACL soft-validation confidence scores for shapes.

**Query parameters:**

| Parameter | Default | Description |
|---|---|---|
| `shape_iri` | *(all)* | Filter by specific shape |
| `min_score` | `0.0` | Minimum score threshold |

---

### `GET /confidence/shacl-report`

Get a full SHACL validation report with confidence scores per constraint.

---

### `POST /confidence/vacuum`

Vacuum stale confidence score rows from `_pg_ripple.confidence`.

---

## Administrative Endpoints

### `GET /health`

Returns `{"status": "ok"}` when the service is running and the database connection pool is healthy.

**Response:** `200 OK` — `{"status": "ok"}`
**Response:** `503 Service Unavailable` — database unreachable.

---

### `GET /health/ready`

Kubernetes-compatible readiness probe. Checks that the PostgreSQL connection pool has at least one available connection.

---

### `GET /metrics`

Prometheus-format metrics. No authentication required.

**Key metrics:**

| Metric | Type | Description |
|---|---|---|
| `pg_ripple_http_requests_total` | Counter | Total HTTP requests by method, path, status |
| `pg_ripple_http_request_duration_seconds` | Histogram | Request latency |
| `pg_ripple_sparql_queries_total` | Counter | SPARQL queries by type |
| `pg_ripple_sparql_query_duration_seconds` | Histogram | SPARQL query latency |
| `pg_ripple_federation_calls_total` | Counter | SERVICE clause calls by endpoint |
| `pg_ripple_datalog_infer_duration_seconds` | Histogram | Inference run latency |

### `GET /metrics/extension`

Extension-internal Prometheus metrics sourced directly from the pg_ripple extension within PostgreSQL.

---

### `GET /void`

Returns a VoID (Vocabulary of Interlinked Datasets) dataset description, summarizing the RDF store contents: triple counts, predicate frequencies, and class distributions.

**Accept:** `text/turtle` or `application/ld+json`

---

### `GET /service`

Returns a SPARQL 1.1 Service Description document describing the endpoint's capabilities.

**Accept:** `text/turtle` or `application/ld+json`

---

### `GET /openapi.yaml`

Returns the full OpenAPI 3.1 specification for the `pg_ripple_http` API.

---

### `GET /explorer`

Web-based SPARQL explorer UI — an embedded query editor with syntax highlighting, result table, and endpoint configuration.

---

## Error Responses

All error responses use a consistent JSON envelope:

```json
{
  "error": "PT440",
  "message": "query exceeds maximum algebra depth (256)",
  "hint": "Simplify the query or raise pg_ripple.sparql_max_algebra_depth"
}
```

| HTTP Status | Meaning |
|---|---|
| 400 | Invalid request body or SPARQL syntax error |
| 401 | Missing or invalid Bearer token |
| 403 | Operation not permitted for this role |
| 404 | Resource not found |
| 422 | Semantic error (SHACL violation, constraint error) |
| 429 | Rate limit exceeded |
| 500 | Internal error (check PostgreSQL logs) |
| 503 | Database connection unavailable |

---

## Security

- Bearer token authentication controls mutating endpoints
- SSRF protection: federation SERVICE endpoints are checked against `federation_endpoint_policy`
- SQL injection prevention: all queries use parameterized PostgreSQL SPI
- Arrow Flight tickets are HMAC-SHA256 signed; set `pg_ripple.arrow_flight_secret`
- Rate limiting: per-IP, configurable via `PG_RIPPLE_HTTP_RATE_LIMIT`
- HTTPS termination should be handled by a reverse proxy (nginx, Caddy, Traefik)
- Never expose `PG_RIPPLE_HTTP_AUTH_TOKEN` in URLs or logs
