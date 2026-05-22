#!/usr/bin/env python3
"""
scripts/check_docs_links.py — A1 guardrail from plans/documentation-2.md

Scans every Markdown file under docs/ and verifies that relative local links
resolve to an existing file.

Rules:
  - Relative links that start with http/https/mailto are skipped (external).
  - Fragment-only links (#anchor) are skipped.
  - Links to files outside the docs tree that start with / are skipped.
  - Links into top-level blog/, plans/, tests/, results/ are checked by
    converting them to an absolute project path.
  - Query strings are stripped from links before resolving.

Exit code:  0  = all local links resolve
            1  = missing files found or usage error

Usage:
    python3 scripts/check_docs_links.py [--root ROOT] [--verbose]
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path
from typing import NamedTuple


class Finding(NamedTuple):
    source_file: Path
    line: int
    raw_link: str
    resolved: Path


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(
        description="Check that relative Markdown links in docs/ resolve to real files."
    )
    p.add_argument(
        "--root",
        default=None,
        metavar="DIR",
        help="Project root (default: parent of this script).",
    )
    p.add_argument("--verbose", action="store_true", help="Print every checked link.")
    return p.parse_args()


# Match Markdown link targets: [text](target) or ![alt](target)
_LINK_RE = re.compile(r"!?\[[^\]]*\]\(([^)]+)\)")
# Match reference-style link definitions: [id]: target
_REF_RE = re.compile(r"^\s{0,3}\[[^\]]+\]:\s+(\S+)")


def extract_links(text: str) -> list[tuple[int, str]]:
    """Return (1-based line number, raw target) pairs for every link in *text*."""
    results: list[tuple[int, str]] = []
    for lineno, line in enumerate(text.splitlines(), start=1):
        for m in _LINK_RE.finditer(line):
            results.append((lineno, m.group(1)))
        m2 = _REF_RE.match(line)
        if m2:
            results.append((lineno, m2.group(1)))
    return results


def is_external(target: str) -> bool:
    return (
        target.startswith(("http://", "https://", "mailto:", "ftp://"))
        or target.startswith("#")
        or target.startswith("//")
    )


def strip_fragment_and_query(target: str) -> str:
    target = target.split("#")[0]
    target = target.split("?")[0]
    return target


def resolve_link(source_file: Path, raw_target: str, root: Path) -> Path | None:
    """
    Resolve *raw_target* relative to *source_file*.

    Returns the resolved Path, or None if the link cannot be checked (external).
    """
    if is_external(raw_target):
        return None

    clean = strip_fragment_and_query(raw_target)
    if not clean:
        # Fragment-only after stripping
        return None

    if clean.startswith("/"):
        # Absolute path relative to root — skip; out of scope for local checks.
        return None

    resolved = (source_file.parent / clean).resolve()
    return resolved


def check_file(md_file: Path, root: Path, verbose: bool) -> list[Finding]:
    text = md_file.read_text(encoding="utf-8", errors="replace")
    links = extract_links(text)
    missing: list[Finding] = []
    for lineno, raw in links:
        resolved = resolve_link(md_file, raw, root)
        if resolved is None:
            if verbose:
                print(f"  SKIP  {raw}")
            continue
        if not resolved.exists():
            missing.append(Finding(md_file, lineno, raw, resolved))
            if verbose:
                print(f"  MISS  {raw!r}  ->  {resolved}")
        else:
            if verbose:
                print(f"  OK    {raw}")
    return missing


def main() -> int:
    args = parse_args()
    root = Path(args.root).resolve() if args.root else Path(__file__).parent.parent.resolve()
    docs_dir = root / "docs"
    if not docs_dir.is_dir():
        print(f"ERROR: docs/ directory not found at {docs_dir}", file=sys.stderr)
        return 1

    all_missing: list[Finding] = []
    md_files = sorted(docs_dir.rglob("*.md"))
    # Exclude generated book output
    md_files = [f for f in md_files if "docs/book" not in f.as_posix()]

    for md_file in md_files:
        if args.verbose:
            print(f"\n=== {md_file.relative_to(root)} ===")
        missing = check_file(md_file, root, args.verbose)
        all_missing.extend(missing)

    if all_missing:
        print(f"\nmissing={len(all_missing)}")
        for f in all_missing:
            rel_src = f.source_file.relative_to(root)
            rel_res = f.resolved.relative_to(root) if f.resolved.is_relative_to(root) else f.resolved
            print(f"  {rel_src}:{f.line}: {f.raw_link!r}  ->  {rel_res}")
        return 1

    print(f"missing=0")
    return 0


if __name__ == "__main__":
    sys.exit(main())
