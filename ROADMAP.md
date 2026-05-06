# pg_ripple — Roadmap

> **Audience:** Product managers, stakeholders, and technically curious readers
> who want to understand what each release delivers and why it matters —
> without needing to read Rust code or SQL specifications.

> **Authority rule**: [plans/implementation_plan.md](plans/implementation_plan.md) is the authoritative description of the **eventual target architecture**. This roadmap is the delivery sequence for that architecture.

## Versions

### Foundation (v0.1.0 – v0.5.1)

| Version | Theme | Status | Scope | Full details |
|---------|-------|--------|-------|--------------|
| [v0.1.0](roadmap/v0.1.0.md) | Install the extension, store and retrieve facts (VP storage from day one) | ✅ Released | Medium | [Full details](roadmap/v0.1.0-full.md) |
| [v0.2.0](roadmap/v0.2.0.md) | Bulk data import, named graphs, rare-predicate consolidation, N-Triples export | ✅ Released | Medium | [Full details](roadmap/v0.2.0-full.md) |
| [v0.3.0](roadmap/v0.3.0.md) | Ask questions in the standard RDF query language (incl. GRAPH patterns) | ✅ Released | Medium | [Full details](roadmap/v0.3.0-full.md) |
| [v0.4.0](roadmap/v0.4.0.md) | Make statements about statements; LPG-ready storage | ✅ Released | Large | [Full details](roadmap/v0.4.0-full.md) |
| [v0.5.0](roadmap/v0.5.0.md) | Property paths, aggregates, UNION/MINUS, subqueries, BIND/VALUES | ✅ Released | Medium | [Full details](roadmap/v0.5.0-full.md) |
| [v0.5.1](roadmap/v0.5.1.md) | Inline encoding, CONSTRUCT/DESCRIBE, INSERT/DELETE DATA, FTS | ✅ Released | Medium | [Full details](roadmap/v0.5.1-full.md) |

### Storage Architecture & Validation (v0.6.0 – v0.10.0)

| Version | Theme | Status | Scope | Full details |
|---------|-------|--------|-------|--------------|
| [v0.6.0](roadmap/v0.6.0.md) | Heavy reads and writes at the same time; shared-memory cache | ✅ Released | Large | [Full details](roadmap/v0.6.0-full.md) |
| [v0.7.0](roadmap/v0.7.0.md) | Define data quality rules; reject bad data on insert; on-demand and merge-time triple deduplication | ✅ Released | Medium | [Full details](roadmap/v0.7.0-full.md) |
| [v0.8.0](roadmap/v0.8.0.md) | Complex data quality rules with background checking | ✅ Released | Small | [Full details](roadmap/v0.8.0-full.md) |
| [v0.9.0](roadmap/v0.9.0.md) | Import and export data in all standard RDF file formats | ✅ Released | Small | [Full details](roadmap/v0.9.0-full.md) |
| [v0.10.0](roadmap/v0.10.0.md) | Automatically derive new facts from rules and logic | ✅ Released | Very Large | [Full details](roadmap/v0.10.0-full.md) |

### Query, Protocol & Interoperability (v0.11.0 – v0.20.0)

| Version | Theme | Status | Scope | Full details |
|---------|-------|--------|-------|--------------|
| [v0.11.0](roadmap/v0.11.0.md) | Live, always-up-to-date dashboards from SPARQL and Datalog queries | ✅ Released | Medium | [Full details](roadmap/v0.11.0-full.md) |
| [v0.12.0](roadmap/v0.12.0.md) | Pattern-based updates and graph management commands | ✅ Released | Small | [Full details](roadmap/v0.12.0-full.md) |
| [v0.13.0](roadmap/v0.13.0.md) | Speed tuning, benchmarks, production-grade throughput | ✅ Released | Medium | [Full details](roadmap/v0.13.0-full.md) |
| [v0.14.0](roadmap/v0.14.0.md) | Operations tooling, access control, docs, packaging | ✅ Released | Small | [Full details](roadmap/v0.14.0-full.md) |
| [v0.15.0](roadmap/v0.15.0.md) | Standard HTTP API, graph-aware loaders and deletes as SQL functions | ✅ Released | Small | [Full details](roadmap/v0.15.0-full.md) |
| [v0.16.0](roadmap/v0.16.0.md) | Query remote SPARQL endpoints alongside local data | ✅ Released | Small | [Full details](roadmap/v0.16.0-full.md) |
| [v0.17.0](roadmap/v0.17.0.md) | Frame-driven CONSTRUCT queries producing nested JSON-LD | ✅ Released | Small | [Full details](roadmap/v0.17.0-full.md) |
| [v0.18.0](roadmap/v0.18.0.md) | Materialize CONSTRUCT and ASK queries as live, incrementally-updated stream tables | ✅ Released | Small | [Full details](roadmap/v0.18.0-full.md) |
| [v0.19.0](roadmap/v0.19.0.md) | Connection pooling, result caching, query rewriting, and batching for remote SPARQL endpoints | ✅ Released | Small | [Full details](roadmap/v0.19.0-full.md) |
| [v0.20.0](roadmap/v0.20.0.md) | W3C SPARQL 1.1 and SHACL Core test suite compliance, crash recovery and memory safety hardening | ✅ Released | Medium | [Full details](roadmap/v0.20.0-full.md) |

### Correctness & Datalog Optimization (v0.21.0 – v0.32.0)

| Version | Theme | Status | Scope | Full details |
|---------|-------|--------|-------|--------------|
| [v0.21.0](roadmap/v0.21.0.md) | Implement all ~40 missing SPARQL 1.1 built-in functions, fix the FILTER silent-drop hazard, and close critical query-semantics bugs | ✅ Released | Medium | [Full details](roadmap/v0.21.0-full.md) |
| [v0.22.0](roadmap/v0.22.0.md) | Fix HTAP merge race conditions, dictionary cache rollback, shmem cache thrashing, rare-predicate promotion race, and HTTP service security gaps | ✅ Released | Medium | [Full details](roadmap/v0.22.0-full.md) |
| [v0.23.0](roadmap/v0.23.0.md) | Complete the SHACL constraint set, add SPARQL query introspection, and fix Datalog/JSON-LD correctness issues | ✅ Released | Medium | [Full details](roadmap/v0.23.0-full.md) |
| [v0.24.0](roadmap/v0.24.0.md) | Semi-naive Datalog evaluation, complete OWL RL rule set, batch-decode large result sets, bound property-path depth | ✅ Released | Medium | [Full details](roadmap/v0.24.0-full.md) |
| [v0.25.0](roadmap/v0.25.0.md) | GeoSPARQL 1.1 geometry primitives, stabilise internal catalog against OID drift, close remaining medium- and low-priority issues | ✅ Released | Medium | [Full details](roadmap/v0.25.0-full.md) |
| [v0.26.0](roadmap/v0.26.0.md) | Microsoft GraphRAG integration: BYOG Parquet export, Datalog-enriched entity graphs, SHACL quality enforcement, Python CLI bridge | ✅ Released | Small | [Full details](roadmap/v0.26.0-full.md) |
| [v0.27.0](roadmap/v0.27.0.md) | Core pgvector integration — embedding table, HNSW index, `pg:similar()` SPARQL function, bulk embedding, hybrid retrieval modes | ✅ Released | Medium | [Full details](roadmap/v0.27.0-full.md) |
| [v0.28.0](roadmap/v0.28.0.md) | Production-grade RRF fusion, incremental embedding worker, graph-contextualized embeddings, end-to-end RAG retrieval | ✅ Released | Medium | [Full details](roadmap/v0.28.0-full.md) |
| [v0.29.0](roadmap/v0.29.0.md) | Goal-directed inference via magic sets, cost-based body atom reordering, subsumption checking, anti-join negation, filter pushdown | ✅ Released | Medium | [Full details](roadmap/v0.29.0-full.md) |
| [v0.30.0](roadmap/v0.30.0.md) | Aggregation in rule bodies (Datalog^agg), SQL plan caching across inference runs, SPARQL on-demand query speedup | ✅ Released | Medium | [Full details](roadmap/v0.30.0-full.md) |
| [v0.31.0](roadmap/v0.31.0.md) | `owl:sameAs` entity canonicalization, demand transformation for goal-directed rule rewriting, SPARQL query planner integration | ✅ Released | Medium | [Full details](roadmap/v0.31.0-full.md) |
| [v0.32.0](roadmap/v0.32.0.md) | Three-valued semantics for cyclic ontologies, subsumptive result caching for Datalog and SPARQL repeated sub-queries | ✅ Released | Medium | [Full details](roadmap/v0.32.0-full.md) |

### Performance, Conformance & Ecosystem (v0.33.0 – v0.46.0)

