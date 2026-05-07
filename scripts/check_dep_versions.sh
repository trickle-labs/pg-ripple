#!/usr/bin/env bash
# scripts/check_dep_versions.sh — DEP-VER-01
#
# Verify that external dependency versions are consistent across three places:
#   1. .versions.toml      — the single source of truth
#   2. Dockerfile ARG      — build-time pinning for the Docker image
#   3. src/lib.rs constants — runtime probe constants (PG_TRICKLE_TESTED_VERSION, etc.)
#
# PostGIS and pgvector have no corresponding Rust runtime constants (they are
# PostgreSQL extensions not detected at runtime), so only Dockerfile ARGs are
# checked for those.
#
# Usage:
#   bash scripts/check_dep_versions.sh
#   Exit 0 = all consistent; exit 1 = one or more mismatches found.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

VERSIONS_FILE="${ROOT}/.versions.toml"
DOCKERFILE="${ROOT}/Dockerfile"
LIBRS="${ROOT}/src/lib.rs"

FAILURES=0

# ── Helpers ───────────────────────────────────────────────────────────────────

# Read a version string from .versions.toml.
# Format: key = "value"   (under any section header)
# Usage: toml_get "pg_trickle"
toml_get() {
    grep "^$1[[:space:]]*=" "${VERSIONS_FILE}" \
        | head -1 \
        | sed 's/.*=[[:space:]]*"\([^"]*\)".*/\1/'
}

# Read a Dockerfile ARG default value.
# Format: ARG NAME=value
# Usage: dockerfile_get "PG_TRICKLE_VERSION"
dockerfile_get() {
    grep "^ARG $1=" "${DOCKERFILE}" \
        | head -1 \
        | sed 's/ARG [^=]*=\(.*\)/\1/'
}

# Read a Rust const string value from src/lib.rs.
# Format: const NAME: &str = "value";
# Usage: rust_const_get "PG_TRICKLE_TESTED_VERSION"
rust_const_get() {
    grep "^const $1:" "${LIBRS}" \
        | head -1 \
        | sed 's/.*=[[:space:]]*"\([^"]*\)".*/\1/'
}

# Compare expected vs actual and record failures.
# Usage: check "description" "expected" "actual" "fix hint"
check() {
    local desc="$1" expected="$2" actual="$3" hint="$4"
    if [ -z "${actual}" ]; then
        echo "MISSING  ${desc}"
        echo "         Expected : ${expected}"
        echo "         Fix      : ${hint}"
        FAILURES=$(( FAILURES + 1 ))
    elif [ "${expected}" != "${actual}" ]; then
        echo "MISMATCH ${desc}"
        echo "         .versions.toml : ${expected}"
        echo "         actual         : ${actual}"
        echo "         Fix            : ${hint}"
        FAILURES=$(( FAILURES + 1 ))
    else
        echo "OK       ${desc} = ${expected}"
    fi
}

# ── Checks ────────────────────────────────────────────────────────────────────

echo "=== Dependency version consistency check (source: .versions.toml) ==="
echo ""

# pg_trickle
CANONICAL_TRICKLE=$(toml_get "pg_trickle")
check \
    "Dockerfile ARG PG_TRICKLE_VERSION" \
    "${CANONICAL_TRICKLE}" \
    "$(dockerfile_get "PG_TRICKLE_VERSION")" \
    "Set 'ARG PG_TRICKLE_VERSION=${CANONICAL_TRICKLE}' in Dockerfile"
check \
    "src/lib.rs PG_TRICKLE_TESTED_VERSION" \
    "${CANONICAL_TRICKLE}" \
    "$(rust_const_get "PG_TRICKLE_TESTED_VERSION")" \
    "Set 'const PG_TRICKLE_TESTED_VERSION: &str = \"${CANONICAL_TRICKLE}\";' in src/lib.rs"

# pg_tide
CANONICAL_TIDE=$(toml_get "pg_tide")
check \
    "Dockerfile ARG PG_TIDE_VERSION" \
    "${CANONICAL_TIDE}" \
    "$(dockerfile_get "PG_TIDE_VERSION")" \
    "Set 'ARG PG_TIDE_VERSION=${CANONICAL_TIDE}' in Dockerfile"
check \
    "src/lib.rs PG_TIDE_TESTED_VERSION" \
    "${CANONICAL_TIDE}" \
    "$(rust_const_get "PG_TIDE_TESTED_VERSION")" \
    "Set 'const PG_TIDE_TESTED_VERSION: &str = \"${CANONICAL_TIDE}\";' in src/lib.rs"

# PostGIS (Docker only — no runtime probe constant)
CANONICAL_POSTGIS=$(toml_get "postgis")
check \
    "Dockerfile ARG POSTGIS_VERSION" \
    "${CANONICAL_POSTGIS}" \
    "$(dockerfile_get "POSTGIS_VERSION")" \
    "Set 'ARG POSTGIS_VERSION=${CANONICAL_POSTGIS}' in Dockerfile"

# pgvector (Docker only — no runtime probe constant)
CANONICAL_PGVECTOR=$(toml_get "pgvector")
check \
    "Dockerfile ARG PGVECTOR_VERSION" \
    "${CANONICAL_PGVECTOR}" \
    "$(dockerfile_get "PGVECTOR_VERSION")" \
    "Set 'ARG PGVECTOR_VERSION=${CANONICAL_PGVECTOR}' in Dockerfile"

# ── Result ────────────────────────────────────────────────────────────────────

echo ""
if [ "${FAILURES}" -eq 0 ]; then
    echo "All dependency versions are consistent."
    exit 0
else
    echo "ERROR: ${FAILURES} version mismatch(es) found."
    echo "Update .versions.toml (the source of truth) first, then propagate to"
    echo "Dockerfile and src/lib.rs.  See RELEASE.md for the full checklist."
    exit 1
fi
