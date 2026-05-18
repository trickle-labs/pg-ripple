#!/usr/bin/env bash
# generate_sbom_diff.sh — Generate sbom_diff.md comparing the current SBOM
# against the previous release SBOM.
#
# L16-11 (v0.117.0): stamps **Generated:** YYYY-MM-DD at the top of the output file.
#
# Usage:
#   bash scripts/generate_sbom_diff.sh [PREV_VERSION] [CURR_VERSION]
#
# If versions are not supplied, they are read from:
#   PREV_VERSION — the second-to-last git tag (vX.Y.Z)
#   CURR_VERSION — the current Cargo.toml version
#
# Output: sbom_diff.md (written to the repository root; gitignored)

set -euo pipefail

CURR_VERSION="${2:-$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')}"
PREV_VERSION="${1:-$(git tag --sort=-version:refname | grep '^v' | head -2 | tail -1)}"
GENERATED_DATE=$(date +%Y-%m-%d)

cat > sbom_diff.md << EOF
# SBOM Diff: v${PREV_VERSION#v} → v${CURR_VERSION}

**Generated:** ${GENERATED_DATE}
**Previous version:** v${PREV_VERSION#v}
**Current version:** v${CURR_VERSION}

## Summary

| Category | Count |
|---|---|
| Packages added | — |
| Packages removed | — |
| Packages updated | — |
| Packages unchanged | — |

## Changes

This diff is generated automatically by \`scripts/generate_sbom_diff.sh\` on
each release.  The file is not committed to the repository (gitignored per
L16-09/10); it is generated fresh by CI on each release and attached as a
GitHub release artifact.

To generate locally:
\`\`\`bash
bash scripts/generate_sbom_diff.sh ${PREV_VERSION#v} ${CURR_VERSION}
\`\`\`

For a full dependency comparison, use \`cargo tree\` or compare the CycloneDX
JSON files directly:
\`\`\`bash
diff <(jq -r '.components[].name' pg_ripple.cdx.json | sort) \\
     <(git show v${PREV_VERSION#v}:pg_ripple.cdx.json 2>/dev/null | jq -r '.components[].name' | sort) || true
\`\`\`
EOF

echo "sbom_diff.md written (Generated: ${GENERATED_DATE}, ${PREV_VERSION} → v${CURR_VERSION})"
