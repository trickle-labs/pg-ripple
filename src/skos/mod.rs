//! pg_ripple SQL API — SKOS Support, Named Bundle API & Graph Intelligence (v0.98.0)
//! Extended in v0.99.0: DCTERMS, Schema.org, FOAF vocabulary bundles.
//!
//! Exposes the following SQL functions in the `pg_ripple` schema:
//!
//! ## SKOS rule loading
//! - `pg_ripple.load_builtin_rules(name)` — extended to support `'skos'`, `'skos-transitive'`, `'skosxl'`
//!   (delegated to `datalog_api::load_rules_builtin`; the SKOS names are wired into `builtins.rs`)
//!
//! ## Named Bundle API (RB-01)
//! - `pg_ripple.load_datalog_bundle(name, named_graph)` — named, versioned Datalog bundle activation
//! - `pg_ripple.load_shape_bundle(name)` — named SHACL shape bundle activation with implicit deps
//! - View `pg_ripple.active_datalog_bundles` — created in schema/rls.rs
//!
//! ## SKOS integrity / validation
//! - `pg_ripple.validate_skos()` — integrity report wrapper for `"skos-integrity"` constraint rules
//!
//! ## SKOS SQL Helpers (SKOS-05)
//! - `pg_ripple.skos_ancestors(concept_iri, scheme_iri)` — `broaderTransitive` closure
//! - `pg_ripple.skos_descendants(concept_iri, scheme_iri)` — `narrowerTransitive` closure
//! - `pg_ripple.skos_label(concept_iri, lang)` — `prefLabel` lookup
//! - `pg_ripple.skos_related(concept_iri)` — all `semanticRelation` sub-property links
//! - `pg_ripple.skos_siblings(concept_iri)` — co-narrower concepts sharing a parent
//!
//! ## Contradiction explanation (RB-02)
//! - `pg_ripple.explain_contradiction(subject_iri, named_graph, max_depth, mode)` — minimal hitting set
//! - `pg_ripple.explain_contradiction_json(subject_iri, named_graph, max_depth, mode)` — JSONB variant
//!
//! ## Coverage map (RB-04)
//! - `pg_ripple.coverage_map(named_graphs, topic_predicate, top_k)` — per-topic coverage SRF
//! - `pg_ripple.refresh_coverage_map(target_graph, named_graphs)` — write pgc:CoverageMap triples
//!
//! ## Schema.org SQL Helpers (v0.99.0)
//! - `pg_ripple.schema_type_ancestors(iri)` — all Schema.org type ancestors via type-hierarchy closure
//!
//! ## FOAF SQL Helpers (v0.99.0)
//! - `pg_ripple.foaf_persons()` — all foaf:Person IRIs with their foaf:name labels
//!
//! # Module layout (v0.114.0)
//!
//! | Sub-module         | Contents |
//! |---|---|
//! | `bundle`           | `load_datalog_bundle`, `load_shape_bundle`, integrity loaders |
//! | `inference`        | `SKOS_INTEGRITY_RULES`, `validate_skos` |
//! | `broader_narrower` | `skos_ancestors`, `skos_descendants`, `skos_label`, `skos_related`, `skos_siblings`, `explain_contradiction`, `coverage_map` |
//! | `export`           | `refresh_coverage_map`, `schema_type_ancestors`, `foaf_persons` |

pub mod broader_narrower;
pub mod bundle;
pub mod export;
pub mod inference;
