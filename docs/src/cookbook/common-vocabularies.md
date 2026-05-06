# Common Vocabulary Bundles: DCTERMS, Schema.org, and FOAF

This cookbook chapter shows how to load and use the three most widely-deployed
linked-data vocabularies in pg_ripple — Dublin Core Terms (DCTERMS), Schema.org,
and FOAF — using the named bundle API introduced in v0.98.0.

Together with SKOS (v0.98.0), these bundles complete the "Big 5" vocabularies
that cover the vast majority of real-world knowledge-graph interoperability.

## Why These Vocabularies?

| Vocabulary | Namespace prefix | Typical use |
|---|---|---|
| Dublin Core Terms | `dcterms:` | Metadata — creator, date, subject, rights, format |
| Schema.org | `schema:` | Structured data — people, orgs, products, places, events |
| FOAF | `foaf:` | Social-graph data — persons, accounts, social connections |

All three are already used in billions of public RDF documents, Wikidata exports,
and enterprise knowledge graphs. Loading their rule sets lets pg_ripple perform
property-hierarchy inference, type-subsumption reasoning, and integrity validation
over data you already have — without any schema changes.

## Loading the Bundles

```sql
-- DCTERMS: 11 Datalog rules + 8 integrity validators
SELECT pg_ripple.load_datalog_bundle('dcterms');
SELECT pg_ripple.load_shape_bundle('dcterms-integrity');

-- Schema.org: 15 Datalog rules + 6 integrity validators
SELECT pg_ripple.load_datalog_bundle('schema');
SELECT pg_ripple.load_shape_bundle('schema-integrity');

-- FOAF: 8 Datalog rules + 5 integrity validators
SELECT pg_ripple.load_datalog_bundle('foaf');
SELECT pg_ripple.load_shape_bundle('foaf-integrity');
```

After loading, confirm activation:

```sql
SELECT bundle_name, bundle_version, activated_at
FROM pg_ripple.active_datalog_bundles
ORDER BY activated_at;
```

## DCTERMS — Dublin Core Terms

### Inferred Properties

The `dcterms` bundle adds 11 rules, including:

- **DC11 compatibility aliases** — `dc11:title` is treated equivalently to
  `dcterms:title`, and similarly for `creator`, `subject`, `description`,
  `publisher`, `date`, `type`, `format`, `identifier`, `source`, `language`.
- **Structural inverses** — `dcterms:hasPart` ↔ `dcterms:isPartOf`,
  `dcterms:hasVersion` ↔ `dcterms:isVersionOf`, `dcterms:replaces` ↔
  `dcterms:isReplacedBy`.
- **DC-SKOS bridge** (DC-SKOS-01) — resources whose `dcterms:subject` is a member
  of a SKOS `ConceptScheme` automatically receive a `skos:Concept` type assertion.

### Example

```sql
-- Load some metadata triples using the old dc11: namespace.
SELECT pg_ripple.load_turtle($$
@prefix dc11: <http://purl.org/dc/elements/1.1/> .
@prefix dcterms: <http://purl.org/dc/terms/> .
@prefix ex: <https://example.org/> .

ex:Article1 dc11:creator "Alice" ;
            dcterms:hasPart ex:Section1 .
$$);

-- Load the dcterms rule set.
SELECT pg_ripple.load_datalog_bundle('dcterms');

-- dc11:creator is now mapped to dcterms:creator.
SELECT pg_ripple.sparql_select($$
  SELECT ?article ?creator
  WHERE { ?article <http://purl.org/dc/terms/creator> ?creator }
$$);

-- dcterms:isPartOf is inferred from dcterms:hasPart.
SELECT pg_ripple.sparql_select($$
  SELECT ?part ?whole
  WHERE { ?part <http://purl.org/dc/terms/isPartOf> ?whole }
$$);
```

## Schema.org — Structured Data Vocabulary

### Inferred Properties

The `schema` bundle adds 15 rules:

- **Inverse pairs** — `schema:subjectOf` ↔ `schema:about`, `schema:hasPart` ↔
  `schema:isPartOf`, `schema:workExample` ↔ `schema:exampleOfWork`,
  `schema:member` ↔ `schema:memberOf`.
- **Type hierarchy** — `schema:LocalBusiness` ⊆ `schema:Organization` ⊆
  `schema:Thing`, `schema:Person` ⊆ `schema:Thing`, `schema:Product` ⊆
  `schema:Thing`, and several more.
- **Cross-vocab bridges** — `schema:author` → `foaf:maker` (SCHEMA-FOAF-01),
  `schema:name` → `dcterms:title` (SCHEMA-DC-01), `schema:dataset` → `dcat:Dataset`
  (SCHEMA-DCAT-01).

### SQL Helper: `schema_type_ancestors`

```sql
-- Return all Schema.org type ancestors for a given IRI.
SELECT ancestor_type
FROM pg_ripple.schema_type_ancestors('https://schema.org/LocalBusiness');
-- Returns: schema:LocalBusiness, schema:Organization, schema:Thing
```

