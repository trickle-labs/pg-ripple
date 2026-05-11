# pg_ripple Documentation — Gap Analysis Report

> **Date**: 2026-05-11
> **Scope**: Full audit of `docs/src/`, `blog/`, cross-referenced against `src/`, `CHANGELOG.md`, and `AGENTS.md`
> **Current version**: v0.99.1 (approaching v1.0.0)

---

## Executive Summary

The pg_ripple documentation is **substantially complete and production-quality**. All 112+ doc pages are non-stub, code-example-rich, and cover the full feature set. The weakest areas are **reference completeness** (GUC and HTTP API coverage) and a **buggy example in the most critical page** (Hello World). The structure, tone, and depth are already at a high standard.

### By the Numbers

| Metric | Value | Assessment |
|--------|-------|------------|
| Total doc pages | 112+ | Excellent breadth |
| Stub pages (< 10 lines) | 0 | No gaps |
| Feature pages complete | 29/29 (100%) | Excellent |
| Cookbook recipes | 14/14 (100%) | Excellent |
| Operations pages | 25/25 (100%) | Excellent |
| Blog posts | 39 | Strong — good coverage of all major features |
| SQL functions documented | 156 of ~298 `#[pg_extern]` (52%) | **Gap** — many internal but audit needed |
| GUC parameters documented | 41 of 197 defined (21%) | **Major gap** |
| HTTP endpoints documented | 5 of 55+ routes (9%) | **Major gap** |
| Error codes documented | PT001–PT799 (comprehensive) | Excellent |

---

## Critical Issues (Block v1.0.0)

### CRIT-01: Hello World Step 3 — Buggy SPARQL Query

**File**: `docs/src/getting-started/hello-world.md`, Step 3

The query reuses `?movie` as both the movie entity and the name binding:

```sparql
SELECT ?movie ?director WHERE {
  ?movie schema:director ?person .
  ?movie schema:name ?movie .        -- BUG: ?movie is already bound to the entity
  ?person foaf:name ?director .
}
```

This will produce **incorrect results** or zero rows. The variable `?movie` cannot be both an IRI (the entity) and a literal (the name). Should be `?movieName` or similar.

**Impact**: This is the single most-read page in the documentation. A broken first example destroys trust in the first 90 seconds.

### CRIT-02: Hello World Step 3 — Variable Naming Inconsistency

The query returns `?movie` and `?director`, but the prose says "you should see 'The Graph' directed by 'Alice'". The `?director` variable is bound to the *person's name* (via `foaf:name`), not to a director entity — confusing naming that misleads the reader.

### CRIT-03: GUC Reference Severely Incomplete

**File**: `docs/src/reference/guc-reference.md`

Only **41** of **197** GUC parameters (21%) are documented. Missing categories include:

- **PageRank** (28 GUCs: `pagerank_damping`, `pagerank_incremental`, `pagerank_partition`, etc.)
- **Datalog** (~33 GUCs: `magic_sets`, `dred_enabled`, `tabling`, `probabilistic_datalog`, `owl_profile`, etc.)
- **Storage** (~44 GUCs: `merge_threshold`, `citus_sharding_enabled`, `arrow_flight_secret`, etc.)
- **SPARQL** (~22 GUCs: `wcoj_enabled`, `star_join_collapse`, `sparql_max_rows`, `sparql_max_algebra_depth`, etc.)
- **Observability** (6 GUCs: `tracing_enabled`, `audit_log_enabled`, etc.)

Any DBA tuning a production deployment will hit undocumented parameters.

### CRIT-04: HTTP API Reference — Incomplete

**File**: `docs/src/reference/http-api.md`

Documents only 5 endpoints. The actual `pg_ripple_http` service exposes **55+ routes** including:

- `/datalog/rules/*` (24 routes) — rule management, inference, views, constraints
- `/pagerank/*`, `/centrality/*` (10 routes)
- `/confidence/*` (4 routes)
- `/subscribe/*` (SSE/WebSocket)
- `/openapi.yaml`
- `/explorer`
- `/flight/do_get` (Arrow Flight)
- `/health/ready`, `/metrics/extension`, `/void`, `/service`

---

## High-Priority Improvements

### HIGH-01: Landing Page — Missing Persona Decision Tree

**File**: `docs/src/landing.md`

The "Next steps" section has persona-targeted links, which is good. However, there is no visual decision tree (flowchart or matrix) to immediately orient a new visitor. Compare with the Prisma docs landing page which shows three clear paths at a glance.

