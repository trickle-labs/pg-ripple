//! Datalog explain: prewarm, explain_datalog, explain_inference, justify, vacuum.
//! (extracted from datalog_api/mod.rs in v0.114.0)

#[pgrx::pg_schema]
mod pg_ripple {
    use pgrx::prelude::*;

    /// Prewarm the hot dictionary table by copying short IRIs and predicates.
    ///
    /// Returns the number of rows in the hot table after prewarm.
    #[pg_extern]
    fn prewarm_dictionary_hot() -> i64 {
        crate::dictionary::hot::ensure_hot_table();
        crate::dictionary::hot::prewarm_hot_table();
        pgrx::Spi::get_one::<i64>("SELECT count(*) FROM _pg_ripple.dictionary_hot")
            .unwrap_or(None)
            .unwrap_or(0)
    }

    // ── v0.40.0: explain_datalog ──────────────────────────────────────────────

    /// Return a JSONB explain document for a named Datalog rule set.
    ///
    /// Keys:
    /// - `"strata"` — per-stratum dependency graph with predicate IDs and rule count
    /// - `"rules"` — rewritten rule texts stored in the catalog
    /// - `"sql_per_rule"` — compiled SQL for each rule
    /// - `"last_run_stats"` — per-iteration delta row counts from the last `infer()` run
    #[pg_extern]
    fn explain_datalog(rule_set_name: &str) -> pgrx::JsonB {
        crate::datalog::explain::explain_datalog(rule_set_name)
    }

    // ── v0.61.0: explain_inference_provenance (renamed from explain_inference in v0.101.0) ────

    /// Return the rule-firing provenance chain for a given inferred triple.
    ///
    /// Each row in the returned table represents one step in the derivation:
    /// - `depth INT`           — depth in the derivation tree (0 = the queried triple)
    /// - `rule_id TEXT`        — identifier of the Datalog rule that fired
    /// - `source_sids BIGINT[]`— statement IDs of the source triples that triggered this rule
    /// - `child_triples JSONB[]` — child derivation nodes (recursive)
    ///
    /// Implemented by inspecting the `source` column (0 = explicit, 1 = inferred)
    /// and walking the Datalog rule-firing log.  Returns an empty set when the
    /// triple is not inferred (i.e., its `source = 0`) or when provenance logging
    /// is disabled.
    ///
    /// # Arguments
    ///
    /// - `s` — subject IRI
    /// - `p` — predicate IRI
    /// - `o` — object IRI or literal
    /// - `g` — named graph IRI (`NULL` for the default graph)
    ///
    /// > **Note**: renamed from `explain_inference` to `explain_inference_provenance` in
    /// > v0.101.0 to free the `explain_inference` name for the new NL explanation function.
    #[pg_extern]
    fn explain_inference_provenance(
        s: &str,
        p: &str,
        o: &str,
        g: default!(Option<&str>, "NULL"),
    ) -> TableIterator<
        'static,
        (
            name!(depth, i32),
            name!(rule_id, String),
            name!(source_sids, Vec<i64>),
            name!(child_triples, pgrx::JsonB),
        ),
    > {
        let rows = crate::datalog::explain::explain_inference_impl(s, p, o, g);
        TableIterator::new(rows)
    }

    // ── v0.101.0: explain_inference / explain_inference_jsonb / vacuum_explanation_cache ──

