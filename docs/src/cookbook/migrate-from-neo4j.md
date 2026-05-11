# Migrate from Neo4j to pg_ripple

This recipe walks through migrating a Neo4j property graph to pg_ripple's RDF triple store. You will export Neo4j data, convert it to RDF, load it into pg_ripple, and validate the migration.

## Prerequisites

- Neo4j 5.x (Community or Enterprise)
- pg_ripple v0.90.0 or later
- Python 3.11+ with `neo4j` and `rdflib` packages

```bash
pip install neo4j rdflib
```

---

## Step 1: Export the Neo4j Graph

Use the Neo4j Cypher shell or APOC to export nodes and relationships.

### Option A: APOC Export to CSV

```cypher
-- Export nodes
CALL apoc.export.csv.query(
  "MATCH (n) RETURN id(n) as id, labels(n) as labels, properties(n) as props",
  "/tmp/nodes.csv", {quotes: "always"}
)

-- Export relationships
CALL apoc.export.csv.query(
  "MATCH (a)-[r]->(b) RETURN id(a) as from, type(r) as type, properties(r) as props, id(b) as to",
  "/tmp/rels.csv", {quotes: "always"}
)
```

### Option B: Python Direct Export

```python
from neo4j import GraphDatabase

driver = GraphDatabase.driver("bolt://localhost:7687", auth=("neo4j", "password"))

def export_nodes(session):
    return session.run("MATCH (n) RETURN id(n) as id, labels(n) as labels, properties(n) as props").data()

def export_rels(session):
    return session.run(
        "MATCH (a)-[r]->(b) RETURN id(a) as from, type(r) as rel, properties(r) as props, id(b) as to"
    ).data()

with driver.session() as s:
    nodes = export_nodes(s)
    rels = export_rels(s)
```

---

## Step 2: Convert to RDF (Turtle)

Map Neo4j property graph concepts to RDF:

| Neo4j | RDF |
|---|---|
| Node with label `Person` | `?node rdf:type ex:Person` |
| Node property `name = "Alice"` | `?node ex:name "Alice"` |
| Relationship `KNOWS` | `?from ex:knows ?to` |
| Node ID | `ex:node_{id}` IRI |
| Relationship property | RDF-star quoted triple `<<ex:from ex:knows ex:to>> ex:since "2021"` |

```python
from rdflib import Graph, URIRef, Literal, Namespace, RDF
import json

EX = Namespace("https://example.org/")
g = Graph()

# Bind prefixes
g.bind("ex", EX)
g.bind("rdf", RDF)

# Convert nodes
for node in nodes:
    node_uri = EX[f"node_{node['id']}"]
    # Type assertions
    for label in node["labels"]:
        g.add((node_uri, RDF.type, EX[label]))
    # Properties
    props = json.loads(node["props"]) if isinstance(node["props"], str) else node["props"]
    for key, val in props.items():
        g.add((node_uri, EX[key], Literal(val)))

# Convert relationships
for rel in rels:
    from_uri = EX[f"node_{rel['from']}"]
    to_uri = EX[f"node_{rel['to']}"]
    pred_uri = EX[rel["rel"].lower()]
    g.add((from_uri, pred_uri, to_uri))

# Export to Turtle
turtle_data = g.serialize(format="turtle")
with open("/tmp/neo4j_export.ttl", "w") as f:
    f.write(turtle_data)
print(f"Exported {len(g)} triples to neo4j_export.ttl")
```

---

## Step 3: Load into pg_ripple

```sql
-- Create a named graph for the migrated data
SELECT pg_ripple.insert_triple(
  'https://example.org/neo4j_import',
  'rdf:type',
  'pg_ripple:Graph'
);

-- Load the Turtle file
SELECT pg_ripple.load_turtle(
  pg_read_file('/tmp/neo4j_export.ttl'),
  'https://example.org/neo4j_import'
);

-- Verify the load
SELECT COUNT(*) FROM pg_ripple.sparql(
  'SELECT (COUNT(*) AS ?count) WHERE { GRAPH <https://example.org/neo4j_import> { ?s ?p ?o } }'
);
```

