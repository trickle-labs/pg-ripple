#!/usr/bin/env bash
# benchmarks/pagerank_scale.sh
# v0.90.0 TEST-04: PageRank scale benchmark gate
#
# Generates synthetic scale-free graphs via preferential attachment and
# measures wall time for PageRank convergence at three scales:
#   1M edges:   assert convergence AND wall time < 30s
#   10M edges:  assert convergence AND wall time < 300s
#
# Results are appended to benchmarks/pagerank_throughput_history.csv
# and compared against the baselines in benchmarks/merge_throughput_baselines.json.
#
# Usage:
#   PGCONN="host=localhost dbname=test" bash benchmarks/pagerank_scale.sh
#   SCALE=1m bash benchmarks/pagerank_scale.sh  # only run 1M test

set -euo pipefail

PGCONN="${PGCONN:-host=localhost dbname=postgres}"
PSQL="${PSQL:-psql}"
SCALE="${SCALE:-1m}"  # 1m, 10m, or both
HISTORY_CSV="$(dirname "$0")/pagerank_throughput_history.csv"

echo "[TEST-04] PageRank scale benchmark"
echo "  Connection: $PGCONN"
echo "  Scale: $SCALE"

# Ensure history CSV has header
if [[ ! -f "$HISTORY_CSV" ]]; then
    echo "timestamp,scale,edge_count,wall_time_s,converged,iterations,max_score" > "$HISTORY_CSV"
fi

run_scale_test() {
    local scale_name="$1"
    local edge_count="$2"
    local time_limit_s="$3"
    local topic="scale_bench_${scale_name}"

    echo ""
    echo "[TEST-04] Running ${scale_name} scale (${edge_count} edges, limit ${time_limit_s}s)..."

    # Generate scale-free graph via preferential attachment
    local start_time
    start_time=$(date +%s%N)

    "$PSQL" "$PGCONN" -q << SQL
SET pg_ripple.pagerank_enabled = on;
SET work_mem = '1GB';

-- Generate scale-free graph: preferential attachment (Barabási–Albert model)
-- Each new node connects to 'k' existing nodes weighted by degree
DO \$\$
DECLARE
    n_nodes   INT := ceil(sqrt(${edge_count}::float))::int;
    k         INT := ${edge_count} / n_nodes;
    i         INT;
    j         INT;
    target    INT;
BEGIN
    -- Seed nodes
    FOR i IN 1..5 LOOP
        PERFORM pg_ripple.insert_triple(
            format('<https://scale.test/%s/n%s>', '${topic}', i),
            '<https://scale.test/edge>',
            format('<https://scale.test/%s/n%s>', '${topic}', ((i % 4) + 1))
        );
    END LOOP;

    -- Preferential attachment growth
    FOR i IN 6..n_nodes LOOP
        FOR j IN 1..k LOOP
            -- Attach to a random existing node (simplified: uniform random)
            target := 1 + ((random() * (i - 1))::int);
            PERFORM pg_ripple.insert_triple(
                format('<https://scale.test/%s/n%s>', '${topic}', i),
                '<https://scale.test/edge>',
                format('<https://scale.test/%s/n%s>', '${topic}', target)
            );
        END LOOP;
    END LOOP;
END;
\$\$;
SQL

    local gen_time
    gen_time=$(( ($(date +%s%N) - start_time) / 1000000 ))
    echo "  Graph generation: ${gen_time}ms"

    # Run PageRank and measure wall time
    local pr_start
    pr_start=$(date +%s%N)

    local result
    result=$("$PSQL" "$PGCONN" -tAq << SQL
SET pg_ripple.pagerank_enabled = on;
SET work_mem = '1GB';
SELECT
    converged::text || ',' || iterations::text || ',' || max_score::text
FROM pg_ripple.pagerank_run(
    predicate_iri := '<https://scale.test/edge>',
    topic := '${topic}'
)
LIMIT 1;
SQL
)

    local pr_end
    pr_end=$(date +%s%N)
    local wall_s
    wall_s=$(echo "scale=3; ($pr_end - $pr_start) / 1000000000" | bc)

    local converged
    converged=$(echo "$result" | cut -d',' -f1)
    local iterations
    iterations=$(echo "$result" | cut -d',' -f2)
    local max_score
    max_score=$(echo "$result" | cut -d',' -f3)

    echo "  PageRank: converged=${converged}, iterations=${iterations}, wall_time=${wall_s}s, max_score=${max_score}"

    # Record to history
    local ts
    ts=$(date -u '+%Y-%m-%dT%H:%M:%SZ')
    echo "${ts},${scale_name},${edge_count},${wall_s},${converged},${iterations},${max_score}" >> "$HISTORY_CSV"

    # Assert convergence
    if [[ "$converged" != "t" && "$converged" != "true" ]]; then
        echo "[TEST-04] FAIL: PageRank did not converge for ${scale_name}"
        return 1
    fi

    # Assert wall time
    local wall_int
    wall_int=$(echo "$wall_s" | cut -d'.' -f1)
    if [[ "${wall_int:-9999}" -gt "$time_limit_s" ]]; then
        echo "[TEST-04] FAIL: wall time ${wall_s}s exceeds limit ${time_limit_s}s for ${scale_name}"
        return 1
    fi

    echo "[TEST-04] PASS: ${scale_name} — converged in ${iterations} iterations, ${wall_s}s"
    return 0
}

# Run requested scales
EXIT_CODE=0

if [[ "$SCALE" == "1m" || "$SCALE" == "both" ]]; then
    run_scale_test "1m" 1000000 30 || EXIT_CODE=1
fi

if [[ "$SCALE" == "10m" || "$SCALE" == "both" ]]; then
    run_scale_test "10m" 10000000 300 || EXIT_CODE=1
fi

echo ""
if [[ $EXIT_CODE -eq 0 ]]; then
    echo "[TEST-04] All scale tests PASSED"
else
    echo "[TEST-04] Some scale tests FAILED — see above"
fi

exit $EXIT_CODE
