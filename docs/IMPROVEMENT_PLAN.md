# pg_ripple Documentation — Improvement Plan

> **Date**: 2026-05-11
> **Input**: [GAP_ANALYSIS.md](GAP_ANALYSIS.md)
> **Sequencing**: Critical → High → Medium → Low. Within each tier, items are ordered by impact and dependency.

---

## Critical Priority — Must Fix Before v1.0.0

---

## DOC-001: Fix Hello World Buggy SPARQL Query

**Priority**: Critical
**Effort**: S (< 1 day)
**Owner Area**: Getting Started
**Blocking**: None — but blocks user trust

### What exists today

Step 3 in `hello-world.md` reuses `?movie` as both the entity variable and the name binding:
```sparql
?movie schema:name ?movie .  -- variable collision
```
Step 3's explanatory text also has confusing variable names (`?director` is actually a person name).

### What needs to change

- Rename `?movie` in the name binding to `?movieName`
- Rename `?director` to `?directorName` for consistency with Step 4
- Verify that all three queries in the walkthrough produce the described results

### Definition of Done

- [ ] Step 3 query uses distinct variable names (`?movieName`, `?directorName`)
- [ ] Step 3 explanatory text matches the query output
- [ ] All three queries (Steps 3, 4, 5) are internally consistent

---

## DOC-002: Complete GUC Reference — Full Coverage

**Priority**: Critical
**Effort**: L (3–7 days)
**Owner Area**: Reference
**Blocking**: DOC-030 (production checklist depends on GUC knowledge)

### What exists today

41 of 197 GUC parameters documented. Missing: PageRank (28), Datalog (33), Storage (44), SPARQL (22), Observability (6), SHACL (3+), LLM (14+).

### What needs to change

Audit every `GucSetting` definition in `src/gucs/`. For each, add to `guc-reference.md`:
- Parameter name (with `pg_ripple.` prefix)
- Type, default, range
- Description
- When to change it
- Interaction notes where relevant

Organize by subsystem with clear section headings.

### Definition of Done

- [ ] Every `pub static` GucSetting in `src/gucs/*.rs` has a corresponding entry
- [ ] Each entry has: type, default, range, description, and "when to change" guidance
- [ ] Subsystem groupings match source module structure

---

## DOC-003: Complete HTTP API Reference

**Priority**: Critical
**Effort**: M (1–3 days)
**Owner Area**: Reference
**Blocking**: DOC-041 (OpenAPI spec)

### What exists today

5 endpoints documented. 55+ routes exist in the source.

### What needs to change

Document all routes from `pg_ripple_http/src/routing/`:
- Datalog API (24 routes)
- PageRank/Centrality (10 routes)
- Confidence (4 routes)
- Streaming/Subscribe routes
- Admin routes (`/openapi.yaml`, `/explorer`, `/void`, `/service`, `/health/ready`, `/metrics/extension`)
- Arrow Flight (`/flight/do_get`)

Each endpoint needs: method, path, request/response format, auth requirements, example.

### Definition of Done

- [ ] Every route in `routing/*.rs` has a corresponding entry
- [ ] Each entry has method, path, description, request/response examples
- [ ] Auth requirements noted for each endpoint

---

## DOC-004: Audit and Validate SQL Function Reference

**Priority**: Critical
**Effort**: M (1–3 days)
**Owner Area**: Reference
**Blocking**: None

### What exists today

156 functions documented. 298 `#[pg_extern]` definitions in source (many are internal).

### What needs to change

- Cross-reference every `#[pg_extern]` against the documented list
- Identify which are public-facing SQL functions vs. internal helpers
- Add any missing public functions
- Verify signatures match source code

### Definition of Done

- [ ] Every public `pg_ripple.*` SQL function is documented
- [ ] No signature mismatches between docs and source
- [ ] Count header updated to reflect actual number

---

## High Priority — Should Fix Before v1.0.0

---

## DOC-010: Add Production-Readiness Checklist

**Priority**: High
**Effort**: M (1–3 days)
**Owner Area**: Operations
**Blocking**: DOC-002 (needs complete GUC knowledge)

### What exists today

