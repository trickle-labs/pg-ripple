#!/usr/bin/env bash
# check_module_sizes.sh — Enforce per-file Rust module size limits.
#
# Policy (from CONTRIBUTING.md):
#   WARN  at 1,200 LOC
#   FAIL  at 1,500 LOC
#
# Usage: scripts/check_module_sizes.sh [src_root]
#   src_root defaults to src/

set -euo pipefail

SRC_ROOT="${1:-src}"
WARN_LOC=1200
FAIL_LOC=1500

warns=0
fails=0

while IFS= read -r file; do
    lines=$(wc -l < "$file")
    rel="${file#./}"
    if (( lines >= FAIL_LOC )); then
        echo "FAIL  $lines LOC  $rel  (limit: ${FAIL_LOC})"
        (( fails++ )) || true
    elif (( lines >= WARN_LOC )); then
        echo "WARN  $lines LOC  $rel  (threshold: ${WARN_LOC})"
        (( warns++ )) || true
    fi
done < <(find "$SRC_ROOT" -name '*.rs' | sort)

if (( warns > 0 )); then
    echo ""
    echo "Warning: ${warns} file(s) exceed ${WARN_LOC} LOC. Consider splitting into sub-modules."
fi

if (( fails > 0 )); then
    echo ""
    echo "Error: ${fails} file(s) exceed the ${FAIL_LOC} LOC hard limit. CI cannot pass until these are split."
    exit 1
fi

echo "Module size check passed. No file exceeds ${FAIL_LOC} LOC."
exit 0
