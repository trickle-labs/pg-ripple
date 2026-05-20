# Temporal Graph Snapshots in pg_ripple: Point-in-Time Named Graphs

pg_ripple v0.125.0 adds first-class support for **temporal graph snapshots** —
a mechanism to materialise a named RDF graph at any past timestamp, query it via
SPARQL, compute the diff between two moments in time, and automatically retire
old snapshots through a configurable retention policy.

## Why temporal snapshots?

RDF knowledge graphs change continuously: facts are asserted, retracted, and
revised as the world evolves.  The `_pg_ripple.temporal_facts` table (introduced
in v0.106.0) stores every fact with a `(valid_from, valid_to)` validity interval,
giving you the full history.  But running a SPARQL query against a point-in-time
view of a specific named graph required you to construct the interval filter
manually.

v0.125.0 wraps this into a single SQL call:

```sql
-- Materialise Graph1 as it looked on 2024-03-01 at noon UTC.
SELECT pg_ripple.graph_at(
    'http://example.org/Graph1',
    '2024-03-01 12:00:00+00'::TIMESTAMPTZ
);
-- Returns: urn:snapshot:http___example_org_Graph1:2024-03-01T12:00:00Z
```

The returned IRI is registered in `_pg_ripple.graph_snapshots` and can be used
directly in a `GRAPH <iri> { … }` SPARQL pattern.

## The `graph_at()` function

```sql
pg_ripple.graph_at(graph_iri TEXT, snapshot_time TIMESTAMPTZ) → TEXT
```

Internally, `graph_at()`:

1. Counts temporal facts for the graph valid at `snapshot_time` (where
   `valid_from <= snapshot_time AND (valid_to IS NULL OR valid_to > snapshot_time)`).
2. Builds a deterministic snapshot IRI of the form
   `urn:snapshot:{sanitised-graph-iri}:{iso8601-utc}`.
3. Upserts a row into `_pg_ripple.graph_snapshots` with the IRI, the triple
   count, and an `expires_at` timestamp set to
   `snapshot_time + pg_ripple.snapshot_retention_days days`.
4. Returns the snapshot IRI.

The function is **idempotent**: calling it twice with the same arguments returns
the same IRI and updates the existing catalog row rather than inserting a
duplicate.

## Computing diffs with `graph_diff()`

```sql
pg_ripple.graph_diff(
    graph_iri TEXT,
    from_ts   TIMESTAMPTZ,
    to_ts     TIMESTAMPTZ
) → TABLE(s BIGINT, p BIGINT, o BIGINT, change TEXT)
```

`graph_diff()` returns the delta between two temporal snapshots of the same
named graph.  `change` is `'added'` for facts present at `to_ts` but not at
`from_ts`, and `'removed'` for facts present at `from_ts` but not at `to_ts`.

```sql
-- What changed in Graph1 between January and July 2024?
SELECT
    pg_ripple.decode(s) AS subject,
    pg_ripple.decode(p) AS predicate,
    pg_ripple.decode(o) AS object,
    change
FROM pg_ripple.graph_diff(
    'http://example.org/Graph1',
    '2024-01-01 00:00:00+00'::TIMESTAMPTZ,
    '2024-07-01 00:00:00+00'::TIMESTAMPTZ
)
ORDER BY change, subject;
```

The raw `(s, p, o)` columns are dictionary-encoded BIGINTs; wrap them in
`pg_ripple.decode()` to get human-readable IRIs and literals.

## HTTP endpoints

The pg_ripple HTTP companion (`pg_ripple_http`) exposes both operations as REST
endpoints:

### Snapshot content as Turtle

```
GET /temporal/graphs/{iri}/snapshot?at=2024-03-01T12:00:00Z
```

Returns the snapshot content as `text/turtle` with an `X-Snapshot-IRI` response
header containing the registered snapshot IRI.

### N-Quads diff

```
GET /temporal/graphs/{iri}/diff?from=2024-01-01T00:00:00Z&to=2024-07-01T00:00:00Z
```

Returns the diff as `application/n-quads`.  Each changed triple is preceded by
a `# added` or `# removed` comment line for easy streaming processing:

```nquads
# added
<http://example.org/Alice> <http://example.org/likes> <http://example.org/Coffee> <http://example.org/Graph1> .
# removed
<http://example.org/Alice> <http://example.org/knows> <http://example.org/Bob> <http://example.org/Graph1> .
```

## Prometheus gauge

The `/metrics` endpoint now includes:

```
# HELP pg_ripple_graph_snapshots_total Current number of registered temporal graph snapshots (FEAT-02)
# TYPE pg_ripple_graph_snapshots_total gauge
pg_ripple_graph_snapshots_total 42
```

The gauge is refreshed on every `/temporal/graphs/{iri}/snapshot` call.

## Automatic GC

Snapshots accumulate over time.  The `pg_ripple.snapshot_retention_days` GUC
(default: 30 days) controls how long snapshots are retained:

```sql
-- Retain snapshots for 90 days instead of the default 30.
ALTER SYSTEM SET pg_ripple.snapshot_retention_days = 90;
SELECT pg_reload_conf();
```

The merge background worker prunes expired snapshots on each tick — any row
where `expires_at <= now()` is deleted.  Set `snapshot_retention_days = 0` to
keep all snapshots indefinitely (useful for audit-compliance scenarios).

## Snapshot catalog

The `_pg_ripple.graph_snapshots` table is the single source of truth for all
registered snapshots:

```sql
SELECT snapshot_id, graph_iri, snapshot_iri, captured_at, triple_count, expires_at
FROM _pg_ripple.graph_snapshots
ORDER BY captured_at DESC;
```

## Migration

Users upgrading from v0.124.0 via `ALTER EXTENSION pg_ripple UPDATE` will have
the `graph_snapshots` table and `snapshot_id_seq` sequence created automatically
by the `sql/pg_ripple--0.124.0--0.125.0.sql` migration script.

## Summary

| Feature | API |
|---|---|
| Materialise snapshot | `pg_ripple.graph_at(graph_iri, ts)` |
| Compute diff | `pg_ripple.graph_diff(graph_iri, from_ts, to_ts)` |
| Count snapshots | `pg_ripple.graph_snapshots_count()` |
| HTTP snapshot (Turtle) | `GET /temporal/graphs/{iri}/snapshot?at=…` |
| HTTP diff (N-Quads) | `GET /temporal/graphs/{iri}/diff?from=…&to=…` |
| Prometheus gauge | `pg_ripple_graph_snapshots_total` |
| Retention GUC | `pg_ripple.snapshot_retention_days` (default 30) |
