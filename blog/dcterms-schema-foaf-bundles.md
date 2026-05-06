# Built-in Vocabulary Bundles for DCTERMS, Schema.org, and FOAF

*Published with pg_ripple v0.99.0*

After shipping SKOS in v0.98.0, the most common question we heard was: "What
about Schema.org, Dublin Core, and FOAF?" Those three vocabularies appear in
virtually every enterprise knowledge graph we have seen. So v0.99.0 ships all
three as first-class named bundles — with entailment rules, integrity validators,
and cross-vocabulary bridges — using the same API introduced in v0.98.0.

## The "Big 5" Vocabulary Suite Is Now Complete

With v0.99.0, pg_ripple covers the five vocabularies that account for the
majority of real-world linked-data interoperability requirements:

| Bundle | API name | Rules | Integrity validators |
|---|---|---|---|
| SKOS (v0.98.0) | `skos` | 28 | via SHACL shapes |
| Dublin Core Terms | `dcterms` | 11 | 8 |
| Schema.org | `schema` | 15 | 6 |
| FOAF | `foaf` | 8 | 5 |
| DCAT (future) | — | — | — |

Each bundle is activated with a single SQL call:

```sql
SELECT pg_ripple.load_datalog_bundle('dcterms');
SELECT pg_ripple.load_datalog_bundle('schema');
SELECT pg_ripple.load_datalog_bundle('foaf');
```

## DCTERMS: Reasoning Over the Metadata Layer

Almost every public RDF dataset on the web uses Dublin Core. The `dcterms` bundle
handles two pain points that come up constantly:

**DC11 compatibility.** The original DC elements (under the `dc:` / `dc11:`
namespace) are technically different IRIs from their DCTERMS counterparts. Without
reasoning, a query for `dcterms:creator` will miss triples that use `dc11:creator`.
The bundle's five compatibility rules bridge the gap automatically.

**Structural inverses.** `dcterms:hasPart` → `dcterms:isPartOf` and the
`hasVersion`/`isVersionOf`, `replaces`/`isReplacedBy` pairs are widely used but
rarely stored in both directions. The rules infer the reverse direction so SPARQL
queries using either predicate return complete results.

## Schema.org: Type-Hierarchy Closure at Scale

Schema.org has a rich type hierarchy (`LocalBusiness` → `Organization` → `Thing`)
that many users want to exploit in queries without manually spelling out every
supertype. The `schema` bundle materialises 15 rules covering:

- The most common type subsumption chains (Person, Organization, Product, Event,
  CreativeWork, Place, Action all connect to Thing)
- Inverse property pairs (`schema:subjectOf`/`schema:about`, `schema:hasPart`/
  `schema:isPartOf`)
- Cross-vocabulary bridges: `schema:author` → `foaf:maker`, `schema:name` →
  `dcterms:title`, `schema:dataset` → `dcat:Dataset`

We also ship a SQL convenience function for exploring the hierarchy:

```sql
SELECT ancestor_type
FROM pg_ripple.schema_type_ancestors('https://schema.org/LocalBusiness');
```

This returns `LocalBusiness`, `Organization`, and `Thing` — whatever the current
graph contains for that resource.

## FOAF: Social-Graph Symmetry and Type Subsumption

FOAF's property semantics are well-specified but rarely implemented in databases.
The eight rules in the `foaf` bundle cover the cases that matter most in practice:

- `foaf:knows` is symmetric — a single assertion in one direction produces both.
- `foaf:Person`, `foaf:Organization`, and `foaf:Group` are all subtypes of
  `foaf:Agent`, so queries that filter by `foaf:Agent` automatically include all
  three.
- `foaf:made` and `foaf:maker` are inverses, and `foaf:account`/`foaf:accountFor`
  are inverses.

The `foaf_persons()` helper surfaces all FOAF persons in the graph:

```sql
SELECT person_iri, name_label
FROM pg_ripple.foaf_persons();
```

## Cross-Vocabulary Reasoning

One of the most valuable features shipped in v0.99.0 is cross-vocabulary
inference. When you load multiple bundles, the bridges between them activate:

```sql
SELECT pg_ripple.load_datalog_bundle('dcterms');
SELECT pg_ripple.load_datalog_bundle('schema');
SELECT pg_ripple.load_datalog_bundle('foaf');
SELECT pg_ripple.load_datalog_bundle('skos');
```

Now:
- A resource described with `schema:author <ex:Alice>` is also discoverable via
  `foaf:maker <ex:Alice>` (SCHEMA-FOAF-01).
- A resource with `dcterms:creator <ex:Alice>` is discoverable via `foaf:maker`
  too (DC-FOAF-01).
- A resource whose `dcterms:subject` is a member of a SKOS scheme receives a
  `skos:Concept` type assertion automatically (DC-SKOS-01).
- Any resource with `schema:name "..."` can also be found via
  `dcterms:title "..."` (SCHEMA-DC-01).

This means legacy datasets, public LOD endpoints, and modern schema.org-annotated
documents can be queried uniformly without data transformation.

## Integrity Validation

Every bundle ships with a corresponding integrity bundle built on top of pg_ripple's
Datalog integrity rules:

```sql
SELECT pg_ripple.load_shape_bundle('dcterms-integrity');
SELECT pg_ripple.load_shape_bundle('schema-integrity');
SELECT pg_ripple.load_shape_bundle('foaf-integrity');
```

The validators catch the most common data-quality issues: missing required
properties, malformed dates, invalid URL patterns, self-referential creator
chains, and cycles in hierarchical structure.

## What's Next

The final milestone before v1.0.0 is v0.99.1 (production hardening) and the 1.0
release itself. With the Big 5 vocabularies shipped, the focus shifts to stress
testing, security audit, and ensuring every API is stable enough to carry a
stability guarantee.

The cookbook chapter [Common Vocabulary Bundles](../docs/src/cookbook/common-vocabularies.md)
contains runnable examples for all three vocabularies.