| Version | Theme | Status | Scope | Full details |
|---------|-------|--------|-------|--------------|
| [v0.33.0](roadmap/v0.33.0.md) | Complete docs site rebuild — CI harness, eight feature-deep-dive chapters, operations guide, reference section, and content governance | ✅ Released | Large | [Full details](roadmap/v0.33.0-full.md) |
| [v0.34.0](roadmap/v0.34.0.md) | Early fixpoint termination for bounded hierarchies (20–50% faster SPARQL property paths); Delete-Rederive for write-correct materialized predicates | ✅ Released | Medium | [Full details](roadmap/v0.34.0-full.md) |
| [v0.35.0](roadmap/v0.35.0.md) | Background-worker parallelism for independent Datalog rules (2–5× faster materialization); add/remove rules without full recompute | ✅ Released | Medium | [Full details](roadmap/v0.35.0-full.md) |
| [v0.36.0](roadmap/v0.36.0.md) | Leapfrog Triejoin for cyclic SPARQL patterns (10×–100× speedup); Datalog^L monotone lattice aggregation | ✅ Released | Medium | [Full details](roadmap/v0.36.0-full.md) |
| [v0.37.0](roadmap/v0.37.0.md) | Fix HTAP merge race, rare-predicate promotion race, dictionary cache rollback; eliminate all hard panics; add GUC validators | ✅ Released | Large | [Full details](roadmap/v0.37.0-full.md) |
| [v0.38.0](roadmap/v0.38.0.md) | Split god-module, PredicateCatalog trait, batch encoding, SCBD, SPARQL Update completeness, SHACL hints in planner | ✅ Released | Large | [Full details](roadmap/v0.38.0-full.md) |
| [v0.39.0](roadmap/v0.39.0.md) | REST API exposing all 27 Datalog SQL functions in `pg_ripple_http`: rule management, inference, goal queries, constraints, admin | ✅ Released | Small | [Full details](roadmap/v0.39.0-full.md) |
| [v0.40.0](roadmap/v0.40.0.md) | Server-side SPARQL cursors, `explain_sparql()`, `explain_datalog()`, OpenTelemetry tracing, resource governors | ✅ Released | Large | [Full details](roadmap/v0.40.0-full.md) |
| [v0.41.0](roadmap/v0.41.0.md) | Complete W3C SPARQL 1.1 test suite harness with parallelized execution; 3,000+ tests in < 2 min CI | ✅ Released | Medium | [Full details](roadmap/v0.41.0-full.md) |
| [v0.42.0](roadmap/v0.42.0.md) | Multi-worker HTAP merge, FedX-style federation planner, parallel SERVICE, live RDF change subscriptions | ✅ Released | Very Large | [Full details](roadmap/v0.42.0-full.md) |
| [v0.43.0](roadmap/v0.43.0.md) | Apache Jena edge-case tests (~1,000) and WatDiv scale-correctness benchmark (10M+ triples, star/chain/snowflake/complex patterns) | ✅ Released | Medium | [Full details](roadmap/v0.43.0-full.md) |
| [v0.44.0](roadmap/v0.44.0.md) | LUBM OWL RL inference correctness across 14 canonical queries; Datalog API validation sub-suite | ✅ Released | Small | [Full details](roadmap/v0.44.0-full.md) |
| [v0.45.0](roadmap/v0.45.0.md) | Close remaining SHACL Core gaps, harden parallel Datalog strata rollback, add crash-recovery scenarios, standardise migration documentation | ✅ Released | Small | [Full details](roadmap/v0.45.0-full.md) |
| [v0.46.0](roadmap/v0.46.0.md) | `proptest` for SPARQL/dictionary invariants, fuzz federation result decoder, W3C OWL 2 RL test suite in CI, TopN push-down, BSBM regression gate | ✅ Released | Medium | [Full details](roadmap/v0.46.0-full.md) |

### Architecture, Observability & Production (v0.47.0 – v0.54.0)

| Version | Theme | Status | Scope | Full details |
|---------|-------|--------|-------|--------------|
| [v0.47.0](roadmap/v0.47.0.md) | Fix parsed-but-not-checked SHACL constraints, wire `preallocate_sid_ranges()`, finish `sparql/translate/` module split, add fuzz targets, GUC validators, security hygiene | ✅ Released | Large | [Full details](roadmap/v0.47.0-full.md) |
| [v0.48.0](roadmap/v0.48.0.md) | Complete all 35 SHACL Core constraints, close OWL 2 RL rule set, add SPARQL Update MOVE/COPY/ADD, fix SPARQL-star variable patterns, WatDiv baselines | ✅ Released | Medium | [Full details](roadmap/v0.48.0-full.md) |
| [v0.49.0](roadmap/v0.49.0.md) | `sparql_from_nl()` NL-to-SPARQL via configurable LLM endpoint; embedding-based entity alignment with `suggest_sameas()` | ✅ Released | Small | [Full details](roadmap/v0.49.0-full.md) |
| [v0.50.0](roadmap/v0.50.0.md) | `explain_sparql(analyze:=true)` interactive query debugger; `rag_context()` RAG pipeline | ✅ Released | Small | [Full details](roadmap/v0.50.0-full.md) |
| [v0.51.0](roadmap/v0.51.0.md) | Non-root container, SPARQL DoS protection, HTTP streaming, OTLP, pg_upgrade compat, CDC docs, conformance gate flips | ✅ Released | Large | [Full details](roadmap/v0.51.0-full.md) |
| [v0.52.0](roadmap/v0.52.0.md) | JSON→RDF helpers, CDC→outbox bridge worker, CDC bridge triggers, JSON-LD event serializer, dedup keys, vocabulary templates, pg-trickle runtime detection | ✅ Released | Medium | [Full details](roadmap/v0.52.0-full.md) |
| [v0.53.0](roadmap/v0.53.0.md) | SHACL-SPARQL, `COPY rdf FROM`, RAG hardening, CDC lifecycle events, architecture module splits, OpenAPI spec | ✅ Released | Medium | [Full details](roadmap/v0.53.0-full.md) |
| [v0.54.0](roadmap/v0.54.0.md) | PG18 logical-decoding RDF replication, Helm chart, CloudNativePG image volume, merge/vector-index performance baselines | ✅ Released | Medium | [Full details](roadmap/v0.54.0-full.md) |

### Quality, Security & Ecosystem (v0.55.0 – v0.59.0)

| Version | Theme | Status | Scope | Full details |
|---------|-------|--------|-------|--------------|
| [v0.55.0](roadmap/v0.55.0.md) | Security hardening (SSRF allowlist, HTAP race fix), error-catalog reconciliation, tombstone GC, named-graph RLS, read-replica routing, VoID, SPARQL Service Description, OpenAPI spec | ✅ Released | Large | [Full details](roadmap/v0.55.0-full.md) |
| [v0.56.0](roadmap/v0.56.0.md) | GeoSPARQL 1.1, SPARQL Entailment Regime tests, Arrow/Flight export, federation circuit breaker, SPARQL audit log, dead-code audit, deprecated GUC removal | ✅ Released | Medium | [Full details](roadmap/v0.56.0-full.md) |
| [v0.57.0](roadmap/v0.57.0.md) | OWL 2 EL/QL reasoning profiles, KG embeddings (TransE/RotatE), entity alignment, LLM SPARQL repair, ontology mapping, multi-tenant graph isolation, columnar VP, adaptive indexing | ✅ Released | Very Large | [Full details](roadmap/v0.57.0-full.md) |
| [v0.58.0](roadmap/v0.58.0.md) | Temporal RDF queries (`point_in_time`), SPARQL-DL, Citus horizontal sharding, PROV-O graph provenance, v1.0.0 readiness integration suite | ✅ Released | Large | [Full details](roadmap/v0.58.0-full.md) |
| [v0.59.0](roadmap/v0.59.0.md) | Citus SPARQL shard-pruning for bound subjects (10–100× speedup), rebalance NOTIFY coordination, `explain_sparql()` Citus section | ✅ Released | Medium | [Full details](roadmap/v0.59.0-full.md) |

### Pre-1.0 Hardening & Ecosystem (v0.60.0 – v0.63.0)

| Version | Theme | Status | Scope | Full details |
|---------|-------|--------|-------|--------------|
| [v0.60.0](roadmap/v0.60.0.md) | Close all v1.0.0 blockers: HTAP cutover atomic swap, Actions SHA pinning, SECURITY DEFINER CI lint, new fuzz targets (GeoSPARQL WKT, R2RML, LLM prompt), `/ready` endpoint, `geof:distance`, merge-throughput trend artifact, pg_dump round-trip CI test, LangChain tool package | Released ✅ | Large | [Full details](roadmap/v0.60.0-full.md) |
| [v0.61.0](roadmap/v0.61.0.md) | Ecosystem depth: per-named-graph RLS, `explain_inference()` derivation tree, GDPR `erase_subject()`, dbt adapter, SHACL-AF rule execution, OTLP traceparent propagation, richer federation call stats; Citus object-based shard pruning and direct-shard bulk-load path | Released ✅ | Large | [Full details](roadmap/v0.61.0-full.md) |
| [v0.62.0](roadmap/v0.62.0.md) | Query frontier: Apache Arrow Flight bulk export, WCOJ planner integration, visual graph explorer in `pg_ripple_http`, `clippy --deny warnings` CI gate; Citus property-path push-down, `vp_rare` cold-entry archival, tiered dictionary cache, distributed inference dispatch, live shard rebalance, multi-hop pruning carry-forward | Released ✅ | Very Large | [Full details](roadmap/v0.62.0-full.md) |
| [v0.63.0](roadmap/v0.63.0.md) | SPARQL CONSTRUCT writeback rules (raw-to-canonical pipelines, incremental delta maintenance, Delete-Rederive, pipeline stratification); Citus scalability: SERVICE result shard pruning, streaming fan-out cursor, HyperLogLog `COUNT(DISTINCT)`, batched dictionary encoding, per-worker SID tables, non-blocking VP promotion, per-graph RLS CI gate, per-worker BRIN summarise | Released ✅ | Large | [Full details](roadmap/v0.63.0-full.md) |

### Assessment Remediation & Release Trust (v0.64.0 – v0.69.0)

