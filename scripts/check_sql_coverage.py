#!/usr/bin/env python3
"""
scripts/check_sql_coverage.py — B3 drift check from plans/documentation-2.md

Enumerates all #[pg_extern] functions in src/**/*.rs and classifies them as:
  public        — must be documented in docs/src/ or README.md
  legacy        — documented as a compatibility alias (mention required)
  internal      — intentionally not in user docs (suppressed)
  deprecated    — documented as deprecated (mention required)

Classification is read from scripts/sql_allowlist.toml. Any function not in the
allowlist defaults to 'public' and must appear in the documentation corpus.

Exit code:  0  = all public/legacy/deprecated functions documented
            1  = undocumented public functions or usage error

Usage:
    python3 scripts/check_sql_coverage.py [--root ROOT] [--strict]
"""

from __future__ import annotations

import argparse
import re
import sys
import tomllib
from pathlib import Path
from dataclasses import dataclass, field


@dataclass
class FunctionInfo:
    name: str
    source_file: str
    source_line: int
    tier: str = "public"   # public | legacy | internal | deprecated


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(
        description="Check that public SQL functions from #[pg_extern] appear in docs.",
    )
    p.add_argument("--root", default=None, metavar="DIR")
    p.add_argument(
        "--strict",
        action="store_true",
        help="Exit 1 on undocumented public functions (default: report-only).",
    )
    return p.parse_args()


def load_allowlist(root: Path) -> dict[str, str]:
    """
    Load scripts/sql_allowlist.toml.
    Returns a dict of {function_name: tier}.
    """
    path = root / "scripts" / "sql_allowlist.toml"
    if not path.exists():
        return {}
    with open(path, "rb") as f:
        data = tomllib.load(f)
    result: dict[str, str] = {}
    for tier in ("internal", "legacy", "deprecated"):
        for name in data.get(tier, []):
            result[name] = tier
    return result


_PG_EXTERN_RE = re.compile(r"#\[pg_extern(?:\s*\([^)]*\))?\]")
_FN_NAME_RE = re.compile(r"\bfn\s+([a-z_][a-z0-9_]*)\s*[(<]")


def extract_pg_extern_functions(src_dir: Path) -> list[FunctionInfo]:
    """Walk src/ and extract all #[pg_extern] function names."""
    funcs: list[FunctionInfo] = []
    for rs_file in sorted(src_dir.rglob("*.rs")):
        lines = rs_file.read_text(encoding="utf-8", errors="replace").splitlines()
        for i, line in enumerate(lines):
            if _PG_EXTERN_RE.search(line):
                # Look up to 6 lines ahead for the fn name
                for j, lookahead in enumerate(lines[i : i + 7]):
                    m = _FN_NAME_RE.search(lookahead)
                    if m:
                        funcs.append(FunctionInfo(
                            name=m.group(1),
                            source_file=str(rs_file.relative_to(src_dir.parent)),
                            source_line=i + j + 1,
                        ))
                        break
    # Deduplicate (keep first occurrence)
    seen: set[str] = set()
    unique: list[FunctionInfo] = []
    for fn in funcs:
        if fn.name not in seen:
            seen.add(fn.name)
            unique.append(fn)
    return unique


def load_corpus(root: Path) -> str:
    parts: list[str] = []
    for path in [root / "README.md", root / "CHANGELOG.md"]:
        if path.exists():
            parts.append(path.read_text(encoding="utf-8", errors="replace"))
    docs_src = root / "docs" / "src"
    if docs_src.is_dir():
        for md in sorted(docs_src.rglob("*.md")):
            parts.append(md.read_text(encoding="utf-8", errors="replace"))
    return "\n".join(parts)


def main() -> int:
    args = parse_args()
    root = (
        Path(args.root).resolve() if args.root else Path(__file__).parent.parent.resolve()
    )

    src_dir = root / "src"
    if not src_dir.is_dir():
        print(f"ERROR: src/ not found at {src_dir}", file=sys.stderr)
        return 1

    allowlist = load_allowlist(root)
    funcs = extract_pg_extern_functions(src_dir)
    corpus = load_corpus(root)

    if not funcs:
        print("WARNING: no #[pg_extern] functions found — check the extractor.")
        return 0

    # Apply allowlist tiers
    for fn in funcs:
        fn.tier = allowlist.get(fn.name, "public")

    undocumented: list[FunctionInfo] = []
    by_tier: dict[str, list[FunctionInfo]] = {
        "public": [], "legacy": [], "deprecated": [], "internal": [],
    }
    for fn in funcs:
        by_tier.setdefault(fn.tier, []).append(fn)
        if fn.tier == "internal":
            continue
        # public, legacy, deprecated — must appear somewhere in the corpus
        if fn.name not in corpus:
            undocumented.append(fn)

    print(
        f"Checked {len(funcs)} pg_extern functions: "
        f"{len(by_tier.get('public', []))} public, "
        f"{len(by_tier.get('legacy', []))} legacy, "
        f"{len(by_tier.get('deprecated', []))} deprecated, "
        f"{len(by_tier.get('internal', []))} internal."
    )

    exit_code = 0

    if undocumented:
        public_missing = [f for f in undocumented if f.tier == "public"]
        other_missing = [f for f in undocumented if f.tier != "public"]

        if public_missing:
            print(f"\nUNDOCUMENTED PUBLIC FUNCTIONS ({len(public_missing)}):")
            for fn in public_missing:
                print(f"  {fn.name:50s}  ({fn.source_file}:{fn.source_line})")
            print(
                "\n  Add documentation in docs/src/ or README.md,\n"
                "  or add to scripts/sql_allowlist.toml if internal/deprecated."
            )
            if args.strict:
                exit_code = 1
            else:
                print("  (pass --strict to fail on undocumented public functions)")

        if other_missing:
            print(f"\nUNDOCUMENTED LEGACY/DEPRECATED FUNCTIONS ({len(other_missing)}):")
            for fn in other_missing:
                print(f"  {fn.name:50s}  tier={fn.tier}  ({fn.source_file}:{fn.source_line})")

    if not undocumented:
        print(
            f"OK: all non-internal pg_extern functions appear in the documentation corpus."
        )

    return exit_code


if __name__ == "__main__":
    sys.exit(main())
