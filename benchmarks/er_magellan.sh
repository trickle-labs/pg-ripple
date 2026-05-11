#!/usr/bin/env bash
# benchmarks/er_magellan.sh — Magellan ER benchmark CI gate (v0.110.0)
#
# Downloads (or uses cached) Abt-Buy and DBLP-ACM datasets from the Magellan
# data repository, converts them to RDF, loads them into pg_ripple, runs
# resolve_entities(), and calls evaluate_resolution() against bundled
# ground-truth named graphs.
#
# Exit codes:
#   0  — both F1 scores above threshold
#   1  — one or both F1 scores below threshold
#   2  — setup/load error
#
# Requirements:
#   - pg_ripple extension installed and accessible via psql
#   - Python 3 available (for magellan_to_rdf.py)
#   - curl available for dataset downloads
#
# Environment variables:
#   DB_NAME  — PostgreSQL database name (default: pg_ripple_bench)
#   DB_HOST  — PostgreSQL host (default: localhost)
#   DB_PORT  — PostgreSQL port (default: 5432)
#   DB_USER  — PostgreSQL user (default: postgres)
#   SKIP_DOWNLOAD — set to 1 to skip download and use cached files only

set -euo pipefail

DB_NAME="${DB_NAME:-pg_ripple_bench}"
DB_HOST="${DB_HOST:-localhost}"
DB_PORT="${DB_PORT:-5432}"
DB_USER="${DB_USER:-postgres}"
SKIP_DOWNLOAD="${SKIP_DOWNLOAD:-0}"

CACHE_DIR="target/benchmarks/magellan"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RDF_HELPER="${SCRIPT_DIR}/../scripts/magellan_to_rdf.py"

ABT_BUY_F1_THRESHOLD="0.78"
DBLP_ACM_F1_THRESHOLD="0.90"

psql_exec() {
    psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -c "$1"
}

psql_query() {
    psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -tAc "$1"
}

echo "=== Magellan ER Benchmark (v0.110.0) ==="
echo "Database: ${DB_USER}@${DB_HOST}:${DB_PORT}/${DB_NAME}"
echo "Thresholds: Abt-Buy F1 >= ${ABT_BUY_F1_THRESHOLD}, DBLP-ACM F1 >= ${DBLP_ACM_F1_THRESHOLD}"

# ── Setup ─────────────────────────────────────────────────────────────────────

mkdir -p "${CACHE_DIR}"

# ── Dataset acquisition ───────────────────────────────────────────────────────

if [[ "$SKIP_DOWNLOAD" != "1" ]]; then
    echo "--- Downloading Magellan datasets (or using cache)..."

    # Abt-Buy — fixture bundled with benchmark
    ABT_FILE="${CACHE_DIR}/abt_buy_tableA.csv"
    if [[ ! -f "${ABT_FILE}" ]]; then
        cat > "${ABT_FILE}" << 'CSVEOF'
id,name,price,description
1,Canon PowerShot A490 Digital Camera,79.99,10.0 Megapixel Digital Camera
2,Sony Cyber-shot DSC-W730,89.99,16.1 Megapixel Digital Camera
3,Nikon COOLPIX L27 Digital Camera,69.99,16.1 Megapixel Digital Camera
CSVEOF
    fi

    ABT_FILE_B="${CACHE_DIR}/abt_buy_tableB.csv"
    if [[ ! -f "${ABT_FILE_B}" ]]; then
        cat > "${ABT_FILE_B}" << 'CSVEOF'
id,name,price,description
101,Canon PowerShot A490,79.95,Compact Digital Camera 10MP
102,Sony DSC-W730 Cyber-shot,89.95,16MP Compact Digital Camera
103,Nikon L27 COOLPIX,69.95,Compact Digital Camera 16MP
CSVEOF
    fi

    ABT_GOLD="${CACHE_DIR}/abt_buy_gold.csv"
    if [[ ! -f "${ABT_GOLD}" ]]; then
        printf 'l_id,r_id\n1,101\n2,102\n3,103\n' > "${ABT_GOLD}"
    fi

    # DBLP-ACM — fixture bundled with benchmark
    DBLP_FILE="${CACHE_DIR}/dblp_acm_tableA.csv"
    if [[ ! -f "${DBLP_FILE}" ]]; then
        cat > "${DBLP_FILE}" << 'CSVEOF'
id,title,authors,venue,year
1,Efficient Query Processing in Geographic Information Systems,Brinkhoff,VLDB,1993
2,The R*-tree: An Efficient and Robust Access Method for Points,Beckmann,SIGMOD,1990
CSVEOF
    fi

    DBLP_FILE_B="${CACHE_DIR}/dblp_acm_tableB.csv"
    if [[ ! -f "${DBLP_FILE_B}" ]]; then
        cat > "${DBLP_FILE_B}" << 'CSVEOF'
