# SKOS Knowledge Organization in PostgreSQL

*How pg_ripple v0.98.0 turns your relational database into a W3C-conformant thesaurus engine*

## Introduction

Most organizations that deal with structured vocabularies — subject headings, product taxonomies, clinical code systems — eventually discover SKOS (Simple Knowledge Organization System). SKOS is the W3C standard for representing thesauri, taxonomies, and classification schemes as linked data. It defines a small but powerful vocabulary with precise semantics: `skos:broader`/`skos:narrower` hierarchies, `skos:related` associative links, multi-language labels, and cross-vocabulary mapping predicates.

pg_ripple v0.98.0 delivers a complete W3C-conformant SKOS entailment stack directly inside PostgreSQL. This means your SPARQL queries over SKOS data automatically benefit from all 28 entailment rules defined in the W3C specification — transitive closures, inverse property inference, sub-property hierarchies, and more.

## Loading an AGROVOC-style Taxonomy

The FAO's AGROVOC thesaurus is one of the largest publicly available SKOS vocabularies, covering agricultural science with over 40,000 concepts in 40+ languages. Here's how to load a subset:

```sql
-- Step 1: activate the SKOS rule set
SELECT pg_ripple.load_datalog_bundle('skos');

-- Step 2: load your thesaurus
SELECT pg_ripple.load_turtle($$
@prefix skos: <http://www.w3.org/2004/02/skos/core#> .
@prefix agro: <http://aims.fao.org/aos/agrovoc/> .

agro:c_330 a skos:Concept ;
    skos:prefLabel "Wheat"@en, "Blé"@fr ;
    skos:broader agro:c_1055 ;  -- Cereals
    skos:exactMatch <http://www.wikidata.org/entity/Q16879> .

agro:c_1055 a skos:Concept ;
    skos:prefLabel "Cereals"@en ;
    skos:broader agro:c_1234 .  -- Crops

agro:c_1234 a skos:Concept ;
    skos:prefLabel "Crops"@en .
$$);

-- Step 3: materialise the entailments
SELECT pg_ripple.infer('skos');
```

## Querying Hierarchies

After inference, querying hierarchical relationships is straightforward:

```sql
-- Find all ancestor concepts of Wheat.
SELECT ancestor_iri, depth
FROM pg_ripple.skos_ancestors('http://aims.fao.org/aos/agrovoc/c_330')
ORDER BY depth;

-- Find all grain crops (descendants of Cereals).
SELECT descendant_iri, depth
FROM pg_ripple.skos_descendants('http://aims.fao.org/aos/agrovoc/c_1055')
ORDER BY depth;
```

The `WITH RECURSIVE … CYCLE` queries inside these helpers guarantee correct
termination even on cyclic data (which occasionally appears in real-world
thesauri with editorial errors).

## Cross-Vocabulary Mapping

SKOS includes five mapping predicates — `exactMatch`, `closeMatch`, `broadMatch`,
`narrowMatch`, and `relatedMatch` — for linking concepts across vocabularies.
pg_ripple's SKOS rules automatically propagate these:

```sql
-- exactMatch is transitive: if A exactMatch B and B exactMatch C, infer A exactMatch C.
SELECT * FROM pg_ripple.sparql($$
    SELECT ?wheat ?wikidata WHERE {
        ?wheat skos:exactMatch ?wikidata .
        FILTER(CONTAINS(STR(?wikidata), "wikidata"))
    }
$$);
```

## SKOS-XL: Rich Label Metadata

When you need to attach provenance or publication dates to labels themselves,
use SKOS-XL:

```sql
SELECT pg_ripple.load_builtin_rules('skosxl');

-- Load SKOS-XL labels.
SELECT pg_ripple.load_ntriples($$
<http://example.org/Wheat> <http://www.w3.org/2008/05/skos-xl#prefLabel> <http://example.org/Wheat_en_label> .
<http://example.org/Wheat_en_label> <http://www.w3.org/2008/05/skos-xl#literalForm> "Wheat"@en .
$$);

SELECT pg_ripple.infer('skosxl');

-- After inference, the plain skos:prefLabel is also available.
SELECT pg_ripple.skos_label('http://example.org/Wheat', 'en');
```

## Integrity Validation

Before publishing a thesaurus update, run the 10-point SKOS integrity check:

```sql
SELECT pg_ripple.load_shape_bundle('skos-integrity');
SELECT * FROM pg_ripple.validate_skos();
```

Common violations the validator catches:

- **SKOS-IC-05**: Two `prefLabel` values sharing the same language tag (S14 violation)
- **SKOS-IC-06**: A concept linked by both `skos:related` and `skos:broaderTransitive` (S27 violation)
- **SKOS-IC-09**: A pair linked by both `skos:exactMatch` and `skos:broadMatch` (S46 violation)

## Named Bundle API for Applications

When your application needs to verify that the correct rule sets are active, use
the named bundle API:

```sql
-- Activate with tracking.
SELECT pg_ripple.load_datalog_bundle('skos');
SELECT pg_ripple.load_datalog_bundle('skos-transitive');

-- Query the bundle catalog from your application.
SELECT bundle_name, bundle_version, loaded_at
FROM pg_ripple.active_datalog_bundles
WHERE bundle_name = 'skos';
```

This is particularly useful for riverbank-style compiler profiles that reference
rule sets by name in their configuration.

## Conclusion

pg_ripple v0.98.0 makes PostgreSQL a first-class citizen in the SKOS ecosystem.
You get W3C-conformant entailment, structural integrity validation, and convenient
SQL helper functions — all within the same database that powers your application.
No external triple store, no separate service, no synchronization headaches.
