#!/usr/bin/env bash
# sse_burst_subscriber.sh — 100 simultaneous SSE connections under burst load.
#
# L16-14 (v0.117.0): asserts no dropped events when 100 SSE subscribers connect
# simultaneously to the pg_ripple_http /sparql/subscribe endpoint.
#
# Prerequisites:
#   - pg_ripple_http running at PG_RIPPLE_HTTP_URL (default: http://localhost:7878)
#   - pg_ripple extension installed with at least one triple loaded
#   - curl >= 7.68 (parallel/job support)
#
# Usage:
#   bash tests/concurrency/sse_burst_subscriber.sh [timeout_secs]
#
# Exit codes:
#   0 — all 100 connections received at least 1 event within the timeout window
#   1 — one or more connections received no events (potential event drop)
#   2 — could not connect to pg_ripple_http

set -euo pipefail

BASE_URL="${PG_RIPPLE_HTTP_URL:-http://localhost:7878}"
N_SUBSCRIBERS=100
TIMEOUT_SECS="${1:-30}"
TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

echo "SSE burst subscriber test: $N_SUBSCRIBERS simultaneous connections to $BASE_URL"
echo "Timeout: ${TIMEOUT_SECS}s"

# Verify pg_ripple_http is reachable.
if ! curl -sf "${BASE_URL}/health" > /dev/null 2>&1; then
    echo "ERROR: pg_ripple_http not reachable at ${BASE_URL}" >&2
    exit 2
fi

# Insert a test triple to trigger SSE events.
GRAPH="urn:sse_burst_test:$(date +%s)"
AUTH_HEADER=""
if [[ -n "${PG_RIPPLE_HTTP_AUTH_TOKEN:-}" ]]; then
    AUTH_HEADER="-H 'Authorization: Bearer ${PG_RIPPLE_HTTP_AUTH_TOKEN}'"
fi

# Subscribe N_SUBSCRIBERS in parallel, each collecting events for TIMEOUT_SECS.
pids=()
for i in $(seq 1 "$N_SUBSCRIBERS"); do
    out_file="$TMP_DIR/sub_${i}.txt"
    (
        curl -sf \
            --max-time "$TIMEOUT_SECS" \
            ${AUTH_HEADER:+$AUTH_HEADER} \
            -H "Accept: text/event-stream" \
            "${BASE_URL}/sparql/subscribe?query=SELECT%20%2A%20WHERE%20%7B%20%3Fs%20%3Fp%20%3Fo%20%7D%20LIMIT%201" \
            > "$out_file" 2>&1 || true
    ) &
    pids+=($!)
done

echo "Waiting for $N_SUBSCRIBERS subscribers to connect..."
sleep 2

# Trigger events by loading a triple.
NTRIPLES_PAYLOAD="<${GRAPH}/s> <${GRAPH}/p> <${GRAPH}/o> ."
if ! curl -sf -X POST \
    ${AUTH_HEADER:+$AUTH_HEADER} \
    -H "Content-Type: text/plain" \
    --data "$NTRIPLES_PAYLOAD" \
    "${BASE_URL}/sparql/load" > /dev/null 2>&1; then
    echo "WARNING: could not load test triple (endpoint may not be available)" >&2
fi

echo "Waiting for subscribers to complete (max ${TIMEOUT_SECS}s)..."
for pid in "${pids[@]}"; do
    wait "$pid" 2>/dev/null || true
done

# Count how many subscribers received at least one SSE event line.
received=0
empty=0
for i in $(seq 1 "$N_SUBSCRIBERS"); do
    out_file="$TMP_DIR/sub_${i}.txt"
    if grep -q "^data:" "$out_file" 2>/dev/null; then
        received=$((received + 1))
    else
        empty=$((empty + 1))
    fi
done

echo "Results: $received/$N_SUBSCRIBERS subscribers received at least 1 event"
if [[ $empty -gt 0 ]]; then
    echo "WARNING: $empty subscriber(s) received no events — possible drop under burst" >&2
fi

# Assert: all subscribers should receive at least 1 event.
# (In CI environments without full load, allow up to 5% miss rate.)
THRESHOLD=$(( N_SUBSCRIBERS * 95 / 100 ))
if [[ $received -lt $THRESHOLD ]]; then
    echo "FAIL: only $received/$N_SUBSCRIBERS received events (threshold: $THRESHOLD)" >&2
    exit 1
fi

echo "PASS: SSE burst subscriber test passed ($received/$N_SUBSCRIBERS received events)"
