# Federation Circuit Breakers in pg_ripple

When a SPARQL query contains a `SERVICE` clause targeting an external SPARQL
endpoint, a slow or unavailable endpoint can stall the entire query. pg_ripple
implements a circuit-breaker pattern that automatically isolates failing
endpoints, failing fast instead of waiting for timeouts.

## The problem: cascading failures in federated queries

A federated SPARQL query like:

```sparql
SELECT * WHERE {
  ?x a ex:Product .
  SERVICE <https://dbpedia.org/sparql> {
    ?x rdfs:label ?label .
  }
}
```

will hang for up to 30 seconds (the default HTTP timeout) if DBpedia is
unavailable. In a workload with hundreds of concurrent queries, this creates a
cascade: connections pile up, the connection pool exhausts, and the primary
store also becomes unavailable.

## How the circuit breaker works

pg_ripple's federation circuit breaker tracks each `SERVICE` endpoint
independently in `_pg_ripple.federation_circuit_state`:

| State | Meaning |
|-------|---------|
| `closed` | Normal operation — requests flow through |
| `open` | Endpoint is isolated — requests fail immediately with `PT0550` |
| `half_open` | Probe state — one request is allowed through to test recovery |

The state machine transitions:

```
closed  →  open      after 5 consecutive failures (configurable)
open    →  half_open after 60 s cooldown (configurable)
half_open → closed   on first success
half_open → open     on any failure
```

## Configuration GUCs

| GUC | Default | Description |
|-----|---------|-------------|
| `pg_ripple.federation_circuit_failure_threshold` | `5` | Failures before opening |
| `pg_ripple.federation_circuit_open_duration_ms` | `60000` | Open-state cooldown in ms |

## Prometheus monitoring

The circuit breaker state is exposed as a Prometheus gauge:

```
pg_ripple_federation_circuit_state{endpoint="https://dbpedia.org/sparql"} 0
# 0=closed, 1=open, 2=half_open
```

Alert rule:

```yaml
- alert: FederationCircuitOpen
  expr: pg_ripple_federation_circuit_state > 0
  for: 1m
  labels:
    severity: warning
  annotations:
    summary: "Federation circuit breaker open for {{ $labels.endpoint }}"
```

## Observability in EXPLAIN

```sql
SELECT pg_ripple.sparql_explain('
  SELECT * WHERE {
    SERVICE <https://dbpedia.org/sparql> { ?x rdfs:label ?l }
  }
');
```

Output includes circuit breaker state in the federation node:

```json
{
  "node": "Service",
  "endpoint": "https://dbpedia.org/sparql",
  "circuit_state": "closed",
  "failure_count": 0,
  "last_failure_at": null
}
```

## Error handling

When the circuit is open, queries fail immediately with:

```
ERROR:  PT0550: federation circuit open for https://dbpedia.org/sparql
HINT:  Endpoint will be retried after 60 s. Use SERVICE SILENT to suppress.
```

Use `SERVICE SILENT` to suppress the error and treat the service result as
empty instead of raising:

```sparql
SERVICE SILENT <https://dbpedia.org/sparql> {
  OPTIONAL { ?x rdfs:label ?label }
}
```

## See also

- [Federation reference](../docs/src/reference/federation.md)
- [Federation credentials (v0.126.0)](../roadmap/v0.126.0.md)
- [Rule library federation](rule-library-federation.md)