Individual operations pages cover security, HA, monitoring, etc. No consolidated checklist.

### What needs to change

Create `docs/src/operations/production-checklist.md` with sections:
- Pre-deployment (PostgreSQL config, shared_preload_libraries, GUC tuning)
- Security (RLS, auth tokens, SSRF protection, TLS)
- Monitoring (Prometheus alerts, key metrics, dashboard examples)
- Backup and recovery (pg_dump, WAL archiving, PITR)
- Performance (merge workers, cache sizing, vacuum tuning)
- Upgrade path (migration scripts, compatibility matrix)

Add to SUMMARY.md.

### Definition of Done

- [ ] Checklist page exists with actionable items
- [ ] Each item links to the relevant detailed page
- [ ] Added to SUMMARY.md under Operations

---

## DOC-011: Landing Page Persona Decision Tree

**Priority**: High
**Effort**: S (< 1 day)
**Owner Area**: Getting Started
**Blocking**: None

### What exists today

The landing page has "Next steps" with persona-targeted links. No visual decision tree.

### What needs to change

Add a clear "Start here" section with three paths:
1. **PostgreSQL DBA** → Installation → Hello World → Operations
2. **Data/AI Engineer** → AI Overview → RAG Pipeline → Grounded Chatbot
3. **Semantic Web Engineer** → Key Concepts → SPARQL → Datalog → SHACL

Use a table or structured layout that is immediately scannable.

### Definition of Done

- [ ] Three-persona decision matrix visible within the first screenful
- [ ] Each path has 3-4 linked steps
- [ ] Existing "Next steps" section preserved

---

## DOC-012: Add Blog Cross-Links to Feature Pages

**Priority**: High
**Effort**: M (1–3 days)
**Owner Area**: Features / Blog
**Blocking**: None

### What exists today

Blog posts exist for most features but are not linked from feature pages.

### What needs to change

For each feature page, add a "Further reading" section linking to the relevant blog post(s):

| Feature Page | Blog Post(s) |
|---|---|
| `key-concepts.md` | `dictionary-encoding-integer-joins.md`, `vertical-partitioning-explained.md` |
| `querying-with-sparql.md` | `sparql-to-sql-translation.md`, `property-paths-recursive-ctes.md` |
| `reasoning-and-inference.md` | `datalog-inside-postgresql.md`, `builtin-reasoning-rules-explained.md` |
| `advanced-inference.md` | `leapfrog-triejoin.md`, `well-founded-semantics.md`, `magic-sets-goal-directed.md` |
| `validating-data-quality.md` | `shacl-data-quality.md` |
| `live-views-and-subscriptions.md` | `construct-views-live-transformations.md`, `ivm-pg-trickle-integration.md` |
| `vector-and-hybrid-search.md` | `vector-sparql-hybrid-search.md` |
| `pagerank.md` | `pagerank.md` (blog) |
| `uncertain-knowledge.md` | `probabilistic-datalog.md` |
| `cdc-subscriptions.md` | `cdc-knowledge-graphs.md` |
| `geospatial.md` | `geosparql-postgis-spatial.md` |
| `graphrag.md` | `graphrag-knowledge-export.md` |
| `multi-tenant-graphs.md` | `multi-tenant-knowledge-graphs.md` |
| `nl-to-sparql.md` | `natural-language-to-sparql.md` |
| `record-linkage.md` | `neuro-symbolic-entity-resolution.md`, `owl-sameas-entity-resolution.md` |
| `r2rml.md` | `r2rml-relational-to-graph.md` |
| `temporal-and-provenance.md` | `temporal-time-travel-queries.md`, `provenance-tracking-prov-o.md` |
| `exporting-and-sharing.md` | `json-ld-framing-nested-json.md` |
| `storing-knowledge.md` | `rdf-star-statements-about-statements.md` |

### Definition of Done

- [ ] Every feature page with a matching blog post has a "Further reading" or "Deep dive" section
- [ ] Links use relative paths

---

## DOC-013: Add "What You'll Learn" to Tutorial Segments

**Priority**: High
**Effort**: S (< 1 day)
**Owner Area**: Getting Started
**Blocking**: None

