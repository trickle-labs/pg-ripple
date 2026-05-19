# Allen's Interval Relations in pg_ripple

Allen's interval algebra defines thirteen mutually exclusive relations between
two time intervals. pg_ripple implements all seven base relations (the
remaining six are converses) as both SQL functions and SPARQL extension
functions, enabling expressive temporal queries over knowledge graphs.

## Background: Allen's 13 relations

James F. Allen's 1983 paper "Maintaining Knowledge About Temporal Intervals"
introduced a complete algebra for reasoning about time. The seven base
relations (plus six converses) are:

| Relation | Symbol | Meaning |
|----------|--------|---------|
| before   | `<`    | A ends before B starts |
| meets    | `m`    | A ends when B starts (adjacent) |
| overlaps | `o`    | A starts before B and they partially overlap |
| during   | `d`    | A is entirely within B |
| finishes | `f`    | A ends with B |
| starts   | `s`    | A starts with B |
| equals   | `=`    | A and B are identical |

## Using Allen relations in SQL

All seven relations are available as SQL functions:

```sql
-- Does the project run before the conference?
SELECT pg_ripple.allen_before(
    '2026-01-01'::timestamptz, '2026-03-31'::timestamptz,  -- project
    '2026-06-01'::timestamptz, '2026-06-05'::timestamptz   -- conference
);  -- true

-- Do the two projects overlap?
SELECT pg_ripple.allen_overlaps(
    '2026-01-01'::timestamptz, '2026-04-30'::timestamptz,
    '2026-03-01'::timestamptz, '2026-07-31'::timestamptz
);  -- true

-- Does sprint A finish at exactly the same time as sprint B?
SELECT pg_ripple.allen_meets(
    '2026-01-01'::timestamptz, '2026-01-14'::timestamptz,
    '2026-01-14'::timestamptz, '2026-01-28'::timestamptz
);  -- true
```

## Using Allen relations in SPARQL

The seven relations are available as SPARQL extension functions via the
`pg:` namespace (`https://pgrdf.io/fn/`):

```sparql
PREFIX pg:  <https://pgrdf.io/fn/>
PREFIX ex:  <https://example.org/>
PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

# Find all events that overlap with a reference interval
SELECT ?event ?label WHERE {
  ?event ex:startTime ?s ;
         ex:endTime   ?e ;
         ex:label     ?label .
  FILTER(pg:overlaps(
    ?s, ?e,
    "2026-06-01T00:00:00Z"^^xsd:dateTime,
    "2026-06-30T23:59:59Z"^^xsd:dateTime
  ))
}
```

## Temporal queries on knowledge graphs

Allen relations shine in event-centric knowledge graphs. Here is a query that
finds all clinical trials that were active during a patient's hospitalisation:

```sparql
PREFIX fhir: <http://hl7.org/fhir/>
PREFIX pg:   <https://pgrdf.io/fn/>

SELECT ?trial ?trialName WHERE {
  # The hospitalisation interval
  <https://example.org/hospitalisation/42>
      fhir:period/fhir:start ?admitDate ;
      fhir:period/fhir:end   ?dischargeDate .

  # All active trials
  ?trial a fhir:ResearchStudy ;
         fhir:title    ?trialName ;
         fhir:period/fhir:start ?trialStart ;
         fhir:period/fhir:end   ?trialEnd .

  # Trial was active at some point during hospitalisation (overlaps or during)
  FILTER(
    pg:overlaps(?trialStart, ?trialEnd, ?admitDate, ?dischargeDate) ||
    pg:during(?trialStart, ?trialEnd, ?admitDate, ?dischargeDate)
  )
}
```

## Implementation notes

Each relation is implemented as a single-expression Boolean SQL function with
`IMMUTABLE` volatility, allowing PostgreSQL to push the filter into an index
scan or a nested-loop join plan. The SPARQL function dispatcher maps `pg:before`
→ `pg_ripple.allen_before` etc. at algebra translation time — no overhead at
execution time.

## See also

- [SQL API reference: Allen's interval relations](../docs/src/reference/sql-api.md#allens-interval-relations)
- [Temporal queries guide](../docs/src/reference/sparql.md)
- [OWL property chain axioms](owl-property-chain-axiom.md)