| Version | Theme | Status | Scope | Full details |
|---------|-------|--------|-------|--------------|
| [v0.64.0](roadmap/v0.64.0.md) | Release truth and safety freeze: feature-status API, deep readiness, immutable GitHub Actions, digest-scanned Docker releases, documentation truth pass, release evidence dashboard foundation | Released ✅ | Large | [Full details](roadmap/v0.64.0-full.md) |
| [v0.65.0](roadmap/v0.65.0.md) | CONSTRUCT writeback correctness closure: real delta maintenance, HTAP-aware retraction, exact provenance capture, parameterized rule catalog writes, full CWB behavior test matrix | ✅ Released | Very Large | [Full details](roadmap/v0.65.0-full.md) |
| [v0.66.0](roadmap/v0.66.0.md) | Streaming and distributed reality: true SPARQL cursors, signed Arrow IPC export, explainable WCOJ mode, integrated Citus pruning/HLL/BRIN/RLS/promotion paths | ✅ Released | Very Large | [Full details](roadmap/v0.66.0-full.md) |
| [v0.67.0](roadmap/v0.67.0.md) | Assessment 9 critical remediation and production evidence: storage mutation journal, VP table RLS coverage, Arrow Flight security/correctness, fail-closed release-truth gates, soak tests, benchmark baselines, security audit | Released ✅ | Very Large | [Full details](roadmap/v0.67.0-full.md) |
| [v0.68.0](roadmap/v0.68.0.md) | Distributed scalability, streaming completion and fuzz hardening: CONSTRUCT cursor streaming, Citus HLL translation, SERVICE pruning, nonblocking VP promotion, scheduled fuzz CI | Released ✅ | Large | [Full details](roadmap/v0.68.0-full.md) |
| [v0.69.0](roadmap/v0.69.0.md) | Module architecture restructuring: split sparql/mod.rs, pg_ripple_http/main.rs, construct_rules.rs, and storage/mod.rs along single-responsibility boundaries | Released ✅ | Large | [Full details](roadmap/v0.69.0-full.md) |

### Assessment 10 Remediation & Production Hardening (v0.70.0 – v0.73.0)

| Version | Theme | Status | Scope | Full details |
|---------|-------|--------|-------|-------------- |
| [v0.70.0](roadmap/v0.70.0.md) | Assessment 10 critical remediation: bulk-load mutation journal, per-statement flush, fail-closed evidence gate, SHACL doc truth, README versioning, RLS SQL quoting, SBOM currency | Released ✅ | Large | [Full details](roadmap/v0.70.0-full.md) |
| [v0.71.0](roadmap/v0.71.0.md) | Arrow Flight streaming validation, Citus multi-node integration test, pg_ripple_http/pg_ripple compatibility matrix, HLL accuracy docs, SERVICE shard benchmark | Released ✅ | Large | [Full details](roadmap/v0.71.0-full.md) |
| [v0.72.0](roadmap/v0.72.0.md) | Architecture and protocol hardening: mutation journal SAVEPOINT safety, plan cache docs, continued module split, ConstructTemplate proptest, SPARQL Update fuzz, conformance gate promotion, Arrow Flight replay protection | Released ✅ | Large | [Full details](roadmap/v0.72.0-full.md) |
| [v0.73.0](roadmap/v0.73.0.md) | SPARQL 1.2 tracking, live SPARQL subscription API (WebSocket/SSE), feature status taxonomy, CONTRIBUTING.md, Helm chart SHA pin, R2RML scope docs | Released ✅ | Large | [Full details](roadmap/v0.73.0-full.md) |

### Assessment 11 Remediation & Production Polish (v0.74.0 – v0.76.0)

| Version | Theme | Status | Scope | Full details |
|---------|-------|--------|-------|-------------- |
| [v0.74.0](roadmap/v0.74.0.md) | Assessment 11 critical/high remediation: evidence-path truthfulness (12 missing docs stubs, CI gate fix), mutation journal wiring for Datalog/R2RML/CDC, SBOM regeneration, HTTP companion version alignment, populated-DB CI validation, plan cache invalidation on VP promotion, per-statement flush deferral | Released ✅ | Large | [Full details](roadmap/v0.74.0-full.md) |
| [v0.75.0](roadmap/v0.75.0.md) | Assessment 11 medium findings: unwrap/panic audit, Citus and Arrow integration test CI wiring, roadmap status validation, RLS error surfacing, role-name doc, property-path edge tests, fuzz duration increase, URL parser fuzz target, HTTP companion production docs, feature-status journal entry | Released ✅ | Large | [Full details](roadmap/v0.75.0-full.md) |
| [v0.76.0](roadmap/v0.76.0.md) | Assessment 11 low-severity and polish: rust-toolchain pin, RLS hash widening, Arrow dep pin, benchmark baseline refresh, test count growth, /metrics auth docs, xact SPI safety citation, log-hook audit, clippy gate verification | Released ✅ | Medium | [Full details](roadmap/v0.76.0-full.md) |

### Bidirectional Integration & Beyond (v0.77.0 – v0.78.0)

| Version | Theme | Status | Scope | Full details |
|---------|-------|--------|-------|-------------- |
| [v0.77.0 + v0.78.0](roadmap/v0.77.0-full.md) | **v0.77.0 — Bidirectional Integration Primitives** (source attribution, conflict resolution with echo-aware `normalize`, late-binding IRI rewrite, sparse-CAS events, linkback with target-assigned IDs, pg-trickle outbox/inbox transport) + **v0.78.0 — Bidirectional Integration Operations** (write-side outbox policy, new-events-only schema evolution, per-subscription side-band auth, write-time redaction, audit, property/chaos tests, reconciliation toolkit, ops surface). Both ship together. **BIDI-SPEC-01:** non-blocking draft RDF Bidirectional Integration Profile v1 for broader ecosystem review. | Released ✅ | Large | [v0.77.0](roadmap/v0.77.0-full.md), [v0.78.0](roadmap/v0.78.0-full.md) |

### Query Engine Completeness (v0.79.0)

| Version | Theme | Status | Scope | Full details |
|---------|-------|--------|-------|-------------- |
| [v0.79.0](roadmap/v0.79.0.md) | Close the last two known query-engine limitations: true Leapfrog Triejoin executor for Worst-Case Optimal Joins (WCOJ-LFTI-01) and full `sh:SPARQLRule` evaluation (SHACL-SPARQL-01). Removes the "Known limitations" table from the README; all `feature_status()` rows become `implemented`. | Released ✅ | Medium | [Full details](roadmap/v0.79.0-full.md) |

### Assessment 12 Remediation (v0.80.0 – v0.83.0)

| Version | Theme | Status | Scope | Full details |
|---------|-------|--------|-------|--------------|
| [v0.80.0](roadmap/v0.80.0.md) | Assessment 12 critical/high remediation + consistency audit: SPARQL Update CWB flush (C-01), R2RML/CDC journal wiring (C-02), property-path cycle detection (C-03), plan cache RLS key (C-14), SQL injection fixes (S-01, S-02), full RFC-1918 SSRF blocklist (S-03), migration chain assertions (T-01), SBOM regeneration + CI gate (DS-01), HTTP error standardisation (A-01), compatibility matrix update (D-01); **new**: /explorer endpoint authentication (EXPLORER-AUTH-01) | Released ✅ | Large | [Full details](roadmap/v0.80.0-full.md) |
| [v0.81.0](roadmap/v0.81.0.md) | Assessment 12 correctness & concurrency closure + consistency audit: HTAP merge SID determinism (C-04), promotion atomicity (C-05), dict encode race (C-06), SHACL subtransaction (C-07), federation URL/truncation (C-08, C-09), OPTIONAL promotion (C-10), Datalog guards (C-11–C-13), federation cache key (C-15), strict dictionary GUC (C-16), SubXact cache invalidation (CC-01), CDC slot cleanup worker (CC-02), per-predicate promotion locks (CC-03), merge fence reduction (CC-04), Datalog SCC fix (CC-05), CDC LSN watermark (CC-06), stuck-promotion recovery (CC-07), strict SPARQL filters GUC (SC-01), replication.rs panic elimination (Q-02); **new (audit-1)**: _PG_fini unload callback (PGFINI-01), plan cache GUC completeness (PLAN-CACHE-GUC-02), BIDI/BIDIOPS feature_status rows (FEATURE-STATUS-BIDI-01), shared_preload_libraries warning (PRELOAD-WARN-01), retract parameterisation (RETRACT-PARAM-01), scheduler error propagation (SCHEDULER-ERR-01), DRed full fixpoint (DRED-FIXPOINT-01), HTAP retract consistency (RETRACT-HTAP-01); **new (audit-2)**: RAG handler parameterized queries (RAG-SQL-INJECT-02), extended plan cache GUC keys (PLAN-CACHE-GUC-02 ext) | Released ✅ | Large | [Full details](roadmap/v0.81.0-full.md) |
| [v0.82.0](roadmap/v0.82.0.md) | Assessment 12 performance & observability + consistency audit: plan cache capacity GUC (P-01), batch decode ANY($1) (P-02), merge worker predicate cache (P-03), federation cost stats (P-04), predicate stats cache table (P-08), Arrow row limit GUC (P-09), pg_stat_statements normalisation (P-10), GUC bounds validators (P-12), Prometheus structured labels (P-13), SPARQL algebra tree in EXPLAIN (O-01), merge worker heartbeat (O-02), graph_stats scan limit GUC (O-03), redacted_error uniformity (O-06), admin lock docs (O-07), SPARQL depth DoS GUC (S-05), tenant name validation (S-06), Unicode role fallback (S-07), federation response cap (S-09), shmem overflow check (S-12), RUSTSEC audit refresh (S-04); **new (audit-1)**: SSE subscription_id length check (LISTEN-LEN-01), property-path predicate limit GUC (PROPPATH-UNBOUNDED-01), vacuum_dictionary batch size (VACUUM-DICT-BATCH-01), merge lock timeout GUC (MERGE-LOCK-GUC-01), datalog cleanup error visibility (DATALOG-SILENT-01), embedding model GUC consistency (EMBED-MODEL-01), federation call counter ordering (FED-COUNTER-ORDER-01); **new (audit-2)**: federation response pre-read size check (FED-BODY-STREAM-01), batch_decode missing-ID warning (DECODE-WARN-01), export_jsonld OOM documentation (EXPORT-JSONLD-OOM-01) | Released ✅ | Large | [Full details](roadmap/v0.82.0-full.md) |
| [v0.83.0](roadmap/v0.83.0.md) | Assessment 12 test coverage, API polish & code quality + consistency audit: 8 error-path regression tests (T-02), N-Triples/N-Quads/TriG fuzz targets (T-03), sparql_update fuzz target (T-04), proptest reference-impl comparisons (T-05), CDC async test barriers (T-06), known_failures.txt annotations (T-07), 13 missing pg_extern regression tests (T-08), json_ld_load deprecation/rename (A-02), RETURNS TABLE graph column (A-04), GUC naming convention docs (A-05), BREAKING changelog tags (A-06), serde_cbor evaluation (DS-02), Renovate config (DS-03), bidi.rs module split (Q-01), shared-memory dict cache evaluation (P-05); **new (audit-1)**: health endpoint build_time field fix (BUILD-TIME-FIELD-01), datalog cost-model divisor GUCs (DL-COST-GUC-01), CHANGELOG heading format lint (CHANGELOG-FMT-01); **new (audit-2)**: merge worker exponential backoff (MERGE-BACKOFF-01), metrics route auth documentation (METRICS-AUTH-DOC-01), WWW-Authenticate header on 401 (HTTP-401-WWW-AUTH-01), JSON auth error envelope (AUTH-RESP-FMT-01), blank node label export validation (EXPORT-BNODE-VALID-01), Datalog max-iteration test (DATALOG-MAXITER-TEST-01) | Released ✅ | Large | [Full details](roadmap/v0.83.0-full.md) |

