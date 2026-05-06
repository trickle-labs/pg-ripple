# Jena Conformance Suite Results

The Apache Jena SPARQL 1.1 conformance suite (approximately 1,000 tests) covers the
full SPARQL 1.1 query and update specification. pg_ripple targets ≥95% pass rate on
this suite as of v0.55.0.

## What the Jena suite tests

The Jena conformance tests are derived from the W3C SPARQL 1.1 test suite and supplemented
by Apache Jena's own test cases. They cover:

| Category | Tests (approx.) | What it exercises |
|---|---|---|
| Basic Graph Patterns | ~150 | BGP matching, triple patterns, blank nodes |
| SPARQL Algebra | ~200 | UNION, OPTIONAL, MINUS, JOIN |
| Aggregates | ~80 | COUNT, SUM, AVG, GROUP BY, HAVING |
| Property paths | ~100 | `*`, `+`, `?`, `|`, `^`, `/` operators |
| SPARQL Update | ~150 | INSERT DATA, DELETE DATA, MODIFY, COPY, MOVE |
| SPARQL Protocol | ~50 | Content negotiation, result formats |
| Numeric/String functions | ~120 | All SPARQL built-in functions |
| RDF-star | ~50 | Quoted triples, TRIPLE(), isTriple() |
| Subqueries / VALUES | ~60 | Correlated sub-SELECTs, VALUES rows |
| Other | ~40 | Miscellaneous edge cases |

## Pass rate

The pass rate is measured by CI on each push to `main` using
`cargo test --test jena_suite`. The current pass rate is ≥95%.

Badge status is updated automatically by the CI `jena-suite` job.

## Known exclusions

Some tests in the Jena suite exercise features that are non-blocking until
the full suite requirement is enforced at ≥95% pass rate:

- Tests requiring named-graph write operations via SPARQL UPDATE that conflict
  with PostgreSQL transaction semantics (platform-specific)
- Tests for features gated on `spargebra` SPARQL 1.2 grammar support

## Running locally

```bash
# Fetch Jena test data (requires network access)
bash scripts/fetch_conformance_tests.sh --jena

# Run the suite
cargo test --test jena_suite -- --nocapture
```

The suite generates a report at `tests/conformance/report.json`.
