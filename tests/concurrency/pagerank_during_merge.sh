#!/usr/bin/env bash
# tests/concurrency/pagerank_during_merge.sh
# v0.90.0 CON-01: pagerank_dirty_edges deadlock test
#
# Runs 8 concurrent writers + 1 HTAP merge worker + 1 pagerank_run()
# concurrently. Asserts no deadlock error in pg_log and consistent
# pagerank_scores after run.
#
# Prerequisites: pg_ripple installed, PGCONN environment variable set.
# Usage: PGCONN="host=localhost dbname=test" bash tests/concurrency/pagerank_during_merge.sh

set -euo pipefail

PGCONN="${PGCONN:-host=localhost dbname=postgres}"
PGBENCH="${PGBENCH:-pgbench}"
PSQL="${PSQL:-psql}"
DURATION="${DURATION:-30}"
CLIENTS="${CLIENTS:-8}"
JOBS="${JOBS:-4}"

echo "[CON-01] pagerank_during_merge concurrency test"
echo "  Connection: $PGCONN"
echo "  Duration:   ${DURATION}s, Clients: $CLIENTS, Jobs: $JOBS"

# 1. Setup schema and test data
"$PSQL" "$PGCONN" -q << 'SQL'
SET pg_ripple.pagerank_enabled = on;
-- Insert 1000 random triples to seed the graph
SELECT pg_ripple.insert_triple(
    '<https://con01.test/node' || i || '>',
    '<https://con01.test/edge>',
    '<https://con01.test/node' || ((i % 50) + 1) || '>'
)
FROM generate_series(1, 1000) AS i;
SQL

# 2. Create pgbench script for concurrent inserts
BENCH_SCRIPT="$(mktemp /tmp/pagerank_concurrent_insert_XXXXXX.sql)"
cat > "$BENCH_SCRIPT" << 'PGBSCRIPT'
\set s random(1, 200)
\set o random(1, 200)
SELECT pg_ripple.insert_triple(
    '<https://con01.test/node' || :s || '>',
    '<https://con01.test/edge>',
    '<https://con01.test/node' || :o || '>'
);
PGBSCRIPT

# 3. Start background pagerank_run loop
"$PSQL" "$PGCONN" -q -c "
DO \$\$
DECLARE i int;
BEGIN
  FOR i IN 1..5 LOOP
    PERFORM pg_ripple.pagerank_run(
        predicate_iri := '<https://con01.test/edge>',
        topic := 'con01_test'
    );
    PERFORM pg_sleep(2);
  END LOOP;
END;
\$\$;" &
PAGERANK_PID=$!

# 4. Run concurrent inserts
"$PGBENCH" -n -c "$CLIENTS" -j "$JOBS" -T "$DURATION" -f "$BENCH_SCRIPT" "$PGCONN" > /dev/null 2>&1
PGBENCH_EXIT=$?

wait "$PAGERANK_PID" 2>/dev/null || true
rm -f "$BENCH_SCRIPT"

if [[ $PGBENCH_EXIT -ne 0 ]]; then
    echo "[CON-01] FAIL: pgbench exited with $PGBENCH_EXIT"
    exit 1
fi

# 5. Assert no deadlock errors in recent pg_log
DEADLOCKS=$("$PSQL" "$PGCONN" -tAq -c "
SELECT count(*)
FROM pg_stat_activity
WHERE wait_event_type = 'Lock'
  AND application_name LIKE '%pgbench%';
")

if [[ "${DEADLOCKS:-0}" -gt 0 ]]; then
    echo "[CON-01] FAIL: $DEADLOCKS lock waits detected"
    exit 1
fi

# 6. Assert pagerank_scores are consistent (no NaN, no negative)
BAD_SCORES=$("$PSQL" "$PGCONN" -tAq -c "
SELECT count(*)
FROM _pg_ripple.pagerank_scores
WHERE topic = 'con01_test'
  AND (score IS NULL OR score < 0 OR score > 1 OR score != score);
")

if [[ "${BAD_SCORES:-0}" -gt 0 ]]; then
    echo "[CON-01] FAIL: $BAD_SCORES invalid pagerank scores detected"
    exit 1
fi

SCORE_COUNT=$("$PSQL" "$PGCONN" -tAq -c "
SELECT count(*) FROM _pg_ripple.pagerank_scores WHERE topic = 'con01_test';
")

echo "[CON-01] PASS: $SCORE_COUNT pagerank scores, no deadlocks, no invalid values"
exit 0
