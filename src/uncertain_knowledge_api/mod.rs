//! pg_ripple SQL API — Uncertain Knowledge Engine (v0.87.0 / v0.90.0 module split CQ-04)
//!
//! Exposes the following SQL functions:
//! - `pg_ripple.load_triples_with_confidence(data, confidence, format, graph_uri)` — bulk load with confidence
//! - `pg_ripple.vacuum_confidence()` — purge orphaned confidence rows
//! - `pg_ripple.shacl_score(graph_iri)` — weighted SHACL quality score
//! - `pg_ripple.shacl_report_scored(graph_iri)` — per-violation scored report
//! - `pg_ripple.log_shacl_score(graph_iri)` — log the score to the history table
//!
//! ## Sub-module layout (v0.90.0 CQ-04)
//! - `confidence_table` — bulk loader and vacuum helpers
//! - `fuzzy` — `pg:fuzzy_match()` / `pg:token_set_ratio()` guard functions
//! - `shacl` — SHACL scoring and reporting
//! - `prov` — PROV-O provenance-derived confidence (stub)

//! - `pg_ripple.shacl_score(graph_iri)` — weighted SHACL quality score
//! - `pg_ripple.shacl_report_scored(graph_iri)` — per-violation scored report
//! - `pg_ripple.log_shacl_score(graph_iri)` — log the score to the history table

// v0.90.0 CQ-04: pre-emptive split sub-modules
// Q14-08: these sub-modules are split for future refactoring but their public
// symbols are re-exported via pg_extern macros. The compiler cannot see through
// pgrx macro expansion so dead_code is suppressed intentionally (Q13-05 pattern).
#[allow(dead_code)]
pub mod confidence_table;
#[allow(dead_code)]
pub mod fuzzy;
#[allow(dead_code)]
pub mod prov;
#[allow(dead_code)]
pub mod shacl;

#[pgrx::pg_schema]
mod pg_ripple {
    use pgrx::prelude::*;

    // ── LOAD-CONF-01: confidence-aware bulk loader ────────────────────────────

    /// Load triples with a uniform confidence score attached to all inserted SIDs.
    ///
    /// `format` may be `'ntriples'` (default), `'nquads'`, or `'turtle'`.
    /// `graph_uri` routes all triples to a named graph when provided.
    ///
    /// Returns the number of triples loaded.
    ///
    /// ```sql
    /// SELECT pg_ripple.load_triples_with_confidence(
    ///   '<ex:alice> <ex:knows> <ex:bob> .',
    ///   confidence => 0.85
    /// );
    /// ```
    #[pg_extern]
    fn load_triples_with_confidence(
        data: &str,
        confidence: default!(f64, 1.0),
        format: default!(&str, "'ntriples'"),
        graph_uri: default!(Option<&str>, "NULL"),
    ) -> i64 {
        crate::bulk_load::load_triples_with_confidence(data, confidence, format, graph_uri)
    }

    // ── CONF-GC-01d: vacuum_confidence ────────────────────────────────────────

    /// Purge orphaned confidence rows — rows whose statement_id does not exist
    /// in any VP table.  Called manually or by the maintenance hook.
    ///
    /// Returns the number of rows deleted.
    ///
    /// ```sql
    /// SELECT pg_ripple.vacuum_confidence();
    /// ```
    #[pg_extern]
    fn vacuum_confidence() -> i64 {
        // Ensure the confidence table exists (idempotent).
        crate::bulk_load::ensure_confidence_catalog();
        // Collect all VP tables and build a multi-UNION EXISTS check.
        let pred_rows: Vec<(i64, bool)> = Spi::connect(|c| {
            c.select(
                "SELECT id, (table_oid IS NOT NULL) AS dedicated \
                 FROM _pg_ripple.predicates ORDER BY id",
                None,
                &[],
            )
            .unwrap_or_else(|e| pgrx::error!("vacuum_confidence: predicates scan: {e}"))
            .map(|row| {
                let id = row.get::<i64>(1).ok().flatten().unwrap_or(0);
                let dedicated = row.get::<bool>(2).ok().flatten().unwrap_or(false);
                (id, dedicated)
            })
            .collect()
        });

        // Build an EXISTS subquery across all VP tables.
        let mut exists_parts: Vec<String> = Vec::new();
        exists_parts.push("SELECT 1 FROM _pg_ripple.vp_rare WHERE i = c.statement_id".to_owned());
        for (pred_id, dedicated) in &pred_rows {
            if *dedicated {
                exists_parts.push(format!(
                    "SELECT 1 FROM _pg_ripple.vp_{pred_id} WHERE i = c.statement_id"
                ));
            }
        }

        let exists_sql = exists_parts.join(" UNION ALL ");
        let delete_sql = format!(
            "WITH deleted AS (\
               DELETE FROM _pg_ripple.confidence c \
               WHERE NOT EXISTS ({exists_sql}) \
               RETURNING 1 \
             ) SELECT COUNT(*)::bigint FROM deleted"
        );

        Spi::get_one::<i64>(&delete_sql)
            .unwrap_or(None)
            .unwrap_or(0)
    }

