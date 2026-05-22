# Read Replicas

pg_ripple_http (v0.120.0+) supports routing read-only SPARQL queries to a
PostgreSQL read replica. This page explains the routing semantics, eligible
query types, pool exhaustion behaviour, and Prometheus alerting recipes.

## Configuration

Set the `PG_RIPPLE_REPLICA_DATABASE_URL` environment variable to point the HTTP
companion at your read-replica PostgreSQL instance:

```bash
PG_RIPPLE_REPLICA_DATABASE_URL="postgres://user:pass@replica-host:5432/rippledb"
```

When this variable is absent the companion operates in primary-only mode and all
queries go to the primary pool (`PG_RIPPLE_DATABASE_URL`).

## Routing semantics — `?replica=ok`

Append `?replica=ok` to any SPARQL HTTP endpoint URL to request replica routing:

```
GET /sparql?query=SELECT+…&replica=ok
```

The companion evaluates the request against the following decision tree:

1. **Eligible query type?** — Only `SELECT`, `CONSTRUCT`, `ASK`, and `DESCRIBE`
   queries may be routed to the replica. `UPDATE` (write) queries always go to
   the primary regardless of `?replica=ok`.
2. **Replica pool configured?** — If `PG_RIPPLE_REPLICA_DATABASE_URL` is unset,
   the request falls back to the primary silently.
3. **Pool connection available?** — If the replica pool is exhausted (all
   connections in use), the request falls back to the primary. No error is
   returned to the caller; the response is identical either way.
4. **Replica healthy?** — If the replica returns a connection error, the
   companion falls back to the primary and increments
   `pg_ripple_http_errors_total`.

## Pool exhaustion fallback

When the replica pool is exhausted, queries automatically fall back to the
primary. No client-visible error is raised. This behaviour is intentional:
replica routing is a performance optimisation, not a hard requirement. If you
need to enforce replica-only execution (e.g., for capacity-separated workloads),
implement that policy at the load-balancer or proxy layer.

## Prometheus gauges (OBS-M-01)

The HTTP companion exposes two Prometheus gauges for replica pool observability:

| Metric | Type | Description |
|--------|------|-------------|
| `pg_ripple_http_replica_pool_size{pool="replica"}` | gauge | Total connection pool capacity |
| `pg_ripple_http_replica_pool_available{pool="replica"}` | gauge | Currently idle (available) connections |

Both gauges are updated at every `/metrics` scrape. When no replica pool is
configured both values are `0`.

### Example Prometheus alert rules

```yaml
groups:
  - name: pg_ripple_replica_pool
    rules:
      # Alert when the replica pool is completely saturated.
      - alert: ReplicaPoolExhausted
        expr: |
          pg_ripple_http_replica_pool_available{pool="replica"} == 0
          and pg_ripple_http_replica_pool_size{pool="replica"} > 0
        for: 2m
        labels:
          severity: warning
        annotations:
          summary: "pg_ripple read-replica pool exhausted"
          description: >
            All {{ $value }} replica pool connections are in use. Queries are
            falling back to the primary. Consider increasing the pool size or
            reducing query concurrency.

      # Alert when available connections drop below 20 % of pool capacity.
      - alert: ReplicaPoolLow
        expr: |
          (pg_ripple_http_replica_pool_available{pool="replica"}
            / pg_ripple_http_replica_pool_size{pool="replica"}) < 0.2
        for: 5m
        labels:
          severity: info
        annotations:
          summary: "pg_ripple replica pool running low"
          description: >
            Only {{ $value | humanizePercentage }} of replica pool connections
            are idle. Pool may become exhausted under load.
```

## See also

- [Configuration reference](../reference/guc-reference.md)
- [OpenTelemetry observability](observability-otel.md)
- [Compatibility matrix](compatibility.md)