For large exports (millions of triples), use the bulk loader instead:

```sql
-- Stream via COPY for maximum throughput
SELECT pg_ripple.bulk_load_turtle_file(
  '/tmp/neo4j_export.ttl',
  'https://example.org/neo4j_import'
);
```

---

## Step 4: Define SPARQL Views for Cypher-Style Queries

Map common Neo4j query patterns to SPARQL:

### Neo4j: Shortest Path

```cypher
MATCH p = shortestPath((a:Person {name: "Alice"})-[:KNOWS*]-(b:Person {name: "Bob"}))
RETURN p
```

```sparql
-- pg_ripple equivalent (property path)
SELECT ?path WHERE {
  <https://example.org/Alice> ex:knows+ <https://example.org/Bob> .
}
```

### Neo4j: Pattern Matching

```cypher
MATCH (p:Person)-[:WORKS_AT]->(c:Company)<-[:WORKS_AT]-(q:Person)
WHERE p.name = "Alice"
RETURN q.name
```

```sparql
SELECT ?coworkerName WHERE {
  <https://example.org/Alice> ex:worksAt ?company .
  ?company rdf:type ex:Company .
  ?coworker ex:worksAt ?company .
  ?coworker ex:name ?coworkerName .
  FILTER(?coworker != <https://example.org/Alice>)
}
```

---

## Step 5: Add SHACL Constraints

Recreate Neo4j uniqueness and existence constraints as SHACL shapes:

```turtle
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix ex: <https://example.org/> .

ex:PersonShape a sh:NodeShape ;
  sh:targetClass ex:Person ;
  sh:property [
    sh:path ex:email ;
    sh:maxCount 1 ;           -- Neo4j uniqueness constraint equivalent
    sh:datatype xsd:string
  ] ;
  sh:property [
    sh:path ex:name ;
    sh:minCount 1 ;           -- NOT NULL equivalent
    sh:datatype xsd:string
  ] .
```

```sql
SELECT pg_ripple.load_shacl($$
  @prefix sh: <http://www.w3.org/ns/shacl#> .
  @prefix ex: <https://example.org/> .
  -- paste shapes here
$$);
```

---

## Step 6: Validate and Reconcile

```sql
-- Check constraint violations
SELECT pg_ripple.validate();

-- Find nodes without required properties (replaces Neo4j NULL checks)
SELECT binding->>'?node' AS node
FROM pg_ripple.sparql($$
  SELECT ?node WHERE {
    ?node rdf:type ex:Person .
    FILTER NOT EXISTS { ?node ex:name ?n }
  }
$$);

-- Verify counts match Neo4j
SELECT binding->>'?count' AS triple_count
FROM pg_ripple.sparql('SELECT (COUNT(*) AS ?count) WHERE { ?s ?p ?o }');
```

---

## Mapping Reference

| Neo4j Concept | RDF / pg_ripple |
|---|---|
| Node label | `rdf:type` assertion |
| Node property | predicate-object pair |
| Relationship type | predicate IRI |
| Relationship property | RDF-star quoted triple or reification |
| Node ID | IRI (UUID recommended) |
| Unique constraint | `sh:maxCount 1` shape |
| Existence constraint | `sh:minCount 1` shape |
| Index on property | VP table automatic indexing |
| Cypher `MATCH` | SPARQL `SELECT … WHERE { }` |
| Cypher `CREATE` | `pg_ripple.insert_triple()` |
| Cypher `MERGE` | `INSERT … ON CONFLICT DO NOTHING` via bulk load |
| Cypher `shortestPath` | `ex:pred+` property path |
| Cypher `CALL db.schema()` | `pg_ripple.list_predicates()` |

---

## Performance Tips

1. **Batch load** with `bulk_load_turtle_file()` — 10–100× faster than individual inserts.
2. **Set `pg_ripple.vp_promotion_threshold = 100`** during migration to create individual VP tables for more predicates, then restore the default after.
3. **Run `VACUUM ANALYZE`** on `_pg_ripple.dictionary` after a large load.
4. **Use `pg_ripple.run_merge()`** to flush deltas to the main partition before heavy analytic queries.
