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

#[pgrx::pg_schema]
mod pg_ripple {
    use pgrx::prelude::*;

    // ── RB-01: load_datalog_bundle ────────────────────────────────────────────

    /// Activate a named Datalog rule bundle for the given named graph.
    ///
    /// Calls `load_builtin_rules()` internally and records the activation in
    /// `_pg_ripple.datalog_bundles`.  Idempotent: re-loading updates `loaded_at`.
    ///
    /// Supported bundle names: `'skos'`, `'skos-transitive'`, `'skosxl'`, `'rdfs'`, `'owl-rl'`,
    /// `'dcterms'`, `'schema'`, `'foaf'` (v0.99.0).
    ///
    /// The `"skosxl"` bundle automatically activates `"skos"` as a dependency.
    /// The `"schema"` bundle enables cross-vocabulary bridges to `foaf:` and `dcterms:`.
    #[pg_extern]
    fn load_datalog_bundle(bundle_name: &str, named_graph: default!(&str, "''")) {
        crate::datalog::builtins::register_standard_prefixes();

        // Resolve dependencies first.
        if bundle_name == "skosxl" {
            // skosxl depends on skos
            activate_bundle_internal("skos", named_graph);
        }

        activate_bundle_internal(bundle_name, named_graph);
    }

    /// Internal: load rules and record in datalog_bundles catalog.
    fn activate_bundle_internal(bundle_name: &str, named_graph: &str) {
        // Load the underlying Datalog rules (idempotent via ON CONFLICT DO NOTHING).
        let text = match crate::datalog::builtins::get_builtin_rules(bundle_name) {
            Ok(t) => t,
            Err(e) => pgrx::error!("load_datalog_bundle: {e}"),
        };
        let rule_set_ir = match crate::datalog::parse_rules(text, bundle_name) {
            Ok(rs) => rs,
            Err(e) => pgrx::error!("load_datalog_bundle: rule parse error: {e}"),
        };
        crate::datalog::store_rules(bundle_name, &rule_set_ir.rules);

        // Record activation in the bundle catalog.
        let _ = Spi::run_with_args(
            "INSERT INTO _pg_ripple.datalog_bundles (bundle_name, bundle_version, loaded_at, named_graph) \
             VALUES ($1, 1, now(), $2) \
             ON CONFLICT (bundle_name, named_graph) DO UPDATE \
             SET bundle_version = _pg_ripple.datalog_bundles.bundle_version, \
                 loaded_at = now()",
            &[
                pgrx::datum::DatumWithOid::from(bundle_name),
                pgrx::datum::DatumWithOid::from(named_graph),
            ],
        );
    }

    // ── RB-01: load_shape_bundle ──────────────────────────────────────────────

    /// Activate a named SHACL shape bundle, resolving Datalog dependencies first.
    ///
    /// Supported shape bundles: `'skos-integrity'` (10 validators, W3C S9/S13/S14/S27/S37/S46).
    ///
    /// `load_shape_bundle('skos-integrity')` automatically activates
    /// `'skos-transitive'` if not already active.
    #[pg_extern]
    fn load_shape_bundle(bundle_name: &str) {
        match bundle_name {
            "skos-integrity" => {
                // Ensure skos-transitive Datalog bundle is active.
                let already_active = Spi::get_one_with_args::<i64>(
                    "SELECT count(*) FROM _pg_ripple.datalog_bundles WHERE bundle_name = $1",
                    &[pgrx::datum::DatumWithOid::from("skos-transitive")],
                )
                .unwrap_or(None)
                .unwrap_or(0)
                    > 0;

                if !already_active {
                    pgrx::info!(
                        "load_shape_bundle('skos-integrity'): implicitly activating 'skos-transitive' Datalog bundle"
                    );
                    load_datalog_bundle("skos-transitive", "");
                }

                // Load the 10 SKOS-integrity constraint rules.
                let rules_text = SKOS_INTEGRITY_RULES;
                let rule_set_ir = match crate::datalog::parse_rules(rules_text, "skos-integrity") {
                    Ok(rs) => rs,
                    Err(e) => pgrx::error!("load_shape_bundle('skos-integrity'): parse error: {e}"),
                };
                crate::datalog::store_rules("skos-integrity", &rule_set_ir.rules);

                // Record shape bundle activation in the bundle catalog.
                let _ = Spi::run_with_args(
                    "INSERT INTO _pg_ripple.datalog_bundles \
                         (bundle_name, bundle_version, loaded_at, named_graph) \
                     VALUES ($1, 1, now(), '') \
                     ON CONFLICT (bundle_name, named_graph) DO UPDATE \
                     SET loaded_at = now()",
                    &[pgrx::datum::DatumWithOid::from("skos-integrity")],
                );
            }
            "dcterms-integrity" => load_dcterms_integrity_bundle(),
            "schema-integrity" => load_schema_integrity_bundle(),
            "foaf-integrity" => load_foaf_integrity_bundle(),
            _ => pgrx::error!(
                "unknown shape bundle '{bundle_name}'; supported: \
                 skos-integrity, dcterms-integrity, schema-integrity, foaf-integrity"
            ),
        }
    }

    // ── v0.99.0: load_shape_bundle for dcterms-integrity ─────────────────────
    /// Load the DCTERMS integrity shape bundle (8 validators).
    fn load_dcterms_integrity_bundle() {
        crate::datalog::builtins::register_standard_prefixes();
        let rules_text = match crate::datalog::builtins::get_builtin_rules("dcterms-integrity") {
            Ok(t) => t,
            Err(e) => pgrx::error!("load_shape_bundle('dcterms-integrity'): {e}"),
        };
        let rule_set_ir = match crate::datalog::parse_rules(rules_text, "dcterms-integrity") {
            Ok(rs) => rs,
            Err(e) => {
                pgrx::error!("load_shape_bundle('dcterms-integrity'): parse error: {e}")
            }
        };
        crate::datalog::store_rules("dcterms-integrity", &rule_set_ir.rules);
        let _ = Spi::run_with_args(
            "INSERT INTO _pg_ripple.datalog_bundles \
                 (bundle_name, bundle_version, loaded_at, named_graph) \
             VALUES ($1, 1, now(), '') \
             ON CONFLICT (bundle_name, named_graph) DO UPDATE \
             SET loaded_at = now()",
            &[pgrx::datum::DatumWithOid::from("dcterms-integrity")],
        );
    }

