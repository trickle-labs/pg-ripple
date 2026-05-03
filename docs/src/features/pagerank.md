# PageRank & Graph Analytics (v0.88.0)

pg_ripple v0.88.0 adds a **Datalog-native PageRank engine** that runs entirely inside PostgreSQL
via SPI and the existing Datalog aggregation infrastructure.  All scores are persisted in
`_pg_ripple.pagerank_scores` and are queryable with standard SQL — no external process required.

## Quick Start

```sql
-- Load some RDF triples
SELECT pg_ripple.load_ntriples($nt$
  <http://example.org/Alice> <http://xmlns.com/foaf/0.1/knows> <http://example.org/Bob> .
  <http://example.org/Bob>   <http://xmlns.com/foaf/0.1/knows> <http://example.org/Carol> .
  <http://example.org/Carol> <http://xmlns.com/foaf/0.1/knows> <http://example.org/Alice> .
$nt$, 'http://example.org/graph');

-- Run PageRank (default: damping=0.85, 100 iterations)
SELECT node_iri, score
FROM pg_ripple.pagerank_run()
ORDER BY score DESC;
```

## SQL Functions

### `pg_ripple.pagerank_run(...)`

Runs iterative PageRank and persists results.  Returns one row per node.

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `damping` | `float8` | `0.85` | Damping factor (teleportation probability = 1 − damping) |
| `max_iterations` | `int` | `100` | Maximum power-iteration steps |
| `convergence_delta` | `float8` | `0.0001` | L1-norm convergence threshold |
| `direction` | `text` | `'forward'` | `'forward'`, `'reverse'`, or `'undirected'` |
| `topic` | `text` | `''` | Topic label — enables topic-sensitive PageRank |
| `edge_predicates` | `text[]` | `NULL` | Restrict to these predicate IRIs (default: all) |
| `graph_uri` | `text` | `NULL` | Named graph to evaluate (default: all graphs) |
| `decay_rate` | `float8` | `0.0` | Temporal decay exponent (0 = disabled) |
| `bias` | `float8` | `0.15` | Personalization vector weight |

All parameters are optional and can be passed as **named arguments** (API-03, v0.91.0):

```sql
-- Named-argument invocation (recommended for clarity)
SELECT node_iri, score
FROM pg_ripple.pagerank_run(
    damping          => 0.85,
    max_iterations   => 50,
    topic            => 'citation'
);

-- Restrict to a specific named graph, reverse direction
SELECT node_iri, score
FROM pg_ripple.pagerank_run(
    graph_uri  => 'http://example.org/graph1',
    direction  => 'reverse'
)
ORDER BY score DESC LIMIT 20;
```

### `pg_ripple.centrality_run(metric text, ...)`

Computes an alternative centrality measure and stores results in
`_pg_ripple.centrality_scores`.

Supported `metric` values: `'betweenness'`, `'closeness'`, `'degree'`, `'pagerank'`.

```sql
SELECT COUNT(*) FROM pg_ripple.centrality_run('betweenness');
SELECT * FROM _pg_ripple.centrality_scores ORDER BY score DESC LIMIT 10;
```

### `pg_ripple.explain_pagerank(node_iri text, top_k int DEFAULT 5)`

Returns the score explanation tree for a node — which other nodes contributed how much.

```sql
SELECT depth, contributor_iri, contribution, path
FROM pg_ripple.explain_pagerank('<http://example.org/Alice>', 5);
```

### `pg_ripple.explain_pagerank_json(node_iri text, top_k int DEFAULT 5)` *(API-05, v0.91.0)*

Returns the same explanation tree as `explain_pagerank()`, but serialised as a JSONB object.
Use this when you need to return the explanation from a PL/pgSQL function or REST handler:

```sql
SELECT pg_ripple.explain_pagerank_json('<http://example.org/Alice>');
-- Returns JSONB: {"node_iri": "...", "total_score": 0.023, "contributors": [...]}

-- Useful in REST queries via pg_ripple_http /pagerank/explain/:iri?format=json
```

### `pg_ripple.export_pagerank(format text, top_k bigint DEFAULT 10000, topic text DEFAULT '')`

Serialises scores in `'csv'`, `'turtle'`, `'ntriples'`, or `'jsonld'` format.

```sql
SELECT pg_ripple.export_pagerank('turtle', 1000);
```

### `pg_ripple.pagerank_queue_stats()`

Returns IVM dirty-edge queue metrics useful for monitoring.

```sql
SELECT * FROM pg_ripple.pagerank_queue_stats();
-- queued_edges | max_delta | oldest_enqueue | estimated_drain_seconds
```

### `pg_ripple.vacuum_pagerank_dirty()`

Drains processed entries from `_pg_ripple.pagerank_dirty_edges`.

### `pg_ripple.pagerank_find_duplicates(metric text, centrality_threshold float8, fuzzy_threshold float8)`

Returns candidate duplicate node pairs detected via centrality + label similarity.

## GUC Parameters

