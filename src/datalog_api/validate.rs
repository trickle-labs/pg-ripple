//! Datalog inference: infer, infer_with_stats, infer_goal, infer_agg, infer_demand, infer_wfs, wcoj.
//! (extracted from datalog_api/mod.rs in v0.114.0)

#[pgrx::pg_schema]
mod pg_ripple {
    use pgrx::prelude::*;

    /// Run inference for the named rule set and materialise derived triples.
    ///
    /// Returns the number of triples derived.
    #[pg_extern]
    fn infer(rule_set: default!(&str, "'custom'")) -> i64 {
        let derived = crate::datalog::run_inference(rule_set);
        // v0.103.0 CONFLICT-02: when block_on_conflict is on, run runtime
        // conflict detection after inference completes and raise PT0451 if any
        // contradictions are found.
        if crate::BLOCK_ON_CONFLICT.get() {
            let conflicts = crate::datalog::rule_conflicts(rule_set, "runtime");
            if let Some(arr) = conflicts.as_array()
                && !arr.is_empty()
            {
                pgrx::error!(
                    "inference halted: rule conflict detected in ruleset '{}' \
                     (set pg_ripple.block_on_conflict = off to continue despite conflicts) (PT0451)",
                    rule_set
                );
            }
        }
        derived
    }

    /// Run semi-naive inference for the named rule set and materialise derived triples.
    ///
    /// Returns a JSONB object with:
    /// - `"derived"`: total number of triples derived (i64)
    /// - `"iterations"`: number of fixpoint iterations performed (i32)
    /// - `"eliminated_rules"`: array of rule texts eliminated by subsumption checking (v0.29.0)
    /// - `"parallel_groups"`: number of independent rule groups detected in the first stratum (v0.35.0)
    /// - `"max_concurrent"`: effective worker count that would be used given `datalog_parallel_workers` (v0.35.0)
    ///
    /// Semi-naive evaluation avoids re-examining unchanged rows on each iteration,
    /// achieving iteration counts bounded by the longest derivation chain rather
    /// than the full relation size.  Subsumption checking (v0.29.0) removes rules
    /// whose body is a superset of another rule's body, reducing SQL statements per
    /// iteration.
    #[pg_extern]
    fn infer_with_stats(rule_set: default!(&str, "'custom'")) -> pgrx::JsonB {
        let (derived, iterations, eliminated, parallel_groups, max_concurrent) =
            crate::datalog::run_inference_seminaive_full(rule_set);
        let mut obj = serde_json::Map::new();
        obj.insert(
            "derived".to_owned(),
            serde_json::Value::Number(serde_json::Number::from(derived)),
        );
        obj.insert(
            "iterations".to_owned(),
            serde_json::Value::Number(serde_json::Number::from(iterations)),
        );
        obj.insert(
            "eliminated_rules".to_owned(),
            serde_json::Value::Array(
                eliminated
                    .into_iter()
                    .map(serde_json::Value::String)
                    .collect(),
            ),
        );
        obj.insert(
            "parallel_groups".to_owned(),
            serde_json::Value::Number(serde_json::Number::from(parallel_groups as i64)),
        );
        obj.insert(
            "max_concurrent".to_owned(),
            serde_json::Value::Number(serde_json::Number::from(max_concurrent as i64)),
        );
        pgrx::JsonB(serde_json::Value::Object(obj))
    }

    /// Run goal-directed inference using magic sets (v0.29.0).
    ///
    /// Materialises only facts relevant to the goal triple pattern and returns
    /// a JSONB object with:
    /// - `"derived"`: total triples derived by inference
    /// - `"iterations"`: fixpoint iteration count
    /// - `"matching"`: count of triples in the store matching the goal pattern
    ///
    /// The `goal` parameter is a whitespace-delimited triple pattern:
    /// - `?varname` — free variable (any value matches)
    /// - `<iri>` — bound IRI
    /// - `prefix:local` — bound prefixed IRI
    /// - `"literal"` — bound literal
    ///
    /// Example: `pg_ripple.infer_goal('rdfs', '?x rdf:type foaf:Person')`
    ///
    /// When `pg_ripple.magic_sets = false`, runs full materialization and
    /// filters the results post-hoc (functionally correct but slower).
    #[pg_extern]
    fn infer_goal(rule_set: &str, goal: &str) -> pgrx::JsonB {
        let goal_pattern = match crate::datalog::parse_goal(goal) {
            Ok(g) => g,
            Err(e) => {
                pgrx::warning!("infer_goal: failed to parse goal '{}': {e}", goal);
                // Return empty result on parse error.
                let mut obj = serde_json::Map::new();
                obj.insert(
                    "derived".to_owned(),
                    serde_json::Value::Number(serde_json::Number::from(0i64)),
                );
                obj.insert(
                    "iterations".to_owned(),
                    serde_json::Value::Number(serde_json::Number::from(0i32)),
                );
                obj.insert(
                    "matching".to_owned(),
                    serde_json::Value::Number(serde_json::Number::from(0i64)),
                );
                return pgrx::JsonB(serde_json::Value::Object(obj));
            }
        };

        // issue #89 (v0.112.0): validate the goal predicate against known rule
        // head predicates and base VP predicates.  Only fires when goal.p is bound.
        if let Some(pred_id) = goal_pattern.p {
            crate::datalog::validate_goal_predicate(Some(rule_set), pred_id);
        }

        let (matching, derived, iterations) =
            match crate::datalog::run_infer_goal(rule_set, &goal_pattern) {
                Ok(r) => r,
                Err(e) => {
                    pgrx::warning!("infer_goal: inference failed: {e}");
                    (0, 0, 0)
                }
            };

        let mut obj = serde_json::Map::new();
        obj.insert(
            "derived".to_owned(),
            serde_json::Value::Number(serde_json::Number::from(derived)),
        );
        obj.insert(
            "iterations".to_owned(),
            serde_json::Value::Number(serde_json::Number::from(iterations)),
        );
        obj.insert(
            "matching".to_owned(),
            serde_json::Value::Number(serde_json::Number::from(matching)),
        );
        pgrx::JsonB(serde_json::Value::Object(obj))
    }

