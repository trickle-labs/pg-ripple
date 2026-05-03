# SPARQL Compliance Matrix

pg_ripple implements the full SPARQL 1.1 specification suite. This page details conformance status for every feature in the W3C SPARQL 1.1 Query, Update, and Protocol recommendations.

```admonish success title="Full compliance"
As of v0.46.0, pg_ripple passes 100% of the W3C SPARQL 1.1 test suite (~3 000 tests), ≥ 99.9% of the Apache Jena edge-case suite (~1 000 tests), all 100 WatDiv query templates at 10 M-triple scale with correctness validated to ±0.1% row-count baselines, all 14 LUBM queries with OWL RL inference correctness, and ≥ 80% of the W3C OWL 2 RL conformance suite.
```

---

## SPARQL 1.1 Query — Query Forms

| Feature | Status | Since | Notes |
|---|---|---|---|
| `SELECT` | ✅ Supported | v0.1.0 | Full projection with expressions |
| `CONSTRUCT` | ✅ Supported | v0.8.0 | Returns triples as JSON, Turtle, or JSON-LD |
| `ASK` | ✅ Supported | v0.8.0 | Returns boolean |
| `DESCRIBE` | ✅ Supported | v0.8.0 | Symmetric concise bounded description |

---

## SPARQL 1.1 Query — Algebra Operations

| Feature | Status | Since | Notes |
|---|---|---|---|
| Basic Graph Pattern (BGP) | ✅ Supported | v0.1.0 | Translated to VP table joins |
| Join (inner) | ✅ Supported | v0.1.0 | |
| LeftJoin (`OPTIONAL`) | ✅ Supported | v0.1.0 | Downgraded to INNER JOIN when SHACL `sh:minCount 1` is set |
| Filter | ✅ Supported | v0.1.0 | All comparison, logical, and arithmetic operators |
| Union | ✅ Supported | v0.5.0 | `UNION ALL` in generated SQL |
| Minus | ✅ Supported | v0.5.0 | `EXCEPT` in generated SQL |
| Extend (`BIND`) | ✅ Supported | v0.1.0 | |
| Group (`GROUP BY`) | ✅ Supported | v0.5.0 | |
| Having | ✅ Supported | v0.5.0 | |
| OrderBy | ✅ Supported | v0.1.0 | |
| Project | ✅ Supported | v0.1.0 | |
| Distinct | ✅ Supported | v0.1.0 | Omitted when SHACL `sh:maxCount 1` is set |
| Reduced | ✅ Supported | v0.5.0 | Treated as hint; may or may not deduplicate |
| Slice (`LIMIT`/`OFFSET`) | ✅ Supported | v0.1.0 | |
| Service (`SERVICE`) | ✅ Supported | v0.16.0 | Federated query via HTTP |
| Service Silent (`SERVICE SILENT`) | ✅ Supported | v0.16.0 | Returns empty on endpoint failure |
| Values (`VALUES`) | ✅ Supported | v0.5.0 | Inline data bindings |
| Lateral (`LATERAL`) | ✅ Supported | v0.22.0 | PostgreSQL `LATERAL JOIN` |
| Subqueries | ✅ Supported | v0.5.0 | Nested `SELECT` |
| Negation (`NOT EXISTS`) | ✅ Supported | v0.5.0 | |
| Negation (`EXISTS`) | ✅ Supported | v0.5.0 | |

---

## SPARQL 1.1 Query — Property Paths

| Feature | Status | Since | Notes |
|---|---|---|---|
| Sequence path (`/`) | ✅ Supported | v0.5.0 | |
| Alternative path (`\|`) | ✅ Supported | v0.5.0 | |
| Inverse path (`^`) | ✅ Supported | v0.5.0 | |
| Zero-or-more (`*`) | ✅ Supported | v0.5.0 | `WITH RECURSIVE … CYCLE` |
| One-or-more (`+`) | ✅ Supported | v0.5.0 | `WITH RECURSIVE … CYCLE` |
| Zero-or-one (`?`) | ✅ Supported | v0.5.0 | |
| Negated property set (`!(p1\|p2)`) | ✅ Supported | v0.5.0 | |
| Fixed-length path (`{n}`) | ✅ Supported | v0.5.0 | Unrolled to `n` joins |
| Variable-length path (`{n,m}`) | ✅ Supported | v0.5.0 | Bounded recursion |

```admonish note title="Cycle detection"
All recursive property paths use PostgreSQL 18's native `CYCLE` clause for hash-based cycle detection, bounded by `pg_ripple.max_path_depth` (default: 10).
```

