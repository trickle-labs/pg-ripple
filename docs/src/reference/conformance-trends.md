# Conformance Trends

> **T13-04 (v0.86.0)**: this page explains how pg_ripple tracks pass rates across the conformance suites over time.

The raw trend data lives in the repository at
[`tests/conformance/history.csv`](https://github.com/trickle-labs/pg-ripple/blob/main/tests/conformance/history.csv).
The CI job `conformance-trends` appends a row after successful main-branch
builds. This page does not duplicate the CSV inline, because copied trend rows
go stale quickly and can drift from the CI artifact.

---

## Current Release Gates

| Suite | Current CI role | Release expectation |
|---|---|---|
| W3C SPARQL 1.1 | Smoke subset is blocking; full suite is informational | Smoke subset must pass |
| Apache Jena | Informational until the project consistently clears the threshold | Target pass rate ≥ 95% |
| WatDiv | Correctness and performance signal | Non-blocking, reviewed for regressions |
| LUBM | OWL RL reasoning gate | Required |
| OWL 2 RL | Entailment conformance signal | Informational until ≥ 95% pass rate |

---

## Suite Descriptions

| Suite | Tests | Required Pass Rate | Notes |
|---|---|---|---|
| W3C SPARQL 1.1 | ~357 (smoke subset) | 100% (smoke); informational (full) | Smoke subset is a CI gate |
| Apache Jena | ~1,021 | ≥ 95% | Non-blocking until threshold met |
| WatDiv | 100 templates | Correctness + performance | Non-blocking |
| LUBM | 14 OWL RL queries | 100% | Required CI gate |
| OWL 2 RL | ~258 | ≥ 95% | Informational until threshold met |

---

## CI Badges (v0.122.0)

The Apache Jena pass rate is published as a CI badge after each main-branch
build. The `jena-suite` CI job writes the generated badge payload to
`results/jena-badge.json` in the workflow artifact; the file may not exist in a
fresh local checkout until that job has run.

The badge data format follows the [shields.io endpoint schema](https://shields.io/endpoint):

```json
{
  "schemaVersion": 1,
  "label": "Jena",
  "message": "99% pass",
  "color": "green"
}
```

Color thresholds: green ≥ 95%, yellow ≥ 90%, orange ≥ 80%, red < 80%.

---

## How to Regenerate

```bash
cargo pgrx test pg18 2>&1 | scripts/parse_conformance_results.py >> tests/conformance/history.csv
```

Or run the full conformance test suite locally:

```bash
cargo pgrx regress pg18 --postgresql-conf "allow_system_table_mods=on"
```
