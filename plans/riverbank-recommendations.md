# pg-ripple — Improvements Identified from riverbank Planning

> **Date:** 2026-05-05
> **Source:** Analysis of the [riverbank roadmap](../ROADMAP.md) and strategy
> documents during riverbank planning. These items were initially scoped to
> riverbank but belong in pg-ripple: they are graph-computable without LLM
> calls, useful independently of riverbank, and operate entirely on data already
> inside the database.
>
> **Principle:** pg-ripple should own anything that is graph-computable without
> LLM calls, useful independent of any application layer, and operates on data
> already inside the database. riverbank owns anything that requires
> orchestrating LLMs, managing a compilation lifecycle, or integrating with
> external systems.

---

## 1. SKOS structural integrity shape bundle

### Background

riverbank's v0.4.0 roadmap plans to ship a SHACL shape library as
`riverbank/shapes/skos-integrity.ttl`, loaded into pg_ripple at
`riverbank init`. The shapes enforce structural standards from ISO 25964-1
(Thesauri and interoperability with other vocabularies) and ANSI/NISO Z39.19
(Guidelines for the Construction, Format, and Management of Monolingual
Controlled Vocabularies).

These shapes are **domain-agnostic**. They enforce fundamental SKOS correctness
rules that apply to any user of pg_ripple who works with SKOS data — taxonomy
builders, thesaurus maintainers, knowledge graph operators — regardless of
whether they use riverbank.

pg_ripple already ships built-in Datalog rule templates for vocabulary alignment
(Schema.org↔SAREF, Schema.org↔FHIR, etc.) and a `skos-transitive` bundle that
derives transitive `skos:broader` closures. The integrity shapes are the natural
complement: the Datalog bundle derives the transitive facts; the SHACL shapes
validate the structural correctness of the input before those derivations run.
They belong together in the same layer.

### Proposed addition

A built-in SHACL shape bundle named `pg:skos-integrity` (or loaded from
`pg_ripple/shapes/skos-integrity.ttl` at extension install), activated by:

```sql
SELECT pg_ripple.load_shape_bundle('skos-integrity');
```

Shapes in the bundle:

| Shape name | What it checks | Recommended severity |
|---|---|---|
| `skos:prefLabelRequired` | Every `skos:Concept` has exactly one `skos:prefLabel` per language tag | `sh:Violation` |
| `skos:scopeNoteRequired` | Every `skos:Concept` has a `skos:scopeNote` | `sh:Warning` |
| `skos:broaderCycleCheck` | No concept is its own transitive `skos:broader` ancestor (cycle detection) | `sh:Violation` |
| `skos:conflictingMatchCheck` | A concept pair does not simultaneously hold `skos:exactMatch` and `skos:broadMatch` | `sh:Violation` |
| `skos:orphanConceptCheck` | Every `skos:Concept` has at least one `skos:broader`, `skos:narrower`, or `skos:related` link | `sh:Warning` |
| `skos:altLabelCollisionCheck` | No `skos:altLabel` on concept A matches the `skos:prefLabel` of a different concept B without an explicit `skos:exactMatch` link between them | `sh:Violation` |

`skos:broaderCycleCheck` requires the transitive closure produced by the
`skos-transitive` Datalog bundle to be available. Activating `skos-integrity`
should therefore implicitly activate `skos-transitive` if not already enabled,
or document the dependency clearly.

### riverbank impact

If pg_ripple ships this bundle, riverbank's v0.4.0 deliverable changes from
"ship a Turtle file and load it at init" to "activate the built-in bundle at
init":

```python
# riverbank/catalog/migrations/versions/0004_init_shapes.py
conn.execute("SELECT pg_ripple.load_shape_bundle('skos-integrity')")
```

The `shapes/` directory and `skos-integrity.ttl` file are removed from the
riverbank repository layout. `riverbank lint --layer vocab` becomes a call
against the built-in shape set rather than a locally-loaded Turtle file.

### Acceptance criteria

- `SELECT pg_ripple.load_shape_bundle('skos-integrity')` registers all six
  shapes in the pg_ripple SHACL registry without error on a clean database.
- Each shape has a paired passing and failing fixture in the pg_ripple test
  suite.
- `shacl_report_scored()` includes findings from `skos-integrity` shapes in its
  output without additional configuration.
- The `skos-transitive` Datalog bundle is activated automatically (or a clear
  error is raised) when `skos-integrity` is loaded and the transitive closure
  is absent.

---

## 2. `explain_contradiction()` — minimal contradiction explanation

### Background