---

## SPARQL 1.1 Query — Aggregates

| Feature | Status | Since | Notes |
|---|---|---|---|
| `COUNT` | ✅ Supported | v0.5.0 | Including `COUNT(DISTINCT *)` |
| `SUM` | ✅ Supported | v0.5.0 | |
| `AVG` | ✅ Supported | v0.5.0 | |
| `MIN` | ✅ Supported | v0.5.0 | |
| `MAX` | ✅ Supported | v0.5.0 | |
| `GROUP_CONCAT` | ✅ Supported | v0.5.0 | With custom separator |
| `SAMPLE` | ✅ Supported | v0.5.0 | |

---

## SPARQL 1.1 Query — Built-in Functions

| Function | Status | Since |
|---|---|---|
| `STR()` | ✅ Supported | v0.1.0 |
| `LANG()` | ✅ Supported | v0.3.0 |
| `DATATYPE()` | ✅ Supported | v0.3.0 |
| `IRI()` / `URI()` | ✅ Supported | v0.5.0 |
| `BNODE()` | ✅ Supported | v0.5.0 |
| `RAND()` | ✅ Supported | v0.5.0 |
| `ABS()` | ✅ Supported | v0.1.0 |
| `CEIL()` | ✅ Supported | v0.1.0 |
| `FLOOR()` | ✅ Supported | v0.1.0 |
| `ROUND()` | ✅ Supported | v0.1.0 |
| `CONCAT()` | ✅ Supported | v0.5.0 |
| `STRLEN()` | ✅ Supported | v0.1.0 |
| `UCASE()` | ✅ Supported | v0.1.0 |
| `LCASE()` | ✅ Supported | v0.1.0 |
| `ENCODE_FOR_URI()` | ✅ Supported | v0.5.0 |
| `CONTAINS()` | ✅ Supported | v0.1.0 |
| `STRSTARTS()` | ✅ Supported | v0.1.0 |
| `STRENDS()` | ✅ Supported | v0.1.0 |
| `STRBEFORE()` | ✅ Supported | v0.5.0 |
| `STRAFTER()` | ✅ Supported | v0.5.0 |
| `YEAR()` | ✅ Supported | v0.5.0 |
| `MONTH()` | ✅ Supported | v0.5.0 |
| `DAY()` | ✅ Supported | v0.5.0 |
| `HOURS()` | ✅ Supported | v0.5.0 |
| `MINUTES()` | ✅ Supported | v0.5.0 |
| `SECONDS()` | ✅ Supported | v0.5.0 |
| `TIMEZONE()` | ✅ Supported | v0.5.0 |
| `TZ()` | ✅ Supported | v0.5.0 |
| `NOW()` | ✅ Supported | v0.5.0 |
| `UUID()` | ✅ Supported | v0.5.0 |
| `STRUUID()` | ✅ Supported | v0.5.0 |
| `MD5()` | ✅ Supported | v0.5.0 |
| `SHA1()` | ✅ Supported | v0.5.0 |
| `SHA256()` | ✅ Supported | v0.5.0 |
| `SHA384()` | ✅ Supported | v0.5.0 |
| `SHA512()` | ✅ Supported | v0.5.0 |
| `COALESCE()` | ✅ Supported | v0.1.0 |
| `IF()` | ✅ Supported | v0.1.0 |
| `STRLANG()` | ✅ Supported | v0.5.0 |
| `STRDT()` | ✅ Supported | v0.5.0 |
| `isIRI()` / `isURI()` | ✅ Supported | v0.1.0 |
| `isBlank()` | ✅ Supported | v0.1.0 |
| `isLiteral()` | ✅ Supported | v0.1.0 |
| `isNumeric()` | ✅ Supported | v0.5.0 |
| `REGEX()` | ✅ Supported | v0.1.0 |
| `REPLACE()` | ✅ Supported | v0.5.0 |
| `SUBSTR()` | ✅ Supported | v0.5.0 |
| `BOUND()` | ✅ Supported | v0.1.0 |
| `IN` / `NOT IN` | ✅ Supported | v0.5.0 |
| `TRIPLE()` (RDF-star) | ✅ Supported | v0.4.0 |
| `SUBJECT()` (RDF-star) | ✅ Supported | v0.4.0 |
| `PREDICATE()` (RDF-star) | ✅ Supported | v0.4.0 |
| `OBJECT()` (RDF-star) | ✅ Supported | v0.4.0 |
| `isTRIPLE()` (RDF-star) | ✅ Supported | v0.4.0 |

---

## SPARQL 1.1 Query — Typed Literals

