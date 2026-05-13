//! Datalog rule management: load, list, add, remove, enable/disable rule sets.
//! (extracted from datalog_api/mod.rs in v0.114.0)

#[pgrx::pg_schema]
mod pg_ripple {
    use pgrx::prelude::*;

    // ── Datalog Reasoning Engine (v0.10.0) ────────────────────────────────────

    /// Load Datalog rules from a text string.
    ///
    /// `rules` is a Turtle-flavoured Datalog rule set.
    /// `rule_set` is the name for this group of rules (default: 'custom').
    /// Returns the number of rules stored.
    #[pg_extern]
    fn load_rules(rules: &str, rule_set: default!(&str, "'custom'")) -> i64 {
        crate::datalog::builtins::register_standard_prefixes();
        // Invalidate plan cache for this rule set (v0.30.0).
        crate::datalog::cache::invalidate(rule_set);
        // Invalidate tabling cache (v0.32.0).
        crate::datalog::tabling_invalidate_all();
        let rule_set_ir = match crate::datalog::parse_rules(rules, rule_set) {
            Ok(rs) => rs,
            Err(e) => pgrx::error!("rule parse error: {e}"),
        };
        let count = crate::datalog::store_rules(rule_set, &rule_set_ir.rules);
        // v0.103.0 CONFLICT-01: run static conflict analysis on load when the
        // GUC pg_ripple.rule_conflict_check_on_load is enabled.
        if crate::RULE_CONFLICT_CHECK_ON_LOAD.get() {
            let conflicts = crate::datalog::rule_conflicts(rule_set, "static");
            if let Some(arr) = conflicts.as_array() {
                for c in arr {
                    let pattern = c
                        .get("conflicting_pattern")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown conflict");
                    let ctype = c
                        .get("conflict_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    pgrx::warning!(
                        "rule conflict detected in rule set '{}' ({}): {}",
                        rule_set,
                        ctype,
                        pattern
                    );
                }
            }
        }
        count
    }

    /// Load a built-in rule set by name.
    ///
    /// Supported names: `'rdfs'`, `'owl-rl'`.
    /// Returns the number of rules stored.
    #[pg_extern]
    fn load_rules_builtin(name: &str) -> i64 {
        crate::datalog::builtins::register_standard_prefixes();
        let text = match crate::datalog::builtins::get_builtin_rules(name) {
            Ok(t) => t,
            Err(e) => pgrx::error!("{e}"),
        };
        let rule_set_ir = match crate::datalog::parse_rules(text, name) {
            Ok(rs) => rs,
            Err(e) => pgrx::error!("built-in rule parse error: {e}"),
        };
        crate::datalog::store_rules(name, &rule_set_ir.rules)
    }

    /// List all stored Datalog rules as JSONB rows.
    ///
    /// Returns one row per rule with fields: id, rule_set, rule_text, head_pred,
    /// stratum, is_recursive, active.
    #[pg_extern]
    fn list_rules() -> pgrx::JsonB {
        crate::datalog::ensure_catalog();
        let rows = pgrx::Spi::connect(|client| {
            client
                .select(
                    "SELECT id, rule_set, rule_text, head_pred, stratum, is_recursive, active \
                     FROM _pg_ripple.rules \
                     ORDER BY rule_set, stratum, id",
                    None,
                    &[],
                )
                .unwrap_or_else(|e| pgrx::error!("list_rules SPI error: {e}"))
                .map(|row| {
                    let mut obj = serde_json::Map::new();
                    obj.insert(
                        "id".to_owned(),
                        row.get::<i64>(1)
                            .ok()
                            .flatten()
                            .map(serde_json::Value::from)
                            .unwrap_or(serde_json::Value::Null),
                    );
                    obj.insert(
                        "rule_set".to_owned(),
                        row.get::<String>(2)
                            .ok()
                            .flatten()
                            .map(serde_json::Value::String)
                            .unwrap_or(serde_json::Value::Null),
                    );
                    obj.insert(
                        "rule_text".to_owned(),
                        row.get::<String>(3)
                            .ok()
                            .flatten()
                            .map(serde_json::Value::String)
                            .unwrap_or(serde_json::Value::Null),
                    );
                    obj.insert(
                        "stratum".to_owned(),
                        row.get::<i32>(5)
                            .ok()
                            .flatten()
                            .map(|v| serde_json::Value::from(v as i64))
                            .unwrap_or(serde_json::Value::Null),
                    );
                    obj.insert(
                        "is_recursive".to_owned(),
                        row.get::<bool>(6)
                            .ok()
                            .flatten()
                            .map(serde_json::Value::Bool)
                            .unwrap_or(serde_json::Value::Null),
                    );
                    obj.insert(
                        "active".to_owned(),
                        row.get::<bool>(7)
                            .ok()
                            .flatten()
                            .map(serde_json::Value::Bool)
                            .unwrap_or(serde_json::Value::Null),
                    );
                    serde_json::Value::Object(obj)
                })
                .collect::<Vec<_>>()
        });
        pgrx::JsonB(serde_json::Value::Array(rows))
    }