    // ── SOFT-SHACL-01b: shacl_score ──────────────────────────────────────────

    /// Return a weighted SHACL data-quality score for a named graph.
    ///
    /// Score formula:
    ///   `score = 1 − (Σᵢ wᵢ × violationsᵢ) / (Σᵢ wᵢ × applicableᵢ)`
    ///
    /// Shapes with `sh:severityWeight xsd:decimal` use that weight; others default to 1.0.
    /// Returns 1.0 when there are no applicable constraints.
    ///
    /// ```sql
    /// SELECT pg_ripple.shacl_score('http://example.org/data');
    /// ```
    #[pg_extern]
    fn shacl_score(graph_iri: &str) -> f64 {
        crate::shacl_scoring::compute_shacl_score(graph_iri)
    }

    // ── SOFT-SHACL-01c: shacl_report_scored ─────────────────────────────────

    /// Return SHACL validation results with per-violation severity scores.
    ///
    /// ```sql
    /// SELECT * FROM pg_ripple.shacl_report_scored('http://example.org/data');
    /// ```
    #[pg_extern]
    fn shacl_report_scored(
        graph_iri: &str,
    ) -> TableIterator<
        'static,
        (
            name!(focus_node, String),
            name!(shape_iri, String),
            name!(result_severity, String),
            name!(result_severity_score, f64),
            name!(message, String),
        ),
    > {
        crate::shacl_scoring::shacl_report_scored(graph_iri)
    }

    // ── SOFT-SHACL-01d: log_shacl_score ─────────────────────────────────────

    /// Log the current SHACL quality score to `_pg_ripple.shacl_score_log`.
    ///
    /// ```sql
    /// SELECT pg_ripple.log_shacl_score('http://example.org/data');
    /// ```
    #[pg_extern]
    fn log_shacl_score(graph_iri: &str) {
        let score = crate::shacl_scoring::compute_shacl_score(graph_iri);
        Spi::run_with_args(
            "INSERT INTO _pg_ripple.shacl_score_log (graph_iri, score) VALUES ($1, $2)",
            &[
                pgrx::datum::DatumWithOid::from(graph_iri),
                pgrx::datum::DatumWithOid::from(score),
            ],
        )
        .unwrap_or_else(|e| pgrx::warning!("log_shacl_score insert error: {e}"));
    }

    // ── OBS-02 (v0.91.0): vacuum_shacl_score_log ─────────────────────────────

    /// Delete `_pg_ripple.shacl_score_log` rows older than
    /// `pg_ripple.shacl_score_log_retention_days` days.
    ///
    /// Returns the number of rows deleted. A return of 0 either means nothing
    /// was old enough to delete, or the GUC is set to 0 (disabled).
    ///
    /// ```sql
    /// SELECT pg_ripple.vacuum_shacl_score_log();
    /// ```
    #[pg_extern]
    fn vacuum_shacl_score_log() -> i64 {
        let retention_days = crate::gucs::observability::SHACL_SCORE_LOG_RETENTION_DAYS.get();
        if retention_days <= 0 {
            return 0;
        }
        Spi::get_one_with_args::<i64>(
            "WITH deleted AS ( \
                DELETE FROM _pg_ripple.shacl_score_log \
                WHERE logged_at < NOW() - ($1 || ' days')::interval \
                RETURNING 1 \
             ) SELECT COUNT(*)::bigint FROM deleted",
            &[pgrx::datum::DatumWithOid::from(retention_days as i64)],
        )
        .unwrap_or(None)
        .unwrap_or(0)
    }

    // ── CONF-EXPORT-01a: export_turtle_with_confidence ───────────────────────

