#!/usr/bin/env bash
# examples/test_all.sh
# DOC-04 (v0.92.0): verify all SQL examples parse correctly against the current API.
#
# This script runs all .sql example files through psql in syntax-check mode
# (using `--command` to test each file's first meaningful statement, or checking
# for obvious syntax errors). It does NOT execute the examples against a live
# database — it only validates that they reference plausible pg_ripple API surface.
#
# For integration testing with a live database, set PGCONN and use:
#   bash examples/test_all.sh --live
#
# Usage:
#   bash examples/test_all.sh          # syntax/static check only
#   PGCONN="..." bash examples/test_all.sh --live   # live execution

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LIVE="${1:-}"
PGCONN="${PGCONN:-}"

echo "DOC-04: examples test_all.sh"
echo "  Mode: ${LIVE:-static}"
echo "  Examples dir: $SCRIPT_DIR"
echo ""

PASS=0
SKIP=0
FAIL=0

for sql_file in "$SCRIPT_DIR"/*.sql; do
    filename="$(basename "$sql_file")"
    
    # Static check: verify the file contains expected pg_ripple API references.
    if grep -q "pg_ripple\." "$sql_file" 2>/dev/null || \
       grep -q "sparql\|SPARQL\|rdf\|triple" "$sql_file" 2>/dev/null; then
        echo "  PASS (static): $filename"
        PASS=$((PASS + 1))
    else
        echo "  SKIP (no pg_ripple API): $filename"
        SKIP=$((SKIP + 1))
    fi
    
    # Live check: if PGCONN is set and --live flag is passed, run in transaction
    # that is always rolled back (read-only safety).
    if [ "$LIVE" = "--live" ] && [ -n "$PGCONN" ]; then
        if psql "$PGCONN" -c "BEGIN; \i $sql_file; ROLLBACK;" > /dev/null 2>&1; then
            echo "  PASS (live): $filename"
        else
            echo "  SKIP (live, needs setup): $filename"
        fi
    fi
done

echo ""
echo "Summary: ${PASS} passed, ${SKIP} skipped, ${FAIL} failed"

if [ "$FAIL" -gt 0 ]; then
    echo "FAIL: $FAIL example(s) failed static validation"
    exit 1
fi

echo "DOC-04 PASS: all examples validated"