### What exists today

Tutorial segments have time estimates but no "What you'll learn" preamble.

### What needs to change

Add a brief (2-3 bullet) "What you'll learn" box at the top of each segment.

### Definition of Done

- [ ] Each of the 4 tutorial segments has a learning objective summary
- [ ] Uses consistent formatting (admonish or blockquote)

---

## DOC-014: Standardize Callout System

**Priority**: High
**Effort**: S (< 1 day)
**Owner Area**: All
**Blocking**: None

### What exists today

Mix of `````admonish note````` plugin syntax and `> **Note**:` markdown blockquotes.

### What needs to change

Standardize on the mdBook admonish plugin (`admonish note`, `admonish warning`, `admonish tip`, `admonish important`) throughout. This provides consistent styling.

### Definition of Done

- [ ] No `> **Note**:` / `> **Warning**:` raw blockquotes remain (replaced with admonish)
- [ ] Spot-check 10+ pages for consistency

---

## Medium Priority — v1.0.0 Nice-to-Have

---

## DOC-020: Create "Migrate from Neo4j" Recipe

**Priority**: Medium
**Effort**: M (1–3 days)
**Owner Area**: Cookbook
**Blocking**: None

### What exists today

`features/lpg-mapping.md` documents Cypher → RDF translation patterns.

### What needs to change

Create `cookbook/migrate-from-neo4j.md` — a step-by-step recipe covering:
- Export from Neo4j (APOC, CSV, JSON)
- Property-graph → RDF mapping patterns
- pg_ripple import
- Query translation (Cypher → SPARQL cheat sheet)
- Validation

### Definition of Done

- [ ] Recipe page exists with end-to-end worked example
- [ ] Added to SUMMARY.md and cookbook index
- [ ] Cross-linked from `features/lpg-mapping.md`

---

## DOC-021: Create "Build a RAG Pipeline from Scratch" Tutorial

**Priority**: Medium
**Effort**: M (1–3 days)
**Owner Area**: Cookbook / AI
**Blocking**: None

### What exists today

`user-guide/rag-pipeline.md` documents the API. `cookbook/grounded-chatbot.md` shows a recipe. Neither starts from zero.

### What needs to change

Create a unified end-to-end tutorial: Docker setup → load domain data → configure embeddings → run `rag_context()` → call LLM → evaluate answer quality. Include LangChain/Python example.

### Definition of Done

- [ ] Tutorial page exists with complete code from Docker to working chatbot
- [ ] Includes LangChain Python integration
- [ ] Added to SUMMARY.md

---

## DOC-022: Add Architecture Diagrams (Mermaid)

**Priority**: Medium
**Effort**: M (1–3 days)
**Owner Area**: Features
**Blocking**: None

### What exists today

ASCII diagram on landing page. No Mermaid diagrams in feature pages.

### What needs to change

Add Mermaid diagrams to:
- HTAP storage architecture (delta/main/merge flow)
- CDC pipeline (write → delta → NOTIFY → consumer)
- Federation query execution (local + remote SERVICE planning)

### Definition of Done

- [ ] 3+ Mermaid diagrams added to feature deep-dive pages
- [ ] Diagrams render correctly in mdBook

---

## DOC-023: Add "Limits and Quotas" Reference Page

**Priority**: Medium
**Effort**: S (< 1 day)
**Owner Area**: Reference
**Blocking**: None

### What exists today

No dedicated page. Limits are scattered across GUC docs and operations pages.

### What needs to change

Create `docs/src/reference/limits.md`:
- Tested triple counts (100M+)
- Max predicate count
- SPARQL query complexity limits (algebra depth, triple patterns)
- Federation limits
- Statement ID sequence capacity
- Dictionary size

### Definition of Done

- [ ] Page exists with concrete numbers and citations
- [ ] Added to SUMMARY.md

---

## DOC-024: GUC Tuning Profiles

**Priority**: Medium
**Effort**: S (< 1 day)
**Owner Area**: Operations
**Blocking**: DOC-002

### What exists today

`operations/tuning.md` and `operations/performance.md` exist.

### What needs to change