| Datatype | Status | Notes |
|---|---|---|
| `xsd:integer` | ✅ Supported | Maps to PostgreSQL `BIGINT` |
| `xsd:decimal` | ✅ Supported | Maps to `NUMERIC` |
| `xsd:float` | ✅ Supported | Maps to `REAL` |
| `xsd:double` | ✅ Supported | Maps to `DOUBLE PRECISION` |
| `xsd:boolean` | ✅ Supported | Maps to `BOOLEAN` |
| `xsd:string` | ✅ Supported | Default literal type |
| `xsd:dateTime` | ✅ Supported | Maps to `TIMESTAMPTZ` |
| `xsd:date` | ✅ Supported | Maps to `DATE` |
| `xsd:time` | ✅ Supported | Maps to `TIME` |
| `xsd:gYear` | ✅ Supported | Stored as string, compared lexically |
| Language-tagged strings | ✅ Supported | `"text"@en` syntax |

---

## SPARQL 1.1 Update

| Operation | Status | Since | Notes |
|---|---|---|---|
| `INSERT DATA` | ✅ Supported | v0.7.0 | |
| `DELETE DATA` | ✅ Supported | v0.7.0 | |
| `DELETE WHERE` | ✅ Supported | v0.7.0 | |
| `DELETE/INSERT WHERE` | ✅ Supported | v0.7.0 | |
| `INSERT WHERE` | ✅ Supported | v0.7.0 | |
| `LOAD` | ✅ Supported | v0.7.0 | Via `pg_ripple_http` or direct file |
| `CLEAR GRAPH` | ✅ Supported | v0.7.0 | |
| `CLEAR DEFAULT` | ✅ Supported | v0.7.0 | |
| `CLEAR NAMED` | ✅ Supported | v0.7.0 | |
| `CLEAR ALL` | ✅ Supported | v0.7.0 | |
| `DROP GRAPH` | ✅ Supported | v0.7.0 | |
| `DROP DEFAULT` | ✅ Supported | v0.7.0 | |
| `DROP NAMED` | ✅ Supported | v0.7.0 | |
| `DROP ALL` | ✅ Supported | v0.7.0 | |
| `CREATE GRAPH` | ✅ Supported | v0.7.0 | |
| `CREATE SILENT GRAPH` | ✅ Supported | v0.7.0 | |
| `COPY` | ✅ Supported | v0.21.0 | |
| `MOVE` | ✅ Supported | v0.21.0 | |
| `ADD` | ✅ Supported | v0.21.0 | |
| Multi-statement (`;` separator) | ✅ Supported | v0.7.0 | |
| `USING` / `USING NAMED` | ✅ Supported | v0.7.0 | Dataset clause for updates |

---

## SPARQL 1.1 Protocol

| Feature | Status | Notes |
|---|---|---|
| Query via HTTP GET | ✅ Supported | Via `pg_ripple_http` |
| Query via HTTP POST (form-encoded) | ✅ Supported | Via `pg_ripple_http` |
| Query via HTTP POST (direct body) | ✅ Supported | Via `pg_ripple_http` |
| Update via HTTP POST | ✅ Supported | Via `pg_ripple_http` |
| Content negotiation (`Accept` header) | ✅ Supported | JSON, Turtle, N-Triples, XML |
| `default-graph-uri` parameter | ✅ Supported | |
| `named-graph-uri` parameter | ✅ Supported | |
| Multiple `default-graph-uri` | ✅ Supported | |
| Multiple `named-graph-uri` | ✅ Supported | |

```admonish note title="Protocol endpoint"
SPARQL Protocol support requires the `pg_ripple_http` companion service. See [APIs and Integration](../features/apis-and-integration.md) for setup instructions.
```

---

## SPARQL 1.1 Service Description

| Feature | Status | Notes |
|---|---|---|
| Service description at endpoint root | ✅ Supported | Via `pg_ripple_http` |
| `sd:supportedLanguage` | ✅ Supported | Reports SPARQL 1.1 Query and Update |
| `sd:resultFormat` | ✅ Supported | JSON, XML, CSV, TSV |
| `sd:defaultDataset` | ✅ Supported | |
| `sd:feature` | ✅ Supported | Reports `sd:UnionDefaultGraph`, `sd:RequiresDataset` |

---

## SPARQL 1.1 Graph Store HTTP Protocol