### Assessment 13 Remediation (v0.84.0 – v0.86.0)

| Version | Theme | Status | Scope | Full details |
|---------|-------|--------|-------|-------------- |
| [v0.84.0](roadmap/v0.84.0.md) | Assessment 13 critical/high & operational remediation: HTTP companion version sync, STRICT_COMPAT env var, docker-compose image tag currency, SECURITY DEFINER annotations, CI SQL-injection-check gate, migration-chain checkpoints for v0.80–v0.83, gucs/registration.rs split, nested OPTIONAL+EXISTS fix, /health/ready deep-check, plan cache double-parse elimination, justfile bump-version/regen-sbom/regen-openapi recipes | Released ✅ | Large | [Full details](roadmap/v0.84.0-full.md) |
| [v0.85.0](roadmap/v0.85.0.md) | Assessment 13 correctness, performance & code quality: strict decode, mutation journal assertions, plan cache normalisation, IRI length bounds, encode batch API, merge-worker throttling, cycle pre-check, HOT-path metrics, schema.rs/federation.rs splits, per-file line-count CI gate, per-predicate merge fence | Released ✅ | Large | [Full details](roadmap/v0.85.0-full.md) |
| [v0.86.0](roadmap/v0.86.0.md) | Assessment 13 tests, API polish, observability, supply chain & security: sparql_roundtrip proptest vs reference evaluator, CONSTRUCT/SHACL-SPARQL fuzz targets, conformance trend artifacts, benchmark regression gate, OpenAPI CI, error-code registry, structured JSON logs, axum graceful shutdown, dependency upgrades, CORS counter, Arrow 413 guard, line coverage badge + llvm-cov CI gate | Released ✅ | Large | [Full details](roadmap/v0.86.0-full.md) |

### Uncertain Knowledge & Soft Reasoning (v0.87.0)

> **Probabilistic feature specification**: see [plans/probabilistic-features.md](plans/probabilistic-features.md) for the full architecture, algorithm details, and API design for the uncertain knowledge engine. (D13-05, v0.86.0)

| Version | Theme | Status | Scope | Full details |
|---------|-------|--------|-------|-------------- |
| [v0.87.0](roadmap/v0.87.0.md) | **Uncertain knowledge engine** — Probabilistic Datalog with `@weight(FLOAT)` rule annotations, multiplicative confidence propagation, and noisy-OR multi-path combination; `pg:confidence()` SPARQL function and `load_triples_with_confidence()` bulk-loader; fuzzy SPARQL filters (`pg:fuzzy_match()` trigram/token-set similarity, confidence-threshold edge filtering in property paths via `pg:confPath()`); soft SHACL scoring with numerical `sh:severityWeight` annotations and `pg_ripple.shacl_score()` composite data-quality function; provenance-weighted confidence derived automatically from PROV-O source trust metadata via Datalog rules | Released ✅ | Very Large | [Full details](roadmap/v0.87.0-full.md) |

### Graph Analytics: PageRank (v0.88.0)

| Version | Theme | Status | Scope | Full details |
|---------|-------|--------|-------|-------------- |
| [v0.88.0](roadmap/v0.88.0.md) | **Datalog-native PageRank & graph analytics** — iterative PageRank via Datalog^agg + tabling; `pg:pagerank()` / `pg:pagerank(?node, ?topic)` SPARQL functions; personalized + predicate-scoped PR; magic-sets partial-graph PR; `pg_ripple.pagerank_run()` SQL function; **pg-trickle incremental refresh** (K-hop Z-set, score bounds, staleness columns, selective recomputation); confidence-weighted edges (v0.87 integration); topic-sensitive multi-run; edge-weight predicates; reverse/in-degree direction; temporal decay; SHACL constraint-aware ranking (`sh:importance`, `sh:excludeFromRanking`, `shacl_score()` threshold); sketch-based `pg:topN_approx()`; score-explanation trees (`explain_pagerank()`); graph-partitioned parallel computation; standard-format export (Turtle/JSON-LD/CSV/N-Triples); federation blend mode; four alternative centrality measures via `pg:centrality()` (betweenness, closeness, eigenvector, Katz); IVM queue metrics; **six v0.87×v0.88 synergies**: confidence-attenuated K-hop propagation (PR-TRICKLE-CONF-01), probabilistic PageRank via `@weight` rules (PR-PROB-DATALOG-01), centrality-guided entity deduplication (PR-ENTITY-RESOLUTION-01), source-trust-weighted eigenvector centrality (PR-TRUST-EIGEN-01), confidence-gated federation edges (PR-FED-CONF-01), temporal authority via Katz centrality (PR-KATZ-TEMPORAL-01); PT0401–PT0423 error catalog | Released ✅ | Very Large | [Full details](roadmap/v0.88.0-full.md) |

### Assessment 14 Remediation (v0.89.0 – v0.92.0)

| Version | Theme | Status | Scope | Full details |
|---------|-------|--------|-------|-------------- |
| [v0.89.0](roadmap/v0.89.0.md) | **A14 High remediation** — delete `src/gucs/registration.rs.bak` + CI lint; migration-chain checkpoints v0.84–v0.88 + structural version-sync assertion; bump `COMPATIBLE_EXTENSION_MIN` to v0.88.0; `just bump-version X.Y.Z` recipe; confidence noisy-OR proptest vs reference oracle; `check_auth_write` on mutating PageRank handlers; GUC name audit before API freeze; default rate limit 100 req/s; `fuzzy_max_input_length` + `pagerank_max_seeds` guards; IRI escaping in `export_pagerank()` | Released ✅ | Large | [Full details](roadmap/v0.89.0-full.md) |
| [v0.90.0](roadmap/v0.90.0.md) | **A14 Medium: correctness, performance, concurrency, code quality** — PageRank convergence-norm GUC + K-hop drift bound doc; SPARQL MINUS blank-scope regression; export format enum validation; WCOJ integration for large-graph PageRank; `clippy::unwrap_used` workspace lint gate; PageRank streaming without temp materialisation; embedding fast-path gate; confidence ANALYZE; `pagerank_dirty_edges` deadlock test; advisory lock for concurrent PageRank runs; confidence + PageRank proptests; confidence-loader fuzz target; PageRank scale gate; concurrent SPARQL+PageRank test; pre-emptive splits of seven 1,300–1,700 line files; `src/pagerank/` + `src/uncertain/` module splits; probabilistic weight parser validation; cyclic-convergence documentation | Released ✅ | Large | [Full details](roadmap/v0.90.0-full.md) |
| [v0.91.0](roadmap/v0.91.0.md) | **A14 Medium: observability, API, standards, build, docs** — PageRank IVM Prometheus gauges; SHACL score-log retention GUC + background vacuum; SSE endpoint verification; HTTP routing middleware extraction; Arrow Flight COUNT→EXPLAIN swap; `explain_pagerank_json()` JSONB variant; PT error code registry (PT0301–PT0423); SPARQL 1.2 tracking update; RDF-star compliance matrix; ProbLog citation; `lint-version-sync` CI extension; dedicated `migration-chain.yml` workflow; ureq + arrow/parquet upgrade triage; IVM boundary documentation; CWB confidence-propagation test; CDC LSN watermark batching; compatibility matrix v0.87/v0.88 rows; `pagerank.md` completeness audit | ✅ Released | Medium | [Full details](roadmap/v0.91.0-full.md) |
| [v0.92.0](roadmap/v0.92.0.md) | **A14 Low-severity polish** — PageRank bounds source comment; damping tuning guide; SERVICE SILENT TLS test; `describe_form` alias contract; RSA dependency audit; `pagerank_dirty_edges` RLS; `pagerank_find_duplicates` STABLE; cargo-audit `--deny unmaintained`; `pagerank_partition` auto-tune default; `fuzzy_match` STABLE; Datalog cyclic-dep regression; confidence sub-xact rollback test; benchmark throughput history; `.unwrap()` audit v0.87/v0.88; `diagnostic_report()` v0.87/v0.88 catalog; `SOURCE_DATE_EPOCH` reproducible builds; `owl:sameAs` PageRank dedup doc; CDC `pg_notify` payload bound; SSE backpressure load test; WC-01–WC-05 post-v1.0 aspirational tracking issues filed | ✅ Released | Medium | [Full details](roadmap/v0.92.0-full.md) |

### pg_tide Integration (v0.93.0)