Add a "Tuning Profiles" section with recommended GUC settings for:
- **OLTP** (many small queries, low latency)
- **OLAP** (few large queries, bulk inference)
- **Small instances** (< 1M triples, 512MB RAM)
- **Large instances** (> 50M triples, 16GB+ RAM)

### Definition of Done

- [ ] 4 named profiles with GUC settings
- [ ] Each profile explains the trade-offs

---

## DOC-025: SHACL Quick-Reference Card

**Priority**: Medium
**Effort**: S (< 1 day)
**Owner Area**: Reference
**Blocking**: None

### What exists today

`reference/shacl.md` is comprehensive. No quick-reference card.

### What needs to change

Add a one-page "SHACL cheat sheet" with all constraint components, their syntax, and one-line examples.

### Definition of Done

- [ ] Table listing all supported SHACL constraints with syntax
- [ ] Linked from feature page and reference page

---

## DOC-030: v1.0.0 Announcement Blog Post

**Priority**: Medium
**Effort**: M (1–3 days)
**Owner Area**: Blog
**Blocking**: v1.0.0 release

### What exists today

No announcement post.

### What needs to change

Write `blog/v1-what-is-new.md`: what shipped, who it's for, key numbers, migration guidance, roadmap beyond v1.0.

### Definition of Done

- [ ] Blog post covers all major v1.0 features
- [ ] Links to relevant docs pages
- [ ] Migration guidance for users on v0.99.x

---

## Low Priority — Post-v1.0.0

---

## DOC-040: Blog Freshness Audit

**Priority**: Low
**Effort**: M (1–3 days)
**Owner Area**: Blog
**Blocking**: None

### What exists today

39 blog posts. Some may reference older API patterns.

### What needs to change

Review each post for API accuracy. Add version badges or update notes where patterns have changed.

### Definition of Done

- [ ] Every blog post reviewed
- [ ] Outdated examples annotated with version notes

---

## DOC-041: OpenAPI Spec Generation

**Priority**: Low
**Effort**: M (1–3 days)
**Owner Area**: Reference
**Blocking**: DOC-003

### What exists today

`/openapi.yaml` endpoint exists in the HTTP service.

### What needs to change

Verify the generated spec is complete, or generate a comprehensive one from the routing code. Host it on the docs site.

### Definition of Done

- [ ] OpenAPI spec covers all HTTP endpoints
- [ ] Spec is accessible from the docs

---

## DOC-042: Expand Glossary

**Priority**: Low
**Effort**: S (< 1 day)
**Owner Area**: Reference
**Blocking**: None

### What exists today

Glossary exists with core terms.

### What needs to change

Add pg_ripple-specific terms: VP table, HTAP split, merge worker, statement ID, dictionary encoding, rare-predicate consolidation.

### Definition of Done

- [ ] 10+ pg_ripple-specific terms added
- [ ] Each with a 1-2 sentence definition

---

## DOC-043: Expand FAQ

**Priority**: Low
**Effort**: S (< 1 day)
**Owner Area**: Reference
**Blocking**: None

### What exists today

FAQ exists with common questions.

### What needs to change

Add questions based on common patterns:
- "Can I use pg_ripple with PostGIS?"
- "How do I back up and restore?"
- "What happens when a VP table gets very large?"
- "Can I run pg_ripple on read replicas?"

### Definition of Done

- [ ] 5+ new FAQ entries
- [ ] Each with a direct, actionable answer

---

## Delivery Sequence

### Immediate (this session)

1. **DOC-001**: Fix Hello World buggy query
2. **DOC-011**: Landing page persona decision tree
3. **DOC-013**: Tutorial "What you'll learn" sections
4. **DOC-012**: Blog cross-links on key feature pages

### Next sprint

5. **DOC-002**: Complete GUC reference
6. **DOC-003**: Complete HTTP API reference
7. **DOC-004**: Audit SQL function reference
8. **DOC-010**: Production-readiness checklist
9. **DOC-014**: Standardize callout system

### Following sprint

10. **DOC-020–025**: Medium-priority content additions
11. **DOC-030**: v1.0.0 announcement post

### Ongoing

12. **DOC-040–043**: Low-priority polish
