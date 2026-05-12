#!/usr/bin/env python3
"""magellan_to_rdf.py — Convert Magellan ER benchmark CSV datasets to Turtle RDF.

Usage:
    python3 magellan_to_rdf.py \
        --table-a abt_tableA.csv \
        --table-b abt_tableB.csv \
        --gold    abt_buy_gold.csv \
        --graph-a http://magellan.org/abt \
        --graph-b http://magellan.org/buy \
        --gold-graph http://magellan.org/abt_buy_gold \
        --output  abt_buy.ttl \
        --entity-prefix http://magellan.org/product/
"""
import argparse
import csv
import sys
from pathlib import Path


def csv_to_rdf(
    path: str,
    graph_iri: str,
    entity_prefix: str,
    out_lines: list,
    id_col: str = "id",
) -> None:
    """Convert a CSV table to N-Quads-style Turtle triples in a named graph."""
    schema_base = "https://schema.org/"
    with open(path, newline="", encoding="utf-8") as f:
        reader = csv.DictReader(f)
        for row in reader:
            eid = row.get(id_col, "").strip()
            if not eid:
                continue
            entity_iri = f"{entity_prefix}{eid}"
            out_lines.append(f"  <{entity_iri}> a <https://schema.org/Product> ;")
            for col, val in row.items():
                if col == id_col or not val.strip():
                    continue
                # Escape quotes in literal values.
                escaped = val.replace("\\", "\\\\").replace('"', '\\"')
                out_lines.append(
                    f'    <{schema_base}{col}> "{escaped}" ;'
                )
            # Close the subject block.
            if out_lines and out_lines[-1].endswith(";"):
                out_lines[-1] = out_lines[-1][:-1] + "."
            out_lines.append("")


def gold_to_rdf(
    path: str,
    gold_graph_iri: str,
    entity_prefix_a: str,
    entity_prefix_b: str,
    out_lines: list,
) -> None:
    """Convert a gold-standard match CSV to owl:sameAs triples."""
    owl_same_as = "http://www.w3.org/2002/07/owl#sameAs"
    with open(path, newline="", encoding="utf-8") as f:
        reader = csv.DictReader(f)
        for row in reader:
            l_id = row.get("l_id", "").strip()
            r_id = row.get("r_id", "").strip()
            if not l_id or not r_id:
                continue
            out_lines.append(
                f"  <{entity_prefix_a}{l_id}> <{owl_same_as}> <{entity_prefix_b}{r_id}> ."
            )
    out_lines.append("")


def main() -> None:
    parser = argparse.ArgumentParser(description="Magellan CSV → Turtle RDF converter")
    parser.add_argument("--table-a", required=True, help="Path to left-side CSV table")
    parser.add_argument("--table-b", required=True, help="Path to right-side CSV table")
    parser.add_argument("--gold",    required=True, help="Path to gold-standard match CSV")
    parser.add_argument("--graph-a", required=True, help="Named graph IRI for table A")
    parser.add_argument("--graph-b", required=True, help="Named graph IRI for table B")
    parser.add_argument("--gold-graph", required=True, help="Named graph IRI for gold matches")
    parser.add_argument("--output",  required=True, help="Output Turtle file path")
    parser.add_argument("--entity-prefix", default="http://magellan.org/entity/",
                        help="IRI prefix for entities")
    args = parser.parse_args()

    lines: list = [
        "@prefix schema: <https://schema.org/> .",
        "@prefix owl:    <http://www.w3.org/2002/07/owl#> .",
        "",
    ]

    # Table A
    lines.append(f"GRAPH <{args.graph_a}> {{")
    csv_to_rdf(args.table_a, args.graph_a, args.entity_prefix, lines)
    lines.append("}")
    lines.append("")

    # Table B — use a distinct prefix suffix to avoid IRI clashes.
    entity_prefix_b = args.entity_prefix.rstrip("/") + "_b/"
    lines.append(f"GRAPH <{args.graph_b}> {{")
    csv_to_rdf(args.table_b, args.graph_b, entity_prefix_b, lines)
    lines.append("}")
    lines.append("")

    # Gold matches
    lines.append(f"GRAPH <{args.gold_graph}> {{")
    gold_to_rdf(args.gold, args.gold_graph, args.entity_prefix, entity_prefix_b, lines)
    lines.append("}")
    lines.append("")

    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text("\n".join(lines), encoding="utf-8")
    print(f"Wrote {output} ({output.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