### HIGH-02: Tutorial — Missing "What you'll learn" Preamble

**File**: `docs/src/getting-started/tutorial.md`

The tutorial has a good structure (four segments, time estimates) but no "What you'll learn" summary at the top. Each segment should open with a 1-sentence learning objective.

### HIGH-03: Docker Quickstart — Not Prominent Enough

**File**: `docs/src/getting-started/installation.md`

Docker is listed first (good) but the section could be more prominent. A callout box like `> **Fastest path**: docker compose up -d` would help.

### HIGH-04: SQL Function Reference — 52% Coverage of `#[pg_extern]`

**File**: `docs/src/reference/sql-functions.md`

156 functions documented out of ~298 `#[pg_extern]` definitions. Many of the undocumented ones may be internal, but an audit is needed to identify which public-facing functions are missing. The `user-guide/sql-reference/` section adds detail but may not cover everything.

### HIGH-05: Blog Posts — No Cross-Links from Feature Pages

Blog posts are standalone and not linked from relevant feature deep-dive pages. For example:
- `blog/dictionary-encoding-integer-joins.md` should be linked from the Key Concepts page
- `blog/htap-reads-and-writes.md` should be linked from the HTAP feature page
- `blog/leapfrog-triejoin.md` should be linked from Advanced Inference

### HIGH-06: Version References

The `pg_ripple.control` shows v0.99.1 but various docs may still reference older versions. A sweep is needed to ensure version consistency.

### HIGH-07: Missing Production-Readiness Checklist

There is no single "production-readiness checklist" page. Operations pages cover individual topics (security, HA, monitoring, etc.) but there's no consolidated checklist a DevOps engineer can walk through before going live. `docs/src/operations/bidi-production-checklist.md` exists for the bidirectional relay specifically, but not for pg_ripple overall.

---

## Medium-Priority Improvements

### MED-01: Terminology Inconsistencies

- "triple" vs "statement" — used interchangeably in a few places
- "graph" vs "named graph" — sometimes ambiguous
- "rule" vs "constraint" — Datalog constraints vs SHACL constraints need clearer distinction

### MED-02: Some Feature Pages Missing Cross-Links

Feature pages generally have "Next steps" but not all cross-reference related cookbook recipes or blog posts.

### MED-03: Missing Tutorial — Migrate from Neo4j

There is an LPG → RDF mapping feature page (`features/lpg-mapping.md`) but no end-to-end migration recipe.

### MED-04: Missing Tutorial — Build a RAG Pipeline from Scratch

The `user-guide/rag-pipeline.md` and `cookbook/grounded-chatbot.md` cover RAG but there's no single end-to-end tutorial that goes from "empty database" to "working LangChain + pg_ripple chatbot".

### MED-05: Missing Tutorial — Kubernetes Production Setup

`operations/kubernetes.md` exists but a step-by-step tutorial from Helm install to running queries would help.

### MED-06: Missing "Limits and Quotas" Page

No page documenting tested limits: max triple count, max predicate count, max SPARQL query complexity, etc.

### MED-07: Callout System Inconsistency

Some pages use `````admonish note````` (mdBook admonish plugin), others use `> **Note**:` markdown blockquotes. Should be standardized.

### MED-08: Missing Mermaid Architecture Diagrams

The landing page has an ASCII architecture diagram. Feature deep dives for HTAP, CDC, and federation would benefit from proper Mermaid diagrams.

---

## Low-Priority / Nice-to-Have

### LOW-01: Blog Post Freshness Audit

39 blog posts exist. Some may reference older API patterns. A sweep to update or annotate with version notes would be valuable.

### LOW-02: Glossary Could Be More Comprehensive

`reference/glossary.md` exists but could be expanded with more pg_ripple-specific terms.

### LOW-03: FAQ Could Be Expanded

`reference/faq.md` exists but could address more common questions based on support patterns.

### LOW-04: Search Optimization

mdBook search works but page titles could be more search-friendly (e.g., "SPARQL Query Reference" rather than just "SPARQL Reference").

### LOW-05: Code Example Language Tags

A few code blocks may be missing language tags for syntax highlighting. A sweep would catch these.

---

## Coverage Matrix