riverbank's v0.7.0 roadmap plans to implement `riverbank explain-conflict <iri>`
as: *"computes the smallest set of facts and rules producing a contradiction
using a SAT-style minimal-cause algorithm over the inference dependency graph."*

This is pure graph reasoning. It requires access to:
- The set of triples that together produce an inconsistency
- The Datalog rules that fired to derive intermediate facts
- The RDF-star annotations recording confidence and provenance on each triple

All of this is entirely inside pg_ripple. The algorithm does not require Python
or LLM calls. The natural analogy is `explain_pagerank()` and
`explain_pagerank_json()`, which already traverse the inference graph to explain
*why* a node holds its importance score. `explain_contradiction()` is the same
traversal pattern applied to unsatisfiable rule combinations.

Implementing this in riverbank's Python layer would be a brittle wrapper around
information that was always inside the database — and would be unavailable to
any pg_ripple user not using riverbank.

### Proposed API

```sql
-- Returns the minimal set of triples and rules that together produce an
-- inconsistency involving subject_iri.
SELECT * FROM pg_ripple.explain_contradiction(
    subject_iri  => 'https://example.org/entity/Acme',
    named_graph  => '<trusted>',          -- optional; defaults to all graphs
    max_depth    => 10                    -- optional; limits rule-chain traversal
);
```

Return type (one row per contributing element):

| Column | Type | Description |
|---|---|---|
| `element_kind` | `text` | `'triple'`, `'rule'`, `'assumption'` |
| `subject` | `text` | IRI or blank node |
| `predicate` | `text` | IRI |
| `object` | `text` | IRI or literal |
| `named_graph` | `text` | Which graph the triple lives in |
| `confidence` | `float4` | `pg:confidence` annotation, if present |
| `rule_name` | `text` | Datalog rule identifier, if `element_kind = 'rule'` |
| `contribution` | `text` | Human-readable explanation of why this element contributes |
| `depth` | `int` | Distance from the contradiction root |

A JSONB variant for programmatic consumption:

```sql
SELECT pg_ripple.explain_contradiction_json(
    subject_iri => 'https://example.org/entity/Acme'
);
```

### Algorithm sketch

1. Run the SHACL validator against the subject's named graph and collect all
   `sh:ValidationResult` nodes that involve `subject_iri` (directly or as an
   object in a failing triple).
2. For each violation, trace the triple's `prov:wasDerivedFrom` chain backwards
   through the Datalog inference log to the source triples that triggered it.
3. Compute the minimal hitting set: the smallest subset of source triples whose
   removal would eliminate all violations involving `subject_iri`. This is the
   explanation — not all contributing triples, only the minimal causal set.
4. Return each element in the minimal set with its provenance, confidence, and
   the rule that used it.

The minimal-hitting-set step can be approximated efficiently with a greedy
algorithm for practical graph sizes; an exact SAT-style solver is available as
an opt-in via a `mode => 'exact'` parameter.

### riverbank impact

`riverbank explain-conflict <iri>` becomes a thin CLI wrapper:

```python
result = conn.execute(
    "SELECT * FROM pg_ripple.explain_contradiction(%s)", [iri]
).fetchall()
```

The v0.7.0 deliverable is reduced in scope — riverbank ships a CLI command and
display formatter; pg_ripple provides the reasoning engine.

### Acceptance criteria

- `explain_contradiction(subject_iri)` returns a non-empty result set for a
  deliberately constructed inconsistent graph in the test suite.
- The minimal set returned is verifiably minimal: removing any single element
  eliminates the contradiction.
- Performance on a graph of 100,000 triples with a 5-element causal chain is
  under 500 ms with `mode => 'greedy'`.
- `explain_contradiction_json()` returns valid JSONB parseable by standard
  tooling.

---

## 3. SPARQL `SERVICE` keyword support

### Background

riverbank's v0.8.0 roadmap plans a "remote profile type" that pulls
`SERVICE`-federated triples from a peer pg_ripple instance into a local
compilation context.

The `SERVICE` keyword is part of the SPARQL 1.1 Federation Extensions standard
(W3C, 2013). It belongs in the SPARQL query engine — pg_ripple — not in
application-level Python. Any pg_ripple user who wants to federate a SPARQL
query across multiple endpoints should be able to do so without riverbank.

pg_ripple's v0.88 federation blend mode (`pagerank_run()` pulling edges from
remote `SERVICE` endpoints) already demonstrates the pattern exists in the
codebase. Full `SERVICE` support in the `sparql()` function would generalise
this to any SPARQL query.

### Proposed addition