    /// Return a natural-language explanation of why pg_ripple derived a given fact.
    ///
    /// Retrieves the proof tree from `_pg_ripple.derivations` via `justify()`,
    /// decodes all dictionary IDs to human-readable IRI strings and literal values,
    /// then either:
    ///
    /// a) Sends the structured proof tree to the configured LLM endpoint
    ///    (`pg_ripple.llm_endpoint`) for a narrative explanation, or
    /// b) Falls back to a deterministic indented-text renderer when the endpoint
    ///    is not configured or returns an error (never raises an error — always
    ///    returns something readable).
    ///
    /// Results are cached in `_pg_ripple.explanation_cache` keyed by `(sid, format,
    /// model)` and expire after `pg_ripple.explanation_cache_ttl` seconds (default 3600).
    ///
    /// # Arguments
    ///
    /// - `subject`   — subject IRI (without angle brackets)
    /// - `predicate` — predicate IRI
    /// - `object`    — object IRI or literal
    /// - `format`    — `'text'` (default) or `'markdown'`
    ///
    /// # Return value
    ///
    /// Returns `NULL` for base (non-inferred) facts.
    ///
    /// # Prerequisites
    ///
    /// `pg_ripple.record_derivations` must have been `on` during the `infer()` run
    /// that produced the fact.
    #[pg_extern]
    fn explain_inference(
        subject: &str,
        predicate: &str,
        object: &str,
        format: default!(&str, "'text'"),
    ) -> Option<String> {
        crate::datalog::nlexplain::explain_inference_impl(subject, predicate, object, format)
    }

    /// Return a JSONB document containing both the structured proof tree and the
    /// natural-language narrative for a Datalog-derived fact.
    ///
    /// Shape:
    /// ```json
    /// { "proof_tree": { … }, "narrative": "The fact was derived because …" }
    /// ```
    ///
    /// Returns `NULL` for base (non-inferred) facts.
    #[pg_extern]
    fn explain_inference_jsonb(
        subject: &str,
        predicate: &str,
        object: &str,
    ) -> Option<pgrx::JsonB> {
        crate::datalog::nlexplain::explain_inference_jsonb_impl(subject, predicate, object)
            .map(pgrx::JsonB)
    }

    /// Remove expired rows from `_pg_ripple.explanation_cache`.
    ///
    /// Deletes rows whose `cached_at` is older than `pg_ripple.explanation_cache_ttl`
    /// seconds.  Returns the number of rows removed.  Call this function periodically
    /// (e.g., via `pg_cron`) or after bulk inference runs.
    #[pg_extern]
    fn vacuum_explanation_cache() -> i64 {
        crate::datalog::nlexplain::vacuum_explanation_cache_impl()
    }

    // ── v0.100.0: justify / vacuum_derivations (PROOF-TREE-01) ───────────────

    /// Return the backward-chaining proof tree for a Datalog-derived triple as JSONB.
    ///
    /// Requires `pg_ripple.record_derivations = on` to have been set before the
    /// `infer()` / `infer_agg()` call that produced the fact.  Returns `NULL` when:
    ///
    /// - The triple is not in the knowledge base.
    /// - No derivation provenance was recorded for this triple (either it is a
    ///   base fact or `record_derivations` was off during inference).
    ///
    /// The returned JSONB has the shape:
    /// ```json
    /// {
    ///   "type": "inferred",
    ///   "sid": 42,
    ///   "triple": { "subject": "...", "predicate": "...", "object": "..." },
    ///   "derivations": [
    ///     {
    ///       "rule": "?x <pred_b> ?y :- ?x <pred_a> ?y .",
    ///       "rule_set": "my_rules",
    ///       "antecedents": [ { "type": "base", "sid": 7, "triple": { ... } } ]
    ///     }
    ///   ]
    /// }
    /// ```
    ///
    /// Cycle protection is built in: if the derivation graph contains a cycle,
    /// the node is tagged `{"cycle": true}` and recursion stops.
    #[pg_extern]
    fn justify(subject: &str, predicate: &str, object: &str) -> Option<pgrx::JsonB> {
        crate::datalog::derivations::justify_impl(subject, predicate, object).map(pgrx::JsonB)
    }

    /// Remove orphan rows from `_pg_ripple.derivations` — rows whose `derived_sid`
    /// no longer exists in `_pg_ripple.vp_rare`.
    ///
    /// Orphans are created when a derived fact is retracted (via DRed or manual
    /// deletion) but the corresponding derivation row is not immediately cleaned
    /// up.  Call this function after bulk retractions or as a scheduled task.
    ///
    /// Returns the number of rows removed.
    #[pg_extern]
    fn vacuum_derivations() -> i64 {
        crate::datalog::derivations::vacuum_orphan_derivations()
    }

    // ── v0.102.0: hypothetical_inference ──────────────────────────────────────

}
