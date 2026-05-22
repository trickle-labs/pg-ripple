#!/usr/bin/env python3
"""
D — Executable example labeling check.

Scans SQL code blocks in Getting Started and Feature pages.
Reports SQL blocks that lack a preceding label comment
(e.g. ``-- run as superuser``, ``-- requires: pgvector``) or
a role/setup annotation.

This is a *report-only* check; it never exits with a non-zero code.
Run with --strict to make it blocking once you're ready.

Usage:
    python3 scripts/check_docs_examples.py [--strict]
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

DOCS_ROOT = Path(__file__).parent.parent / "docs" / "src"

# Pages to audit for labeled SQL examples
SCAN_DIRS = [
    DOCS_ROOT / "getting-started",
    DOCS_ROOT / "features",
    DOCS_ROOT / "user-guide",
]

# Regex matching a single-line "label comment" immediately before a code fence.
# Acceptable forms:
#   <!-- label: ... -->
#   -- label: ...
#   # Label: ...
#   Any comment line containing "requires", "run as", "note:", or "example"
LABEL_RE = re.compile(
    r"(<!--.*?-->|--\s+\S|#\s+\S|>\s+\*\*Note|>\s+Requires|^\*\*Requires\*\*)",
    re.IGNORECASE,
)

# Code fence with an explicit language tag (```sql, ```sparql, ```bash, etc.)
CODE_FENCE_RE = re.compile(r"^```(\w+)\s*$")

# Languages we audit for labeling
AUDITED_LANGUAGES = {"sql", "sparql"}


def check_file(path: Path) -> list[str]:
    """Return list of warning strings for unlabeled code blocks in *path*."""
    text = path.read_text(encoding="utf-8")
    lines = text.splitlines()
    warnings = []
    in_fence = False
    fence_lang = ""
    fence_start = 0
    preceding_lines: list[str] = []

    for i, line in enumerate(lines, start=1):
        if not in_fence:
            m = CODE_FENCE_RE.match(line)
            if m:
                fence_lang = m.group(1).lower()
                fence_start = i
                in_fence = True
                # Collect the last 3 non-blank lines before this fence
                preceding_lines = []
                for j in range(max(0, i - 4), i - 1):
                    if lines[j].strip():
                        preceding_lines.append(lines[j])
        else:
            if line.strip() == "```":
                # End of fence
                if fence_lang in AUDITED_LANGUAGES:
                    # Check if any preceding line looks like a label
                    labelled = any(
                        LABEL_RE.search(pl) for pl in preceding_lines
                    )
                    if not labelled:
                        rel = path.relative_to(DOCS_ROOT.parent.parent)
                        warnings.append(
                            f"  {rel}:{fence_start}  ({fence_lang}) — no label before code block"
                        )
                in_fence = False
                fence_lang = ""
                preceding_lines = []

    return warnings


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--strict",
        action="store_true",
        help="Exit with code 1 if any unlabeled blocks are found.",
    )
    args = parser.parse_args()

    warnings: list[str] = []
    files_checked = 0

    for scan_dir in SCAN_DIRS:
        if not scan_dir.exists():
            continue
        for md_file in sorted(scan_dir.rglob("*.md")):
            w = check_file(md_file)
            warnings.extend(w)
            files_checked += 1

    if warnings:
        print(f"UNLABELED CODE BLOCKS ({len(warnings)} across {files_checked} files):")
        for w in warnings:
            print(w)
        print()
        print("Add a brief comment or callout before each SQL/SPARQL block explaining")
        print("what it does, what role is needed, or whether it is copy-paste ready.")
        print("Example:  <!-- Example: grant read access to a named graph -->")
        if args.strict:
            sys.exit(1)
    else:
        print(
            f"OK: checked {files_checked} pages — all SQL/SPARQL blocks have a preceding label."
        )


if __name__ == "__main__":
    main()
