#!/usr/bin/env bash
# check_no_unreachable_in_production.sh — M15-01 (v0.95.0)
#
# Ensures that bare `unreachable!()` calls are not present in production
# source files under src/pagerank/ and src/storage/.  These paths represent
# extension code that runs inside the PostgreSQL server process; a panic
# from unreachable!() would crash the backend.  Use pgrx::error!() instead
# to produce a clean, user-visible error message.
#
# Usage:  bash scripts/check_no_unreachable_in_production.sh
# Exit:   0 = clean; 1 = violations found

set -euo pipefail

DIRS=(src/pagerank src/storage)
PATTERN='unreachable!()'
VIOLATIONS=0

for dir in "${DIRS[@]}"; do
    if [[ ! -d "$dir" ]]; then
        continue
    fi
    while IFS= read -r -d '' file; do
        if grep -n 'unreachable!()' "$file"; then
            echo "ERROR: $file contains bare unreachable!() — replace with pgrx::error!()" >&2
            VIOLATIONS=$((VIOLATIONS + 1))
        fi
    done < <(find "$dir" -name '*.rs' -print0)
done

if [[ $VIOLATIONS -gt 0 ]]; then
    echo ""
    echo "Found $VIOLATIONS file(s) with unreachable!() in production source."
    echo "Replace each occurrence with pgrx::error!(\"...\") to produce a clean error."
    exit 1
fi

echo "check_no_unreachable_in_production: clean (no bare unreachable!() in ${DIRS[*]})"
exit 0
