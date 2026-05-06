# SKOS Thesaurus Management in PostgreSQL

This cookbook chapter shows how to load, query, and validate a SKOS taxonomy
inside pg_ripple using the W3C-conformant SKOS entailment rules added in v0.98.0.

## What is SKOS?

SKOS (Simple Knowledge Organization System) is the W3C standard vocabulary for
thesauri, taxonomies, classification schemes, and subject heading systems. A SKOS
taxonomy consists of `skos:Concept` nodes linked by `skos:broader`/`skos:narrower`
hierarchies, `skos:related` associative links, and labelled with `skos:prefLabel`.

## Loading the SKOS Rule Set

```sql
-- Load all 28 W3C SKOS entailment rules (S7–S45).
SELECT pg_ripple.load_builtin_rules('skos');

-- Or use the named bundle API for versioned activation:
SELECT pg_ripple.load_datalog_bundle('skos');
```

Once loaded, pg_ripple automatically infers:

- `skos:broaderTransitive` and `skos:narrowerTransitive` transitive closures
- Inverse `skos:narrower`/`skos:broader` pairs
- `skos:related` symmetry
- Sub-property hierarchies for mapping predicates (broadMatch, exactMatch, etc.)
- Label sub-properties (`prefLabel` → `rdfs:label`)

## Loading a Thesaurus

```sql
-- Load Turtle-encoded thesaurus data.
SELECT pg_ripple.load_turtle($$
@prefix skos: <http://www.w3.org/2004/02/skos/core#> .
@prefix ex:   <http://example.org/thesaurus/> .

ex:Animals a skos:ConceptScheme ;
    skos:hasTopConcept ex:Mammals .

ex:Mammals a skos:Concept ;
    skos:prefLabel "Mammals"@en ;
    skos:inScheme ex:Animals .

ex:Cats a skos:Concept ;
    skos:prefLabel "Cats"@en ;
    skos:broader ex:Mammals .

ex:Dogs a skos:Concept ;
    skos:prefLabel "Dogs"@en ;
    skos:broader ex:Mammals .

ex:Cats skos:related ex:Dogs .
$$);

-- Run inference to materialise transitive closures.
SELECT pg_ripple.infer('skos');
```

## Querying Hierarchies with SPARQL

```sql
-- Find all broader transitive ancestors of Cats.
SELECT *
FROM pg_ripple.sparql($$
    SELECT ?ancestor WHERE {
        <http://example.org/thesaurus/Cats>
            <http://www.w3.org/2004/02/skos/core#broaderTransitive> ?ancestor .
    }
$$);

-- Find all concepts in the Mammals subtree (narrowerTransitive closure).
SELECT *
FROM pg_ripple.sparql($$
    SELECT ?child WHERE {
        <http://example.org/thesaurus/Mammals>
            <http://www.w3.org/2004/02/skos/core#narrowerTransitive> ?child .
    }
$$);

-- Cross-scheme mapping: find exactMatch counterparts.
SELECT *
FROM pg_ripple.sparql($$
    SELECT ?concept ?match WHERE {
        ?concept <http://www.w3.org/2004/02/skos/core#exactMatch> ?match .
    }
$$);
```

## SQL Helper Functions

The five SQL helper functions provide convenient access to common thesaurus
operations without writing SPARQL:

```sql
-- Traverse the broaderTransitive closure.
SELECT * FROM pg_ripple.skos_ancestors(
    'http://example.org/thesaurus/Cats'
);
-- Returns: (ancestor_iri, depth)

-- Traverse the narrowerTransitive closure.
SELECT * FROM pg_ripple.skos_descendants(
    'http://example.org/thesaurus/Mammals'
);
-- Returns: (descendant_iri, depth)

-- Look up the preferred label in English.
SELECT pg_ripple.skos_label(
    'http://example.org/thesaurus/Cats',
    'en'
);
-- Returns: "Cats"

-- Find all related concepts.
SELECT * FROM pg_ripple.skos_related(
    'http://example.org/thesaurus/Cats'
);
-- Returns: (related_iri, relation) — e.g. Dogs, skos:related

-- Find sibling concepts (sharing a common skos:broader parent).
SELECT * FROM pg_ripple.skos_siblings(
    'http://example.org/thesaurus/Cats'
);
-- Returns: (sibling_iri, shared_broader_iri) — e.g. Dogs, Mammals
```

## Enabling SKOS-XL Extended Labels

SKOS-XL allows attaching structured metadata to labels. pg_ripple's SKOS-XL
rule set automatically projects `skosxl:Label` instances to plain `skos:prefLabel`
triples:

```sql
SELECT pg_ripple.load_builtin_rules('skosxl');
SELECT pg_ripple.infer('skosxl');
```

After loading, a triple `<concept> skosxl:prefLabel <label>` with
`<label> skosxl:literalForm "text"@en` will produce
`<concept> skos:prefLabel "text"@en` automatically.

## Running Integrity Checks

The `"skos-integrity"` shape bundle validates 10 structural integrity conditions
from the W3C SKOS specification:

```sql
-- Activate the integrity shape bundle (automatically loads skos-transitive).
SELECT pg_ripple.load_shape_bundle('skos-integrity');

-- Run validation.
SELECT * FROM pg_ripple.validate_skos();
-- Returns: (violation_id, subject, message)
-- Example violation: SKOS-IC-05: multiple prefLabels with same language tag.
```

The 10 validators cover:

| ID | W3C Rule | Constraint |
|---|---|---|
| SKOS-IC-01 | S9 | ConceptScheme and Concept are disjoint |
| SKOS-IC-02 | S13 | prefLabel and altLabel must not share literal+lang |
| SKOS-IC-03 | S13 | prefLabel and hiddenLabel must not share literal+lang |
| SKOS-IC-04 | S13 | altLabel and hiddenLabel must not share literal+lang |
| SKOS-IC-05 | S14 | At most one prefLabel per language per concept |
| SKOS-IC-06 | S27 | related and broaderTransitive are disjoint |
| SKOS-IC-07 | S37 | Collection and Concept are disjoint |
| SKOS-IC-08 | S37 | Collection and ConceptScheme are disjoint |
| SKOS-IC-09 | S46 | exactMatch and broadMatch are disjoint |
| SKOS-IC-10 | S46 | exactMatch and relatedMatch are disjoint |

## Named Bundle API

Use `load_datalog_bundle` instead of `load_builtin_rules` when you need
versioned, machine-checkable bundle tracking:

```sql
SELECT pg_ripple.load_datalog_bundle('skos');
SELECT pg_ripple.load_datalog_bundle('skos-transitive');

-- Check what is active.
SELECT * FROM pg_ripple.active_datalog_bundles;
-- Returns: (bundle_name, bundle_version, loaded_at, named_graph)
```

## Performance Notes

- `skos:broaderTransitive` closure materialisation via `WITH RECURSIVE … CYCLE` takes ~200 ms for 1,000 concepts and ~8 s for 100,000 concepts on first materialisation.
- Subsequent IVM updates (when enabled) take ~10 ms per new `skos:broader` triple.
- `skos_ancestors()` runs a live `WITH RECURSIVE` query in ~5 ms for depth ≤ 10, using the materialised `broaderTransitive` triples as the base.

## See Also

- [SKOS Reference](https://www.w3.org/TR/skos-reference/) — W3C Recommendation
- [Datalog Built-in Rules](../reference/datalog-built-in-rules.md) — full rule catalogue
- [SQL Functions Reference](../reference/sql-functions.md) — all pg_ripple SQL functions
