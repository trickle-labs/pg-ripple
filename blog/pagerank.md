# Graph Analytics with PageRank Inside PostgreSQL

> *Published with pg_ripple v0.88.0 — 3 May 2026*

pg_ripple v0.88.0 introduces **Datalog-native PageRank** — iterative graph centrality
computation that runs entirely inside PostgreSQL 18, using the same VP-table storage
and integer-join infrastructure as SPARQL queries.

## Why PageRank in PostgreSQL?

Moving data out of the database for PageRank computation adds latency, complicates
infrastructure, and breaks transactional consistency. With pg_ripple's `pagerank_run()`,
you compute centrality scores over your knowledge graph without ETL:

```sql
-- Compute PageRank over the entire knowledge graph
SELECT node_iri, score
FROM pg_ripple.pagerank_run()
ORDER BY score DESC
LIMIT 10;
```

## How It Works

PageRank is expressed as a Datalog^agg rule — an iterative fixpoint computation with
aggregation in rule bodies. pg_ripple's semi-naive evaluation engine executes this
efficiently using SPI-generated SQL with integer-ID joins.

The convergence test uses the L1 norm (configurable via `pg_ripple.pagerank_convergence_norm`).

## Incremental Refresh

The `_pg_ripple.pagerank_dirty_edges` queue tracks which edges changed since the last
full computation. When `pg_ripple.pagerank_incremental = on`, a K-hop propagation
computes score bounds for affected nodes without rerunning the full graph.

## Centrality Measures

In addition to PageRank, v0.88.0 provides four centrality measures via `pg:centrality()`:

- **Betweenness** — fraction of shortest paths through a node
- **Closeness** — inverse of average shortest path length
- **Eigenvector** — recursive centrality (similar to PageRank but unnormalised)
- **Katz** — Katz centrality with configurable attenuation factor α

## Entity Deduplication

`pg_ripple.pagerank_find_duplicates()` uses centrality scores + fuzzy matching to
suggest pairs of nodes that may be the same entity, ranked by similarity.

## Learn More

- [PageRank Feature Reference](../docs/src/features/pagerank.md)
- [SPARQL PageRank Functions](../docs/src/reference/sparql.md)
- [Graph Analytics REST API](../docs/src/reference/http-api.md)