    /// Run inference for a rule set that may contain aggregate body literals
    /// (Datalog^agg, v0.30.0).
    ///
    /// Supports `COUNT(?aggVar WHERE subject pred object) = ?resultVar` syntax
    /// in rule bodies.  Aggregate rules derive facts by grouping over a base
    /// predicate and computing COUNT, SUM, MIN, MAX, or AVG per group.
    ///
    /// Returns a JSONB object with:
    /// - `"derived"`: total triples derived (aggregate + non-aggregate)
    /// - `"aggregate_derived"`: triples derived by aggregate rules only
    /// - `"iterations"`: fixpoint iteration count for non-aggregate rules
    ///
    /// Emits a WARNING with PT510 code if aggregation-stratification is violated.
    #[pg_extern]
    fn infer_agg(rule_set: default!(&str, "'custom'")) -> pgrx::JsonB {
        let (total, agg_derived, iterations) = crate::datalog::run_inference_agg(rule_set);
        let mut obj = serde_json::Map::new();
        obj.insert(
            "derived".to_owned(),
            serde_json::Value::Number(serde_json::Number::from(total)),
        );
        obj.insert(
            "aggregate_derived".to_owned(),
            serde_json::Value::Number(serde_json::Number::from(agg_derived)),
        );
        obj.insert(
            "iterations".to_owned(),
            serde_json::Value::Number(serde_json::Number::from(iterations)),
        );
        pgrx::JsonB(serde_json::Value::Object(obj))
    }

    /// Run inference for a rule set, restricted to rules that can contribute to
    /// the given demand patterns (demand transformation, v0.31.0).
    ///
    /// `demands` is a JSONB array of goal patterns, e.g.:
    /// ```json
    /// [{"p": "<https://example.org/transitive>"}, {"s": "<https://ex.org/a>", "p": "<https://ex.org/childOf>"}]
    /// ```
    /// Each element has optional `"s"`, `"p"`, `"o"` keys with IRI values.
    /// Omitted keys are treated as free variables.
    ///
    /// When `demands` is an empty array (`'[]'`), runs full inference (same as
    /// `infer()`).
    ///
    /// Returns a JSONB object with:
    /// - `"derived"`: total triples derived
    /// - `"iterations"`: fixpoint iteration count
    /// - `"demand_predicates"`: array of predicate IRI strings that were used as
    ///   demand seeds (decoded from dictionary)
    ///
    /// Also applies `owl:sameAs` canonicalization when
    /// `pg_ripple.sameas_reasoning` is `on` (default).
    #[pg_extern]
    fn infer_demand(
        rule_set: default!(&str, "'custom'"),
        demands: default!(pgrx::JsonB, "'[]'::jsonb"),
    ) -> pgrx::JsonB {
        let demands_str = demands.0.to_string();
        let demand_specs = crate::datalog::parse_demands_json(&demands_str);

        let (derived, iterations, demand_pred_ids) =
            crate::datalog::run_infer_demand(rule_set, &demand_specs);

        // Decode demand predicate IDs back to IRI strings for the output.
        let demand_preds_json: serde_json::Value = if demand_pred_ids.is_empty() {
            serde_json::Value::Array(vec![])
        } else {
            let decoded: Vec<serde_json::Value> = demand_pred_ids
                .iter()
                .filter_map(|&id| crate::dictionary::decode(id).map(serde_json::Value::String))
                .collect();
            serde_json::Value::Array(decoded)
        };

        let mut obj = serde_json::Map::new();
        obj.insert(
            "derived".to_owned(),
            serde_json::Value::Number(serde_json::Number::from(derived)),
        );
        obj.insert(
            "iterations".to_owned(),
            serde_json::Value::Number(serde_json::Number::from(iterations)),
        );
        obj.insert("demand_predicates".to_owned(), demand_preds_json);
        pgrx::JsonB(serde_json::Value::Object(obj))
    }

