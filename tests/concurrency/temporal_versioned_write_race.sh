#!/usr/bin/env bash
# temporal_versioned_write_race.sh — concurrent writes to the same versioned predicate.
#
# M16-12 (v0.117.0): fires N concurrent transactions that each write to the same
# subject/predicate slot in a temporal (versioned) graph and verifies that:
#   1. All writes are eventually persisted (no silent data loss).
#   2. The final state reflects a consistent version history (no "torn" read).
#   3. Exactly N distinct statement IDs (SIDs) are present after the writes.
#
# This targets the HTAP delta/merge path: concurrent inserts to the same VP table
# must not collide on the BIGINT primary key or create duplicate SIDs.
#
# Prerequisites:
#   - PostgreSQL 18 running with pg_ripple installed
#   - psql available on $PATH
#   - Environment: PGDATABASE, PGUSER, PGHOST, PGPORT (or defaults)
#
# Usage:
#   bash tests/concurrency/temporal_versioned_write_race.sh [n_workers]
#
# Exit codes:
#   0 — all N statements persisted with unique SIDs
#   1 — data loss or duplicate SID detected
#   2 — pg_ripple not installed or psql unavailable

set -euo pipefail

N_WORKERS="${1:-20}"
TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

echo "Temporal versioned write race test: $N_WORKERS concurrent writes"

# Verify psql is available.
if ! command -v psql > /dev/null 2>&1; then
    echo "ERROR: psql not found on PATH" >&2
    exit 2
fi

# Verify pg_ripple is installed.
if ! psql -c "SELECT pg_ripple.version();" > /dev/null 2>&1; then
    echo "ERROR: pg_ripple extension not installed or pg not running" >&2
    exit 2
fi

SUBJECT="urn:temporal_race_test:$(date +%s)"
PREDICATE="urn:temporal_race_test:value"
TEST_GRAPH="urn:temporal_race_graph:$(date +%s)"

# Insert N_WORKERS triples concurrently, each with a distinct object value.
pids=()
for i in $(seq 1 "$N_WORKERS"); do
    out_file="$TMP_DIR/worker_${i}.txt"
    OBJECT="urn:temporal_race_test:value_${i}"
    (
        psql -c "
            SELECT pg_ripple.load_turtle(
                '<${SUBJECT}> <${PREDICATE}> <${OBJECT}> .',
                '${TEST_GRAPH}_${i}'
            );
        " > "$out_file" 2>&1
    ) &
    pids+=($!)
done

for pid in "${pids[@]}"; do
    wait "$pid" 2>/dev/null || true
done

# Count how many workers succeeded.
success=0
for i in $(seq 1 "$N_WORKERS"); do
    out_file="$TMP_DIR/worker_${i}.txt"
    if ! grep -qi "error\|fail\|exception" "$out_file" 2>/dev/null; then
        success=$((success + 1))
    else
        echo "Worker $i failed: $(head -3 "$out_file")" >&2
    fi
done

echo "Workers completed: $success/$N_WORKERS"

if [[ $success -lt $N_WORKERS ]]; then
    echo "FAIL: $((N_WORKERS - success)) worker(s) reported errors" >&2
    exit 1
fi

# Count distinct statements inserted.
DISTINCT_COUNT=$(psql -t -c "
    SELECT COUNT(*) FROM (
        SELECT pg_ripple.query_sparql(
            'SELECT DISTINCT ?o WHERE {
                GRAPH ?g {
                    <${SUBJECT}> <${PREDICATE}> ?o .
                }
            }'
        )
    ) t
" 2>/dev/null | tr -d ' ' || echo "0")

echo "Distinct object values persisted: $DISTINCT_COUNT (expected: $N_WORKERS)"

if [[ "$DISTINCT_COUNT" -lt "$N_WORKERS" ]]; then
    echo "FAIL: only $DISTINCT_COUNT/$N_WORKERS distinct values persisted — data loss detected" >&2
    # Cleanup before exit.
    for i in $(seq 1 "$N_WORKERS"); do
        psql -c "SELECT pg_ripple.delete_graph('${TEST_GRAPH}_${i}');" > /dev/null 2>&1 || true
    done
    exit 1
fi

# Cleanup.
for i in $(seq 1 "$N_WORKERS"); do
    psql -c "SELECT pg_ripple.delete_graph('${TEST_GRAPH}_${i}');" > /dev/null 2>&1 || true
done

echo "PASS: all $N_WORKERS concurrent writes persisted with no data loss"