### Example

```sql
-- Load a local business.
SELECT pg_ripple.load_turtle($$
@prefix schema: <https://schema.org/> .
@prefix ex: <https://example.org/> .

ex:AcmeCafe a schema:CafeOrCoffeeShop ;
    schema:name "ACME Café" ;
    schema:address ex:CafeAddress .
$$);

SELECT pg_ripple.load_datalog_bundle('schema');

-- CafeOrCoffeeShop is a FoodEstablishment, LocalBusiness, Organization, Thing.
SELECT pg_ripple.sparql_select($$
  SELECT ?type
  WHERE { <https://example.org/AcmeCafe> a ?type }
  ORDER BY ?type
$$);
```

## FOAF — Friend-of-a-Friend

### Inferred Properties

The `foaf` bundle adds 8 rules:

- **`foaf:knows` symmetry** — if Alice knows Bob, Bob knows Alice.
- **Type subsumption** — `foaf:Person` ⊆ `foaf:Agent`, `foaf:Organization` ⊆
  `foaf:Agent`, `foaf:Group` ⊆ `foaf:Agent`.
- **Inverse properties** — `foaf:account` ↔ `foaf:accountFor`, `foaf:made` ↔
  `foaf:maker`.
- **DC-FOAF bridge** (DC-FOAF-01) — `dcterms:creator` triples generate a
  corresponding `foaf:maker` assertion on the referenced resource.

### SQL Helper: `foaf_persons`

```sql
-- Return all foaf:Person IRIs and their foaf:name labels.
SELECT person_iri, name_label
FROM pg_ripple.foaf_persons();
```

### Example

```sql
-- Load FOAF data.
SELECT pg_ripple.load_turtle($$
@prefix foaf: <http://xmlns.com/foaf/0.1/> .
@prefix ex: <https://example.org/> .

ex:Alice a foaf:Person ;
    foaf:name "Alice" ;
    foaf:knows ex:Bob .
ex:Bob a foaf:Person ;
    foaf:name "Bob" .
$$);

SELECT pg_ripple.load_datalog_bundle('foaf');

-- foaf:knows is symmetric: Bob knows Alice.
SELECT pg_ripple.sparql_select($$
  SELECT ?who ?knows
  WHERE { ?who <http://xmlns.com/foaf/0.1/knows> ?knows }
$$);

-- foaf:Person ⊆ foaf:Agent: Alice is also a foaf:Agent.
SELECT pg_ripple.sparql_select($$
  SELECT ?agent
  WHERE { ?agent a <http://xmlns.com/foaf/0.1/Agent> }
$$);
```

## Cross-Vocabulary Inference

When all three bundles (and SKOS) are loaded together, cross-vocabulary bridges
activate:

| Rule ID | Effect |
|---|---|
| DC-FOAF-01 | `dcterms:creator ?x` → `foaf:maker ?x` |
| SCHEMA-FOAF-01 | `schema:author ?x` → `foaf:maker ?x` |
| SCHEMA-DC-01 | `schema:name ?x` → `dcterms:title ?x` |
| DC-SKOS-01 | `dcterms:subject ?x` where `?x` is in a ConceptScheme → `?x a skos:Concept` |

This means a resource described with `schema:author` can be found via a
`foaf:maker` query, and a resource with `schema:name` can be found via a
`dcterms:title` filter — without any data transformation.

## Integrity Validation

Each vocabulary ships with a corresponding integrity bundle:

```sql
-- Validate DCTERMS constraints (title cardinality, date format, etc.)
SELECT pg_ripple.load_shape_bundle('dcterms-integrity');

-- Validate Schema.org constraints (required name, URL format, price range, etc.)
SELECT pg_ripple.load_shape_bundle('schema-integrity');

-- Validate FOAF constraints (homepage IRI, mbox mailto:, name cardinality)
SELECT pg_ripple.load_shape_bundle('foaf-integrity');
```

Integrity violations are surfaced through the standard SHACL validation report.

## Checking Active Bundles

```sql
SELECT bundle_name, bundle_version, activated_at
FROM pg_ripple.active_datalog_bundles
ORDER BY bundle_name;

-- bundle_name         | bundle_version | activated_at
-- --------------------+----------------+---------------------------
-- dcterms             | 0.99.0         | 2026-05-07 10:00:00+00
-- foaf                | 0.99.0         | 2026-05-07 10:00:01+00
-- schema              | 0.99.0         | 2026-05-07 10:00:02+00
-- skos                | 0.98.0         | 2026-05-07 09:59:59+00
```

## See Also

- [SKOS Thesaurus Management](skos-thesaurus.md) — the fourth member of the
  "Big 5" vocabulary bundle suite
- [SHACL + Datalog Data Quality Pipeline](shacl-datalog-quality.md) — how to
  combine rules and shapes for a full data-quality pipeline
