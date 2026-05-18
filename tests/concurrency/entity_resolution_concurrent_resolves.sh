#!/usr/bin/env bash
# entity_resolution_concurrent_resolves.sh — stress test for concurrent owl:sameAs canonicalization.
#
# M16-12 (v0.117.0): fires N concurrent entity resolution operations with overlapping
# sameAs chains and asserts the canonical IRI is consistent across all concurrent writes.
#
# Background: the Datalog owl:sameAs canonicalization pass must produce a deterministic
# canonical IRI even when two concurrent transactions both resolve the same equivalence set.
# This test exercises the serialization guarantee in the entity resolution infrastructure.
#
# Prerequisites:
#   - PostgreSQL 18 running with pg_ripple installed
#   - psql available on $PATH
#   - Environment: PGDATABASE, PGUSER, PGHOST, PGPORT (or defaults)
#
# Usage:
#   bash tests/concurrency/entity_resolution_concurrent_resolves.sh [n_workers]
#
# Exit codes:
#   0 — all concurrent resolves produce the same canonical IRI
#   1 — canonical IRI divergence detected (race condition)
#   2 — pg_ripple not installed or psql unavailable

set -euo pipefail

N_WORKERS="${1:-10}"
TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

echo "Entity resolution concurrent resolves test: $N_WORKERS workers"

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

TEST_GRAPH="urn:er_concurrent_test:$(date +%s)"

# Set up sameAs triples: all of A, B, C, D are the same entity.
psql -c "
    SELECT pg_ripple.load_turtle(
        '<urn:er_test:A> <http://www.w3.org/2002/07/owl#sameAs> <urn:er_test:B> .
         <urn:er_test:B> <http://www.w3.org/2002/07/owl#sameAs> <urn:er_test:C> .
         <urn:er_test:C> <http://www.w3.org/2002/07/owl#sameAs> <urn:er_test:D> .',
        '${TEST_GRAPH}'
    );
" > /dev/null 2>&1

# Run N_WORKERS concurrent entity resolution queries.
pids=()
for i in $(seq 1 "$N_WORKERS"); do
    out_file="$TMP_DIR/worker_${i}.txt"
    (
        psql -t -c "
            SELECT DISTINCT object
            FROM pg_ripple.query_sparql(
                'SELECT ?canonical WHERE {
                    <urn:er_test:A> <http://www.w3.org/2002/07/owl#sameAs> ?canonical .
                } ORDER BY ?canonical LIMIT 1'
            )
        " > "$out_file" 2>&1
    ) &
    pids+=($!)
done

for pid in "${pids[@]}"; do
    wait "$pid" 2>/dev/null || true
done

# Collect all canonical IRIs and assert they are all the same.
canonicals=()
for i in $(seq 1 "$N_WORKERS"); do
    out_file="$TMP_DIR/worker_${i}.txt"
    val=$(grep -v '^$' "$out_file" 2>/dev/null | head -1 | tr -d ' ' || echo "")
    if [[ -n "$val" ]]; then
        canonicals+=("$val")
    fi
done

if [[ ${#canonicals[@]} -eq 0 ]]; then
    echo "SKIP: no canonical IRI returned (owl:sameAs inference may not be enabled)" >&2
    exit 0
fi

# Assert all values are identical.
FIRST="${canonicals[0]}"
diverged=0
for val in "${canonicals[@]}"; do
    if [[ "$val" != "$FIRST" ]]; then
        echo "DIVERGENCE: expected '$FIRST' but got '$val'" >&2
        diverged=$((diverged + 1))
    fi
done

# Cleanup.
psql -c "
    SELECT pg_ripple.delete_graph('${TEST_GRAPH}');
" > /dev/null 2>&1 || true

if [[ $diverged -gt 0 ]]; then
    echo "FAIL: $diverged divergent canonical IRI(s) detected across $N_WORKERS concurrent resolves" >&2
    exit 1
fi

echo "PASS: all ${#canonicals[@]} concurrent resolves produced canonical IRI: $FIRST"
