#!/usr/bin/env python3
"""
scripts/check_docs_summary.py — A2 guardrail from plans/documentation-2.md

Verifies that every .md file under docs/src/ (except SUMMARY.md itself and
any files listed in the allowlist) is referenced exactly once in SUMMARY.md.

Exit code:  0  = no orphans, no duplicates
            1  = orphans or duplicates found

Allowlist:
  Add filenames (relative to docs/src/) to ALLOWLIST below to suppress
  warnings for intentionally hidden pages (e.g. partial drafts).

Usage:
    python3 scripts/check_docs_summary.py [--root ROOT] [--strict]
"""

from __future__ import annotations

import argparse
import re
import sys
from collections import Counter
from pathlib import Path


# Pages that are intentionally not in SUMMARY.md.
# Add entries as relative paths from docs/src/, e.g. "drafts/wip.md".
ALLOWLIST: frozenset[str] = frozenset(
    [
        # blog mirror pages are copied in by CI, not stored in the tree
        # "blog/",
    ]
)


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(
        description="Check that all docs/src/ pages appear in SUMMARY.md.",
    )
    p.add_argument("--root", default=None, metavar="DIR")
    p.add_argument(
        "--strict",
        action="store_true",
        help="Also fail on duplicate SUMMARY.md entries (default: warn only).",
    )
    return p.parse_args()


_LINK_RE = re.compile(r"\]\(([^)]+\.md)(?:#[^)]*)?\)")


def extract_summary_links(summary_text: str) -> list[str]:
    """Return all .md link targets from SUMMARY.md."""
    return _LINK_RE.findall(summary_text)


def main() -> int:
    args = parse_args()
    root = (
        Path(args.root).resolve() if args.root else Path(__file__).parent.parent.resolve()
    )
    docs_src = root / "docs" / "src"
    summary_path = docs_src / "SUMMARY.md"

    if not summary_path.exists():
        print(f"ERROR: {summary_path} not found", file=sys.stderr)
        return 1

    summary_text = summary_path.read_text(encoding="utf-8", errors="replace")
    raw_links = extract_summary_links(summary_text)

    # Normalise summary links to paths relative to docs/src/
    summary_targets: list[Path] = []
    for link in raw_links:
        target = (summary_path.parent / link).resolve()
        if target.is_relative_to(docs_src):
            summary_targets.append(target.relative_to(docs_src))
        # else: link points outside docs/src — skip

    link_counts: Counter[Path] = Counter(summary_targets)

    # Collect all .md files under docs/src/ (excluding SUMMARY.md and book output)
    all_pages: list[Path] = []
    for md in sorted(docs_src.rglob("*.md")):
        rel = md.relative_to(docs_src)
        if rel.parts[0] in ("SUMMARY.md",) or str(rel) == "SUMMARY.md":
            continue
        all_pages.append(rel)

    # Filter allowlisted pages
    checked_pages = [p for p in all_pages if str(p) not in ALLOWLIST]

    orphans = [p for p in checked_pages if link_counts[p] == 0]
    duplicates = [p for p, cnt in link_counts.items() if cnt > 1]

    exit_code = 0

    if orphans:
        print(f"ORPHAN PAGES ({len(orphans)} not in SUMMARY.md):")
        for p in sorted(orphans):
            print(f"  {p}")
        exit_code = 1

    if duplicates:
        msg = f"DUPLICATE ENTRIES ({len(duplicates)} pages linked more than once in SUMMARY.md):"
        print(msg)
        for p in sorted(duplicates):
            print(f"  {p}  (×{link_counts[p]})")
        if args.strict:
            exit_code = 1
        else:
            print("  (pass --strict to fail on duplicates)")

    if exit_code == 0:
        print(f"orphans=0  (checked {len(checked_pages)} pages)")

    return exit_code


if __name__ == "__main__":
    sys.exit(main())