`SERVICE` keyword handling in `pg_ripple.sparql()` and
`pg_ripple.sparql_from_nl()`:

```sql
SELECT * FROM pg_ripple.sparql($$
    PREFIX schema: <https://schema.org/>
    SELECT ?product ?review
    WHERE {
        ?product a schema:Product .
        SERVICE <https://peer.example.org/sparql> {
            ?review schema:itemReviewed ?product ;
                    schema:reviewRating ?rating .
            FILTER(?rating > 4)
        }
    }
$$);
```

Configuration:

```sql
-- Register a trusted remote endpoint with optional auth and confidence floor.
INSERT INTO pg_ripple.federation_endpoints (name, endpoint_url, auth_token, min_confidence)
VALUES ('peer-catalogue', 'https://peer.example.org/sparql', '${env:PEER_TOKEN}', 0.6);
```

The `${env:VAR}` secret interpolation convention matches pg-tide's pattern,
keeping credential management consistent across the stack.

Security constraints:
- Only endpoints registered in `pg_ripple.federation_endpoints` are reachable;
  arbitrary `SERVICE` IRIs are rejected unless `pg_ripple.allow_unregistered_service_endpoints = on`
  is set explicitly.
- Requests carry a configurable timeout (`federation_timeout_ms`, default 5000).
- Results from remote endpoints are tagged with a `pg:sourceTrust` value
  derived from the endpoint's `min_confidence` setting, ensuring foreign triples
  never silently inherit local confidence scores.

### riverbank impact

riverbank's v0.8.0 "remote profile type" is simplified: it configures a
`federation_endpoints` entry and issues a standard SPARQL query with a
`SERVICE` clause. The Python layer does not implement federation logic.

The compilation decision — when to pull, what confidence to apply, where to
write the result — remains in riverbank. The network call and response parsing
move to pg_ripple.

### Acceptance criteria

- A `SERVICE` clause in `sparql()` against a registered endpoint returns
  federated results merged with local graph data.
- An unregistered `SERVICE` IRI raises a pg_ripple error (not a silent empty
  result) when `allow_unregistered_service_endpoints` is off.
- Remote triples in the result carry the endpoint's configured `min_confidence`
  as their `pg:sourceTrust` value.
- A registered endpoint that times out raises a structured error rather than
  hanging the query.

---

## 4. `coverage_map()` — graph-native knowledge coverage computation

### Background

riverbank's v0.7.0 roadmap plans a daily Prefect flow that computes per-topic
source density, mean extraction confidence, and unanswered-question count,
writing results to `pgc:CoverageMap` triples.

The source density and mean confidence computations are pure aggregate queries
over the graph — SPARQL `GROUP BY` with `AVG()`, `COUNT()`, and `MAX()` over
named graphs and RDF-star confidence annotations. This is standard pg_ripple
territory. Implemented as a pg_trickle stream table in `FULL` refresh mode, the
coverage data updates nightly (or on demand) without requiring Prefect
orchestration.

The **unanswered-question count** is different: it requires joining graph
coverage against the `competency_questions` array in `_riverbank.profiles`. That
join lives on the riverbank side and stays there.

### Proposed addition

A SQL function that computes per-topic coverage over a named graph or set of
named graphs:

```sql
SELECT * FROM pg_ripple.coverage_map(
    named_graphs  => ARRAY['<trusted>', '<human-review>'],  -- defaults to all non-system graphs
    topic_predicate => 'skos:broader',                      -- how to identify topic clusters
    top_k         => 50                                     -- return top-k topics by triple count
);
```

Return type (one row per topic cluster):

| Column | Type | Description |
|---|---|---|
| `topic_iri` | `text` | The topic concept IRI (e.g., a `skos:Concept`) |
| `topic_label` | `text` | `skos:prefLabel` if available |
| `triple_count` | `int` | Total triples in this topic's named subgraph |
| `source_count` | `int` | Distinct `prov:wasDerivedFrom` source IRIs contributing to this topic |
| `mean_confidence` | `float4` | Mean `pg:confidence` across all triples in the topic |
| `min_confidence` | `float4` | Minimum confidence in the topic (worst fact) |
| `contradiction_count` | `int` | Number of `sh:ValidationResult` violations involving triples in this topic |
| `newest_fact_at` | `timestamptz` | Transaction time of the most recently written triple |
| `oldest_fact_at` | `timestamptz` | Transaction time of the oldest triple |

A companion function writes results as `pgc:CoverageMap` triples into a named
graph for SPARQL consumption and `rag_context()` integration:

