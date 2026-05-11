# Production Readiness Checklist

Use this checklist before deploying pg_ripple to production. Each item links to the relevant documentation for details.

---

## PostgreSQL Configuration

- [ ] **PostgreSQL 18** installed — pg_ripple requires PostgreSQL 18.x
- [ ] **`shared_preload_libraries`** includes `'pg_ripple'` — required for the background merge worker and shared-memory dictionary cache ([Configuration](configuration.md))
- [ ] **`pg_ripple.worker_database`** set to your target database — the merge worker connects to this database ([Merge Workers](merge-workers.md))
- [ ] **Shared memory** sized correctly — `pg_ripple.dictionary_cache_size` determines shared memory usage; check OS limits ([Troubleshooting §6](troubleshooting.md))
- [ ] **PostgreSQL restarted** after `shared_preload_libraries` changes

## Security

- [ ] **Row-Level Security (RLS)** enabled on named graphs if multi-tenant — `pg_ripple.enable_graph_rls()` + role grants ([Security](security.md), [Multi-Tenant Graphs](../features/multi-tenant-graphs.md))
- [ ] **Federation SSRF protection** configured — `pg_ripple.federation_allow_private = off` (default) prevents SERVICE queries to private IPs ([GUC Reference](../reference/guc-reference.md))
- [ ] **`pg_ripple_http` auth token** set — `PG_RIPPLE_HTTP_AUTH_TOKEN` environment variable for Bearer token authentication ([HTTP API Reference](../reference/http-api.md))
- [ ] **TLS termination** configured — use a reverse proxy (nginx, Caddy) for HTTPS; pg_ripple_http does not handle TLS directly
- [ ] **`pg_hba.conf`** restricts connections to the pg_ripple_http service account
- [ ] **Embedding API keys** not stored in `postgresql.conf` — use `ALTER SYSTEM` or inject via session `SET` ([GUC Reference](../reference/guc-reference.md))

## Performance

- [ ] **Merge workers** tuned — `pg_ripple.merge_workers` = 2–4 for workloads with many predicates ([Merge Workers](merge-workers.md))
- [ ] **Dictionary cache** sized to working set — monitor `encode_cache_evictions` via `pg_ripple.stats()`, target > 90% hit rate ([Troubleshooting §7](troubleshooting.md))
- [ ] **Autovacuum** tuned for VP tables — consider `autovacuum_vacuum_scale_factor = 0.01` on high-churn delta tables ([Performance](performance.md))
- [ ] **`work_mem`** adequate for SPARQL-generated SQL — 64–256 MB for large queries
- [ ] **Property path depth** bounded — `pg_ripple.max_path_depth` prevents runaway recursion (default: 10) ([Troubleshooting §3](troubleshooting.md))

## Monitoring

- [ ] **Prometheus metrics** configured — `pg_ripple_http` exposes `/metrics` endpoint ([Monitoring](monitoring.md))
- [ ] **Key metrics monitored**:
  - `pg_ripple_triple_count` — total stored triples
  - `pg_ripple_merge_worker_lag` — merge backlog
  - `pg_ripple_dictionary_cache_hit_rate` — encoding efficiency
  - `pg_ripple_sparql_query_duration_seconds` — query latency
- [ ] **Health check** configured — `GET /health` and `GET /health/ready` for load balancer probes
- [ ] **Log-based alerting** on PT-series error codes — see [Error Catalog](../reference/error-catalog.md)

## Backup and Recovery

- [ ] **`pg_dump`** tested — pg_ripple stores all data in standard PostgreSQL tables; `pg_dump`/`pg_restore` works without special steps ([Backup](backup-recovery.md))
- [ ] **WAL archiving** enabled for point-in-time recovery
- [ ] **Backup schedule** documented and tested for restore

## Upgrade Path

- [ ] **Migration scripts** verified — `ALTER EXTENSION pg_ripple UPDATE` applies sequential migration scripts ([Upgrading](upgrading.md))
- [ ] **Compatibility matrix** checked — if using `pg_ripple_http`, verify version compatibility ([Compatibility](compatibility.md))
- [ ] **Test in staging** before production upgrade — run `cargo pgrx regress` or the pg_regress suite against the new version

## Optional Components

- [ ] **pgvector** installed if using vector/hybrid search — `CREATE EXTENSION vector` ([Vector Search](../features/vector-and-hybrid-search.md))
- [ ] **pg_trickle** installed if using live views or CDC bridge — ([CDC Operations](cdc.md))
- [ ] **PostGIS** installed if using GeoSPARQL — ([GeoSPARQL](../features/geospatial.md))

## Smoke Test

After deployment, verify the extension is working:

```sql
-- Check extension version
SELECT pg_ripple.build_info();

-- Verify merge worker is running
SELECT (pg_ripple.stats()->>'merge_worker_pid')::int > 0 AS merge_worker_alive;

-- Verify dictionary cache is active
SELECT pg_ripple.stats()->>'encode_cache_capacity' AS cache_capacity;

-- Load a test triple and query it
SELECT pg_ripple.insert_triple(
    'http://example.org/test',
    'http://example.org/status',
    '"production-ready"'
);
SELECT * FROM pg_ripple.sparql($$
    SELECT ?status WHERE {
        <http://example.org/test> <http://example.org/status> ?status
    }
$$);
```