> **Context**: pg-trickle v0.46.0 extracted its full relay, outbox, and inbox subsystem into the
> new standalone `pg_tide` extension (`trickle-labs/pg-tide`). After v0.46.0, `pg_trickle` provides
> IVM only. See [plans/PLAN_PG_TIDE.md](plans/PLAN_PG_TIDE.md) for the full impact analysis.

| Version | Theme | Status | Scope | Full details |
|---------|-------|--------|-------|-------------- |
| [v0.93.0](roadmap/v0.93.0.md) | **pg_tide integration & documentation modernisation** — Add `has_pg_tide()` runtime detection + `pg_ripple.pg_tide_available()` SQL function (TIDE-1); update BIDI-OUTBOX-01/BIDI-INBOX-01 doc comments to reference `pg_tide` (TIDE-2); add `PGTIDE_HINT` constant for relay error paths (TIDE-3); full rewrite of `docs/src/operations/pg-trickle-relay.md` to `tide.*` API — 20+ call sites updated, new outbox publish trigger pattern, `pg-tide-relay` binary, updated prerequisites and architecture diagram (TIDE-4); update `blog/semantic-hub-trickle-relay.md` hub-and-spoke examples (TIDE-5); add backward-compat note to `plans/pg_trickle_relay_integration.md` (TIDE-6); add inline notes to `roadmap/v0.52.0.md` and `roadmap/v0.77.0-full.md` (TIDE-7); extend compatibility matrix with `pg_tide ≥ 0.1.0` rows (TIDE-8); add comment-only migration script `sql/pg_ripple--0.92.0--0.93.0.sql`; update Dockerfile to build and ship pg_tide alongside pg_ripple, pg_trickle, PostGIS, and pgvector (TIDE-DOCKER-01) | ✅ Released | Small | [Full details](roadmap/v0.93.0.md) |

### Assessment 15 Remediation (v0.94.0 – v0.97.0)

| Version | Theme | Status | Scope | Full details |
|---------|-------|--------|-------|-------------- |
| [v0.94.0](roadmap/v0.94.0.md) | **A15 High remediation** — implement `just bump-version X.Y.Z` + bump `COMPATIBLE_EXTENSION_MIN` to v0.93.0 (H15-01); add `SET search_path = pg_catalog, _pg_ripple, public` to `_pg_ripple.ddl_guard_vp_tables()` + CI `check_security_definer_search_path.sh` lint (H15-02); bounded bidirectional relay channel with `pg_ripple.bidi_relay_max_inflight` GUC, drop-oldest overflow policy, and `pg_ripple_bidi_relay_dropped_total` Prometheus counter (H15-03/L15-13); migrate `src/bulk_load.rs` to `COPY ... FROM STDIN BINARY` for dictionary-encoded triple stream gated on `pg_ripple.bulk_load_use_copy` GUC; extract shared `copy_into_vp()` helper used by bulk loader, R2RML, and CDC paths (H15-05/M15-20) | ✅ Released | Large | [Full details](roadmap/v0.94.0-full.md) |
| [v0.95.0](roadmap/v0.95.0.md) | **A15 Medium: correctness, security, storage** — replace both `unreachable!()` in `pagerank/export.rs` and `pagerank/centrality.rs` with `pgrx::error!` + CI zero-unreachable-in-production lint (M15-01); resolve-once DNS rebinding fix in `federation/policy.rs` — validate every resolved IP against the SSRF blocklist, connect to resolved IP with pinned `Host:` header (M15-02); `sql_drop` event trigger for `DROP EXTENSION` replication-slot cleanup (M15-03); `redacted_error()` for SSE initialisation error paths in `stream.rs` (M15-04); scheduled `VACUUM ANALYZE _pg_ripple.dictionary` after bulk encode above threshold GUC, plus `autovacuum_scale_factor` `reloptions` on the dictionary table (M15-07); explicit regression tests for `OPTIONAL` + property paths inside `GRAPH {}` with `vp_rare` predicates (M15-08); NaN/Inf/out-of-range rejection in `load_triples_with_confidence()` and `INSERT ON CONFLICT` confidence paths — raise PT0302/PT0303 (M15-09); fold `_pg_ripple.schema_generation` counter into plan cache key, bump on every VP promotion and `ensure_vp_table()` call (M15-10); integrate `ADD`/`COPY`/`MOVE` SPARQL Update operations through the main UPDATE pipeline with CDC and SHACL queue integration tests (M15-12) | ✅ Released | Large | [Full details](roadmap/v0.95.0-full.md) |
| [v0.96.0](roadmap/v0.96.0.md) | **A15 Medium: performance, code quality, test coverage** — HTAP tombstone-skip optimisation: maintain `tombstone_count` in `_pg_ripple.predicates` and rebuild view to elide the `LEFT JOIN` when count = 0 (M15-05); star-pattern self-join collapse in `sparql/optimizer.rs` — detect `(?s p1 ?o1 . ?s p2 ?o2 . …)` star shapes, emit single subject-seeded CTE, gate on `pg_ripple.star_join_collapse` GUC (M15-06); separate `pg_ripple.federation_connect_timeout_secs` GUC for TCP/TLS connect vs query-body timeout (M15-11); complete `mod.rs` sub-splits for the five 1,489–1,625 line files (`sparql/expr`, `datalog/compiler`, `storage/ops`, `export`, `sparql/execute`) targeting every `mod.rs` < 800 lines (M15-13); sub-split `datalog_handlers.rs` into `routing/datalog/{rules,inference,query,admin}.rs` (M15-14); `#![warn(missing_docs)]` in `src/lib.rs` + public API doc pass (M15-15); `pagerank_with_writes.sh` concurrent-load test: 4 pgbench writers + 1 reader + 1 PageRank background (M15-17); `shacl_report_scored` column-order regression test (M15-18); four missing Prometheus metrics: `pg_ripple_merge_cycle_duration_seconds`, `pg_ripple_datalog_stratum_duration_seconds`, `pg_ripple_shacl_validation_queue_depth`, `pg_ripple_cdc_replication_slot_lag_bytes` (M15-19); verify cyclic-group pre-check source in parallel Datalog (M15-21); Arrow Flight `EXPLAIN (FORMAT JSON)` row-estimate path replacing the `COUNT(*)` pre-check (M15-22) | ✅ Released | Large | [Full details](roadmap/v0.96.0-full.md) |
| [v0.97.0](roadmap/v0.97.0.md) | **A15 Low-severity polish & supply-chain** — fix CHANGELOG v0.90.0 date placeholder (L15-01); add Arrow Flight, PageRank, and bidi relay example files (L15-02); wire `examples/test_all.sh --live` in CI against `cargo pgrx start pg18` (L15-03); enforce `clippy::missing_safety_doc` + `undocumented_unsafe_blocks` for 1:1 unsafe/SAFETY ratio (L15-04); `#[allow(...)]` justification audit with `// Q15-xx:` convention (L15-05); `gen_random_uuid()` availability check at `_PG_init` with WARNING if pgcrypto absent (L15-06); serde_cbor consumer upgrade: `cargo tree -i serde_cbor` + bump the consumer if a newer version drops the transitive dep (M15-16); RDF-star `<<>>` position support matrix in `docs/src/reference/sparql-compliance.md` (L15-08); `cargo doc --no-deps` missing-documentation gate in CI (L15-09); auto-compute `HIGHEST_CHECKPOINT` in `test_migration_chain.sh` from `ls sql/pg_ripple--*--*.sql &#124; sort -V &#124; tail -1` eliminating the hand-maintained constant (L15-10); document `statement_id_seq` exhaustion behaviour in `docs/src/operations/scaling.md` (L15-11); `owl_sameas_cycle.sql` regression test asserting graceful handling of `(a sameAs b, b sameAs a)` cycles (L15-12); conformance-suite pass-rate badges (Jena, WatDiv, OWL 2 RL) in `README.md` updated by CI workflow (L15-14) | Planned | Small | [Full details](roadmap/v0.97.0-full.md) |

### SKOS Vocabulary Support (v0.98.0)

| Version | Theme | Status | Scope | Full details |
|---------|-------|--------|-------|-------------- |
| [v0.98.0](roadmap/v0.98.0.md) | **SKOS support, named bundle API & graph intelligence** — `"skos"` built-in Datalog rule set (28 rules) implementing all W3C SKOS entailments (S7–S45): `skos:broaderTransitive`/`skos:narrowerTransitive` closures, `skos:narrower`/`skos:broader` inverse inference, `skos:related` symmetry, mapping property propagation (`broadMatch`→`broader`, `exactMatch` transitivity/symmetry), label and documentation sub-properties, concept-type assertions; `"skosxl"` rule set (3 rules, S55–S57); `"skos-integrity"` SHACL shape bundle (10 validators, W3C S9/S13/S14/S27/S37/S46 + ISO 25964-1 structural rules); formal named bundle API: `load_datalog_bundle(name, graph)` / `load_shape_bundle(name)` / `active_datalog_bundles` catalog view with `bundle_version` (required by riverbank compiler profiles); implicit dependency resolution (`load_shape_bundle('skos-integrity')` auto-activates `'skos-transitive'`); `explain_contradiction(subject_iri, mode)` — greedy and exact minimal-hitting-set contradiction explainer tracing which triples and Datalog rules produce an inconsistency, plus JSONB variant; `pg_ripple.federation_endpoints` table with `min_confidence` per endpoint and `pg:sourceTrust` tagging of remote `SERVICE` triples; `coverage_map(named_graphs, topic_predicate, top_k)` returning per-topic triple count, source count, mean/min confidence, violation count, and time range; `refresh_coverage_map()` writing `pgc:CoverageMap` triples schedulable via pg_trickle; five SQL helper functions (`skos_ancestors`, `skos_descendants`, `skos_label`, `skos_related`, `skos_siblings`); `validate_skos()`; 60+ pg_regress tests; 100-concept AGROVOC fixture; cookbook chapter; blog post | Planned | Large | [Full details](roadmap/v0.98.0-full.md) |

