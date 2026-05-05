#!/usr/bin/env bash
# scripts/check_security_definer_search_path.sh — v0.94.0
#
# H15-02 (v0.94.0): Verify every SECURITY DEFINER function in the current
# extension source has an explicit SET search_path clause.
#
# A SECURITY DEFINER function without a pinned search_path is vulnerable to
# search-path injection attacks.  Pinning prevents an unprivileged caller
# from overriding the resolution of unqualified names.
#
# Scans: src/**/*.rs  (Rust source — contains embedded SQL in extension_sql! macros)
# Skips: sql/pg_ripple--*.sql  (historical migration scripts — immutable once shipped)
#
# A SECURITY DEFINER occurrence is a violation unless:
#   (a) it appears in a comment line (starts with -- or //), OR
#   (b) a "SET search_path" clause appears within ±8 lines of the same function.
#
# Usage:
#   bash scripts/check_security_definer_search_path.sh
#   Exit 0 = all SECURITY DEFINER functions have a SET search_path.
#   Exit 1 = at least one is missing.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

echo "Scanning src/ for SECURITY DEFINER without SET search_path ..."

VIOLATIONS=0

check_file() {
    local file="$1"
    local total
    total=$(wc -l < "$file")

    while IFS=: read -r lineno line; do
        # Skip comment lines (-- is SQL comment; // is Rust comment).
        stripped="${line#"${line%%[![:space:]]*}"}"   # ltrim
        if [[ "$stripped" == --* ]] || [[ "$stripped" == //* ]]; then
            continue
        fi

        # Build context window ±8 lines.
        local start end
        start=$(( lineno - 8 ))
        end=$(( lineno + 8 ))
        [[ $start -lt 1 ]] && start=1
        [[ $end -gt $total ]] && end=$total

        context=$(awk "NR>=${start} && NR<=${end}" "$file")
        if echo "$context" | grep -qiE "SET[[:space:]]+search_path[[:space:]]*(=|TO)"; then
            echo "OK (search_path set) in ${file}:${lineno}"
        else
            echo "VIOLATION in ${file}:${lineno} — SECURITY DEFINER without SET search_path:"
            awk "NR==${lineno}" "$file"
            echo "  Fix: Add 'SET search_path = pg_catalog, _pg_ripple, public' to the function."
            VIOLATIONS=$(( VIOLATIONS + 1 ))
        fi
    done < <(grep -n "SECURITY[[:space:]]\+DEFINER" "$file" 2>/dev/null || true)
}

# Only scan Rust source files — migration SQL is immutable once shipped.
rs_matches=$(grep -ril "SECURITY[[:space:]]\+DEFINER" "${ROOT}/src" --include="*.rs" 2>/dev/null || true)

if [[ -z "${rs_matches}" ]]; then
    echo "OK: no SECURITY DEFINER directives found in src/."
    exit 0
fi

while IFS= read -r file; do
    check_file "$file"
done <<< "${rs_matches}"

if [[ $VIOLATIONS -gt 0 ]]; then
    echo ""
    echo "ERROR: $VIOLATIONS SECURITY DEFINER function(s) in src/ lack a SET search_path clause."
    echo "Add \"SET search_path = pg_catalog, _pg_ripple, public\" to each SECURITY DEFINER function."
    exit 1
fi

echo "OK: all SECURITY DEFINER functions in src/ have a SET search_path clause."
