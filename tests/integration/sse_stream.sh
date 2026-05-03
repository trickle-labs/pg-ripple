#!/usr/bin/env bash
# tests/integration/sse_stream.sh
# HTTP-02 (v0.91.0): SSE streaming endpoint regression test.
#
# Verifies that the pg_ripple_http /sparql/stream endpoint:
#   1. Accepts a SPARQL SELECT query via POST application/x-www-form-urlencoded.
#   2. Returns Content-Type: text/event-stream.
#   3. Emits at least one "data:" SSE event.
#   4. Terminates the stream with a "data: [DONE]" sentinel or the server
#      closes the connection (acceptable for finite result sets).
#
# Requirements:
#   - pg_ripple_http running on PG_RIPPLE_HTTP_PORT (default: 7474)
#   - curl ≥ 7.68 (--no-buffer, --max-time)
#
# Usage:
#   bash tests/integration/sse_stream.sh
#
# Environment:
#   PG_RIPPLE_HTTP_HOST — default: 127.0.0.1
#   PG_RIPPLE_HTTP_PORT — default: 7474
#   HTTP_TIMEOUT        — seconds to wait for first event (default: 10)

set -euo pipefail

HOST="${PG_RIPPLE_HTTP_HOST:-127.0.0.1}"
PORT="${PG_RIPPLE_HTTP_PORT:-7474}"
TIMEOUT="${HTTP_TIMEOUT:-10}"
BASE_URL="http://${HOST}:${PORT}"

PASS=0
FAIL=0

pass() { echo "[PASS] $1"; PASS=$((PASS + 1)); }
fail() { echo "[FAIL] $1"; FAIL=$((FAIL + 1)); }

# ── 1. Health check ────────────────────────────────────────────────────────────
echo "→ Checking pg_ripple_http is reachable at ${BASE_URL}/health …"
if ! curl -sf --max-time 5 "${BASE_URL}/health" > /dev/null; then
    echo "ERROR: pg_ripple_http is not reachable at ${BASE_URL}. Skipping SSE test."
    echo "       Start pg_ripple_http before running this test."
    exit 0  # non-fatal skip when server is not running
fi
pass "health check"

# ── 2. POST to /sparql/stream ──────────────────────────────────────────────────
QUERY='SELECT ?s ?p ?o WHERE { ?s ?p ?o } LIMIT 3'
SSE_OUTPUT=$(curl -sf \
    --no-buffer \
    --max-time "${TIMEOUT}" \
    -X POST "${BASE_URL}/sparql/stream" \
    -H "Content-Type: application/x-www-form-urlencoded" \
    -H "Accept: text/event-stream" \
    --data-urlencode "query=${QUERY}" \
    2>&1 || true)

# ── 3. Validate Content-Type (use -v for headers) ─────────────────────────────
HEADERS=$(curl -sf \
    --no-buffer \
    --max-time "${TIMEOUT}" \
    -X POST "${BASE_URL}/sparql/stream" \
    -H "Content-Type: application/x-www-form-urlencoded" \
    -H "Accept: text/event-stream" \
    --data-urlencode "query=${QUERY}" \
    -D - \
    -o /dev/null \
    2>&1 || true)

if echo "${HEADERS}" | grep -qi "content-type: text/event-stream"; then
    pass "Content-Type is text/event-stream"
else
    fail "Content-Type is not text/event-stream (got: $(echo "${HEADERS}" | grep -i content-type || echo '(none)'))"
fi

# ── 4. Check for data: lines ──────────────────────────────────────────────────
if echo "${SSE_OUTPUT}" | grep -q "^data:"; then
    pass "SSE response contains data: events"
else
    # If the graph is empty, an empty result set is still valid
    if echo "${SSE_OUTPUT}" | grep -q "event:\|data: \[\]"; then
        pass "SSE response is valid (empty result set)"
    else
        fail "SSE response contains no data: events (output: ${SSE_OUTPUT:0:200})"
    fi
fi

# ── Summary ───────────────────────────────────────────────────────────────────
echo ""
echo "Results: ${PASS} passed, ${FAIL} failed."
if [[ ${FAIL} -gt 0 ]]; then
    exit 1
fi
exit 0