| Operation | Status | Notes |
|---|---|---|
| `GET` (retrieve graph) | ✅ Supported | Via `pg_ripple_http` |
| `PUT` (replace graph) | ✅ Supported | Via `pg_ripple_http` |
| `POST` (merge into graph) | ✅ Supported | Via `pg_ripple_http` |
| `DELETE` (drop graph) | ✅ Supported | Via `pg_ripple_http` |
| `?default` parameter | ✅ Supported | |
| `?graph=<uri>` parameter | ✅ Supported | |

---

## RDF-star / SPARQL-star

| Feature | Status | Since | Notes |
|---|---|---|---|
| Quoted triple storage | ✅ Supported | v0.4.0 | `qt_s`, `qt_p`, `qt_o` dictionary columns |
| Quoted triple in BGP | ✅ Supported | v0.4.0 | Ground patterns only |
| `TRIPLE()` constructor | ✅ Supported | v0.4.0 | |
| `SUBJECT()`, `PREDICATE()`, `OBJECT()` | ✅ Supported | v0.4.0 | |
| `isTRIPLE()` | ✅ Supported | v0.4.0 | |
| Annotation syntax (`{| |}`) | ✅ Supported | v0.4.0 | Turtle-star and SPARQL-star |

---

## Extensions Beyond W3C

pg_ripple extends the SPARQL standard with additional capabilities:

| Feature | Notes |
|---|---|
| `pg:similar()` custom function | Vector similarity within SPARQL FILTER |
| `pg:fts()` custom function | Full-text search within SPARQL FILTER |
| `pg:embed()` custom function | Inline embedding generation |
| Datalog-materialized predicates | Inferred triples queryable via standard SPARQL |
| SHACL-optimized query plans | Cardinality hints from SHACL shapes |
| Plan cache | Compiled SQL plans cached across queries |

---

## Known Limitations

| Feature | Status | Notes |
|---|---|---|
| `langMatches()` | ⚠️ Partial | Returns 0 rows; full BCP 47 matching planned |
| Custom aggregate extensions | ❌ Not supported | Standard aggregates fully supported |
| Variable-in-quoted-triple `<< ?s ?p ?o >>` | ⚠️ Partial | Returns 0 rows with WARNING; ground patterns work |
| `LOAD <url>` from arbitrary HTTP | ⚠️ Depends | Requires `pg_ripple_http` or server-side file |
| `DESCRIBE` strategy customization | ✅ Supported | Four strategies via GUC (v0.55.0) |
| Multiple result formats for `SELECT` | ⚠️ Partial | JSON primary; XML/CSV/TSV via `pg_ripple_http` only |

---

## DESCRIBE Strategy Reference (SC13-04, v0.86.0 — supersedes v0.55.0)

pg_ripple supports three DESCRIBE algorithms selectable via the **`pg_ripple.describe_form`**
GUC (default: `cbd`), introduced in v0.86.0. The older `pg_ripple.describe_strategy` GUC is
deprecated (see [deprecated-gucs.md](deprecated-gucs.md)) and will be removed in v1.0.0.

### `cbd` — Concise Bounded Description (default)

Returns all triples where the described resource appears as subject,
plus all triples reachable by following blank-node objects recursively.
This is the minimal W3C-defined DESCRIBE semantics.

```sql
SET pg_ripple.describe_form = 'cbd';
SELECT * FROM pg_ripple.sparql('DESCRIBE <https://example.org/Alice>');
```

### `scbd` — Symmetric Concise Bounded Description

Extends CBD by also including all triples where the described resource
appears as **object**. This captures both outgoing and incoming edges.
Suitable when you need the full neighbourhood of a resource.

```sql
SET pg_ripple.describe_form = 'scbd';
SELECT * FROM pg_ripple.sparql('DESCRIBE <https://example.org/Alice>');
```

### `symmetric` — Alias for `scbd`

`symmetric` is a normalised alias for `scbd` for readability:

```sql
SET pg_ripple.describe_form = 'symmetric';
SELECT * FROM pg_ripple.sparql('DESCRIBE <https://example.org/Alice>');
```

### Choosing a Form

| Form | Outgoing edges | Incoming edges | Blank-node closure | Speed |
|---|---|---|---|---|
| `cbd` | ✅ | ❌ | ✅ | Medium |
| `scbd` / `symmetric` | ✅ | ✅ | ✅ | Slower |

The GUC can be set at the session or transaction level:

```sql
-- Session-level
SET pg_ripple.describe_form = 'scbd';

-- Transaction-level
BEGIN;
SET LOCAL pg_ripple.describe_form = 'cbd';
SELECT * FROM pg_ripple.sparql('DESCRIBE <https://example.org/Bob>');
COMMIT;
```

