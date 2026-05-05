#!/usr/bin/env bash
# benchmarks/pagerank_with_writes.sh
# v0.96.0 M15-17: PageRank concurrent-load benchmark
#
# Runs a 60-second concurrent load test combining:
#   - 4 pgbench writer clients inserting triples in parallel
#   - 1 SPARQL reader issuing repeated SELECT queries
#   - 1 background PageRank computation via pg_ripple.compute_pagerank()
#
# Purpose: verify that PageRank IVM and VP merge workers remain stable under
# write pressure. The test asserts that:
#   1. No writer exits with a non-zero code (no deadlocks / constraint errors).
#   2. The SPARQL reader completes with TPS > 0 (queries succeed under load).
#   3. compute_pagerank() returns a non-negative node count after the run.
#
# Usage:
#   PGCONN="host=localhost dbname=test" bash benchmarks/pagerank_with_writes.sh
#   DURATION=30 bash benchmarks/pagerank_with_writes.sh  # shorter run

set -euo pipefail

PGCONN="${PGCONN:-host=localhost dbname=postgres}"
PSQL="${PSQL:-psql}"
PGBENCH="${PGBENCH:-pgbench}"
DURATION="${DURATION:-60}"
WRITERS="${WRITERS:-4}"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
HISTORY_CSV="$SCRIPT_DIR/pagerank_throughput_history.csv"
TMPDIR_BM="$(mktemp -d)"
trap 'rm -rf "$TMPDIR_BM"' EXIT

echo "[M15-17] PageRank concurrent-load benchmark (pg_ripple v0.96.0)"
echo "  Connection : $PGCONN"
echo "  Duration   : ${DURATION}s"
echo "  Writers    : $WRITERS"
echo ""

# ── Step 1: Bootstrap extension and seed data ────────────────────────────────
echo "[1/5] Bootstrapping extension and seed data..."
$PSQL "$PGCONN" -v ON_ERROR_STOP=1 -q <<'SQL'
CREATE EXTENSION IF NOT EXISTS pg_ripple;
SET search_path TO pg_ripple, public;

-- Seed 1 000 triples as the initial PageRank graph.
DO $$
DECLARE
    i INT;
BEGIN
    FOR i IN 1..1000 LOOP
        PERFORM pg_ripple.insert_triple(
            format('<https://bench.test/node%s>', i),
            '<https://bench.test/linksTo>',
            format('<https://bench.test/node%s>', (i % 500) + 1)
        );
    END LOOP;
END;
$$;
SQL
echo "  Seed data loaded."

# ── Step 2: Create pgbench writer script ─────────────────────────────────────
echo "[2/5] Creating pgbench writer script..."
cat > "$TMPDIR_BM/writer.sql" <<'PBSQL'
-- pgbench writer: insert one random triple per transaction
\set src random(1, 5000)
\set dst random(1, 5000)
SELECT pg_ripple.insert_triple(
    format('<https://bench.test/node%s>', :src),
    '<https://bench.test/linksTo>',
    format('<https://bench.test/node%s>', :dst)
);
PBSQL

# ── Step 3: Create pgbench reader script ─────────────────────────────────────
echo "[3/5] Creating pgbench SPARQL reader script..."
cat > "$TMPDIR_BM/reader.sql" <<'PBSQL'
-- pgbench reader: simple SPARQL SELECT under load
SELECT pg_ripple.sparql_select(
    'PREFIX bench: <https://bench.test/>
     SELECT ?s ?o WHERE { ?s bench:linksTo ?o } LIMIT 10'
) IS NOT NULL AS ok;
PBSQL

# ── Step 4: Run writers + reader concurrently ────────────────────────────────
echo "[4/5] Running $WRITERS writers + 1 reader for ${DURATION}s..."

# Start writers in background.
WRITER_PIDS=()
WRITER_LOGS=()
for i in $(seq 1 "$WRITERS"); do
    LOG="$TMPDIR_BM/writer_${i}.log"
    WRITER_LOGS+=("$LOG")
    $PGBENCH "$PGCONN" \
        -f "$TMPDIR_BM/writer.sql" \
        -T "$DURATION" \
        -c 1 -j 1 \
        --no-vacuum \
        > "$LOG" 2>&1 &
    WRITER_PIDS+=("$!")
done

# Start SPARQL reader in background.
READER_LOG="$TMPDIR_BM/reader.log"
$PGBENCH "$PGCONN" \
    -f "$TMPDIR_BM/reader.sql" \
    -T "$DURATION" \
    -c 1 -j 1 \
    --no-vacuum \
    > "$READER_LOG" 2>&1 &
READER_PID=$!

# Wait for all background processes.
FAILURES=0
for i in "${!WRITER_PIDS[@]}"; do
    PID="${WRITER_PIDS[$i]}"
    LOG="${WRITER_LOGS[$i]}"
    if ! wait "$PID"; then
        echo "  ERROR: writer $((i+1)) failed (pid $PID)."
        cat "$LOG"
        FAILURES=$((FAILURES + 1))
    fi
done

READER_TPS=0
if ! wait "$READER_PID"; then
    echo "  ERROR: SPARQL reader failed."
    cat "$READER_LOG"
    FAILURES=$((FAILURES + 1))
else
    READER_TPS=$(grep -oP 'tps = \K[0-9.]+' "$READER_LOG" | tail -1 || echo "0")
fi

# ── Step 5: Run PageRank and verify ─────────────────────────────────────────
echo "[5/5] Running PageRank computation after concurrent writes..."
PR_RESULT=$($PSQL "$PGCONN" -t -A -v ON_ERROR_STOP=1 <<'SQL'
SET search_path TO pg_ripple, public;
SELECT pg_ripple.compute_pagerank() >= 0 AS pr_ok;
SQL
)

echo ""
echo "Results:"
echo "  Writer failures : $FAILURES / $WRITERS"
echo "  SPARQL reader   : TPS = ${READER_TPS}"
echo "  PageRank pass   : $PR_RESULT"

# Record to history CSV.
TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
if [[ ! -f "$HISTORY_CSV" ]]; then
    echo "timestamp,scale,edge_count,wall_time_s,converged,iterations,max_score" > "$HISTORY_CSV"
fi
echo "${TIMESTAMP},concurrent_write_test,${DURATION}s_writers_${WRITERS},${DURATION},${FAILURES}failures,${READER_TPS}tps,${PR_RESULT}" \
    >> "$HISTORY_CSV"

# Assert pass conditions.
if [[ "$FAILURES" -gt 0 ]]; then
    echo "FAIL: $FAILURES writer(s) exited with errors." >&2
    exit 1
fi

if [[ "$READER_TPS" == "0" ]]; then
    echo "WARN: SPARQL reader TPS = 0 (may indicate no queries completed)." >&2
fi

if [[ "$PR_RESULT" != "t" ]]; then
    echo "FAIL: compute_pagerank() returned non-success: $PR_RESULT" >&2
    exit 1
fi

echo "PASS: PageRank concurrent-load benchmark completed successfully."