```sql
SELECT pg_ripple.refresh_coverage_map(
    target_graph  => '<coverage>',
    named_graphs  => ARRAY['<trusted>']
);
```

This can be scheduled as a pg_trickle stream table in `FULL` mode to refresh
nightly without Prefect.

### riverbank impact

riverbank's v0.7.0 coverage map Prefect flow is reduced to:

1. Call `pg_ripple.refresh_coverage_map('<coverage>', ARRAY['<trusted>'])` to
   compute the graph-native coverage metrics.
2. Join the result against `_riverbank.profiles.competency_questions` to compute
   the unanswered-question count per topic — the one piece that requires
   riverbank's catalog.
3. Write the enriched `pgc:CoverageMap` triples (now including
   `pgc:unansweredQuestions`) back to the graph.

The Prefect flow orchestrates step 2 and 3. Step 1 can run independently as a
pg_trickle-scheduled SQL job.

### Acceptance criteria

- `coverage_map()` returns correct `triple_count`, `source_count`, and
  `mean_confidence` for a test graph with known structure.
- `refresh_coverage_map()` writes well-formed `pgc:CoverageMap` triples
  queryable via `sparql()`.
- A pg_trickle stream table using `refresh_coverage_map()` in `FULL` mode
  refreshes without error on a schedule.
- Topics with zero triples are not included in the output (absent topics are
  distinct from low-coverage topics).

---

## 5. `skos-transitive` Datalog bundle — formalise as named built-in

### Background

The riverbank strategy document (§7.3) refers to a `skos-transitive` Datalog
bundle that pg_ripple provides, deriving:

- Transitive `skos:broader` closures
- Symmetric `skos:related` (if A related B, then B related A)
- Transitive `skos:exactMatch`

This capability is mentioned as existing, but it is not clear whether it is a
formal named bundle activatable by name or an informal convention in the
documentation. If it is not yet a named, activatable bundle, it should be — for
two reasons:

1. riverbank's compiler profiles reference it by name (`skos-transitive` in
   `datalog_rule_bundles`). If the name is not formal, the reference is fragile.
2. The SHACL shape `skos:broaderCycleCheck` (item 1 above) depends on the
   transitive closure this bundle produces. The dependency must be explicit and
   machine-checkable.

### Proposed formalisation

```sql
-- Activate the skos-transitive Datalog rule bundle for a named graph.
SELECT pg_ripple.load_datalog_bundle('skos-transitive', named_graph => '<vocab>');

-- List active bundles.
SELECT * FROM pg_ripple.active_datalog_bundles;
```

Rules in the bundle (expressed as Datalog with standard SKOS semantics):

```datalog
-- Transitive broader
skos:broaderTransitive(?x, ?z) :- skos:broader(?x, ?y), skos:broaderTransitive(?y, ?z).
skos:broaderTransitive(?x, ?y) :- skos:broader(?x, ?y).

-- Symmetric related
skos:related(?y, ?x) :- skos:related(?x, ?y).

-- Transitive exactMatch
skos:exactMatch(?x, ?z) :- skos:exactMatch(?x, ?y), skos:exactMatch(?y, ?z).
```

The bundle name `skos-transitive` should be stable and versioned. A
`bundle_version` column in `pg_ripple.active_datalog_bundles` allows riverbank
to assert a minimum version requirement and detect drift.

### Acceptance criteria

- `load_datalog_bundle('skos-transitive')` activates successfully and appears
  in `active_datalog_bundles`.
- After loading, a SPARQL query for `skos:broaderTransitive` returns the
  correctly computed transitive closure.
- `load_shape_bundle('skos-integrity')` implicitly loads `skos-transitive` if
  not already active, or raises a structured error indicating the dependency.

---

## Summary

| Item | riverbank version affected | Complexity | Priority |
|---|---|---|---|
| SKOS integrity shape bundle | v0.4.0 | Medium | High — blocks riverbank v0.4.0 simplification |
| `explain_contradiction()` | v0.7.0 | High | Medium — riverbank can ship a Python fallback but pg_ripple is the right home |
| SPARQL `SERVICE` keyword | v0.8.0 | High | Medium — federation is a SPARQL standard |
| `coverage_map()` function | v0.7.0 | Low–Medium | Medium — the aggregate queries are straightforward SQL |
| `skos-transitive` bundle formalisation | v0.4.0 | Low | High — required for skos-integrity shapes to work correctly |

The two high-priority items (SKOS integrity shapes and `skos-transitive`
formalisation) are needed before riverbank v0.4.0 ships. The others can be
tracked as pg_ripple features to land before their respective riverbank versions.
