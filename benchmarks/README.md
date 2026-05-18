# Benchmarks

This directory contains all pg_ripple performance benchmarks. The benchmarks
span storage throughput, query performance, Datalog inference, vector search,
PageRank, probabilistic reasoning, and federation.

## Files

| File | What it measures |
|------|-----------------|
| `ci_benchmark.sh` | Orchestrator — runs the full benchmark suite in CI and writes results to `merge_throughput_history.csv` and `pagerank_throughput_history.csv` |
| `insert_throughput.sql` | Triple insertion rate: bulk (`load_ntriples`) and single-triple insert via SPI |
| `merge_throughput.sql` | HTAP background-merge worker throughput: measures time to merge 100K delta rows into main VP tables |
| `pagerank.sql` | PageRank convergence time for a synthetic 100K-node directed graph |
| `pagerank_scale.sh` | PageRank scaling: runs at 10K, 100K, 1M nodes and records convergence time |
| `pagerank_with_writes.sh` | Concurrent writes + PageRank: 4 `pgbench` writer clients + 1 reader + 1 background PageRank |
| `pagerank_throughput_history.csv` | Recorded per-run PageRank timing history (appended by CI) |
| `merge_throughput_history.csv` | Recorded per-run merge timing history (appended by CI) |
| `merge_throughput_baselines.json` | Expected baseline timing for the HTAP merge regression gate |
| `datalog_agg.sql` | Datalog aggregation inference throughput |
| `magic_sets.sql` | Magic-sets goal-directed inference vs full materialization comparison |
| `wcoj.sql` | Worst-case optimal join (Leapfrog Triejoin) for cyclic SPARQL patterns |
| `hybrid_search.sql` | Hybrid vector+SPARQL query latency |
| `confidence_join_scale.sql` | Probabilistic Datalog: confidence join scale test |
| `probabilistic_overhead.sql` | Overhead of `@weight` rule annotations vs plain Datalog |
| `shacl_async_load.sql` | SHACL async validation queue throughput under load |
| `vector_index_compare.sql` | HNSW vs flat-scan vector index comparison |
| `bidi_relay_throughput.sql` | Bidirectional relay event throughput |
| `bidiops_throughput.sql` | Bidirectional operations (outbox publish + inbox drain) throughput |
| `bsbm/` | BSBM (Berlin SPARQL Benchmark) harness — see `bsbm/README.md` |
| `er_magellan.sh` | Entity-resolution F1 vs Magellan Abt-Buy and DBLP-ACM datasets |
| `er_freshness.sh` | Entity-resolution p95 latency at 100 records/s ingestion rate |

## Running benchmarks locally

Prerequisites: a running PostgreSQL 18 instance with pg_ripple installed.

```bash
# Start the pgrx-managed cluster (or point PGPORT at your own PG18 instance).
cargo pgrx start pg18
export PGPORT=28818

# Run the full CI benchmark suite.
bash benchmarks/ci_benchmark.sh

# Run a single benchmark.
psql -p "$PGPORT" -U "$USER" -d pg_ripple -f benchmarks/merge_throughput.sql
```

## How ci_benchmark.sh works

`ci_benchmark.sh` orchestrates the full suite:

1. **Seed data** — loads 100K triples from the bundled N-Triples fixture.
2. **Merge benchmark** — triggers the HTAP background worker and records the
   merge duration.  Results are appended to `merge_throughput_history.csv`.
3. **PageRank benchmark** — runs `pg_ripple.pagerank_run()` over the seeded
   graph and records convergence time.  Results go to
   `pagerank_throughput_history.csv`.
4. **Regression gate** — compares the recorded duration against baselines in
   `merge_throughput_baselines.json`.  Exits non-zero if any benchmark exceeds
   `1.5×` the baseline, causing CI to fail.

```bash
# Inspect the regression gate baselines.
cat benchmarks/merge_throughput_baselines.json
```

## Interpreting CSV history files

Both `merge_throughput_history.csv` and `pagerank_throughput_history.csv` share
the same schema:

```
run_id,started_at,triples,duration_ms,git_sha
```

- **`run_id`** — monotonically increasing integer.
- **`started_at`** — ISO 8601 timestamp of the benchmark run.
- **`triples`** — number of triples in the triple store at the time of the run.
- **`duration_ms`** — wall-clock duration of the benchmark in milliseconds.
- **`git_sha`** — short Git commit hash for the build under test.

Regressions are visible as step-changes in `duration_ms` across commits.
Use the following to plot a quick trend:

```bash
python3 -c "
import csv, sys
reader = csv.DictReader(open('benchmarks/merge_throughput_history.csv'))
for row in reader:
    print(row['started_at'][:10], row['duration_ms'], row['git_sha'])
"
```

## BSBM regression gate

The `bsbm/` subdirectory contains the Berlin SPARQL Benchmark (BSBM) harness.
It runs 100 query templates against a 1M-triple dataset and records per-template
latency.  See `bsbm/README.md` for setup and invocation.