    /// Return statistics for the Datalog rule plan cache (v0.30.0).
    ///
    /// Each row has:
    /// - `rule_set TEXT` — the rule set name
    /// - `hits BIGINT` — number of times the cached SQL was used
    /// - `misses BIGINT` — number of times the cache was consulted but missed
    /// - `entries INT` — total number of entries currently in the cache
    #[pg_extern]
    fn rule_plan_cache_stats() -> TableIterator<
        'static,
        (
            name!(rule_set, String),
            name!(hits, i64),
            name!(misses, i64),
            name!(entries, i32),
        ),
    > {
        let stats = crate::datalog::cache::stats();
        TableIterator::new(
            stats
                .into_iter()
                .map(|s| (s.rule_set, s.hits, s.misses, s.entries)),
        )
    }

    // ── v0.32.0: Well-Founded Semantics ───────────────────────────────────────

    /// Run well-founded semantics inference for the named rule set (v0.32.0).
    ///
    /// For **stratifiable programs** (no cyclic negation): identical to
    /// `infer_with_stats()` — all derived facts have `certainty = 'true'`.
    ///
    /// For **non-stratifiable programs** (cyclic negation detected):
    /// - Facts derivable from purely positive rules → `certainty = 'true'`
    ///   (materialised into VP tables like normal inference).
    /// - Facts only derivable via negation of uncertain atoms → `certainty = 'unknown'`
    ///   (reported in the JSONB output but NOT materialised into VP tables).
    ///
    /// Returns a JSONB object with:
    /// - `"derived"`: total facts (certain + unknown)
    /// - `"certain"`: facts with `certainty = 'true'`
    /// - `"unknown"`: facts with `certainty = 'unknown'`
    /// - `"iterations"`: number of fixpoint passes performed
    /// - `"stratifiable"`: `true` if the program is stratifiable, `false` otherwise
    ///
    /// GUC: `pg_ripple.wfs_max_iterations` (default 100) — safety cap per pass.
    /// Emits WARNING PT520 if a pass does not converge within the limit.
    #[pg_extern]
    fn infer_wfs(rule_set: default!(&str, "'custom'")) -> pgrx::JsonB {
        // Ensure the tabling catalog exists before any call that may try to
        // invalidate it (tabling_invalidate_all checks for the table first).
        crate::datalog::ensure_tabling_catalog();

        // Check tabling cache for a previous result.
        let goal_hash = crate::datalog::compute_goal_hash(&format!("wfs:{rule_set}"));
        if let Some(cached) = crate::datalog::tabling_lookup(goal_hash) {
            return pgrx::JsonB(cached);
        }

        // Cache miss — run WFS inference.
        let start = std::time::Instant::now();
        let (certain, unknown, total, iters, stratifiable) = crate::datalog::run_wfs(rule_set);
        let result = crate::datalog::build_wfs_jsonb(certain, unknown, total, iters, stratifiable);
        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;

        // Store result in the tabling cache for future calls.
        crate::datalog::tabling_store(goal_hash, &result.0, elapsed_ms);

        result
    }

    // ── v0.36.0: WCOJ & Lattice-Based Datalog ────────────────────────────────

    /// Detect whether a SPARQL triangle query is cyclic (v0.36.0).
    ///
    /// Returns `true` if the provided BGP variable pattern sets contain a cycle
    /// (i.e. the variable adjacency graph has a back-edge).  Used internally
    /// by the SPARQL→SQL translator; also exposed for testing and introspection.
    ///
    /// Each row in `pattern_vars_json` is a JSON array of variable name strings
    /// representing the variables co-occurring in one triple pattern.
    ///
    /// Example:
    /// ```sql
    /// SELECT pg_ripple.wcoj_is_cyclic('[["a","b"],["b","c"],["c","a"]]');
    /// -- returns true
    /// ```
    #[pg_extern]
    fn wcoj_is_cyclic(pattern_vars_json: &str) -> bool {
        let patterns: Vec<Vec<String>> = match serde_json::from_str(pattern_vars_json) {
            Ok(v) => v,
            Err(e) => pgrx::error!("wcoj_is_cyclic: invalid JSON input: {e}"),
        };
        crate::sparql::wcoj::detect_cyclic_bgp(&patterns)
    }

    /// Run a triangle-detection query on a VP predicate and return result stats (v0.36.0).
    ///
    /// Returns JSONB with `{"triangle_count": N, "wcoj_applied": bool, "predicate_iri": "..."}`.
    ///
    /// `predicate_iri` — the predicate IRI (without angle brackets) to use for all
    /// three edges of the triangle.
    ///
    /// This function is primarily used by `benchmarks/wcoj.sql` to compare
    /// WCOJ vs. standard planner execution.
    #[pg_extern]
    fn wcoj_triangle_query(predicate_iri: &str) -> pgrx::JsonB {
        let result = crate::sparql::wcoj::run_triangle_query(predicate_iri);
        pgrx::JsonB(serde_json::json!({
            "triangle_count": result.triangle_count,
            "wcoj_applied":   result.wcoj_applied,
            "predicate_iri":  result.predicate_iri
        }))
    }
}
