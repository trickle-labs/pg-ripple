//! Datalog lattice/tabling, retract, check_constraints, hypothetical inference, rule_conflicts.
//! (extracted from datalog_api/mod.rs in v0.114.0)

#[pgrx::pg_schema]
mod pg_ripple {
    use pgrx::prelude::*;

    /// Register a user-defined lattice type for Datalog^L rules (v0.36.0).
    ///
    /// A lattice is an algebraic structure (L, ⊔) where the join operation ⊔
    /// is commutative, associative, and idempotent, with a bottom element ⊥.
    /// Fixpoint computation on a lattice terminates when the ascending chain
    /// condition holds.
    ///
    /// # Parameters
    ///
    /// - `name` — unique lattice identifier (e.g. `'trust'`, `'my_lattice'`).
    /// - `join_fn` — PostgreSQL aggregate function name implementing the join
    ///   (e.g. `'min'`, `'max'`, `'array_agg'`, `'my_custom_agg'`).
    ///   Must be commutative and associative.
    /// - `bottom` — bottom element as a text string (e.g. `'9223372036854775807'`
    ///   for a MinLattice over integer trust scores).
    ///
    /// Returns `true` if the lattice was newly registered, `false` if it already
    /// existed.
    ///
    /// # Built-in lattices
    ///
    /// The following lattices are pre-registered and do not need to be created:
    /// - `'min'` — MinLattice (join = MIN, bottom = i64::MAX)
    /// - `'max'` — MaxLattice (join = MAX, bottom = i64::MIN)
    /// - `'set'` — SetLattice (join = UNION via array_agg, bottom = {})
    /// - `'interval'` — IntervalLattice (join = MAX, bottom = 0)
    ///
    /// # Example
    ///
    /// ```sql
    /// -- Register a MinLattice for trust propagation over [0.0, 1.0] scores.
    /// SELECT pg_ripple.create_lattice('trust_score', 'min', '1.0');
    /// ```
    #[pg_extern]
    fn create_lattice(name: &str, join_fn: &str, bottom: &str) -> bool {
        crate::datalog::register_lattice(name, join_fn, bottom)
    }

    /// List all registered lattice types as JSONB (v0.36.0).
    ///
    /// Returns an array of `{"name": "...", "join_fn": "...", "bottom": "...", "builtin": bool}`.
    #[pg_extern]
    fn list_lattices() -> pgrx::JsonB {
        crate::datalog::ensure_lattice_catalog();
        let rows: Vec<serde_json::Value> = pgrx::Spi::connect(|c| {
            c.select(
                "SELECT name, join_fn, bottom, builtin \
                 FROM _pg_ripple.lattice_types \
                 ORDER BY builtin DESC, name",
                None,
                &[],
            )
            .unwrap_or_else(|e| pgrx::error!("list_lattices: SPI error: {e}"))
            .map(|row| {
                let name: String = row.get::<String>(1).ok().flatten().unwrap_or_default();
                let join_fn: String = row.get::<String>(2).ok().flatten().unwrap_or_default();
                let bottom: String = row.get::<String>(3).ok().flatten().unwrap_or_default();
                let builtin: bool = row.get::<bool>(4).ok().flatten().unwrap_or(false);
                serde_json::json!({
                    "name":    name,
                    "join_fn": join_fn,
                    "bottom":  bottom,
                    "builtin": builtin
                })
            })
            .collect()
        });
        pgrx::JsonB(serde_json::Value::Array(rows))
    }