| GUC | Default | Description |
|-----|---------|-------------|
| `pg_ripple.pagerank_enabled` | `off` | Master switch |
| `pg_ripple.pagerank_damping` | `0.85` | Damping factor |
| `pg_ripple.pagerank_max_iterations` | `100` | Iteration cap |
| `pg_ripple.pagerank_convergence_delta` | `0.0001` | Convergence threshold |
| `pg_ripple.pagerank_convergence_norm` | `'l1'` | Convergence norm (`l1`, `l2`, `linf`) |
| `pg_ripple.pagerank_full_recompute_threshold` | `0.01` | Stale fraction triggering full recompute |
| `pg_ripple.pagerank_wcoj_threshold` | `10` | WCOJ path threshold (millions of edges) |
| `pg_ripple.pagerank_sketch_width` | `2000` | Count-Min Sketch width (columns) |
| `pg_ripple.pagerank_sketch_depth` | `5` | Count-Min Sketch depth (rows/hash functions) |
| `pg_ripple.pagerank_temp_threshold` | `0` | Temp-table threshold bytes (0 = auto) |
| `pg_ripple.pagerank_incremental` | `off` | Enable IVM (k-hop refresh) |
| `pg_ripple.pagerank_confidence_weighted` | `off` | Weight edges by confidence scores |
| `pg_ripple.pagerank_partition` | `off` | Partition evaluation per named graph |
| `pg_ripple.pagerank_probabilistic` | `off` | Probabilistic Datalog score bounds |
| `pg_ripple.pagerank_queue_warn_threshold` | `100000` | Warn when dirty-edge queue exceeds this |

---

## Convergence Norm (v0.90.0)

pg_ripple uses the **L1 norm** (sum of absolute differences) to test convergence
between iterations, consistent with NetworkX's default behaviour. The convergence
threshold `pg_ripple.pagerank_convergence_delta` (default 0.0001) is compared
against the L1 norm of the score delta vector.

Alternative norms can be selected via the `pg_ripple.pagerank_convergence_norm` GUC:

| Value | Formula | Notes |
|-------|---------|-------|
| `'l1'` (default) | $\sum_i \|\Delta s_i\|$ | Fastest to compute; matches NetworkX |
| `'l2'` | $\sqrt{\sum_i \Delta s_i^2}$ | Matches igraph default |
| `'linf'` | $\max_i \|\Delta s_i\|$ | Most conservative; slowest convergence |

```sql
-- Match igraph convergence behaviour
SET pg_ripple.pagerank_convergence_norm = 'l2';
SELECT node_iri, score FROM pg_ripple.pagerank_run();
```

---

## Incremental Refresh Error Bounds (v0.90.0)

The bounded K-hop incremental refresh approximates the full PageRank recomputation.
The theoretical error bound after K hops of incremental propagation is:

$$|\Delta \text{score}| \leq \alpha^K \times \max\_\text{delta\_per\_iteration}$$

where $\alpha$ is the damping factor (default 0.85) and $K$ is
`pagerank_khop_limit` (default 30). At K=30:

$$\alpha^{30} = 0.85^{30} \approx 0.0076$$

The maximum error per dirty node is **< 1% of the per-iteration delta** — acceptable
for most use cases. For high-precision applications, use `pagerank_run()` directly.

### Automatic Full Recompute

When the fraction of `stale = true` rows in `pagerank_scores` for a given topic
exceeds `pg_ripple.pagerank_full_recompute_threshold` (default 0.01 = 1%), the
next IVM worker cycle automatically triggers a full `pagerank_run()` for that topic.

```sql
-- Lower threshold: trigger full recompute when 0.5% of scores are stale
SET pg_ripple.pagerank_full_recompute_threshold = 0.005;
```

---

## Count-Min Sketch Parameters (v0.90.0)

The top-K PageRank query path uses a Count-Min Sketch to track approximate
frequency distributions without materialising the full score table. Two GUCs
control sketch memory:

| GUC | Default | Formula |
|-----|---------|---------|
| `pg_ripple.pagerank_sketch_width` | `2000` | Columns per hash function |
| `pg_ripple.pagerank_sketch_depth` | `5` | Number of hash functions (rows) |

Memory usage: `width × depth × 8 bytes` per active topic. With defaults:
`2000 × 5 × 8 = 80 KB` per topic — negligible even for thousands of topics.

**Error bound**: with probability $1 - e^{-\text{depth}}$ the frequency estimate
overestimates by at most $e / \text{width}$ of the total stream mass.

```sql
-- Increase accuracy for large graphs at the cost of more memory
SET pg_ripple.pagerank_sketch_width = 10000;
SET pg_ripple.pagerank_sketch_depth = 7;
```

## REST API (pg_ripple_http)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/pagerank/run` | POST | Trigger computation |
| `/pagerank/results` | GET | Top-N scores |
| `/pagerank/status` | GET | Last run metadata |
| `/pagerank/vacuum-dirty` | POST | Drain queue |
| `/pagerank/export` | GET | Export scores |
| `/pagerank/explain/{node_iri}` | GET | Score explanation |
| `/pagerank/queue-stats` | GET | IVM queue metrics |
| `/centrality/run` | POST | Compute centrality |
| `/centrality/results` | GET | Centrality scores |
| `/pagerank/find-duplicates` | POST | Entity deduplication |

## Storage

Scores are persisted in `_pg_ripple.pagerank_scores`:

```sql
\d _pg_ripple.pagerank_scores
-- node BIGINT, topic TEXT, score FLOAT8,
-- score_lower FLOAT8, score_upper FLOAT8,
-- computed_at TIMESTAMPTZ, iterations INT, converged BOOL,
-- stale BOOL, stale_since TIMESTAMPTZ
```

The IVM dirty-edge queue (`_pg_ripple.pagerank_dirty_edges`) tracks which edges changed
since the last full run, enabling k-hop incremental refresh (PR-TRICKLE-01).

## Feature Status

```sql
SELECT feature, status, version FROM pg_ripple.feature_status()
WHERE feature LIKE 'pagerank%' OR feature LIKE 'centrality%';
```