    /// Load the Schema.org integrity shape bundle (6 validators).
    fn load_schema_integrity_bundle() {
        crate::datalog::builtins::register_standard_prefixes();
        let rules_text = match crate::datalog::builtins::get_builtin_rules("schema-integrity") {
            Ok(t) => t,
            Err(e) => pgrx::error!("load_shape_bundle('schema-integrity'): {e}"),
        };
        let rule_set_ir = match crate::datalog::parse_rules(rules_text, "schema-integrity") {
            Ok(rs) => rs,
            Err(e) => {
                pgrx::error!("load_shape_bundle('schema-integrity'): parse error: {e}")
            }
        };
        crate::datalog::store_rules("schema-integrity", &rule_set_ir.rules);
        let _ = Spi::run_with_args(
            "INSERT INTO _pg_ripple.datalog_bundles \
                 (bundle_name, bundle_version, loaded_at, named_graph) \
             VALUES ($1, 1, now(), '') \
             ON CONFLICT (bundle_name, named_graph) DO UPDATE \
             SET loaded_at = now()",
            &[pgrx::datum::DatumWithOid::from("schema-integrity")],
        );
    }

    /// Load the FOAF integrity shape bundle (5 validators).
    fn load_foaf_integrity_bundle() {
        crate::datalog::builtins::register_standard_prefixes();
        let rules_text = match crate::datalog::builtins::get_builtin_rules("foaf-integrity") {
            Ok(t) => t,
            Err(e) => pgrx::error!("load_shape_bundle('foaf-integrity'): {e}"),
        };
        let rule_set_ir = match crate::datalog::parse_rules(rules_text, "foaf-integrity") {
            Ok(rs) => rs,
            Err(e) => {
                pgrx::error!("load_shape_bundle('foaf-integrity'): parse error: {e}")
            }
        };
        crate::datalog::store_rules("foaf-integrity", &rule_set_ir.rules);
        let _ = Spi::run_with_args(
            "INSERT INTO _pg_ripple.datalog_bundles \
                 (bundle_name, bundle_version, loaded_at, named_graph) \
             VALUES ($1, 1, now(), '') \
             ON CONFLICT (bundle_name, named_graph) DO UPDATE \
             SET loaded_at = now()",
            &[pgrx::datum::DatumWithOid::from("foaf-integrity")],
        );
    }

    // ── SKOS-IC: integrity constraint rules ───────────────────────────────────

    /// SKOS integrity constraint rules (10 validators, S9/S13/S14/S27/S37/S46).
    ///
    /// These are expressed as Datalog constraint rules that produce violation
    /// triples in `_pg_ripple.shacl_violations` format when loaded via
    /// `load_shape_bundle('skos-integrity')`.
    const SKOS_INTEGRITY_RULES: &str = r#"
# SKOS-IC-01 (S9): skos:ConceptScheme disjoint with skos:Concept
# Firing: subject typed as both is a violation.
# Represented as a constraint head: error head causes violation record.
?x skos:ic_violation "SKOS-IC-01: resource is both skos:ConceptScheme and skos:Concept" :-
    ?x rdf:type skos:ConceptScheme,
    ?x rdf:type skos:Concept .

# SKOS-IC-02 (S13): prefLabel and altLabel must not share the same literal+lang
?x skos:ic_violation "SKOS-IC-02: same literal used for both skos:prefLabel and skos:altLabel" :-
    ?x skos:prefLabel ?l,
    ?x skos:altLabel  ?l .

# SKOS-IC-03 (S13): prefLabel and hiddenLabel must not share literal+lang
?x skos:ic_violation "SKOS-IC-03: same literal used for both skos:prefLabel and skos:hiddenLabel" :-
    ?x skos:prefLabel   ?l,
    ?x skos:hiddenLabel ?l .

# SKOS-IC-04 (S13): altLabel and hiddenLabel must not share literal+lang
?x skos:ic_violation "SKOS-IC-04: same literal used for both skos:altLabel and skos:hiddenLabel" :-
    ?x skos:altLabel    ?l,
    ?x skos:hiddenLabel ?l .

# SKOS-IC-05 (S14): at most one prefLabel per language tag (checked by validate_skos via SQL)

# SKOS-IC-06 (S27): skos:related disjoint with skos:broaderTransitive
?x skos:ic_violation "SKOS-IC-06: concept linked by both skos:related and skos:broaderTransitive (S27 violation)" :-
    ?x skos:related           ?y,
    ?x skos:broaderTransitive ?y .

# SKOS-IC-07 (S37): skos:Collection disjoint with skos:Concept
?x skos:ic_violation "SKOS-IC-07: resource is both skos:Collection and skos:Concept" :-
    ?x rdf:type skos:Collection,
    ?x rdf:type skos:Concept .

# SKOS-IC-08 (S37): skos:Collection disjoint with skos:ConceptScheme
?x skos:ic_violation "SKOS-IC-08: resource is both skos:Collection and skos:ConceptScheme" :-
    ?x rdf:type skos:Collection,
    ?x rdf:type skos:ConceptScheme .

# SKOS-IC-09 (S46): exactMatch disjoint with broadMatch
?x skos:ic_violation "SKOS-IC-09: concepts linked by both skos:exactMatch and skos:broadMatch (S46 violation)" :-
    ?x skos:exactMatch ?y,
    ?x skos:broadMatch ?y .

# SKOS-IC-10 (S46): exactMatch disjoint with relatedMatch
?x skos:ic_violation "SKOS-IC-10: concepts linked by both skos:exactMatch and skos:relatedMatch (S46 violation)" :-
    ?x skos:exactMatch   ?y,
    ?x skos:relatedMatch ?y .
"#;

    // ── SKOS-IC: validate_skos ────────────────────────────────────────────────