    /// Drop all rules in the named rule set.
    ///
    /// Returns the number of rules deleted.
    #[pg_extern]
    fn drop_rules(rule_set: &str) -> i64 {
        crate::datalog::ensure_catalog();
        // Invalidate plan cache for this rule set (v0.30.0).
        crate::datalog::cache::invalidate(rule_set);
        // Invalidate tabling cache (v0.32.0).
        crate::datalog::tabling_invalidate_all();
        pgrx::Spi::get_one_with_args::<i64>(
            "WITH deleted AS ( \
                 DELETE FROM _pg_ripple.rules WHERE rule_set = $1 RETURNING 1 \
             ) SELECT count(*) FROM deleted",
            &[pgrx::datum::DatumWithOid::from(rule_set)],
        )
        .unwrap_or(None)
        .unwrap_or(0)
    }

    // ── v0.34.0: Incremental rule updates ────────────────────────────────────

    /// Add a single rule to an existing rule set (v0.34.0).
    ///
    /// The rule is parsed, stored in the catalog, and its head predicate gets
    /// one fresh seed pass against the current VP tables.  Other derived
    /// predicates are not affected.  Returns the new rule's catalog ID.
    ///
    /// This is more efficient than calling `drop_rules()` + `load_rules()` when
    /// adding rules to a large live rule set, because only the new rule's derived
    /// predicate needs re-evaluation.
    #[pg_extern]
    fn add_rule(rule_set: &str, rule_text: &str) -> i64 {
        match crate::datalog::add_rule_to_set(rule_set, rule_text) {
            Ok(id) => id,
            Err(e) => pgrx::error!("add_rule error: {e}"),
        }
    }

    /// Remove a single rule by its catalog ID (v0.34.0).
    ///
    /// The rule is marked inactive and any derived facts solely supported by it
    /// are retracted using DRed (when `pg_ripple.dred_enabled = true`).  Falls
    /// back to full re-materialization when DRed is disabled or detects a cycle
    /// (error code PT530).  Returns the number of derived triples permanently
    /// retracted.
    ///
    /// Obtain the rule ID from `pg_ripple.list_rules()`.
    #[pg_extern]
    fn remove_rule(rule_id: i64) -> i64 {
        match crate::datalog::remove_rule_by_id(rule_id) {
            Ok(n) => n,
            Err(e) => pgrx::error!("remove_rule error: {e}"),
        }
    }

    /// Invoke DRed incremental retraction for a deleted base triple (v0.34.0).
    ///
    /// Normally called automatically by the CDC delete path.  This function
    /// exposes the DRed algorithm for testing and manual invocation.
    ///
    /// `pred_id` — dictionary ID of the deleted triple's predicate.
    /// `s_val`   — dictionary ID of the deleted triple's subject.
    /// `o_val`   — dictionary ID of the deleted triple's object.
    /// `g_val`   — dictionary ID of the deleted triple's graph (0 = default).
    ///
    /// Returns the number of derived triples permanently retracted.
    #[pg_extern]
    fn dred_on_delete(pred_id: i64, s_val: i64, o_val: i64, g_val: i64) -> i64 {
        crate::datalog::run_dred_on_delete(pred_id, s_val, o_val, g_val)
    }

    /// Enable a named rule set (set active = true).
    #[pg_extern]
    fn enable_rule_set(name: &str) {
        crate::datalog::ensure_catalog();
        let _ = pgrx::Spi::run_with_args(
            "UPDATE _pg_ripple.rules SET active = true WHERE rule_set = $1; \
             UPDATE _pg_ripple.rule_sets SET active = true WHERE name = $1",
            &[pgrx::datum::DatumWithOid::from(name)],
        );
    }

    /// Disable a named rule set (set active = false) without dropping it.
    #[pg_extern]
    fn disable_rule_set(name: &str) {
        crate::datalog::ensure_catalog();
        let _ = pgrx::Spi::run_with_args(
            "UPDATE _pg_ripple.rules SET active = false WHERE rule_set = $1; \
             UPDATE _pg_ripple.rule_sets SET active = false WHERE name = $1",
            &[pgrx::datum::DatumWithOid::from(name)],
        );
    }
}