### Foundational Vocabulary Bundles (v0.99.0)

| Version | Theme | Status | Scope | Full details |
|---------|-------|--------|-------|-------------- |
| [v0.99.0](roadmap/v0.99.0.md) | **DCTERMS, Schema.org & FOAF vocabulary bundles** — completes the "Big 5" foundational vocabulary stack; native Datalog rule sets and SHACL shape bundles for Dublin Core Terms (`"dcterms"`, 11 rules: dc11 backward-compat aliases, hasPart/isPartOf inverses, hasVersion/isVersionOf, replaces/isReplacedBy, DC-SKOS-01 bridge; 8 SHACL validators), Schema.org (`"schema"`, 15 rules: type-hierarchy shortcuts for Person/Organization/Product/Event/CreativeWork, inverse pairs, SCHEMA-FOAF-01/SCHEMA-DC-01/SCHEMA-DCAT-01 cross-vocab bridges; 6 validators), FOAF (`"foaf"`, 8 rules: knows symmetry, Agent subsumption, made/maker inverse, DC-FOAF-01 bridge; 5 validators); cross-bundle rules activate automatically when co-bundles are loaded; `schema_type_ancestors()` and `foaf_persons()` SQL helpers; 45+ pg_regress tests; `docs/src/cookbook/common-vocabularies.md` cookbook chapter | Planned | Medium | [Full details](roadmap/v0.99.0-full.md) |

### Stable Release & Ecosystem (v1.0.0 – v1.2.0)

| Version | Theme | Status | Scope | Full details |
|---------|-------|--------|-------|-------------- |
| [v1.0.0](roadmap/v1.0.0-full.md) | **Production hardening** (ROAD-15-01): 72-hour continuous load test (`bench-bsbm-100m` + WatDiv), third-party security audit (TrailOfBits/Cure53), API stability matrix for every `#[pg_extern]` and GUC generated from `cargo doc` JSON, documentation final audit and freeze, public BSBM/WatDiv benchmark results published | Planned | Medium | [Full details](roadmap/v1.0.0-full.md) |
| [v1.1.0](roadmap/v1.1.0.md) | Post-1.0 ecosystem: Cypher/GQL read-only transpiler (`MATCH … RETURN`) + write operations (`CREATE`/`SET`/`DELETE`), Jupyter SPARQL kernel, LangChain/LlamaIndex tool packages, Kafka CDC sink, materialized SPARQL views, dbt adapter, SPARQL endpoint FDW, pgai in-database embedding generation, logical replication for pg_ripple knowledge graphs | Planned | Large | [Full details](roadmap/v1.1.0-full.md) |
| [v1.2.0](roadmap/v1.2.0.md) | **Custom IndexAM for triple patterns** (WC-01): a native PostgreSQL index access method understanding `(s, p, o, g)` quad patterns, enabling parallel index-only scans for SPARQL BGPs and 2–5× faster large-graph scans; **declarative VP table partitioning** (WC-03): `PARTITION BY LIST (g)` for large multi-tenant deployments with per-tenant partition pruning | Planned | Very Large | [Full details](roadmap/v1.2.0-full.md) |

## How these versions fit together

```
v0.1.0–v0.5.1  ─── Foundation: VP storage, dictionary encoding, SPARQL engine, RDF-star, bulk loading
       │
v0.6–v0.10     ─── Storage architecture: HTAP delta/main split, SHACL validation, Datalog reasoning engine
       │
v0.11–v0.20    ─── Query completeness: SPARQL views, Update, Protocol, federation, JSON-LD, W3C conformance baseline
       │
v0.21–v0.32    ─── Correctness & Datalog: built-in functions, SHACL completion, magic sets, well-founded semantics, entity resolution
       │
v0.33–v0.46    ─── Scale & ecosystem: docs site, parallel stratum eval, WCO joins, full conformance suites (SPARQL 1.1, WatDiv, Jena, LUBM, OWL 2 RL)
       │
v0.47–v0.51    ─── Architecture hardening: dead-code wiring, SHACL completeness, streaming results, AI/LLM integration, security hardening
       │
v0.52–v0.54    ─── Integration: pg-trickle relay, OpenAPI, logical replication, Helm chart
       │
v0.55–v0.56    ─── Quality & security: SSRF allowlist, error-catalog, GeoSPARQL, Arrow/Flight, audit log
       │
v0.57–v0.59    ─── Reasoning & sharding: OWL 2 EL/QL, KG embeddings, temporal queries, Citus sharding & shard-pruning, PROV-O
       │
v0.60          ─── Production hardening sprint: HTAP atomic swap, Actions SHA pinning, SECURITY DEFINER lint,
               │   new fuzz targets, geof:distance, pg_dump round-trip CI test
       │
v0.61          ─── Ecosystem depth: per-graph RLS, explain_inference, GDPR erasure, dbt,
               │   SHACL-AF execution, OTLP traceparent, richer federation call stats;
               │   Citus object shard pruning, direct bulk-load
       │
v0.62          ─── Query frontier: Arrow Flight export, WCOJ planner integration, visual graph explorer;
               │   Citus property-path push-down, vp_rare archival, tiered dict cache,
               │   distributed inference dispatch, live shard rebalance, multi-hop pruning
       │
v0.63          ─── SPARQL CONSTRUCT writeback rules: raw-to-canonical pipelines,
               │   incremental delta maintenance, Delete-Rederive, pipeline stratification;
               │   Citus: SERVICE shard pruning, streaming fan-out, HyperLogLog COUNT(DISTINCT),
               │   batched dict encoding, per-worker SID tables, non-blocking VP promotion
       │
v0.64          ─── Release truth and safety freeze: feature_status, deep readiness,
               │   immutable GitHub Actions, digest-scanned Docker release, documentation truth pass
       │
v0.65          ─── CONSTRUCT writeback correctness closure: real delta maintenance,
               │   HTAP-aware retraction, exact provenance, full behavior test matrix
       │
v0.66          ─── Streaming and distributed reality: true cursors, signed Arrow IPC export,
               │   explainable WCOJ mode, integrated Citus pruning/HLL/BRIN/RLS/promotion paths
       │
v0.67          ─── Assessment 9 critical remediation: storage mutation journal,
               │   VP table RLS coverage, Arrow Flight security/correctness,
               │   fail-closed release-truth gates, soak tests, benchmark baselines
       │
v0.68          ─── Distributed scalability and streaming completion: CONSTRUCT cursor
               │   streaming, Citus HLL translation, SERVICE pruning, nonblocking VP
               │   promotion, scheduled fuzz CI for all 12 targets
       │
v0.69          ─── Module architecture restructuring: split sparql/mod.rs,
               │   pg_ripple_http/main.rs, construct_rules.rs, storage/mod.rs
       │
v0.70          ─── Assessment 10 critical remediation: bulk-load mutation journal,
               │   per-statement flush, fail-closed evidence gate, SHACL doc truth,
               │   README versioning, RLS SQL quoting, SBOM currency
       │
v0.71          ─── Arrow Flight streaming validation, Citus multi-node integration,
               │   compatibility matrix, HLL accuracy docs, SERVICE benchmark
       │
v0.72          ─── Architecture hardening: mutation journal SAVEPOINT safety,
               │   plan cache docs, module split, ConstructTemplate proptest,
               │   SPARQL Update fuzz, conformance gate promotion, replay protection
       │
v0.73          ─── SPARQL 1.2 tracking, live subscription API (SSE/WebSocket),
               │   feature taxonomy, CONTRIBUTING.md, Helm chart SHA, R2RML docs
       │
v0.74–v0.76    ─── Assessment 11 remediation & production polish: evidence truthfulness,
               │   mutation journal wiring, RLS hash widening, toolchain pin, 227 regression tests
       │
v0.77–v0.78    ─── Bidirectional integration: source attribution, CAS conflict resolution,
               │   linkback rendezvous, outbox policy, per-subscription auth, redaction, audit
       │
v0.79          ─── Query engine completeness: true Leapfrog Triejoin executor (WCOJ),
               │   full sh:SPARQLRule evaluation; all feature_status() rows → implemented
       │
v0.80–v0.83    ─── Assessment 12 critical/high/medium/low remediation: SPARQL Update
               │   flush, R2RML/CDC journal, property-path cycle detection, SQL injection
               │   fixes, SSRF blocklist, plan cache GUC keys, HTAP determinism,
               │   federation correctness, migration chain assertions, SBOM gates
       │
v0.84–v0.86    ─── Assessment 13 remediation: HTTP companion readiness, SQL-injection
               │   CI lint, plan cache double-parse, GUC module split, migration chain
               │   extension, strict_dictionary decode, schema.rs/federation.rs splits,
               │   CI module-size lint, hot-path metrics, supply chain upgrades,
               │   observability (EXPLAIN post-opt, JSON logs, graceful shutdown),
               │   conformance trends CSV, PT-code error registry, SPARQL 1.2 tracking
       │
v0.87          ─── Uncertain knowledge: probabilistic Datalog (@weight, noisy-OR,
               │   pg:confidence), fuzzy SPARQL (pg:fuzzy_match, pg:confPath threshold),
               │   soft SHACL scoring (shacl_score), provenance-weighted confidence
               │   from PROV-O source trust via Datalog rules
       │
v0.88          ─── Graph analytics: Datalog-native PageRank + four centrality measures;
               │   confidence-weighted, topic-sensitive, temporal, SHACL-aware PR;
               │   predicate-scoped personalization; edge-weight predicates;
               │   reverse/in-degree direction; pg-trickle K-hop incremental refresh
               │   (score bounds, selective recompute, IVM metrics); sketch top-K;
               │   explain_pagerank(); federation blend; Turtle/JSON-LD/CSV export;
               │   pg:centrality() (betweenness/closeness/eigenvector/Katz);
               │   PT0401–PT0420
       │
v0.89–v0.92    ─── Assessment 14 remediation: delete stale .bak file + CI lint;
               │   migration-chain checkpoints v0.84–v0.88 + structural version-sync;
               │   COMPATIBLE_EXTENSION_MIN bump + just bump-version automation;
               │   confidence noisy-OR proptest; check_auth_write on PageRank handlers;
               │   GUC name audit; WCOJ integration for large-graph PageRank;
               │   clippy::unwrap_used lint gate; module splits (7 files, pagerank/,
               │   uncertain/); PageRank IVM Prometheus gauges; SHACL log retention;
               │   PT0301–PT0423 error registry; SPARQL 1.2 + RDF-star matrices;
               │   migration-chain CI workflow; ureq/arrow upgrades; Low-severity polish
       │
v0.93          ─── pg_tide integration: has_pg_tide() detection, BIDI doc modernisation,
               │   pg-trickle-relay.md rewrite to tide.* API, compatibility matrix update
       │
v0.94–v0.97    ─── Assessment 15 remediation: COMPATIBLE_EXTENSION_MIN automation +
               │   just bump-version; SECURITY DEFINER SET search_path + CI lint;
               │   bidi relay bounded channel + bidi_relay_max_inflight GUC;
               │   bulk loader COPY FROM STDIN BINARY + shared copy_into_vp() helper;
               │   DNS rebinding fix; DROP EXTENSION slot cleanup; SSE error redaction;
               │   dictionary VACUUM scheduling; vp_rare GRAPH {} regression tests;
               │   confidence NaN/Inf validation; plan cache schema_generation key;
               │   SPARQL Update ADD/COPY/MOVE pipeline integration; zero-unreachable lint;
               │   tombstone-skip HTAP optimisation; star-pattern self-join collapse;
               │   federation connect/query timeout separation; large mod.rs sub-splits;
               │   datalog_handlers sub-split; missing_docs CI gate; 4 new Prometheus
               │   metrics; concurrent PageRank+writes load test; Arrow Flight EXPLAIN
               │   row-estimate; Low-severity polish + supply-chain hygiene
       │
v1.0.0         ─── Stable release: 72-hour continuous load test, third-party security
               │   audit, API stability matrix, documentation freeze, public benchmarks
       │
v1.1           ─── Post-stable: Cypher/GQL transpiler (read-only + write ops), Jupyter
               │   kernel, LangChain/LlamaIndex tools, Kafka CDC sink, materialized SPARQL
               │   views, dbt adapter, SPARQL endpoint FDW, pgai embedding, logical replication
       │
v1.2           ─── Custom IndexAM for triple patterns (WC-01); declarative VP table
               │   partitioning by named graph (WC-03)
```

