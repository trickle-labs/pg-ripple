# pg_ripple Blog

> **Note:** Blog posts were written with AI assistance (GitHub Copilot / Claude)
> as a way to explore LLM-generated technical writing for a niche systems
> engineering topic. The technical content has been reviewed for accuracy, but
> treat posts as drafts rather than officially reviewed documentation.

The full blog lives in the repository under [`blog/`](https://github.com/trickle-labs/pg-ripple/tree/main/blog).
Individual posts are linked below.

---

## Core Concepts & Architecture

| Post | Summary |
|------|---------|
| [Why RDF Inside PostgreSQL?](https://github.com/trickle-labs/pg-ripple/blob/main/blog/why-rdf-in-postgresql.md) | The case for a triple store that lives where your data already is — no ETL pipeline, no separate cluster, no impedance mismatch. |
| [Vertical Partitioning: One Table Per Predicate](https://github.com/trickle-labs/pg-ripple/blob/main/blog/vertical-partitioning-explained.md) | Inside pg_ripple's VP storage model and why it beats a single `(s, p, o)` table by 10–100× for selective queries. |
| [Everything Is an Integer](https://github.com/trickle-labs/pg-ripple/blob/main/blog/dictionary-encoding-integer-joins.md) | Dictionary encoding with XXH3-128: why string comparisons in a triple store are a performance bug. |
| [How SPARQL Becomes a PostgreSQL Query Plan](https://github.com/trickle-labs/pg-ripple/blob/main/blog/sparql-to-sql-translation.md) | The translation pipeline from SPARQL text to `spargebra` algebra to SQL to SPI execution. |

## Storage & Performance

| Post | Summary |
|------|---------|
| [HTAP for Triples: Reads and Writes at the Same Time](https://github.com/trickle-labs/pg-ripple/blob/main/blog/htap-reads-and-writes.md) | The delta/main/tombstone split that lets pg_ripple handle concurrent OLTP writes and analytical SPARQL queries without locking. |
| [Leapfrog Triejoin: When Triangle Queries Meet Optimal Joins](https://github.com/trickle-labs/pg-ripple/blob/main/blog/leapfrog-triejoin.md) | Worst-case optimal joins compiled into PostgreSQL — what a 10–100× speedup looks like in practice. |
| [Property Paths Are Just Recursive CTEs](https://github.com/trickle-labs/pg-ripple/blob/main/blog/property-paths-recursive-ctes.md) | SPARQL property paths compiled to `WITH RECURSIVE … CYCLE` using PostgreSQL 18's hash-based cycle detection. |

## Reasoning & Inference

| Post | Summary |
|------|---------|
| [Datalog Inside PostgreSQL](https://github.com/trickle-labs/pg-ripple/blob/main/blog/datalog-inside-postgresql.md) | Automatic fact derivation from rules — RDFS, OWL RL, transitive closure, all running as SQL. |
| [Magic Sets: Ask a Question, Infer Only What You Need](https://github.com/trickle-labs/pg-ripple/blob/main/blog/magic-sets-goal-directed.md) | Goal-directed inference: the difference between 2 million inferred triples and 47. |
| [owl:sameAs Without the Explosion](https://github.com/trickle-labs/pg-ripple/blob/main/blog/owl-sameas-entity-resolution.md) | Entity canonicalization at query time using union-find over `owl:sameAs` chains. |
| [The Four Built-in Rule Sets](https://github.com/trickle-labs/pg-ripple/blob/main/blog/builtin-reasoning-rules-explained.md) | What RDFS, OWL RL, OWL EL, and OWL QL actually do — every rule explained with examples. |
| [Well-Founded Semantics](https://github.com/trickle-labs/pg-ripple/blob/main/blog/well-founded-semantics.md) | Why stratified negation isn't always enough, and how well-founded semantics handles recursive negation. |
| [OWL Property Chain Axioms](https://github.com/trickle-labs/pg-ripple/blob/main/blog/owl-property-chain-axiom.md) | Compiling `owl:propertyChainAxiom` to recursive SQL joins. |

## Data Quality & Validation

| Post | Summary |
|------|---------|
| [SHACL: Schema Validation for the Schema-Less](https://github.com/trickle-labs/pg-ripple/blob/main/blog/shacl-data-quality.md) | How pg_ripple compiles SHACL shapes into DDL constraints and async validation pipelines. |

## AI, RAG & Vector Search

| Post | Summary |
|------|---------|
| [Vector + SPARQL Hybrid Search](https://github.com/trickle-labs/pg-ripple/blob/main/blog/vector-sparql-hybrid-search.md) | Combining pgvector approximate nearest-neighbour search with SPARQL graph traversal. |
| [Natural Language to SPARQL](https://github.com/trickle-labs/pg-ripple/blob/main/blog/natural-language-to-sparql.md) | LLM-powered NL→SPARQL translation with few-shot prompting. |
| [GraphRAG Knowledge Export](https://github.com/trickle-labs/pg-ripple/blob/main/blog/graphrag-knowledge-export.md) | Exporting pg_ripple graphs for Microsoft's GraphRAG pipeline. |
| [Neuro-Symbolic Entity Resolution](https://github.com/trickle-labs/pg-ripple/blob/main/blog/neuro-symbolic-entity-resolution.md) | Combining ML embeddings with Datalog rules for record linkage. |

## Integrations & Operations

| Post | Summary |
|------|---------|
| [CDC → Knowledge Graph](https://github.com/trickle-labs/pg-ripple/blob/main/blog/cdc-knowledge-graphs.md) | Streaming relational change events into the RDF graph via logical replication. |
| [Citus Shard Pruning for SPARQL](https://github.com/trickle-labs/pg-ripple/blob/main/blog/citus-shard-pruning-sparql.md) | How the `SERVICE` clause dispatches federated SPARQL queries to the right shard. |
| [IVM with pg-trickle Integration](https://github.com/trickle-labs/pg-ripple/blob/main/blog/ivm-pg-trickle-integration.md) | Incremental view maintenance over CDC streams. |
| [Semantic Hub: pg-tide Relay](https://github.com/trickle-labs/pg-ripple/blob/main/blog/semantic-hub-trickle-relay.md) | Hub-and-spoke topology for multi-instance RDF synchronisation. |
| [GDPR Right to Erasure](https://github.com/trickle-labs/pg-ripple/blob/main/blog/gdpr-right-to-erasure.md) | Implementing GDPR article 17 — cascading triple deletion with provenance tracking. |
| [Multi-Tenant Knowledge Graphs](https://github.com/trickle-labs/pg-ripple/blob/main/blog/multi-tenant-knowledge-graphs.md) | Named-graph isolation, row-level security, and per-tenant dictionaries. |

## Advanced Features

| Post | Summary |
|------|---------|
| [CONSTRUCT Views: Live Transformations](https://github.com/trickle-labs/pg-ripple/blob/main/blog/construct-views-live-transformations.md) | SPARQL CONSTRUCT rules as materialised views with automatic delta propagation. |
| [JSON-LD Framing and Nested JSON](https://github.com/trickle-labs/pg-ripple/blob/main/blog/json-ld-framing-nested-json.md) | Turning a flat triple store into structured nested JSON documents on demand. |
| [JSON-LD Reverse Mapping](https://github.com/trickle-labs/pg-ripple/blob/main/blog/json-ld-reverse-mapping.md) | Writing RDF changes back to relational tables via JSON-LD contexts. |
| [R2RML: Relational to Graph](https://github.com/trickle-labs/pg-ripple/blob/main/blog/r2rml-relational-to-graph.md) | Mapping relational tables to RDF using the W3C R2RML standard. |
| [RDF-star: Statements About Statements](https://github.com/trickle-labs/pg-ripple/blob/main/blog/rdf-star-statements-about-statements.md) | Annotating individual triples for provenance, confidence, and temporal metadata. |
| [Temporal Graph Snapshots](https://github.com/trickle-labs/pg-ripple/blob/main/blog/temporal-graph-snapshots.md) | Point-in-time named graphs for audit trails and time-travel queries. |
| [Temporal Time-Travel Queries](https://github.com/trickle-labs/pg-ripple/blob/main/blog/temporal-time-travel-queries.md) | Querying historical states of the knowledge graph. |
| [Allen's Interval Relations](https://github.com/trickle-labs/pg-ripple/blob/main/blog/allen-interval-relations.md) | Temporal reasoning using Allen's thirteen interval relations in Datalog. |
| [GeoSPARQL + PostGIS](https://github.com/trickle-labs/pg-ripple/blob/main/blog/geosparql-postgis-spatial.md) | Spatial queries over RDF geometry literals using the GeoSPARQL 1.1 extension. |
| [SKOS Knowledge Organization](https://github.com/trickle-labs/pg-ripple/blob/main/blog/skos-knowledge-organization.md) | Managing thesauri, concept schemes, and taxonomies with SKOS in pg_ripple. |
| [Graph Analytics with PageRank](https://github.com/trickle-labs/pg-ripple/blob/main/blog/pagerank.md) | Datalog-native PageRank with incremental view maintenance, WCOJ, and Prometheus gauges. |
| [Probabilistic Datalog](https://github.com/trickle-labs/pg-ripple/blob/main/blog/probabilistic-datalog.md) | Uncertain knowledge: noisy-OR, confidence propagation, and soft constraints. |
| [Provenance Tracking with PROV-O](https://github.com/trickle-labs/pg-ripple/blob/main/blog/provenance-tracking-prov-o.md) | Recording data lineage using the W3C PROV-O ontology. |
| [Ontology Mapping and Alignment](https://github.com/trickle-labs/pg-ripple/blob/main/blog/ontology-mapping-alignment.md) | Bridging heterogeneous vocabularies with `owl:equivalentClass` and SPARQL CONSTRUCT rules. |
| [SPARQL Federation: Local + Remote](https://github.com/trickle-labs/pg-ripple/blob/main/blog/sparql-federation-local-remote.md) | Mixing local VP table scans with remote SPARQL endpoints in a single query. |
| [EXPLAIN SPARQL: Query Plans](https://github.com/trickle-labs/pg-ripple/blob/main/blog/explain-sparql-query-plans.md) | Reading pg_ripple's query plan output to diagnose slow SPARQL queries. |
| [Rule Library Federation](https://github.com/trickle-labs/pg-ripple/blob/main/blog/rule-library-federation.md) | Publishing and subscribing to shared Datalog rule libraries across pg_ripple instances. |
| [Federation Circuit Breakers](https://github.com/trickle-labs/pg-ripple/blob/main/blog/federation-circuit-breaker.md) | Resilient federated queries with circuit breakers, timeouts, and fallback graphs. |
| [DCTERMS, Schema.org, and FOAF Bundles](https://github.com/trickle-labs/pg-ripple/blob/main/blog/dcterms-schema-foaf-bundles.md) | Built-in vocabulary bundles for common ontologies. |
| [Uncertain Knowledge](https://github.com/trickle-labs/pg-ripple/blob/main/blog/uncertain-knowledge.md) | Probabilistic Datalog in PostgreSQL: representing and querying uncertain facts. |
