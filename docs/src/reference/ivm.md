# IVM (Incremental View Maintenance) — Architecture and Boundary

> **Version**: v0.91.0 (IVM-01)

pg_ripple implements two **independent** incremental view maintenance (IVM) mechanisms:
**CWB-IVM** (CONSTRUCT Writeback IVM) and **PageRank-IVM** (K-hop dirty-edge propagation).
This page explains each mechanism and their operational boundary.

---

## IVM Boundary: CWB vs. PageRank

The two mechanisms operate on different tables and are triggered by different events.
They **do not interact**: a CWB recompute does not automatically trigger PageRank
recomputation, and a PageRank IVM update does not write inferred triples back to VP tables.

| Property | CWB-IVM | PageRank-IVM |
|---|---|---|
| **What is maintained** | Inferred triples derived from SPARQL CONSTRUCT rules | Approximate PageRank scores |
| **Algorithm** | Delete-Rederive (Z-set deltas) | K-hop local push from dirty edges |
| **Source module** | `src/construct_rules/delta.rs` | `src/pagerank/ivm.rs` |
| **Queue table** | `_pg_ripple.cwb_queue` | `_pg_ripple.pagerank_dirty_edges` |
| **Triggered by** | VP table delta INSERT/DELETE via `cwb_queue` | VP table delta INSERT/DELETE via `pagerank_dirty_edges` |
| **Output** | New inferred triples written to VP tables | Updated scores in `_pg_ripple.pagerank_scores` |
| **Full recompute function** | `pg_ripple.run_full_recompute(rule_name)` | `pg_ripple.pagerank_run(...)` |

### CWB-IVM (CONSTRUCT Writeback)

CWB-IVM maintains inferred triples derived from SPARQL CONSTRUCT rules. When a VP table delta
is modified (new triples inserted or deleted), the affected rules are re-evaluated using the
Delete-Rederive algorithm:

1. Compute which previously-inferred triples are no longer derivable (Z-set negatives).
2. Compute which new triples are now derivable (Z-set positives).
3. Apply the delta: retract the negatives, assert the positives.

The `_pg_ripple.cwb_queue` table holds the pending delta events. The queue is drained by
`run_full_recompute()` or by the incremental maintenance background worker.

**Source**: `src/construct_rules/delta.rs`

### PageRank-IVM (K-hop Dirty-Edge Queue)

PageRank-IVM maintains approximate PageRank scores using bounded K-hop propagation from
recently-changed edges. When a VP table delta is modified, affected edges are added to
`_pg_ripple.pagerank_dirty_edges`. The IVM worker processes the queue by:

1. Identifying dirty nodes (nodes whose in-edges changed).
2. Re-computing scores for those nodes and their K-hop neighbourhood.
3. Updating `_pg_ripple.pagerank_scores` for affected nodes.

A full `pagerank_run()` is required after a large CWB recompute if edge predicates are
affected — the K-hop propagation cannot capture large-scale graph restructuring efficiently.

**Source**: `src/pagerank/ivm.rs`

---

## Monitoring

### CWB queue depth

```sql
SELECT COUNT(*) FROM _pg_ripple.cwb_queue;
```

### PageRank queue depth

```sql
SELECT * FROM pg_ripple.pagerank_queue_stats();
-- Returns: queued_edges, max_delta, oldest_enqueue, estimated_drain_seconds
```

Both metrics are exposed via Prometheus (see [Observability Reference](observability.md)):

- `pg_ripple_pagerank_queue_depth{topic=""}` — dirty edges pending PageRank refresh
- `pg_ripple_pagerank_queue_max_delta{topic=""}` — largest pending score delta
- `pg_ripple_pagerank_queue_oldest_enqueue_seconds{topic=""}` — age of oldest dirty entry

---

## GUC Parameters

| GUC | Default | Description |
|---|---|---|
| `pg_ripple.pagerank_incremental` | `off` | Enable K-hop IVM (dirty-edge queue) |
| `pg_ripple.pagerank_queue_warn_threshold` | 10000 | Log a WARNING when queue depth exceeds this |
| `pg_ripple.pagerank_ivm_k_hop` | 3 | Number of hops to propagate from a dirty edge |

---

## Related Pages

- [PageRank](../features/pagerank.md)
- [CONSTRUCT Writeback Rules](construct-rules.md)
- [Observability Reference](observability.md)
