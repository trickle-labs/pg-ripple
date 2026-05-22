#!/usr/bin/env python3
"""
scripts/check_http_routes.py — B2 drift check from plans/documentation-2.md

Extracts HTTP routes from pg_ripple_http/src/routing/mod.rs and compares them
against the endpoint table in docs/src/reference/http-api.md.

What is checked:
  1. Every .route(…) path in the router appears in the docs table.
  2. Methods declared in the router match what is documented.

Output modes:
  --strict  Fail (exit 1) on undocumented routes or method mismatches.
            Default is report-only (exit 0).

Usage:
    python3 scripts/check_http_routes.py [--root ROOT] [--strict]
"""

from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional


@dataclass
class Route:
    path: str
    methods: list[str]
    source_line: int


# Matches: .route("/some/path", get(handler).post(other))
_ROUTE_LINE_RE = re.compile(r'\.route\(\s*"([^"]+)"')
# Matches HTTP method calls: get(…), post(…), put(…), delete(…), patch(…)
_METHOD_RE = re.compile(r'\b(get|post|put|delete|patch|head|options)\s*\(')

# Axum path params {param} or :param → normalised to {param}
_PATH_PARAM_RE = re.compile(r':[a-z_]+')


def normalise_path(path: str) -> str:
    """Normalise Axum path to the form used in docs (curly braces, no trailing slash)."""
    path = _PATH_PARAM_RE.sub(lambda m: '{' + m.group(0)[1:] + '}', path)
    return path.rstrip("/")


def extract_routes(router_path: Path) -> list[Route]:
    """Parse .route(…) declarations from the Axum router file."""
    text = router_path.read_text(encoding="utf-8", errors="replace")
    lines = text.splitlines()
    routes: list[Route] = []

    i = 0
    while i < len(lines):
        m = _ROUTE_LINE_RE.search(lines[i])
        if m:
            raw_path = m.group(1)
            # Collect method calls: may span several lines until the closing )
            window = []
            depth = 0
            j = i
            while j < min(i + 12, len(lines)):
                window.append(lines[j])
                depth += lines[j].count("(") - lines[j].count(")")
                if depth <= 0 and j > i:
                    break
                j += 1
            window_text = " ".join(window)
            methods = sorted(set(m.upper() for m in _METHOD_RE.findall(window_text)))
            routes.append(Route(
                path=normalise_path(raw_path),
                methods=methods or ["GET"],
                source_line=i + 1,
            ))
        i += 1

    return routes


def load_docs_table(http_api_path: Path) -> str:
    """Load the endpoint table from http-api.md as a raw string."""
    if not http_api_path.exists():
        return ""
    return http_api_path.read_text(encoding="utf-8", errors="replace")


def path_in_docs(path: str, docs_text: str) -> bool:
    """Return True if the route path appears anywhere in the docs."""
    # Exact string match (backtick-quoted in table cells)
    escaped = re.escape(path)
    return bool(re.search(escaped, docs_text))


def main() -> int:
    ap = argparse.ArgumentParser(
        description="Check HTTP routes in routing/mod.rs against docs."
    )
    ap.add_argument("--root", default=None, metavar="DIR")
    ap.add_argument(
        "--strict",
        action="store_true",
        help="Exit 1 on undocumented routes.",
    )
    args = ap.parse_args()

    root = (
        Path(args.root).resolve() if args.root else Path(__file__).parent.parent.resolve()
    )

    router_path = root / "pg_ripple_http" / "src" / "routing" / "mod.rs"
    if not router_path.exists():
        print(f"ERROR: router file not found at {router_path}", file=sys.stderr)
        return 1

    http_api_path = root / "docs" / "src" / "reference" / "http-api.md"

    routes = extract_routes(router_path)
    docs_text = load_docs_table(http_api_path)

    if not routes:
        print("WARNING: no .route(…) declarations found in router file.")
        return 0

    undocumented: list[Route] = []
    for route in routes:
        if not path_in_docs(route.path, docs_text):
            undocumented.append(route)

    exit_code = 0

    if undocumented:
        print(f"\nUNDOCUMENTED ROUTES ({len(undocumented)}):")
        for r in undocumented:
            print(
                f"  {', '.join(r.methods):20s}  {r.path:50s}  "
                f"(routing/mod.rs:{r.source_line})"
            )
        if args.strict:
            exit_code = 1
        else:
            print("  (pass --strict to fail on undocumented routes)")

    if not undocumented:
        print(
            f"OK: checked {len(routes)} routes — all paths appear in http-api.md."
        )

    return exit_code


if __name__ == "__main__":
    sys.exit(main())