    /// Run lattice-based Datalog inference for a rule set (v0.36.0).
    ///
    /// Executes a monotone fixpoint computation over the rules in `rule_set`
    /// using `lattice_name` as the lattice type for head derivations.
    ///
    /// Terminates when no new values are derived (convergence), or when
    /// `pg_ripple.lattice_max_iterations` is reached (emits WARNING PT540 and
    /// returns partial results).
    ///
    /// Returns JSONB with:
    /// - `"derived"` — total new lattice values written
    /// - `"iterations"` — fixpoint iterations performed
    /// - `"lattice"` — name of the lattice used
    /// - `"rule_set"` — name of the rule set evaluated
    ///
    /// # Example
    ///
    /// ```sql
    /// -- Trust propagation: min-cost path through a social graph.
    /// SELECT pg_ripple.load_rules($$
    ///     ?x <ex:trust> ?min_t :-
    ///         ?x <ex:knows> ?y, ?y <ex:trust> ?t1, ?x <ex:directTrust> ?t2,
    ///         COUNT(?z WHERE ?z <ex:knows> ?y) AS min_t = LEAST(?t1, ?t2) .
    /// $$, 'trust_rules');
    /// SELECT pg_ripple.infer_lattice('trust_rules', 'min');
    /// ```
    #[pg_extern]
    fn infer_lattice(
        rule_set: default!(&str, "'custom'"),
        lattice_name: default!(&str, "'min'"),
    ) -> pgrx::JsonB {
        pgrx::JsonB(crate::datalog::run_infer_lattice(rule_set, lattice_name))
    }

    // ── v0.32.0: Tabling / memoisation ───────────────────────────────────────

    /// Return statistics for the tabling / memoisation cache (v0.32.0).
    ///
    /// Each row has:
    /// - `goal_hash BIGINT` — XXH3-64 hash of the cached goal string
    /// - `hits BIGINT` — number of cache hits for this entry
    /// - `computed_ms FLOAT` — wall-clock time (ms) for the original computation
    /// - `cached_at TIMESTAMPTZ` — when the entry was last written
    #[pg_extern]
    fn tabling_stats() -> TableIterator<
        'static,
        (
            name!(goal_hash, i64),
            name!(hits, i64),
            name!(computed_ms, f64),
            name!(cached_at, String),
        ),
    > {
        let rows = crate::datalog::tabling_stats_impl();
        TableIterator::new(rows)
    }

    /// List all named rule sets as a table (name, active, rule_count, created_at).
    ///
    /// Returns one row per rule set stored in `_pg_ripple.rule_sets`, including
    /// disabled rule sets.  Use `pg_ripple.enable_rule_set()` /
    /// `pg_ripple.disable_rule_set()` to toggle sets without dropping them.
    #[pg_extern]
    fn list_rule_sets() -> TableIterator<
        'static,
        (
            name!(rule_set, String),
            name!(active, bool),
            name!(rule_count, i64),
            name!(created_at, String),
        ),
    > {
        crate::datalog::ensure_catalog();
        let rows: Vec<(String, bool, i64, String)> = pgrx::Spi::connect(|c| {
            c.select(
                "SELECT rs.name, rs.active, \
                     COUNT(r.id) AS rule_count, \
                     to_char(rs.created_at, 'YYYY-MM-DD HH24:MI:SS') \
                 FROM _pg_ripple.rule_sets rs \
                 LEFT JOIN _pg_ripple.rules r ON r.rule_set = rs.name \
                 GROUP BY rs.name, rs.active, rs.created_at \
                 ORDER BY rs.created_at, rs.name",
                None,
                &[],
            )
            .unwrap_or_else(|e| pgrx::error!("list_rule_sets SPI error: {e}"))
            .map(|row| {
                let name: String = row.get::<String>(1).ok().flatten().unwrap_or_default();
                let active: bool = row.get::<bool>(2).ok().flatten().unwrap_or(true);
                let count: i64 = row.get::<i64>(3).ok().flatten().unwrap_or(0);
                let ts: String = row.get::<String>(4).ok().flatten().unwrap_or_default();
                (name, active, count, ts)
            })
            .collect()
        });
        TableIterator::new(rows)
    }

