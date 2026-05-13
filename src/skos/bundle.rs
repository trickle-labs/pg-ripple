//! SKOS Named Bundle loading: load_datalog_bundle, load_shape_bundle, integrity loaders.
//! (extracted from skos.rs in v0.114.0)

#[pgrx::pg_schema]
mod pg_ripple {
    use pgrx::prelude::*;

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
}