    /// Export a graph as Turtle with RDF* confidence annotations.
    ///
    /// Triples that have confidence rows in `_pg_ripple.confidence` are annotated
    /// with `<< s p o >> pg:confidence "score"^^xsd:float .`
    ///
    /// Controlled by `pg_ripple.export_confidence` GUC (bool, default off).
    /// When the GUC is off, this function behaves identically to `export_turtle()`.
    ///
    /// ```sql
    /// SELECT pg_ripple.export_turtle_with_confidence('http://example.org/data');
    /// ```
    #[pg_extern]
    fn export_turtle_with_confidence(graph: default!(Option<&str>, "NULL")) -> String {
        if !crate::EXPORT_CONFIDENCE.get() {
            // Fall back to plain export when GUC is off.
            return crate::export::export_turtle_impl(graph);
        }
        crate::export::export_turtle_with_confidence_impl(graph)
    }

    // ── CB-03 / SEC-02 (v0.89.0): fuzzy SPARQL guard functions ──────────────

    /// Internal guard for `pg:fuzzy_match(a, b)` — checks pg_trgm availability
    /// and input length limits before calling `similarity(a, b)`.
    ///
    /// Raises PT0302 if pg_trgm is not installed; PT0308 if either input exceeds
    /// `pg_ripple.fuzzy_max_input_length` characters.
    ///
    /// Called from generated SPARQL→SQL; not intended for direct use.
    // PERF-08 (v0.92.0): STABLE (not IMMUTABLE) because the guard checks
    // pg_trgm extension presence (catalog read). STABLE allows the planner
    // to cache results within a query and hoist calls out of inner loops.
    #[pg_extern(
        stable,
        parallel_safe,
        schema = "pg_ripple",
        name = "_fuzzy_match_guard"
    )]
    fn fuzzy_match_guard(a: &str, b: &str) -> f64 {
        fuzzy_guard_checks(a, b);
        pgrx::Spi::get_one_with_args::<f64>(
            "SELECT similarity($1::text, $2::text)",
            &[
                pgrx::datum::DatumWithOid::from(a),
                pgrx::datum::DatumWithOid::from(b),
            ],
        )
        .unwrap_or_else(|e| pgrx::error!("fuzzy_match_guard: {e}"))
        .unwrap_or(0.0)
    }

    /// Internal guard for `pg:token_set_ratio(a, b)` — checks pg_trgm availability
    /// and input length limits before calling `word_similarity(a, b)`.
    ///
    /// Raises PT0302 if pg_trgm is not installed; PT0308 if either input exceeds
    /// `pg_ripple.fuzzy_max_input_length` characters.
    ///
    /// Called from generated SPARQL→SQL; not intended for direct use.
    // PERF-08 (v0.92.0): STABLE (not IMMUTABLE) because the guard checks
    // pg_trgm extension presence (catalog read). STABLE allows the planner
    // to cache results within a query and hoist calls out of inner loops.
    #[pg_extern(
        stable,
        parallel_safe,
        schema = "pg_ripple",
        name = "_token_set_ratio_guard"
    )]
    fn token_set_ratio_guard(a: &str, b: &str) -> f64 {
        fuzzy_guard_checks(a, b);
        pgrx::Spi::get_one_with_args::<f64>(
            "SELECT word_similarity($1::text, $2::text)",
            &[
                pgrx::datum::DatumWithOid::from(a),
                pgrx::datum::DatumWithOid::from(b),
            ],
        )
        .unwrap_or_else(|e| pgrx::error!("token_set_ratio_guard: {e}"))
        .unwrap_or(0.0)
    }

    /// Shared pre-flight checks for fuzzy SPARQL guard functions (CB-03, SEC-02, v0.89.0).
    ///
    /// - Raises PT0302 if pg_trgm is not installed.
    /// - Raises PT0308 if either argument exceeds `pg_ripple.fuzzy_max_input_length`.
    fn fuzzy_guard_checks(a: &str, b: &str) {
        // CB-03: check pg_trgm is installed.
        let trgm_ok = pgrx::Spi::get_one::<bool>(
            "SELECT EXISTS (SELECT 1 FROM pg_catalog.pg_extension WHERE extname = 'pg_trgm')",
        )
        .unwrap_or_else(|e| pgrx::error!("fuzzy_guard: pg_trgm check error: {e}"))
        .unwrap_or(false);

        if !trgm_ok {
            pgrx::error!(
                "pg_ripple fuzzy SPARQL requires pg_trgm — install it with: \
                 CREATE EXTENSION IF NOT EXISTS pg_trgm; (PT0302)"
            );
        }

        // SEC-02: check input length against GUC.
        let max_len = crate::FUZZY_MAX_INPUT_LENGTH.get() as usize;
        if a.len() > max_len || b.len() > max_len {
            pgrx::error!(
                "fuzzy SPARQL input exceeds pg_ripple.fuzzy_max_input_length characters ({}) — \
                 truncate input or raise the GUC (PT0308)",
                max_len
            );
        }
    }
}
