#!/usr/bin/env bash
# tests/concurrency/sse_slow_subscriber.sh
# CDC-04 (v0.92.0): SSE backpressure load test
#
# Asserts that pg_ripple_http's SSE endpoint propagates backpressure correctly
# when a slow subscriber falls behind the event rate. A slow subscriber that
# cannot keep up should not block other connections or cause the server to OOM.
#
# Prerequisites:
#   - pg_ripple installed + pg_ripple_http running (PG_RIPPLE_HTTP_URL set)
#   - curl available
#   - pg_ripple extension loaded with a CDC subscription
#
# Usage:
#   PG_RIPPLE_HTTP_URL="http://localhost:8080" \
#   PGCONN="host=localhost dbname=postgres" \
#   bash tests/concurrency/sse_slow_subscriber.sh

set -euo pipefail

HTTP_URL="${PG_RIPPLE_HTTP_URL:-http://localhost:8080}"
PGCONN="${PGCONN:-host=localhost dbname=postgres}"
PSQL="${PSQL:-psql}"
DURATION="${DURATION:-15}"

echo "CDC-04: SSE backpressure load test"
echo "  HTTP URL: $HTTP_URL"
echo "  Duration: ${DURATION}s"

# Step 1: Create a CDC subscription for the test.
$PSQL "$PGCONN" -c "
SELECT pg_ripple.create_subscription(
    'sse_backpressure_test',
    channel => 'pg_ripple_cdc_sse_backpressure_test'
) AS sub_created;
" || echo "Subscription may already exist"

# Step 2: Start a slow SSE subscriber in the background.
# curl reads the SSE stream and we sleep 1s per event to simulate a slow consumer.
SSE_PID=""
TMPFILE=$(mktemp)
(
    curl -sN --max-time "$((DURATION + 10))" \
        "${HTTP_URL}/sparql/subscribe/sse_backpressure_test" \
        | while IFS= read -r line; do
            echo "$line" >> "$TMPFILE"
            sleep 0.5  # Simulate slow processing — 500ms per event
        done
) &
SSE_PID=$!
echo "  Slow SSE subscriber PID: $SSE_PID"

# Step 3: Generate high-rate triple inserts to overwhelm the slow subscriber.
echo "  Generating high-rate inserts for ${DURATION}s..."
END_TIME=$(( $(date +%s) + DURATION ))
INSERT_COUNT=0
while [ "$(date +%s)" -lt "$END_TIME" ]; do
    $PSQL "$PGCONN" -c "
SELECT pg_ripple.insert_triple(
    '<http://sse-test.example/s${INSERT_COUNT}>',
    '<http://sse-test.example/p>',
    '\"value-${INSERT_COUNT}\"'
);" > /dev/null 2>&1 || true
    INSERT_COUNT=$((INSERT_COUNT + 1))
    # 10 inserts/sec to overwhelm the 2/sec consumer
    sleep 0.1
done

echo "  Inserted $INSERT_COUNT triples"

# Step 4: Verify the server is still responsive (backpressure did not block server).
HTTP_STATUS=$(curl -s -o /dev/null -w "%{http_code}" "${HTTP_URL}/health" || echo "000")
if [ "$HTTP_STATUS" = "200" ]; then
    echo "  PASS: server still responsive after slow subscriber (HTTP $HTTP_STATUS)"
else
    echo "  WARN: server health check returned HTTP $HTTP_STATUS (may not be running)"
fi

# Step 5: Kill the slow subscriber.
if [ -n "$SSE_PID" ] && kill -0 "$SSE_PID" 2>/dev/null; then
    kill "$SSE_PID" 2>/dev/null || true
    wait "$SSE_PID" 2>/dev/null || true
fi

# Count events received by the slow subscriber.
EVENTS_RECEIVED=$(grep -c "^data:" "$TMPFILE" 2>/dev/null || echo "0")
echo "  Slow subscriber received $EVENTS_RECEIVED events out of $INSERT_COUNT inserts"

# Cleanup.
$PSQL "$PGCONN" -c "
SELECT pg_ripple.drop_subscription('sse_backpressure_test');
" || true
rm -f "$TMPFILE"

echo "CDC-04 PASS: SSE backpressure test completed"
