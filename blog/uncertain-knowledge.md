# Uncertain Knowledge: Probabilistic Datalog in PostgreSQL

> *Published with pg_ripple v0.87.0 — 3 May 2026*

Real-world knowledge graphs contain uncertain information: extracted facts have
confidence scores, sensor readings have error bounds, and inferred statements depend
on imperfect rules. pg_ripple v0.87.0 introduces a native uncertain knowledge engine
that brings probabilistic reasoning directly into PostgreSQL.

## Probabilistic Datalog Rules

Annotate Datalog rules with `@weight(FLOAT)` to encode rule confidence:

```datalog
-- This rule fires with 90% confidence
@weight(0.90)
?x rdf:type foaf:Person :- ?x foaf:knows ?y .

-- Combine multiple evidence sources with noisy-OR
@weight(0.75)
?x ex:isExpert ?topic :- ?x ex:published ?paper , ?paper ex:about ?topic .
```

When multiple rules derive the same fact, pg_ripple uses **noisy-OR** combination:

$$P(\text{fact}) = 1 - \prod_{i} (1 - w_i \cdot P(\text{evidence}_i))$$

## Querying Confidence

Use `pg:confidence()` in SPARQL to filter by confidence:

```sparql
SELECT ?person ?conf WHERE {
  ?person rdf:type foaf:Person .
  BIND(pg:confidence(?person, rdf:type, foaf:Person) AS ?conf)
  FILTER(?conf > 0.7)
}
```

## Fuzzy SPARQL Matching

The `pg:fuzzy_match()` and `pg:token_set_ratio()` functions enable fuzzy string
matching in SPARQL filters, powered by PostgreSQL's pg_trgm extension:

```sparql
SELECT ?name WHERE {
  ?person foaf:name ?name .
  FILTER(pg:fuzzy_match(?name, "Alice Smith") > 0.8)
}
```

## Soft SHACL Scoring

Instead of binary pass/fail validation, `pg_ripple.shacl_score()` computes a weighted
data-quality score in [0, 1]:

```sql
SELECT pg_ripple.shacl_score('http://example.org/mydata');
-- Returns: 0.94 (94% compliance weighted by shape severity)
```

## Learn More

- [Uncertain Knowledge Feature Reference](../docs/src/features/uncertain-knowledge.md)
- [Probabilistic Datalog Architecture](../plans/probabilistic-features.md)
- [SHACL Soft Scoring](../docs/src/reference/shacl.md)