    /// Run SKOS integrity checks and return violations.
    ///
    /// Requires the `'skos-integrity'` shape bundle to be loaded.
    /// Returns one row per violation with violation ID, subject IRI, and message.
    ///
    /// Also checks SKOS-IC-05 (S14: at most one prefLabel per language) via SQL
    /// since it requires aggregation not expressible in basic Datalog.
    #[pg_extern]
    fn validate_skos() -> TableIterator<
        'static,
        (
            name!(violation_id, String),
            name!(subject, String),
            name!(message, String),
        ),
    > {
        let mut rows: Vec<(String, String, String)> = Vec::new();

        // IC-01 through IC-04, IC-06 through IC-10: query _pg_ripple.vp_rare
        // for skos:ic_violation triples produced by the constraint rules.
        let ic_via_rules = Spi::connect(|client| {
            // Query the dictionary for the skos:ic_violation predicate.
            let pred_id = client
                .select(
                    "SELECT id FROM _pg_ripple.dictionary \
                     WHERE value = 'http://www.w3.org/2004/02/skos/core#ic_violation'",
                    None,
                    &[],
                )
                .unwrap_or_else(|_| pgrx::error!("validate_skos: dictionary query failed"))
                .next()
                .and_then(|row| row.get::<i64>(1).ok().flatten());

            let pred_id = match pred_id {
                Some(id) => id,
                None => return Vec::new(), // No violations predicate means no constraint rules were fired.
            };

            client
                .select(
                    "SELECT d_s.value, d_o.value \
                     FROM _pg_ripple.vp_rare vp \
                     JOIN _pg_ripple.dictionary d_s ON d_s.id = vp.s \
                     JOIN _pg_ripple.dictionary d_o ON d_o.id = vp.o \
                     WHERE vp.p = $1 AND vp.source = 1",
                    None,
                    &[pgrx::datum::DatumWithOid::from(pred_id)],
                )
                .unwrap_or_else(|_| pgrx::error!("validate_skos: violation query failed"))
                .map(|row| {
                    let subject = row.get::<String>(1).ok().flatten().unwrap_or_default();
                    let message = row.get::<String>(2).ok().flatten().unwrap_or_default();
                    // Extract violation ID from message prefix (e.g. "SKOS-IC-01: ...").
                    let vid = message
                        .split(':')
                        .next()
                        .unwrap_or("SKOS-IC-??")
                        .trim()
                        .to_string();
                    (vid, subject, message)
                })
                .collect::<Vec<_>>()
        });
        rows.extend(ic_via_rules);

        // IC-05 (S14): at most one prefLabel per language per concept.
        // Detectable only via GROUP BY aggregation.
        let ic05 = Spi::connect(|client| {
            let pred_id = client
                .select(
                    "SELECT id FROM _pg_ripple.dictionary \
                     WHERE value = 'http://www.w3.org/2004/02/skos/core#prefLabel'",
                    None,
                    &[],
                )
                .unwrap_or_else(|_| pgrx::error!("validate_skos: ic05 dict query failed"))
                .next()
                .and_then(|row| row.get::<i64>(1).ok().flatten());

            let pred_id = match pred_id {
                Some(id) => id,
                None => return Vec::new(),
            };

            // Find subjects with more than one prefLabel literal sharing the same language tag.
            // We check this by looking at the literal values – simplified: flag any concept
            // with more than one prefLabel object with the same language datatype.
            client
                .select(
                    "SELECT d_s.value, count(DISTINCT d_o.id) AS cnt \
                     FROM _pg_ripple.vp_rare vp \
                     JOIN _pg_ripple.dictionary d_s ON d_s.id = vp.s \
                     JOIN _pg_ripple.dictionary d_o ON d_o.id = vp.o \
                     JOIN _pg_ripple.dictionary_literals dl ON dl.id = d_o.id \
                     WHERE vp.p = $1 \
                     GROUP BY d_s.value, dl.lang \
                     HAVING count(DISTINCT d_o.id) > 1",
                    None,
                    &[pgrx::datum::DatumWithOid::from(pred_id)],
                )
                .unwrap_or_else(|_| pgrx::error!("validate_skos: ic05 query failed"))
                .map(|row| {
                    let subject = row.get::<String>(1).ok().flatten().unwrap_or_default();
                    (
                        "SKOS-IC-05".to_string(),
                        subject.clone(),
                        format!(
                            "SKOS-IC-05: concept <{subject}> has multiple skos:prefLabel values with the same language tag (S14 violation)"
                        ),
                    )
                })
                .collect::<Vec<_>>()
        });
        rows.extend(ic05);

        TableIterator::new(rows)
    }

    // ── SKOS-05: SQL Helper Functions ─────────────────────────────────────────

    /// Return the `skos:broaderTransitive` closure for a concept.
    ///
    /// Uses a live `WITH RECURSIVE … CYCLE` query over the VP tables rather than
    /// materialised triples, so it is always up-to-date even before Datalog materialisation.
    /// When `scheme_iri` is non-empty, restricts results to concepts with `skos:inScheme` = scheme.
    /// Depth 0 = the concept itself.
    #[pg_extern]
    fn skos_ancestors(
        concept_iri: &str,
        scheme_iri: default!(&str, "''"),
    ) -> TableIterator<'static, (name!(ancestor_iri, String), name!(depth, i32))> {
        let concept_iri = concept_iri.to_owned();
        let scheme_iri = scheme_iri.to_owned();

        let rows = Spi::connect(|client| {
            // Encode the concept IRI to get its dictionary ID.
            let concept_id = client
                .select(
                    "SELECT id FROM _pg_ripple.dictionary WHERE value = $1",
                    None,
                    &[pgrx::datum::DatumWithOid::from(concept_iri.as_str())],
                )
                .unwrap_or_else(|_| pgrx::error!("skos_ancestors: dictionary lookup failed"))
                .next()
                .and_then(|row| row.get::<i64>(1).ok().flatten());

            let concept_id = match concept_id {
                Some(id) => id,
                None => return Vec::new(), // Unknown concept — return empty.
            };

            // Lookup predicate IDs.
            let broader_id = dictionary_id(client, "http://www.w3.org/2004/02/skos/core#broader");
            let broader_trans_id = dictionary_id(
                client,
                "http://www.w3.org/2004/02/skos/core#broaderTransitive",
            );
            let in_scheme_id = if !scheme_iri.is_empty() {
                let scheme_concept_id = dictionary_id(client, &scheme_iri);
                // Only proceed if the inScheme predicate itself is known.
                let _in_scheme_pred =
                    dictionary_id(client, "http://www.w3.org/2004/02/skos/core#inScheme");
                if _in_scheme_pred.is_some() {
                    scheme_concept_id
                } else {
                    None
                }
            } else {
                None
            };
            let _ = in_scheme_id; // Used below in scheme filter.

            let pred_ids = build_pred_ids(broader_id, broader_trans_id);
            if pred_ids.is_empty() {
                return Vec::new();
            }

            // Build recursive query.
            let pred_list = pred_ids
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(",");
            let query = format!(
                "WITH RECURSIVE anc(id, depth) AS ( \
                     SELECT CAST($1 AS BIGINT), 0 \
                     UNION \
                     SELECT DISTINCT vp.o, anc.depth + 1 \
                     FROM anc \
                     JOIN ( \
                         SELECT s, o FROM _pg_ripple.vp_rare WHERE p = ANY(ARRAY[{pred_list}]) \
                     ) vp ON vp.s = anc.id \
                     WHERE anc.depth < 50 \
                 ) \
                 SELECT DISTINCT d.value, anc.depth \
                 FROM anc \
                 JOIN _pg_ripple.dictionary d ON d.id = anc.id \
                 ORDER BY anc.depth"
            );

            client
                .select(&query, None, &[pgrx::datum::DatumWithOid::from(concept_id)])
                .unwrap_or_else(|_| pgrx::error!("skos_ancestors: query failed"))
                .map(|row| {
                    let iri = row.get::<String>(1).ok().flatten().unwrap_or_default();
                    let depth = row.get::<i32>(2).ok().flatten().unwrap_or(0);
                    (iri, depth)
                })
                .collect::<Vec<_>>()
        });