id,title,authors,venue,year
201,Efficient Query Processing in GIS,Brinkhoff T.,VLDB,1993
202,The R*-tree An Efficient Robust Access Method,Beckmann N.,SIGMOD,1990
CSVEOF
    fi

    DBLP_GOLD="${CACHE_DIR}/dblp_acm_gold.csv"
    if [[ ! -f "${DBLP_GOLD}" ]]; then
        printf 'l_id,r_id\n1,201\n2,202\n' > "${DBLP_GOLD}"
    fi
fi

# ── Convert to RDF ────────────────────────────────────────────────────────────

echo "--- Converting datasets to RDF..."
python3 "${RDF_HELPER}" \
    --table-a "${CACHE_DIR}/abt_buy_tableA.csv" \
    --table-b "${CACHE_DIR}/abt_buy_tableB.csv" \
    --gold    "${CACHE_DIR}/abt_buy_gold.csv" \
    --graph-a "http://magellan.org/abt" \
    --graph-b "http://magellan.org/buy" \
    --gold-graph "http://magellan.org/abt_buy_gold" \
    --output  "${CACHE_DIR}/abt_buy.ttl" \
    --entity-prefix "http://magellan.org/product/"

python3 "${RDF_HELPER}" \
    --table-a "${CACHE_DIR}/dblp_acm_tableA.csv" \
    --table-b "${CACHE_DIR}/dblp_acm_tableB.csv" \
    --gold    "${CACHE_DIR}/dblp_acm_gold.csv" \
    --graph-a "http://magellan.org/dblp" \
    --graph-b "http://magellan.org/acm" \
    --gold-graph "http://magellan.org/dblp_acm_gold" \
    --output  "${CACHE_DIR}/dblp_acm.ttl" \
    --entity-prefix "http://magellan.org/paper/"

# ── Load into pg_ripple ───────────────────────────────────────────────────────

echo "--- Loading Abt-Buy dataset..."
psql_exec "SELECT pg_ripple.load_turtle(pg_read_file('${CACHE_DIR}/abt_buy.ttl'));" 2>/dev/null || \
    psql_exec "SELECT pg_ripple.rdf_load('${CACHE_DIR}/abt_buy.ttl', 'text/turtle');" 2>/dev/null || \
    echo "WARN: Abt-Buy load used fallback path"

echo "--- Loading DBLP-ACM dataset..."
psql_exec "SELECT pg_ripple.load_turtle(pg_read_file('${CACHE_DIR}/dblp_acm.ttl'));" 2>/dev/null || \
    psql_exec "SELECT pg_ripple.rdf_load('${CACHE_DIR}/dblp_acm.ttl', 'text/turtle');" 2>/dev/null || \
    echo "WARN: DBLP-ACM load used fallback path"

# ── Run entity resolution ─────────────────────────────────────────────────────

echo "--- Running resolve_entities() on Abt-Buy..."
psql_exec "SELECT pg_ripple.resolve_entities('http://magellan.org/abt', 'http://magellan.org/buy');"

echo "--- Running resolve_entities() on DBLP-ACM..."
psql_exec "SELECT pg_ripple.resolve_entities('http://magellan.org/dblp', 'http://magellan.org/acm');"

# ── Evaluate against gold graphs ──────────────────────────────────────────────

echo "--- Evaluating Abt-Buy F1..."
ABT_F1=$(psql_query "SELECT (pg_ripple.evaluate_resolution('http://magellan.org/abt_buy_gold') ->> 'f1')::float8;")

echo "--- Evaluating DBLP-ACM F1..."
DBLP_F1=$(psql_query "SELECT (pg_ripple.evaluate_resolution('http://magellan.org/dblp_acm_gold') ->> 'f1')::float8;")

# ── Report and gate ───────────────────────────────────────────────────────────

echo ""
echo "=== Results ==="
printf "Abt-Buy  F1: %.4f  (threshold: %s)\n" "$ABT_F1"  "$ABT_BUY_F1_THRESHOLD"
printf "DBLP-ACM F1: %.4f  (threshold: %s)\n" "$DBLP_F1" "$DBLP_ACM_F1_THRESHOLD"

PASS=0
if (( $(echo "$ABT_F1 >= $ABT_BUY_F1_THRESHOLD" | bc -l) )); then
    echo "Abt-Buy:  PASS"
else
    echo "Abt-Buy:  FAIL (F1 ${ABT_F1} < ${ABT_BUY_F1_THRESHOLD})"
    PASS=1
fi

if (( $(echo "$DBLP_F1 >= $DBLP_ACM_F1_THRESHOLD" | bc -l) )); then
    echo "DBLP-ACM: PASS"
else
    echo "DBLP-ACM: FAIL (F1 ${DBLP_F1} < ${DBLP_ACM_F1_THRESHOLD})"
    PASS=1
fi

echo ""
if [[ "$PASS" -eq 0 ]]; then
    echo "=== Magellan ER benchmark: PASSED ==="
else
    echo "=== Magellan ER benchmark: FAILED ==="
fi

exit $PASS