> **Migration note**: Replace `SET pg_ripple.describe_strategy = 'simple'` with
> `SET pg_ripple.describe_form = 'cbd'` (the `simple` strategy is now the CBD default).
> The strategy `scbd` maps directly to `describe_form = 'scbd'`.

---

## Blank-Node Limitations in RDF-star Quoted Triples (C13-03, v0.85.0)

Blank nodes inside RDF-star quoted triples (e.g., `<< _:b1 :p :o >>`) do not
have a canonical round-trip form in pg_ripple. When an anonymous blank node
appears as the subject or object of a quoted triple, encoding and then decoding
the same triple may produce a different blank-node label.

**Workaround:** Use named IRIs or well-known blank-node identifiers (e.g., `_:b1`
with a stable label) as subjects/objects of quoted triples. Alternatively, avoid
blank nodes entirely in the subject/object positions of `<< >>` patterns.

**Impact:** This limitation only affects blank-node-in-quoted-triple patterns.
Regular blank nodes in non-quoted triples round-trip correctly.

---

## `GRAPH ?g` Default-Graph Exclusion (C13-06, v0.85.0)

Per SPARQL 1.1 specification §8.3, when `GRAPH ?g { ... }` is used, the
variable `?g` is bound only to **named variable `?g` is bound only to **named variable `?g` is bound only to **named variable `This is conformavariable `?g` is bound only to **named variable `?g` is bound only to wivariable `?g` is bound only to **named variable `?g` is bound only to **na ?pvariable `?g` is bound only to **named variable `?g` is bound only to **naatvariable `?g` is bound only to **named variable `?g` is bound only to **named ns triples.

To query the default graph specifically, use:
`````````````````````````````````````````````````````````````````````````````` WHERE`````````````````````````````````````````````````````````````````````````````` nd```````````````````````````````````````````````````````````````````````valu````````````````````````````````````````````````````````````````````````````cis``````````````````````````````````````````````````````````````truncate```````````````````````````````````````````````````seconds)
when serializing `xsd:dateTime` literals back to N-Triples format.

**Example:**
```
# Input:  "2024-01-01T12:00:00.123456789Z"^^xsd:dateTime
# Stored: 2024-01-01 12:00:00.123457+00  (rounded to microseconds by PG)
# Output: "2024-01-01T12:00:00.123"^^xsd:dateTime  (truncated to 3 decimal places)
```

Sub-millisecond precision is silently dropped in the output. If you require
sub-millisecond precision, store the value as a plain string literal and
perform comparisons manually.

---

## RDF 1.2 / SPARQL-star Compliance Matrix (STD-02, v0.91.0)

pg_ripple supports RDF-star via the `oxrdf` 0.3 data model and the `qt_s`/`qt_p`/`qt_o`
dictionary columns introduced in v0.4.0. The table below maps each RDF 1.2 / SPARQL-star
feature to its implementation status.

| Feature | Status | Since | Notes |
|---|---|---|---|
| `<< s p o >>` in subject position (BGP) | ✅ Implemented | v0.4.0 | Stored via `qt_s/qt_p/qt_o` dictionary columns |
| `<< s p o >>` in `BIND` | ✅ Implemented | v0.16.0 | Full expression support |
| `<< s p o >>` in `FILTER` | ✅ Implemented | v0.16.0 | Comparison and isTriple() |
| `<< s p o >>` in `CONSTRUCT` | ✅ Implemented | v0.16.0 | Emitted as Turtle-star |
| `<< s p o >>` in `SELECT` projections | ✅ Implemented | v0.16.0 | |
| Annotation syntax `{| p o |}` | ⚠ Partial | v0.16.0 | Parse-only; SPARQL write via `INSERT DATA` not yet supported |
| `TRIPLE(s, p, o)` constructor function | ❌ Not implemented | — | Depends on spargebra SPARQL 1.2 grammar update |
| `SUBJECT()` / `PREDICATE()` / `OBJECT()` destructuring | ❌ Not implemented | — | Dictionary join required; planned post-spargebra-1.2 |
| `REIF` keyword (RDF 1.2 reification syntax) | ❌ Not started | — | spargebra grammar update required |
| `isTriple()` function | ✅ Implemented | v0.16.0 | Returns true for quoted-triple subjects |

**Overall**: pg_ripple's RDF-star foundation (v0.4.0) covers the most widely-used SPARQL-star
patterns. Remaining gaps depend on `spargebra` 0.x adopting the SPARQL 1.2 grammar and are
tracked as post-v1.0.0 work (see [SPARQL 1.2 tracking](../../../plans/sparql12_tracking.md)).