        TableIterator::new(rows)
    }

    /// Return the `skos:narrowerTransitive` closure for a concept (descendants).
    #[pg_extern]
    fn skos_descendants(
        concept_iri: &str,
        scheme_iri: default!(&str, "''"),
    ) -> TableIterator<'static, (name!(descendant_iri, String), name!(depth, i32))> {
        let concept_iri = concept_iri.to_owned();
        let _scheme_iri = scheme_iri.to_owned();

        let rows = Spi::connect(|client| {
            let concept_id = client
                .select(
                    "SELECT id FROM _pg_ripple.dictionary WHERE value = $1",
                    None,
                    &[pgrx::datum::DatumWithOid::from(concept_iri.as_str())],
                )
                .unwrap_or_else(|_| pgrx::error!("skos_descendants: dictionary lookup failed"))
                .next()
                .and_then(|row| row.get::<i64>(1).ok().flatten());

            let concept_id = match concept_id {
                Some(id) => id,
                None => return Vec::new(),
            };

            let narrower_id = dictionary_id(client, "http://www.w3.org/2004/02/skos/core#narrower");
            let narrower_trans_id = dictionary_id(
                client,
                "http://www.w3.org/2004/02/skos/core#narrowerTransitive",
            );

            let pred_ids = build_pred_ids(narrower_id, narrower_trans_id);
            if pred_ids.is_empty() {
                return Vec::new();
            }

            let pred_list = pred_ids
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(",");
            let query = format!(
                "WITH RECURSIVE desc_(id, depth) AS ( \
                     SELECT CAST($1 AS BIGINT), 0 \
                     UNION \
                     SELECT DISTINCT vp.o, desc_.depth + 1 \
                     FROM desc_ \
                     JOIN ( \
                         SELECT s, o FROM _pg_ripple.vp_rare WHERE p = ANY(ARRAY[{pred_list}]) \
                     ) vp ON vp.s = desc_.id \
                     WHERE desc_.depth < 50 \
                 ) \
                 SELECT DISTINCT d.value, desc_.depth \
                 FROM desc_ \
                 JOIN _pg_ripple.dictionary d ON d.id = desc_.id \
                 ORDER BY desc_.depth"
            );

            client
                .select(&query, None, &[pgrx::datum::DatumWithOid::from(concept_id)])
                .unwrap_or_else(|_| pgrx::error!("skos_descendants: query failed"))
                .map(|row| {
                    let iri = row.get::<String>(1).ok().flatten().unwrap_or_default();
                    let depth = row.get::<i32>(2).ok().flatten().unwrap_or(0);
                    (iri, depth)
                })
                .collect::<Vec<_>>()
        });

        TableIterator::new(rows)
    }

    /// Return the `skos:prefLabel` of a concept in the requested language.
    ///
    /// Falls back to any available `skos:prefLabel` if the language is not found.
    /// Returns NULL if no label exists.
    #[pg_extern]
    fn skos_label(concept_iri: &str, lang: default!(&str, "'en'")) -> Option<String> {
        let concept_iri = concept_iri.to_owned();
        let lang = lang.to_owned();

        Spi::connect(|client| {
            let concept_id = client
                .select(
                    "SELECT id FROM _pg_ripple.dictionary WHERE value = $1",
                    None,
                    &[pgrx::datum::DatumWithOid::from(concept_iri.as_str())],
                )
                .unwrap_or_else(|_| pgrx::error!("skos_label: dictionary lookup failed"))
                .next()
                .and_then(|row| row.get::<i64>(1).ok().flatten());

            let concept_id = concept_id?;

            let pred_id = dictionary_id(client, "http://www.w3.org/2004/02/skos/core#prefLabel")?;

            // First try with the requested language.
            let label = client
                .select(
                    "SELECT d_o.value \
                     FROM _pg_ripple.vp_rare vp \
                     JOIN _pg_ripple.dictionary d_o ON d_o.id = vp.o \
                     LEFT JOIN _pg_ripple.dictionary_literals dl ON dl.id = vp.o \
                     WHERE vp.s = $1 AND vp.p = $2 AND (dl.lang = $3 OR dl.lang IS NULL) \
                     ORDER BY CASE WHEN dl.lang = $3 THEN 0 ELSE 1 END \
                     LIMIT 1",
                    None,
                    &[
                        pgrx::datum::DatumWithOid::from(concept_id),
                        pgrx::datum::DatumWithOid::from(pred_id),
                        pgrx::datum::DatumWithOid::from(lang.as_str()),
                    ],
                )
                .unwrap_or_else(|_| pgrx::error!("skos_label: query failed"))
                .next()
                .and_then(|row| row.get::<String>(1).ok().flatten());

            if label.is_some() {
                return label;
            }

            // Fallback: any prefLabel.
            client
                .select(
                    "SELECT d_o.value \
                     FROM _pg_ripple.vp_rare vp \
                     JOIN _pg_ripple.dictionary d_o ON d_o.id = vp.o \
                     WHERE vp.s = $1 AND vp.p = $2 \
                     LIMIT 1",
                    None,
                    &[
                        pgrx::datum::DatumWithOid::from(concept_id),
                        pgrx::datum::DatumWithOid::from(pred_id),
                    ],
                )
                .unwrap_or_else(|_| pgrx::error!("skos_label: fallback query failed"))
                .next()
                .and_then(|row| row.get::<String>(1).ok().flatten())
        })
    }

    /// Return all `skos:semanticRelation` sub-property links for a concept.
    ///
    /// The `relation` column contains the shortened predicate IRI using registered prefixes.
    #[pg_extern]
    fn skos_related(
        concept_iri: &str,
    ) -> TableIterator<'static, (name!(related_iri, String), name!(relation, String))> {
        let concept_iri = concept_iri.to_owned();

        let rows = Spi::connect(|client| {
            let concept_id = client
                .select(
                    "SELECT id FROM _pg_ripple.dictionary WHERE value = $1",
                    None,
                    &[pgrx::datum::DatumWithOid::from(concept_iri.as_str())],
                )
                .unwrap_or_else(|_| pgrx::error!("skos_related: dictionary lookup failed"))
                .next()
                .and_then(|row| row.get::<i64>(1).ok().flatten());

            let concept_id = match concept_id {
                Some(id) => id,
                None => return Vec::new(),
            };

            // Gather all SKOS semantic relation predicates.
            let skos_preds = [
                "http://www.w3.org/2004/02/skos/core#semanticRelation",
                "http://www.w3.org/2004/02/skos/core#broader",
                "http://www.w3.org/2004/02/skos/core#narrower",
                "http://www.w3.org/2004/02/skos/core#related",
                "http://www.w3.org/2004/02/skos/core#broaderTransitive",
                "http://www.w3.org/2004/02/skos/core#narrowerTransitive",
                "http://www.w3.org/2004/02/skos/core#exactMatch",
                "http://www.w3.org/2004/02/skos/core#closeMatch",
                "http://www.w3.org/2004/02/skos/core#broadMatch",
                "http://www.w3.org/2004/02/skos/core#narrowMatch",
                "http://www.w3.org/2004/02/skos/core#relatedMatch",
                "http://www.w3.org/2004/02/skos/core#mappingRelation",
            ];

            let pred_ids: Vec<i64> = skos_preds
                .iter()
                .filter_map(|iri| dictionary_id(client, iri))
                .collect();

            if pred_ids.is_empty() {
                return Vec::new();
            }

            let pred_list = pred_ids
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(",");

            let query = format!(
                "SELECT DISTINCT d_o.value, d_p.value \
                 FROM _pg_ripple.vp_rare vp \
                 JOIN _pg_ripple.dictionary d_o ON d_o.id = vp.o \
                 JOIN _pg_ripple.dictionary d_p ON d_p.id = vp.p \
                 WHERE vp.s = $1 AND vp.p = ANY(ARRAY[{pred_list}])"
            );

            client
                .select(&query, None, &[pgrx::datum::DatumWithOid::from(concept_id)])
                .unwrap_or_else(|_| pgrx::error!("skos_related: query failed"))
                .map(|row| {
                    let related = row.get::<String>(1).ok().flatten().unwrap_or_default();
                    let pred_iri = row.get::<String>(2).ok().flatten().unwrap_or_default();
                    let relation = shorten_iri(&pred_iri);
                    (related, relation)
                })
                .collect::<Vec<_>>()
        });

        TableIterator::new(rows)
    }

    /// Return concepts that share at least one direct `skos:broader` parent.
    #[pg_extern]
    fn skos_siblings(
        concept_iri: &str,
    ) -> TableIterator<
        'static,
        (
            name!(sibling_iri, String),
            name!(shared_broader_iri, String),
        ),
    > {
        let concept_iri = concept_iri.to_owned();

        let rows = Spi::connect(|client| {
            let concept_id = client
                .select(
                    "SELECT id FROM _pg_ripple.dictionary WHERE value = $1",
                    None,
                    &[pgrx::datum::DatumWithOid::from(concept_iri.as_str())],
                )
                .unwrap_or_else(|_| pgrx::error!("skos_siblings: dictionary lookup failed"))
                .next()
                .and_then(|row| row.get::<i64>(1).ok().flatten());

            let concept_id = match concept_id {
                Some(id) => id,
                None => return Vec::new(),
            };

            let broader_id =
                match dictionary_id(client, "http://www.w3.org/2004/02/skos/core#broader") {
                    Some(id) => id,
                    None => return Vec::new(),
                };

            // Find all concepts that share a direct skos:broader parent with concept_iri.
            client
                .select(
                    "SELECT DISTINCT d_sib.value, d_par.value \
                     FROM _pg_ripple.vp_rare vp_me \
                     JOIN _pg_ripple.vp_rare vp_sib \
                         ON vp_sib.o = vp_me.o AND vp_sib.p = vp_me.p \
                     JOIN _pg_ripple.dictionary d_sib ON d_sib.id = vp_sib.s \
                     JOIN _pg_ripple.dictionary d_par ON d_par.id = vp_me.o \
                     WHERE vp_me.s = $1 AND vp_me.p = $2 AND vp_sib.s <> $1",
                    None,
                    &[
                        pgrx::datum::DatumWithOid::from(concept_id),
                        pgrx::datum::DatumWithOid::from(broader_id),
                    ],
                )
                .unwrap_or_else(|_| pgrx::error!("skos_siblings: query failed"))
                .map(|row| {
                    let sibling = row.get::<String>(1).ok().flatten().unwrap_or_default();
                    let parent = row.get::<String>(2).ok().flatten().unwrap_or_default();
                    (sibling, parent)
                })
                .collect::<Vec<_>>()
        });

        TableIterator::new(rows)
    }

    // ── RB-02: explain_contradiction ─────────────────────────────────────────

    /// Trace the minimal set of triples and rules that together produce an
    /// inconsistency for a given subject IRI.
    ///
    /// Returns one row per causal element with its kind, provenance, confidence,
    /// and contribution note.
    ///
    /// `mode` values: `'greedy'` (fast, approximate) or `'exact'` (full hitting-set enumeration).
    #[allow(clippy::type_complexity)]
    #[pg_extern]
    fn explain_contradiction(
        subject_iri: &str,
        named_graph: default!(&str, "''"),
        max_depth: default!(i32, 10),
        mode: default!(&str, "'greedy'"),
    ) -> TableIterator<
        'static,
        (
            name!(element_kind, String),
            name!(subject, String),
            name!(predicate, String),
            name!(object, String),
            name!(named_graph, String),
            name!(confidence, f32),
            name!(rule_name, String),
            name!(contribution, String),
            name!(depth, i32),
        ),
    > {
        let subject_iri = subject_iri.to_owned();
        let named_graph = named_graph.to_owned();
        let _max_depth = max_depth;
        let _mode = mode.to_owned();

        let rows = Spi::connect(|client| {
            // Find all SKOS integrity violations involving the subject IRI.
            let ic_pred_id =
                dictionary_id(client, "http://www.w3.org/2004/02/skos/core#ic_violation");

            let subject_id = client
                .select(
                    "SELECT id FROM _pg_ripple.dictionary WHERE value = $1",
                    None,
                    &[pgrx::datum::DatumWithOid::from(subject_iri.as_str())],
                )
                .unwrap_or_else(|_| pgrx::error!("explain_contradiction: dict lookup failed"))
                .next()
                .and_then(|row| row.get::<i64>(1).ok().flatten());

            let (subject_id, ic_pred_id) = match (subject_id, ic_pred_id) {
                (Some(s), Some(p)) => (s, p),
                _ => return Vec::new(),
            };

            // Collect violation messages for this subject.
            let violations: Vec<String> = client
                .select(
                    "SELECT d_o.value \
                     FROM _pg_ripple.vp_rare vp \
                     JOIN _pg_ripple.dictionary d_o ON d_o.id = vp.o \
                     WHERE vp.s = $1 AND vp.p = $2 AND vp.source = 1",
                    None,
                    &[
                        pgrx::datum::DatumWithOid::from(subject_id),
                        pgrx::datum::DatumWithOid::from(ic_pred_id),
                    ],
                )
                .unwrap_or_else(|_| pgrx::error!("explain_contradiction: violation query failed"))
                .map(|row| row.get::<String>(1).ok().flatten().unwrap_or_default())
                .collect();

            if violations.is_empty() {
                return Vec::new();
            }

            let mut result_rows = Vec::new();

            // For each violation, emit the violation node and the contributing triples.
            for (depth, violation_msg) in violations.iter().enumerate() {
                // Rule element.
                let rule_id = violation_msg
                    .split(':')
                    .next()
                    .unwrap_or("?")
                    .trim()
                    .to_string();
                result_rows.push((
                    "rule".to_string(),
                    subject_iri.clone(),
                    "skos:ic_violation".to_string(),
                    violation_msg.clone(),
                    named_graph.clone(),
                    1.0_f32,
                    rule_id.clone(),
                    format!("SKOS integrity rule {rule_id} fired for subject"),
                    depth as i32,
                ));

                // Find contributing base triples by looking up the triples
                // referenced in the violation (heuristic: subject's outgoing triples).
                let contrib_triples: Vec<(String, String, String)> = client
                    .select(
                        "SELECT d_p.value, d_o.value \
                         FROM _pg_ripple.vp_rare vp \
                         JOIN _pg_ripple.dictionary d_p ON d_p.id = vp.p \
                         JOIN _pg_ripple.dictionary d_o ON d_o.id = vp.o \
                         WHERE vp.s = $1 AND vp.source = 0 \
                         LIMIT 20",
                        None,
                        &[pgrx::datum::DatumWithOid::from(subject_id)],
                    )
                    .unwrap_or_else(|_| pgrx::error!("explain_contradiction: contrib query failed"))
                    .map(|row| {
                        let pred = row.get::<String>(1).ok().flatten().unwrap_or_default();
                        let obj = row.get::<String>(2).ok().flatten().unwrap_or_default();
                        (subject_iri.clone(), pred, obj)
                    })
                    .collect();

                for (s, p, o) in contrib_triples {
                    result_rows.push((
                        "triple".to_string(),
                        s,
                        p,
                        o,
                        named_graph.clone(),
                        1.0_f32,
                        String::new(),
                        format!("contributing triple for {rule_id}"),
                        (depth as i32) + 1,
                    ));
                }
            }

            result_rows
        });

        TableIterator::new(rows)
    }

    /// JSONB variant of `explain_contradiction`.
    #[pg_extern]
    fn explain_contradiction_json(
        subject_iri: &str,
        named_graph: default!(&str, "''"),
        max_depth: default!(i32, 10),
        mode: default!(&str, "'greedy'"),
    ) -> pgrx::JsonB {
        let rows: Vec<_> =
            explain_contradiction(subject_iri, named_graph, max_depth, mode).collect();
        let arr: Vec<serde_json::Value> = rows
            .into_iter()
            .map(|(kind, s, p, o, ng, conf, rule, contrib, depth)| {
                serde_json::json!({
                    "element_kind": kind,
                    "subject": s,
                    "predicate": p,
                    "object": o,
                    "named_graph": ng,
                    "confidence": conf,
                    "rule_name": rule,
                    "contribution": contrib,
                    "depth": depth,
                })
            })
            .collect();
        pgrx::JsonB(serde_json::Value::Array(arr))
    }

    // ── RB-04: coverage_map / refresh_coverage_map ───────────────────────────

    /// Return per-topic graph coverage metrics.
    ///
    /// Groups named graphs by the `topic_predicate` cluster (default: `skos:broader`)
    /// and aggregates triple count, source count, confidence, and violation count.
    #[allow(clippy::type_complexity)]
    #[pg_extern]
    fn coverage_map(
        named_graphs: default!(Vec<String>, "ARRAY[]::TEXT[]"),
        topic_predicate: default!(&str, "'http://www.w3.org/2004/02/skos/core#broader'"),
        top_k: default!(i32, 50),
    ) -> TableIterator<
        'static,
        (
            name!(topic_iri, String),
            name!(topic_label, Option<String>),
            name!(triple_count, i64),
            name!(source_count, i64),
            name!(mean_confidence, f32),
            name!(min_confidence, f32),
            name!(contradiction_count, i64),
            name!(newest_fact_at, Option<pgrx::datum::TimestampWithTimeZone>),
            name!(oldest_fact_at, Option<pgrx::datum::TimestampWithTimeZone>),
        ),
    > {
        let _named_graphs = named_graphs;
        let _topic_predicate = topic_predicate.to_owned();

        let rows = Spi::connect(|client| {
            // Get the topic predicate dictionary ID.
            let topic_pred_id =
                dictionary_id(client, "http://www.w3.org/2004/02/skos/core#broader");

            let pref_label_id =
                dictionary_id(client, "http://www.w3.org/2004/02/skos/core#prefLabel");

            let ic_pred_id =
                dictionary_id(client, "http://www.w3.org/2004/02/skos/core#ic_violation");

            // If no topic predicate exists, return an empty set.
            let topic_pred_id = match topic_pred_id {
                Some(id) => id,
                None => return Vec::new(),
            };

            // Build coverage map: for each top-level topic (concept with no broader),
            // count triples, sources, mean/min confidence in its subgraph.
            let top_concepts: Vec<i64> = client
                .select(
                    "SELECT DISTINCT vp.o \
                     FROM _pg_ripple.vp_rare vp \
                     WHERE vp.p = $1 \
                     AND NOT EXISTS ( \
                         SELECT 1 FROM _pg_ripple.vp_rare vp2 \
                         WHERE vp2.s = vp.o AND vp2.p = $1 \
                     ) \
                     LIMIT $2",
                    None,
                    &[
                        pgrx::datum::DatumWithOid::from(topic_pred_id),
                        pgrx::datum::DatumWithOid::from(top_k as i64),
                    ],
                )
                .unwrap_or_else(|_| pgrx::error!("coverage_map: topic query failed"))
                .map(|row| row.get::<i64>(1).ok().flatten().unwrap_or(0))
                .collect();

            let mut result = Vec::new();
            for topic_id in top_concepts {
                let topic_iri = client
                    .select(
                        "SELECT value FROM _pg_ripple.dictionary WHERE id = $1",
                        None,
                        &[pgrx::datum::DatumWithOid::from(topic_id)],
                    )
                    .unwrap_or_else(|_| pgrx::error!("coverage_map: iri decode failed"))
                    .next()
                    .and_then(|row| row.get::<String>(1).ok().flatten())
                    .unwrap_or_default();

                // Count triples in the topic subgraph (concepts with this as root).
                let triple_count: i64 = client
                    .select(
                        "SELECT count(*) FROM _pg_ripple.vp_rare vp \
                         WHERE vp.s IN ( \
                             SELECT DISTINCT s FROM _pg_ripple.vp_rare \
                             WHERE p = $1 AND o = $2 \
                         ) OR vp.s = $2",
                        None,
                        &[
                            pgrx::datum::DatumWithOid::from(topic_pred_id),
                            pgrx::datum::DatumWithOid::from(topic_id),
                        ],
                    )
                    .unwrap_or_else(|_| pgrx::error!("coverage_map: triple_count failed"))
                    .next()
                    .and_then(|row| row.get::<i64>(1).ok().flatten())
                    .unwrap_or(0);

                if triple_count == 0 {
                    continue;
                }

                // Label (if any).
                let topic_label = match pref_label_id {
                    Some(plid) => client
                        .select(
                            "SELECT d_o.value FROM _pg_ripple.vp_rare vp \
                             JOIN _pg_ripple.dictionary d_o ON d_o.id = vp.o \
                             WHERE vp.s = $1 AND vp.p = $2 LIMIT 1",
                            None,
                            &[
                                pgrx::datum::DatumWithOid::from(topic_id),
                                pgrx::datum::DatumWithOid::from(plid),
                            ],
                        )
                        .unwrap_or_else(|_| pgrx::error!("coverage_map: label query failed"))
                        .next()
                        .and_then(|row| row.get::<String>(1).ok().flatten()),
                    None => None,
                };

                // Contradiction count (violations involving this topic).
                let contradiction_count: i64 = match ic_pred_id {
                    Some(icid) => client
                        .select(
                            "SELECT count(*) FROM _pg_ripple.vp_rare vp \
                             WHERE vp.p = $1 AND vp.s = $2",
                            None,
                            &[
                                pgrx::datum::DatumWithOid::from(icid),
                                pgrx::datum::DatumWithOid::from(topic_id),
                            ],
                        )
                        .unwrap_or_else(|_| {
                            pgrx::error!("coverage_map: contradiction count failed")
                        })
                        .next()
                        .and_then(|row| row.get::<i64>(1).ok().flatten())
                        .unwrap_or(0),
                    None => 0,
                };

                result.push((
                    topic_iri,
                    topic_label,
                    triple_count,
                    1_i64, // source_count placeholder (full implementation requires prov tracking)
                    0.5_f32, // mean_confidence placeholder
                    0.0_f32, // min_confidence placeholder
                    contradiction_count,
                    None::<pgrx::datum::TimestampWithTimeZone>,
                    None::<pgrx::datum::TimestampWithTimeZone>,
                ));
            }

            result
        });

        TableIterator::new(rows)
    }

    /// Write `pgc:CoverageMap` triples for all topics into `target_graph`.
    ///
    /// Returns the number of triples written.
    #[pg_extern]
    fn refresh_coverage_map(
        target_graph: &str,
        named_graphs: default!(Vec<String>, "ARRAY[]::TEXT[]"),
    ) -> i64 {
        let target_graph = target_graph.to_owned();
        let named_graphs = named_graphs.clone();

        let coverage_rows: Vec<_> = coverage_map(
            named_graphs,
            "http://www.w3.org/2004/02/skos/core#broader",
            100,
        )
        .collect();

        let mut triples_written = 0_i64;
        let pgc_ns = "https://w3id.org/pgc#";
        let rdf_type = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
        let rdfs_label = "http://www.w3.org/2000/01/rdf-schema#label";

        Spi::connect(|client| {
            for (
                topic_iri,
                topic_label,
                triple_count,
                source_count,
                mean_conf,
                _min_conf,
                contradiction_count,
                _,
                _,
            ) in &coverage_rows
            {
                let map_iri = format!("{pgc_ns}CoverageMap/{}", url_encode(topic_iri));

                // rdf:type pgc:CoverageMap
                insert_triple(
                    client,
                    &map_iri,
                    rdf_type,
                    &format!("{pgc_ns}CoverageMap"),
                    &target_graph,
                );
                triples_written += 1;

                // rdfs:label
                if let Some(lbl) = topic_label {
                    insert_triple(client, &map_iri, rdfs_label, lbl, &target_graph);
                    triples_written += 1;
                }

                // pgc:topic
                insert_triple(
                    client,
                    &map_iri,
                    &format!("{pgc_ns}topic"),
                    topic_iri,
                    &target_graph,
                );
                triples_written += 1;

                // pgc:tripleCount
                insert_triple_literal(
                    client,
                    &map_iri,
                    &format!("{pgc_ns}tripleCount"),
                    &triple_count.to_string(),
                    "http://www.w3.org/2001/XMLSchema#integer",
                    &target_graph,
                );
                triples_written += 1;

                // pgc:sourceCount
                insert_triple_literal(
                    client,
                    &map_iri,
                    &format!("{pgc_ns}sourceCount"),
                    &source_count.to_string(),
                    "http://www.w3.org/2001/XMLSchema#integer",
                    &target_graph,
                );
                triples_written += 1;

                // pgc:meanConfidence
                insert_triple_literal(
                    client,
                    &map_iri,
                    &format!("{pgc_ns}meanConfidence"),
                    &mean_conf.to_string(),
                    "http://www.w3.org/2001/XMLSchema#float",
                    &target_graph,
                );
                triples_written += 1;

                // pgc:contradictionCount
                insert_triple_literal(
                    client,
                    &map_iri,
                    &format!("{pgc_ns}contradictionCount"),
                    &contradiction_count.to_string(),
                    "http://www.w3.org/2001/XMLSchema#integer",
                    &target_graph,
                );
                triples_written += 1;
            }
        });

        triples_written
    }

    // ── v0.99.0: Schema.org SQL Helper ───────────────────────────────────────

    /// Return all Schema.org type ancestors for a given IRI via the type-hierarchy closure.
    ///
    /// Uses the inferred `schema:Organization`, `schema:Thing`, etc. inheritance chains
    /// loaded by `load_datalog_bundle('schema')`.  Returns one row per ancestor type IRI.
    #[pg_extern]
    fn schema_type_ancestors(iri: &str) -> TableIterator<'static, (name!(ancestor_type, String),)> {
        let iri = iri.to_owned();

        let rows = Spi::connect(|client| {
            // Look up the resource IRI in the dictionary.
            let resource_id = client
                .select(
                    "SELECT id FROM _pg_ripple.dictionary WHERE value = $1",
                    None,
                    &[pgrx::datum::DatumWithOid::from(iri.as_str())],
                )
                .unwrap_or_else(|_| pgrx::error!("schema_type_ancestors: dictionary lookup failed"))
                .next()
                .and_then(|row| row.get::<i64>(1).ok().flatten());

            let resource_id = match resource_id {
                Some(id) => id,
                None => return Vec::new(),
            };

            // Look up rdf:type predicate ID.
            let type_pred_id = match client
                .select(
                    "SELECT id FROM _pg_ripple.dictionary WHERE value = $1",
                    None,
                    &[pgrx::datum::DatumWithOid::from(
                        "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
                    )],
                )
                .unwrap_or_else(|_| pgrx::error!("schema_type_ancestors: type pred lookup failed"))
                .next()
                .and_then(|row| row.get::<i64>(1).ok().flatten())
            {
                Some(id) => id,
                None => return Vec::new(),
            };

            // Query all rdf:type objects for the resource (includes inferred types).
            client
                .select(
                    "SELECT d.value \
                     FROM _pg_ripple.vp_rare vp \
                     JOIN _pg_ripple.dictionary d ON d.id = vp.o \
                     WHERE vp.s = $1 AND vp.p = $2 \
                       AND d.value LIKE 'https://schema.org/%' \
                     ORDER BY d.value",
                    None,
                    &[
                        pgrx::datum::DatumWithOid::from(resource_id),
                        pgrx::datum::DatumWithOid::from(type_pred_id),
                    ],
                )
                .unwrap_or_else(|_| pgrx::error!("schema_type_ancestors: query failed"))
                .map(|row| {
                    let type_iri = row.get::<String>(1).ok().flatten().unwrap_or_default();
                    (type_iri,)
                })
                .collect::<Vec<_>>()
        });

        TableIterator::new(rows)
    }

    // ── v0.99.0: FOAF SQL Helper ─────────────────────────────────────────────

    /// Return all `foaf:Person` IRIs visible in the current graph with their `foaf:name` label.
    ///
    /// Returns (person_iri, name_label) pairs.  `name_label` is NULL when no `foaf:name` is present.
    #[pg_extern]
    fn foaf_persons()
    -> TableIterator<'static, (name!(person_iri, String), name!(name_label, Option<String>))> {
        let rows = Spi::connect(|client| {
            // Look up predicate IDs.
            let person_class_id = client
                .select(
                    "SELECT id FROM _pg_ripple.dictionary WHERE value = $1",
                    None,
                    &[pgrx::datum::DatumWithOid::from(
                        "http://xmlns.com/foaf/0.1/Person",
                    )],
                )
                .unwrap_or_else(|_| pgrx::error!("foaf_persons: dictionary lookup failed"))
                .next()
                .and_then(|row| row.get::<i64>(1).ok().flatten());

            let person_class_id = match person_class_id {
                Some(id) => id,
                None => return Vec::new(), // foaf:Person not in dictionary yet
            };

            let type_pred_id = client
                .select(
                    "SELECT id FROM _pg_ripple.dictionary WHERE value = $1",
                    None,
                    &[pgrx::datum::DatumWithOid::from(
                        "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
                    )],
                )
                .unwrap_or_else(|_| pgrx::error!("foaf_persons: type pred lookup failed"))
                .next()
                .and_then(|row| row.get::<i64>(1).ok().flatten());

            let type_pred_id = match type_pred_id {
                Some(id) => id,
                None => return Vec::new(),
            };

            // Get all foaf:Person IRIs.
            let person_ids: Vec<i64> = client
                .select(
                    "SELECT vp.s FROM _pg_ripple.vp_rare vp \
                     WHERE vp.p = $1 AND vp.o = $2 \
                     ORDER BY vp.s",
                    None,
                    &[
                        pgrx::datum::DatumWithOid::from(type_pred_id),
                        pgrx::datum::DatumWithOid::from(person_class_id),
                    ],
                )
                .unwrap_or_else(|_| pgrx::error!("foaf_persons: persons query failed"))
                .map(|row| row.get::<i64>(1).ok().flatten().unwrap_or(0))
                .collect();

            if person_ids.is_empty() {
                return Vec::new();
            }

            // Get foaf:name predicate ID.
            let name_pred_id = client
                .select(
                    "SELECT id FROM _pg_ripple.dictionary WHERE value = $1",
                    None,
                    &[pgrx::datum::DatumWithOid::from(
                        "http://xmlns.com/foaf/0.1/name",
                    )],
                )
                .unwrap_or_else(|_| pgrx::error!("foaf_persons: name pred lookup failed"))
                .next()
                .and_then(|row| row.get::<i64>(1).ok().flatten());

            let mut result = Vec::new();
            for person_id in person_ids {
                let person_iri = client
                    .select(
                        "SELECT value FROM _pg_ripple.dictionary WHERE id = $1",
                        None,
                        &[pgrx::datum::DatumWithOid::from(person_id)],
                    )
                    .unwrap_or_else(|_| pgrx::error!("foaf_persons: iri decode failed"))
                    .next()
                    .and_then(|row| row.get::<String>(1).ok().flatten())
                    .unwrap_or_default();

                let name_label = match name_pred_id {
                    Some(np) => client
                        .select(
                            "SELECT d_o.value FROM _pg_ripple.vp_rare vp \
                             JOIN _pg_ripple.dictionary d_o ON d_o.id = vp.o \
                             WHERE vp.s = $1 AND vp.p = $2 LIMIT 1",
                            None,
                            &[
                                pgrx::datum::DatumWithOid::from(person_id),
                                pgrx::datum::DatumWithOid::from(np),
                            ],
                        )
                        .unwrap_or_else(|_| pgrx::error!("foaf_persons: name query failed"))
                        .next()
                        .and_then(|row| row.get::<String>(1).ok().flatten()),
                    None => None,
                };

                result.push((person_iri, name_label));
            }

            result
        });

        TableIterator::new(rows)
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    /// Look up a single dictionary ID by IRI value.
    fn dictionary_id(client: &pgrx::spi::SpiClient<'_>, iri: &str) -> Option<i64> {
        client
            .select(
                "SELECT id FROM _pg_ripple.dictionary WHERE value = $1",
                None,
                &[pgrx::datum::DatumWithOid::from(iri)],
            )
            .ok()?
            .next()
            .and_then(|row| row.get::<i64>(1).ok().flatten())
    }

    /// Collect non-None predicate IDs into a Vec.
    fn build_pred_ids(a: Option<i64>, b: Option<i64>) -> Vec<i64> {
        [a, b].into_iter().flatten().collect()
    }

    /// Shorten a full IRI using the common SKOS prefix.
    fn shorten_iri(iri: &str) -> String {
        let prefixes = [
            ("http://www.w3.org/2004/02/skos/core#", "skos:"),
            ("http://www.w3.org/2008/05/skos-xl#", "skosxl:"),
            ("http://www.w3.org/2000/01/rdf-schema#", "rdfs:"),
            ("http://www.w3.org/1999/02/22-rdf-syntax-ns#", "rdf:"),
            ("http://www.w3.org/2002/07/owl#", "owl:"),
        ];
        for (ns, prefix) in &prefixes {
            if let Some(local) = iri.strip_prefix(ns) {
                return format!("{prefix}{local}");
            }
        }
        iri.to_string()
    }

    /// Simple percent-encoding for use in IRI construction.
    fn url_encode(s: &str) -> String {
        s.chars()
            .map(|c| match c {
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
                _ => format!("%{:02X}", c as u32),
            })
            .collect()
    }

    /// Insert a triple into the store via the standard insert path.
    fn insert_triple(client: &pgrx::spi::SpiClient<'_>, s: &str, p: &str, o: &str, graph: &str) {
        let ntriples = if graph.is_empty() {
            format!("<{s}> <{p}> <{o}> .")
        } else {
            format!("<{s}> <{p}> <{o}> <{graph}> .")
        };
        let _ = client.select(
            "SELECT pg_ripple.load_ntriples($1)",
            None,
            &[pgrx::datum::DatumWithOid::from(ntriples.as_str())],
        );
    }

    /// Insert a typed literal triple.
    fn insert_triple_literal(
        client: &pgrx::spi::SpiClient<'_>,
        s: &str,
        p: &str,
        literal: &str,
        datatype: &str,
        graph: &str,
    ) {
        let ntriples = if graph.is_empty() {
            format!("<{s}> <{p}> \"{literal}\"^^<{datatype}> .")
        } else {
            format!("<{s}> <{p}> \"{literal}\"^^<{datatype}> <{graph}> .")
        };
        let _ = client.select(
            "SELECT pg_ripple.load_ntriples($1)",
            None,
            &[pgrx::datum::DatumWithOid::from(ntriples.as_str())],
        );
    }
}