v0.1.0 through v0.5.1 build the complete core storage and query engine.
v0.6.0 through v0.10.0 add the HTAP architecture, SHACL validation, and the full
Datalog reasoning engine. v0.11.0 through v0.20.0 complete the SPARQL query and
update surfaces and establish the W3C conformance baseline. v0.21.0 through v0.32.0
harden correctness and deliver production-grade Datalog optimizations including
magic sets, semi-naive evaluation, well-founded semantics, and entity resolution.
v0.33.0 through v0.46.0 deliver the documentation site, parallel evaluation,
worst-case optimal joins, full conformance suites, and the AI/LLM integration layer.
v0.47.0 through v0.51.0 complete the architecture refactor and shipping hardening
required for a production release. v0.52.0 through v0.54.0 deliver the pg-trickle
relay integration and high-availability story. v0.55.0 through v0.56.0 address all
open security findings from PLAN_OVERALL_ASSESSMENT_6 (SSRF allowlist, error-catalog
drift) and add GeoSPARQL 1.1, federation circuit breaker, and the SPARQL audit log.
v0.57.0 through v0.59.0 extend the reasoning platform to OWL 2 EL/QL, add KG
embeddings, entity alignment, temporal RDF queries, Citus sharding with shard-pruning,
and PROV-O provenance. v0.60.0 through v0.62.0 are the pre-1.0 hardening and
ecosystem sprint: v0.60.0 closes the remaining v1.0.0 blockers identified in
PLAN_OVERALL_ASSESSMENT_7 (HTAP atomic swap, CI supply-chain hardening, fuzz target
gaps, `geof:distance`); v0.61.0 delivers ecosystem depth (per-graph RLS, inference
explainability, GDPR erasure, dbt adapter, SHACL-AF execution, richer federation call stats);
v0.62.0 delivers the query frontier (Arrow Flight bulk export, WCOJ planner
integration, visual graph explorer) plus six Citus scalability improvements (property-path
push-down, `vp_rare` cold-entry archival, tiered dictionary cache, distributed inference
dispatch, live shard rebalance, multi-hop pruning carry-forward). v0.63.0 introduces
SPARQL CONSTRUCT writeback rules: any CONSTRUCT query can be registered as a persistent
rule that writes its derived triples directly into a target named graph inside the VP
storage layer and maintains them incrementally — inserts trigger a delta derivation path,
deletes trigger Delete-Rederive retraction — enabling raw-to-canonical model pipelines
where the canonical graph is always consistent with the latest raw data.
v0.63.0 also delivers eight Citus scalability improvements (CITUS-30–37): SERVICE
result shard pruning, streaming coordinator fan-out via SPARQL cursor, approximate
`COUNT(DISTINCT)` via HyperLogLog, batched dictionary encoding, per-worker statement-ID
local tables, non-blocking VP promotion via shadow-table pattern, per-graph RLS
propagation CI gate, and per-worker BRIN summarise after merge. v0.64.0 through
v0.64.0 through
v0.69.0 convert the findings from PLAN_OVERALL_ASSESSMENT_8 and PLAN_OVERALL_ASSESSMENT_9 into explicit
roadmap work: v0.64.0 adds the truth-in-release guardrails (feature status,
deep readiness, immutable CI actions, release digest scanning, and documentation
correction); v0.65.0 closes CONSTRUCT writeback correctness (delta maintenance,
HTAP-aware retraction, exact provenance, and the full behavior test matrix);
v0.66.0 makes the streaming and distributed claims real or explicitly labels
them as planner hints/stubs/helpers (true SPARQL cursors, signed Arrow IPC export,
explainable WCOJ mode, and integrated Citus pruning/HLL/BRIN/RLS/promotion paths);
v0.67.0 addresses all four Critical findings from Assessment 9 (storage mutation
journal closing all CONSTRUCT writeback bypass paths, VP table RLS coverage,
Arrow Flight ticket security and tombstone-aware export, fail-closed release-truth
scripts) and gathers production evidence (soak tests, audit or threat-model closure,
public benchmarks, upgrade/backup acceptance, and mandatory release evidence
artifacts); v0.68.0 completes the distributed execution and streaming contracts
that were labelled partial or planned in v0.62–v0.66 (true CONSTRUCT streaming,
Citus HLL aggregate translation, Citus SERVICE pruning, nonblocking VP promotion,
and scheduled fuzz CI for all twelve targets); and v0.69.0 restructures the large
source modules along single-responsibility boundaries to make the codebase
maintainable for a v1.0.0 API freeze.
v0.70.0 through v0.73.0 address the findings from PLAN_OVERALL_ASSESSMENT_10:
v0.70.0 closes all four Critical findings (bulk-load mutation journal bypass,
per-triple flush overhead, missing evidence file citations, SHACL-SPARQL docs)
and six High/Medium items (README stale, RLS DDL quoting, SBOM currency, missing
test files, legacy script cleanup, roadmap status correction); v0.71.0 validates
the Arrow Flight streaming contract with an RSS-bounded 10 M-row integration test,
implements the previously-missing Citus RLS propagation integration test, adds an
extension/HTTP companion compatibility matrix, and documents HLL accuracy bounds;
v0.72.0 hardens the mutation journal against PostgreSQL SAVEPOINT/ROLLBACK via
xact callbacks, continues the v0.69.0 module split for the three largest remaining
files, adds a ConstructTemplate proptest suite and a SPARQL Update fuzz target,
promotes W3C conformance and BSBM gates to required CI, adds Arrow Flight replay
protection, and tests the Datalog→CWB interaction chain; v0.73.0 tracks SPARQL 1.2,
delivers a live SPARQL subscription API prototype via SSE, and completes the
ecosystem hardening items (CONTRIBUTING.md, Helm chart SHA pinning, feature status
taxonomy, and R2RML scope documentation).
v0.74.0 through v0.76.0 address Assessment 11 findings: evidence-path truthfulness
(twelve doc stubs, CI gate fix), mutation journal wiring for Datalog/R2RML/CDC,
HTTP companion version alignment, unwrap/panic audit, URL parser fuzz target, RLS
hash widening, Rust toolchain pin, and benchmark baseline refresh. v0.77.0 and
v0.78.0 deliver bidirectional RDF integration: source attribution, conflict
resolution, linkback, outbox/inbox transport, per-subscription side-band auth,
write-time redaction, audit trail, and the non-blocking draft RDF Bidirectional
Integration Profile v1. v0.79.0 closes the last two known query-engine limitations
— true Leapfrog Triejoin executor (WCOJ) and full `sh:SPARQLRule` evaluation — so
every `feature_status()` row reads `implemented`. v0.80.0 through v0.83.0 address
all findings from PLAN_OVERALL_ASSESSMENT_12: critical SQL injection and SSRF
security hardening, property-path cycle detection, SPARQL Update and Datalog
mutation journal wiring, HTAP merge SID determinism, federation URL/truncation
correctness, plan cache GUC completeness, migration chain test assertions through
the latest release, SBOM CI gate, and full regression coverage for all previously
untested `pg_extern` functions. v0.84.0 through v0.86.0 address all 82 findings
from PLAN_OVERALL_ASSESSMENT_13: v0.84.0 closes the ten "must-fix before v1.0.0"
items — HTTP companion readiness check, SECURITY
DEFINER inline justification, CI format-check gate, migration-chain checkpoints for
v0.80–v0.83, `gucs/registration.rs` split, `/health/ready`
deep-check, plan cache double-parse elimination, and `justfile` automation recipes;
v0.85.0 delivers correctness and code-quality improvements — `strict_dictionary` in
`batch_decode`, `schema.rs` and `federation.rs` module splits, CI module-size lint
gate, `describe_cbd` depth GUC, per-predicate merge fence lock, and a VP-promotion
crash-recovery regression test; v0.86.0 closes the remaining 30+ backlog items across
test coverage (proptest vs reference evaluator, fuzz targets, conformance trends CSV),
API polish (PT-code error registry, deprecated GUCs doc, OpenAPI YAML commit), supply
chain (dependency upgrades, SBOM gate, toolchain update), observability (post-optimiser
EXPLAIN field, JSON log mode, graceful shutdown), standards conformance (SPARQL 1.2
tracking, GeoSPARQL inventory, DESCRIBE algorithm doc), and HTTP companion security
(Arrow info leak, docker-compose random password, bulk-loader path canonicalization).
v0.87.0 delivers the uncertain knowledge engine:
probabilistic Datalog with `@weight(FLOAT)` rule annotations and noisy-OR multi-path
confidence combination; `pg:confidence()` SPARQL function and
`load_triples_with_confidence()` bulk-loader; fuzzy SPARQL filters
(`pg:fuzzy_match()`, `pg:token_set_ratio()`, and confidence-threshold
`pg:confPath()` property-path queries); soft SHACL scoring via
`pg_ripple.shacl_score()`; and provenance-weighted confidence derived
automatically from PROV-O source trust metadata via Datalog rules.
v0.88.0 delivers Datalog-native PageRank and a comprehensive graph analytics layer:
iterative PageRank computed entirely inside pg_ripple's Datalog engine using
aggregation, tabling, and convergence-aware early termination; `pg:pagerank()` and
`pg:pagerank(?node, ?topic)` SPARQL functions; personalized PageRank with
predicate-scoped bias; `pg_ripple.pagerank_run()` with damping factor, iteration cap,
convergence threshold, direction, edge-weight predicate, topic, temporal decay, and
seed parameters; a materialized `pagerank_scores` view with `topic`, `score_lower`,
`score_upper`, `stale`, and `stale_since` columns; a **pg-trickle incremental
refresh path** (K-hop Z-set local push, score-bounds propagation, selective
recomputation, IVM queue metrics); confidence-weighted edges integrating with
v0.87.0's uncertain knowledge engine; topic-sensitive multi-run scoring;
reverse/in-degree ranking for hub-and-authority decomposition; temporal edge-weight
decay; SHACL constraint-aware ranking via `sh:importance`, `sh:excludeFromRanking`,
and `shacl_score()` quality threshold; sketch-based `pg:topN_approx()` for
sub-millisecond approximate top-K; score explanation trees via
`pg_ripple.explain_pagerank()`; graph-partitioned parallel computation;
standard-format export (Turtle/JSON-LD/CSV/N-Triples); federation blend mode with
confidence-gated remote edge filtering (PR-FED-CONF-01); four alternative centrality
measures (betweenness, closeness, eigenvector, Katz) via `pg:centrality()` and
`pg_ripple.centrality_run()`; and six cross-version synergies that deepen the
v0.87.0 integration: confidence-attenuated K-hop propagation (PR-TRICKLE-CONF-01),
probabilistic PageRank rules via `@weight` Datalog annotations (PR-PROB-DATALOG-01),
centrality-guided entity deduplication combining betweenness + `pg:fuzzy_match()`
(PR-ENTITY-RESOLUTION-01), source-trust-weighted eigenvector centrality seeded by
`pg:sourceTrust` values (PR-TRUST-EIGEN-01), confidence-gated federation edges
(PR-FED-CONF-01), and temporal authority detection via Katz centrality with
time-aware edge weights (PR-KATZ-TEMPORAL-01); PT0401–PT0423 error catalog.
v0.89.0 through v0.92.0 address all 97 findings from PLAN_OVERALL_ASSESSMENT_14:
v0.89.0 closes the seven High findings and the five Must-fix pre-v1.0.0 backlog items
— deleting the stale `src/gucs/registration.rs.bak` backup file (72 KB, bypassing the
file-size CI lint), adding migration-chain checkpoints for v0.84–v0.88 with a
structural version-sync assertion that eliminates the recurrence class, bumping
`COMPATIBLE_EXTENSION_MIN` to v0.88.0 and implementing `just bump-version X.Y.Z`
to make every future bump atomic, adding a confidence noisy-OR proptest vs a reference
oracle, hardening mutating PageRank HTTP handlers to require write-level auth,
auditing v0.87/v0.88 GUC names before the API freeze, and adding `fuzzy_max_input_length`
/ `pagerank_max_seeds` guards plus IRI escaping in `export_pagerank()`; v0.90.0 sweeps
the Medium correctness, performance, concurrency, and code-quality findings — PageRank
convergence-norm GUC, K-hop drift bound documentation, SPARQL MINUS blank-scope
regression test, export-format enum validation, WCOJ integration for large-graph
PageRank (10M+ edges), `clippy::unwrap_used` workspace lint gate, streaming VP scans
to avoid 4–8 GB temp materialisation on 100M-edge graphs, seven pre-emptive module
splits before the 1,800-line CI gate is tripped, `src/pagerank/` and `src/uncertain/`
submodule restructuring, probabilistic `@weight` parser validation, and cyclic
convergence documentation; v0.91.0 addresses the remaining Medium observability, API,
standards, build, and documentation findings — PageRank IVM Prometheus gauges,
SHACL score-log retention GUC, SSE endpoint verification, HTTP routing middleware
extraction, Arrow Flight row-count estimation improvement, `explain_pagerank_json()`
JSONB variant, PT error code registry completeness (PT0301–PT0423), SPARQL 1.2
tracking page update, RDF-star compliance matrix, dedicated `migration-chain.yml`
CI workflow, and compatibility matrix rows for v0.87/v0.88; v0.92.0 polishes all
39 Low-severity findings — bounds source comments, damping tuning guide, SERVICE
SILENT TLS test, RLS on `pagerank_dirty_edges`, `fuzzy_match` IMMUTABLE annotation,
`pagerank_partition` auto-tune, `SOURCE_DATE_EPOCH` reproducible builds, and
WC-01–WC-05 post-v1.0.0 aspirational tracking issues filed for the v1.1.0–v1.2.0 arc.
v0.93.0 integrates the new `pg_tide` standalone extension (extracted from pg-trickle
v0.46.0), updating all relay/outbox/inbox call sites, rewriting the relay operations
doc to the `tide.*` API, and extending the compatibility matrix with `pg_tide ≥ 0.1.0`
rows. v0.94.0 through v0.97.0 address all 41 findings from PLAN_OVERALL_ASSESSMENT_15:
v0.94.0 closes the five High findings — the perennially recurring
`COMPATIBLE_EXTENSION_MIN` lag is finally eliminated structurally with a `just
bump-version X.Y.Z` recipe that atomically updates all seven version tokens; the lone
SECURITY DEFINER function gains the `SET search_path` clause required before the
third-party audit; the bidirectional relay gains a bounded channel with an explicit
drop-oldest overflow policy; and the bulk loader migrates to `COPY ... FROM STDIN
BINARY` with a shared `copy_into_vp()` helper used by all three high-volume insert
paths (bulk loader, R2RML, CDC); v0.95.0 sweeps Medium correctness and security
findings — DNS rebinding in the SSRF federation check, `DROP EXTENSION` replication-slot
cleanup, SSE error leakage, dictionary VACUUM scheduling, NaN/Inf confidence input
validation, plan cache schema-generation key, and `ADD`/`COPY`/`MOVE` SPARQL Update
pipeline integration; v0.96.0 addresses Medium performance, code-quality, and test-
coverage findings — HTAP tombstone-skip optimisation, star-pattern self-join collapse,
federation timeout separation, five large `mod.rs` sub-splits, `datalog_handlers.rs`
sub-split, missing-docs CI gate, four new Prometheus metrics, concurrent PageRank+writes
load test, and Arrow Flight EXPLAIN-based row estimate; v0.97.0 polishes all Low items
— CHANGELOG date fix, missing examples, `unsafe`/SAFETY 1:1 enforcement, `#[allow(...)]`
justification convention, `gen_random_uuid` availability check at `_PG_init`, serde_cbor
consumer upgrade, RDF-star position matrix, `cargo doc` CI gate, auto-computed migration
chain checkpoint, sequence exhaustion docs, `owl:sameAs` cycle regression test, and
conformance-suite pass-rate badges.
v1.0.0 is the stable release: a 72-hour continuous load test, a
third-party security audit, an API stability matrix for every `#[pg_extern]` and GUC,
documentation final audit and freeze, and public BSBM/WatDiv benchmark results.
v1.1.0 delivers post-stable improvements: Cypher/GQL transpiler (read-only and write
operations), Jupyter SPARQL kernel, LangChain/LlamaIndex tool packages, Kafka CDC sink,
materialized SPARQL views, a dbt adapter, a SPARQL endpoint FDW, pgai in-database
embedding generation, and logical replication for pg_ripple knowledge graphs. v1.2.0
delivers the Custom IndexAM for triple patterns (WC-01) — a native PostgreSQL index
access method that understands `(s, p, o, g)` quad patterns for parallel index-only
BGP scans — and declarative VP table partitioning by named graph (WC-03) for large
multi-tenant deployments.

