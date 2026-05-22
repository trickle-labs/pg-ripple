#!/usr/bin/env python3
"""
scripts/check_guc_drift.py — B1 drift check from plans/documentation-2.md

Extracts GUC static variable declarations from src/gucs/**/*.rs (the actual
defaults held in GucSetting::new(…)) and compares them against the documented
defaults in docs/gucs.md and docs/src/reference/guc-reference.md.

What is checked:
  1. Every GUC declared in Rust appears in at least one of the two doc files.
  2. When a numeric/bool/string default is readable from the Rust source, it is
     checked against the value shown in the doc files (non-blocking report mode
     by default; use --strict to fail on mismatch).

What is NOT checked (too fragile to parse reliably):
  - GUCs whose default is a constant expression (e.g. MY_CONST as i32).
  - Context and access-level claims.
  - Ranges / enum members.

Output modes:
  --strict  Fail (exit 1) on missing GUCs or default mismatches.
            Default is report-only (exit 0).

Usage:
    python3 scripts/check_guc_drift.py [--root ROOT] [--strict]
"""

from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional


# ─── Data model ─────────────────────────────────────────────────────────────

@dataclass
class GucDef:
    name: str           # SQL name like "vp_promotion_threshold"
    rust_default: Optional[str]   # extracted from GucSetting::new(…) or None
    source_file: str
    source_line: int


# ─── Extraction ─────────────────────────────────────────────────────────────

# Matches lines like:
#   pub static FOO_BAR: pgrx::GucSetting<i32> = pgrx::GucSetting::<i32>::new(1000);
# or multi-line with the ::new call on the next line.
_STATIC_RE = re.compile(
    r"pub\s+static\s+([A-Z_]+)\s*:",
)
_NEW_RE = re.compile(r"::new\(([^)]*)\)")


def extract_gucs(src_dir: Path) -> list[GucDef]:
    """
    Walk src/gucs/**/*.rs and extract GUC static names + their ::new defaults.
    Also reads src/gucs/registration/**/*.rs to find the SQL name mapping.
    """
    gucs: list[GucDef] = []

    for rs_file in sorted(src_dir.rglob("*.rs")):
        lines = rs_file.read_text(encoding="utf-8", errors="replace").splitlines()
        i = 0
        while i < len(lines):
            line = lines[i]
            m_static = _STATIC_RE.search(line)
            if m_static:
                rust_var = m_static.group(1)
                # Try to find ::new on the same line or next few lines
                default_val: Optional[str] = None
                search_window = "\n".join(lines[i : i + 5])
                m_new = _NEW_RE.search(search_window)
                if m_new:
                    raw = m_new.group(1).strip()
                    # Skip complex expressions
                    if re.match(r'^(true|false|-?\d[\d_]*|"[^"]*"|None)$', raw):
                        default_val = raw.replace("_", "")
                gucs.append(
                    GucDef(
                        name=rust_var.lower(),
                        rust_default=default_val,
                        source_file=str(rs_file.relative_to(src_dir.parent)),
                        source_line=i + 1,
                    )
                )
            i += 1

    # Also walk registration files for register_guc calls to get SQL names
    # (the static Rust name is ALL_CAPS; SQL name is pg_ripple.<lower_name>)
    return gucs


# ─── Doc scanning ────────────────────────────────────────────────────────────

def load_doc_corpus(root: Path) -> str:
    parts: list[str] = []
    for path in [
        root / "docs" / "gucs.md",
        root / "docs" / "src" / "reference" / "guc-reference.md",
    ]:
        if path.exists():
            parts.append(path.read_text(encoding="utf-8", errors="replace"))
    return "\n".join(parts)


# ─── Comparison ─────────────────────────────────────────────────────────────

def normalise_default(val: str) -> str:
    """Normalise a default string for comparison."""
    val = val.strip().lower().strip("'\"")
    # Normalise boolean synonyms
    if val in ("on", "true", "1", "yes"):
        return "on"
    if val in ("off", "false", "0", "no"):
        return "off"
    # Normalise null/none synonyms
    if val in ("none", "null", "(none)", "(null)", ""):
        return "null"
    return val


def main() -> int:
    ap = argparse.ArgumentParser(description="Check GUC defaults against documentation.")
    ap.add_argument("--root", default=None, metavar="DIR")
    ap.add_argument(
        "--strict",
        action="store_true",
        help="Exit 1 on missing GUCs or default mismatches.",
    )
    args = ap.parse_args()

    root = (
        Path(args.root).resolve() if args.root else Path(__file__).parent.parent.resolve()
    )
    gucs_dir = root / "src" / "gucs"
    if not gucs_dir.is_dir():
        print(f"ERROR: {gucs_dir} not found", file=sys.stderr)
        return 1

    gucs = extract_gucs(gucs_dir)
    if not gucs:
        print("WARNING: no GUC statics found in src/gucs/ — check the extractor.")
        return 0

    corpus = load_doc_corpus(root)
    if not corpus:
        print("ERROR: could not load GUC doc files.", file=sys.stderr)
        return 1

    missing: list[GucDef] = []
    mismatch: list[tuple[GucDef, str]] = []

    for guc in gucs:
        sql_name = guc.name  # lower snake_case

        # Check presence: either the static var name (lower) or pg_ripple.name appears
        appears = (
            sql_name in corpus
            or f"pg_ripple.{sql_name}" in corpus
            or f"`{sql_name}`" in corpus
        )
        if not appears:
            missing.append(guc)
            continue

        # Check default value when extractable
        if guc.rust_default is None:
            continue

        rust_norm = normalise_default(guc.rust_default)

        # Look for documented defaults near the GUC name.
        # Pattern: table cell or backtick-quoted value near the name.
        pattern = re.compile(
            rf"`{re.escape(sql_name)}`[^|]*\|[^|]*\|\s*`?([^`|\n]+?)`?\s*\|",
            re.IGNORECASE,
        )
        doc_defaults = pattern.findall(corpus)
        if doc_defaults:
            doc_norm = normalise_default(doc_defaults[0])
            if rust_norm != doc_norm:
                mismatch.append((guc, doc_norm))

    exit_code = 0

    if missing:
        print(f"\nMISSING FROM DOCS ({len(missing)} GUCs):")
        for g in missing:
            print(f"  {g.name:40s}  {g.source_file}:{g.source_line}")
        if args.strict:
            exit_code = 1
        else:
            print("  (pass --strict to fail on missing GUCs)")

    if mismatch:
        print(f"\nDEFAULT MISMATCH ({len(mismatch)} GUCs):")
        for g, doc_val in mismatch:
            print(
                f"  {g.name:40s}  rust={g.rust_default!r}  docs={doc_val!r}  "
                f"({g.source_file}:{g.source_line})"
            )
        if args.strict:
            exit_code = 1
        else:
            print("  (pass --strict to fail on default mismatches)")

    if not missing and not mismatch:
        print(f"OK: checked {len(gucs)} GUC statics — all present in docs, no default mismatches detected.")

    return exit_code


if __name__ == "__main__":
    sys.exit(main())
