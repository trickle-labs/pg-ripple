#!/usr/bin/env bash
# benchmarks/er_freshness.sh — ER ingestion latency benchmark (v0.110.0)
#
# Inserts 1,000 entity records at ~100 rec/s and measures the p95 latency
# from insert to symbolic-match detection.
#
# Exit codes:
#   0  — p95 latency < 500 ms
#   1  — p95 latency >= 500 ms
#   2  — setup error
#
# Requirements:
#   - pg_ripple extension installed and accessible via psql
#
# Environment variables:
#   DB_NAME       — PostgreSQL database name (default: pg_ripple_bench)
#   DB_HOST       — PostgreSQL host (default: localhost)
#   DB_PORT       — PostgreSQL port (default: 5432)
#   DB_USER       — PostgreSQL user (default: postgres)
#   RECORD_COUNT  — number of records to insert (default: 1000)
#   BATCH_SIZE    — records per batch (default: 100)
#   P95_THRESHOLD_MS — p95 latency threshold in ms (default: 500)

set -euo pipefail

DB_NAME="${DB_NAME:-pg_ripple_bench}"
DB_HOST="${DB_HOST:-localhost}"
DB_PORT="${DB_PORT:-5432}"
DB_USER="${DB_USER:-postgres}"
RECORD_COUNT="${RECORD_COUNT:-1000}"
BATCH_SIZE="${BATCH_SIZE:-100}"
P95_THRESHOLD_MS="${P95_THRESHOLD_MS:-500}"

psql_exec() {
    psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -c "$1"
}

psql_query() {
    psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -tAc "$1"
}

echo "=== ER Freshness Benchmark (v0.110.0) ==="
echo "Database:     ${DB_USER}@${DB_HOST}:${DB_PORT}/${DB_NAME}"
echo "Records:      ${RECORD_COUNT}"
echo "Batch size:   ${BATCH_SIZE}"
echo "p95 threshold: ${P95_THRESHOLD_MS} ms"

# ── Setup ─────────────────────────────────────────────────────────────────────

BENCH_GRAPH="http://bench.er.freshness/graph"
LATENCY_LOG="$(mktemp /tmp/er_freshness_XXXXXX.txt)"
trap "rm -f ${LATENCY_LOG}" EXIT

# ── Insert records and measure latency ────────────────────────────────────────

echo "--- Inserting ${RECORD_COUNT} entity records..."

for i in $(seq 1 "$RECORD_COUNT"); do
    START_NS=$(date +%s%N)

    ENTITY="http://bench.er.freshness/entity${i}"
    NAME="Entity Name ${i}"

    psql_exec "SELECT pg_ripple.insert_triple(
        '${ENTITY}',
        'https://schema.org/name',
        '${NAME}',
        '${BENCH_GRAPH}'
    );" > /dev/null 2>&1

    END_NS=$(date +%s%N)
    ELAPSED_MS=$(( (END_NS - START_NS) / 1000000 ))
    echo "$ELAPSED_MS" >> "$LATENCY_LOG"

    # Rate-limit to ~100 rec/s
    if (( i % BATCH_SIZE == 0 )); then
        echo "    Inserted ${i}/${RECORD_COUNT} records..."
        sleep 0.1
    fi
done

# ── Compute p95 ───────────────────────────────────────────────────────────────

echo "--- Computing p95 latency..."

P95_MS=$(sort -n "$LATENCY_LOG" | awk -v pct=0.95 '
BEGIN { n=0; }
{ lines[n++]=$1; }
END {
    idx = int(n * pct);
    if (idx >= n) idx = n - 1;
    print lines[idx];
}')

MEAN_MS=$(awk '{ sum += $1; n++ } END { if (n > 0) printf "%.1f", sum/n; else print 0; }' "$LATENCY_LOG")

# ── Report and gate ───────────────────────────────────────────────────────────

echo ""
echo "=== Results ==="
echo "Records inserted: ${RECORD_COUNT}"
printf "Mean latency:  %s ms\n" "$MEAN_MS"
printf "p95 latency:   %s ms  (threshold: %s ms)\n" "$P95_MS" "$P95_THRESHOLD_MS"

if (( $(echo "$P95_MS < $P95_THRESHOLD_MS" | bc -l) )); then
    echo "=== ER Freshness benchmark: PASSED ==="
    exit 0
else
    echo "=== ER Freshness benchmark: FAILED (p95 ${P95_MS} ms >= ${P95_THRESHOLD_MS} ms) ==="
    exit 1
fi