| Feature | Feature Page | Tutorial | Cookbook | Reference | Blog Post |
|---------|:-----------:|:--------:|:-------:|:---------:|:---------:|
| SPARQL 1.1 Query | ✅ | ✅ | ✅ | ✅ | ✅ |
| SPARQL 1.1 Update | ✅ | — | — | ✅ | — |
| SPARQL Federation | ✅ | — | ✅ | ✅ | ✅ |
| SHACL Core | ✅ | ✅ | ✅ | ✅ | ✅ |
| SHACL-SPARQL Rules | ✅ | — | ✅ | ✅ | — |
| Datalog (basic) | ✅ | ✅ | ✅ | ✅ | ✅ |
| Datalog (magic sets) | ✅ | — | — | ✅ | ✅ |
| Datalog (WFS/tabling) | ✅ | — | — | ✅ | ✅ |
| Datalog (aggregation) | ✅ | — | — | ✅ | — |
| HTAP Storage | ✅ | — | — | ✅ | ✅ |
| RDF-star | ✅ | — | — | ✅ | ✅ |
| JSON-LD Framing | ✅ | ✅ | — | ✅ | ✅ |
| CONSTRUCT/DESCRIBE/ASK Views | ✅ | — | — | ✅ | ✅ |
| Vector + Hybrid Search | ✅ | — | — | ✅ | ✅ |
| GraphRAG | ✅ | — | — | ✅ | ✅ |
| PageRank | ✅ | — | — | ✅ | ✅ |
| Probabilistic Reasoning | ✅ | — | ✅ | ✅ | ✅ |
| Bidirectional Relay (CDC) | ✅ | — | ✅ | ✅ | ✅ |
| CDC Subscriptions | ✅ | — | — | ✅ | ✅ |
| EXPLAIN SPARQL | ✅ | — | — | ✅ | ✅ |
| Cost-based Federation | ✅ | — | ✅ | ✅ | ✅ |
| Citus Integration | ✅ | — | — | ✅ | ✅ |
| Multi-tenant Graphs | ✅ | — | — | ✅ | ✅ |
| GeoSPARQL | ✅ | — | — | ✅ | ✅ |
| Full-text Search | ✅ | — | — | ✅ | — |
| Temporal/Provenance | ✅ | — | ✅ | ✅ | ✅ |
| R2RML | ✅ | — | ✅ | ✅ | ✅ |
| Cypher/LPG → RDF | ✅ | — | — | — | — |
| KG Embeddings | ✅ | — | ✅ | ✅ | ✅ |
| NL-to-SPARQL | ✅ | — | ✅ | ✅ | ✅ |
| AI Agent Integration | ✅ | — | — | — | — |
| Record Linkage | ✅ | — | ✅ | ✅ | ✅ |
| SKOS | — | — | ✅ | — | ✅ |
| DCTERMS/Schema.org/FOAF | — | — | ✅ | — | ✅ |
| OWL 2 RL/EL/QL | ✅ | — | — | ✅ | — |
| Arrow Flight | — | — | — | ✅ | — |
| Lattice Datalog | ✅ | — | — | ✅ | — |
| WCOJ | ✅ | — | — | — | ✅ |

---

## Missing Tutorials (Prioritized)

1. **Migrate from Neo4j to pg_ripple** — highest demand from the target audience
2. **Build a RAG Pipeline from Scratch** (end-to-end with LangChain) — AI persona critical path
3. **Kubernetes Production Deployment** — ops persona critical path
4. **GDPR Compliance and Right-to-Erasure** — enterprise demand
5. **Multi-Tenant SaaS Knowledge Graph** — production pattern
6. **Real-Time Dashboard with CDC** — streaming use case

---

## Structural Assessment

### What Works Well

- **Navigation structure**: Clear taxonomy (Evaluate → Getting Started → Features → Cookbook → Operations → Reference)
- **SUMMARY.md**: Comprehensive, well-organized, no orphan pages
- **Tone**: Warm, direct, peer-level — avoids academic jargon
- **Code examples**: Present on 94%+ of pages with correct language tags
- **Error catalog**: Comprehensive PT001–PT799 with cause and fix for every code
- **Troubleshooting**: Real failure scenarios with diagnostic steps
- **Blog**: 39 posts covering all major features
- **Conformance pages**: W3C, Jena, WatDiv, LUBM, OWL 2 RL all documented

### What Needs Work

- **Reference completeness**: GUC and HTTP API coverage are the biggest gaps
- **Cross-linking**: Blog → feature pages and vice versa
- **Callout consistency**: Mix of admonish plugin and markdown blockquotes
- **Hello World correctness**: The most critical page has a broken example