    /// Retract all materialised (inferred) triples for a named rule set.
    ///
    /// Deletes every triple with `source = 1` (derived) that was produced by
    /// any rule in the given rule set.  Only triples whose head predicate
    /// belongs exclusively to this rule set are deleted; predicates shared
    /// across rule sets are left untouched.
    ///
    /// This is the bulk-retraction counterpart to `pg_ripple.infer()`.  For
    /// fine-grained per-triple retraction use `pg_ripple.remove_rule()` with
    /// DRed enabled.
    ///
    /// Returns the total number of triples deleted.
    #[pg_extern]
    fn retract_inferred(rule_set: &str) -> i64 {
        crate::datalog::ensure_catalog();
        // Collect all head predicate IDs for the rule set.
        let pred_ids: Vec<i64> = pgrx::Spi::connect(|c| {
            c.select(
                "SELECT DISTINCT head_pred FROM _pg_ripple.rules \
                 WHERE rule_set = $1 AND head_pred IS NOT NULL",
                None,
                &[pgrx::datum::DatumWithOid::from(rule_set)],
            )
            .unwrap_or_else(|e| pgrx::error!("retract_inferred: SPI error: {e}"))
            .map(|row| row.get::<i64>(1).ok().flatten().unwrap_or(0))
            .filter(|&id| id != 0)
            .collect()
        });

        let mut total_deleted: i64 = 0;

        for pred_id in pred_ids {
            // Check if there is a dedicated VP table for this predicate.
            let has_vp: bool = pgrx::Spi::get_one_with_args::<bool>(
                "SELECT EXISTS(\
                     SELECT 1 FROM _pg_ripple.predicates WHERE id = $1 AND table_oid IS NOT NULL\
                 )",
                &[pgrx::datum::DatumWithOid::from(pred_id)],
            )
            .unwrap_or(None)
            .unwrap_or(false);

            if has_vp {
                // Delete from delta table (inferred triples land there).
                let delta_tbl = format!("_pg_ripple.vp_{pred_id}_delta");
                let deleted_delta = pgrx::Spi::get_one_with_args::<i64>(
                    &format!(
                        "WITH d AS (DELETE FROM {delta_tbl} WHERE source = 1 RETURNING 1) \
                              SELECT count(*) FROM d"
                    ),
                    &[],
                )
                .unwrap_or(None)
                .unwrap_or(0);
                total_deleted += deleted_delta;
            }

            // Also clean up vp_rare for this predicate.
            let deleted_rare = pgrx::Spi::get_one_with_args::<i64>(
                "WITH d AS (DELETE FROM _pg_ripple.vp_rare WHERE p = $1 AND source = 1 RETURNING 1) \
                 SELECT count(*) FROM d",
                &[pgrx::datum::DatumWithOid::from(pred_id)],
            )
            .unwrap_or(None)
            .unwrap_or(0);
            total_deleted += deleted_rare;
        }

        // Invalidate caches so subsequent queries reflect the retraction.
        crate::datalog::cache::invalidate(rule_set);
        crate::datalog::tabling_invalidate_all();

        total_deleted
    }

    /// Check all active constraint rules and return violations as JSONB.
    ///
    /// Each element has fields: `rule` (text), `violated` (bool).
    /// Pass `rule_set` to check only that rule set; pass NULL to check all.
    #[pg_extern]
    fn check_constraints(rule_set: default!(Option<&str>, "NULL")) -> pgrx::JsonB {
        let violations = crate::datalog::check_all_constraints(rule_set);
        pgrx::JsonB(serde_json::Value::Array(
            violations.into_iter().map(|v| v.0).collect(),
        ))
    }

    /// Run what-if inference on hypothetical facts without touching real VP tables.
    ///
    /// Asserts and retracts the triples in `hypotheses` in an isolated savepoint,
    /// runs semi-naive Datalog inference for `rules`, then rolls everything back.
    /// Returns a JSONB diff of what *would* be derived or retracted.
    ///
    /// # Argument format
    ///
    /// ```json
    /// {
    ///   "assert":  [{"s": "<iri>", "p": "<iri>", "o": "<iri-or-literal>"}],
    ///   "retract": [{"s": "<iri>", "p": "<iri>", "o": "<iri-or-literal>"}]
    /// }
    /// ```
    ///
    /// IRI values may be bare (`http://...`) or angle-bracketed (`<http://...>`).
    ///
    /// # Return value
    ///
    /// ```json
    /// {
    ///   "derived":   [{"s": "...", "p": "...", "o": "..."}],
    ///   "retracted": [{"s": "...", "p": "...", "o": "..."}]
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// - PT0450: total hypothesis size exceeds `pg_ripple.hypothetical_max_assertions`.
    ///
    /// # Isolation guarantee
    ///
    /// No row is written to any VP table after this function returns.
    /// All changes exist only inside a PostgreSQL SAVEPOINT that is always
    /// rolled back before the result is returned.
    #[pg_extern]
    fn hypothetical_inference(
        hypotheses: pgrx::JsonB,
        rules: default!(&str, "'default'"),
    ) -> pgrx::JsonB {
        pgrx::JsonB(crate::hypothetical::hypothetical_inference_impl(
            hypotheses.0,
            rules,
        ))
    }

    // ── v0.103.0 Conflict Detection ──────────────────────────────────────────

    /// Detect conflicting rules in a rule set.
    ///
    /// `ruleset` — name of the rule set to analyse (matches `rule_set` in
    ///             `_pg_ripple.rules`).
    /// `mode`    — `'static'` (default) for structural analysis over the rule
    ///             AST and the SHACL shape catalog; `'runtime'` to scan
    ///             `_pg_ripple.derivations` for already-derived contradictions.
    ///
    /// Returns a JSONB array of conflict objects; an empty array means no
    /// conflicts were found.
    ///
    /// Each conflict object has the shape:
    /// ```json
    /// {
    ///   "mode": "static" | "runtime",
    ///   "rule_a": "<rule text>",
    ///   "rule_b": "<rule text or null>",
    ///   "conflict_type": "same_head_opposing_values | rule_vs_shacl | runtime_violation",
    ///   "head_predicate": "<IRI>",
    ///   "conflicting_pattern": "<description>",
    ///   "shacl_constraint": "<shape IRI or null>",
    ///   "example_triple": null
    /// }
    /// ```
    ///
    /// # Mode: `'static'`
    ///
    /// Detects two classes of structural contradiction at rule registration
    /// time or on demand (no VP table reads):
    ///
    /// 1. **`same_head_opposing_values`**: pairs of rules with the same head
    ///    predicate and different constant object terms — e.g. one rule derives
    ///    `?x ex:eligible "true"` and another derives `?x ex:eligible "false"`.
    ///
    /// 2. **`rule_vs_shacl`**: a rule that derives triples for a predicate that
    ///    is also referenced by a `sh:not`, `sh:disjoint`, or `sh:in` SHACL
    ///    constraint.
    ///
    /// # Mode: `'runtime'`
    ///
    /// Queries `_pg_ripple.derivations` (requires
    /// `pg_ripple.record_derivations = on` before calling `infer()`) joined
    /// with `_pg_ripple.vp_rare` to find already-derived contradictions:
    ///
    /// 1. Same subject, same predicate, two different inferred values.
    /// 2. `sh:disjoint` violations where the same subject has inferred values
    ///    for both disjoint properties.
    ///
    /// # Error codes
    ///
    /// - PT0451: raised during `infer()` when `pg_ripple.block_on_conflict = on`
    ///   and a runtime contradiction is found.
    #[pg_extern]
    fn rule_conflicts(ruleset: &str, mode: default!(&str, "'static'")) -> pgrx::JsonB {
        crate::datalog::builtins::register_standard_prefixes();
        pgrx::JsonB(crate::datalog::rule_conflicts(ruleset, mode))
    }
}
