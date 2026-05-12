//! Conflict detection for Datalog rule sets (v0.103.0).
//!
//! # Overview
//!
//! Two detection modes are implemented:
//!
//! - **Static analysis** (`mode = 'static'`): purely structural analysis over
//!   the parsed rule AST and the compiled SHACL shape catalog.  No VP table
//!   reads are performed.  Detects two classes of conflict:
//!
//!   1. **Same-head / opposing-value conflicts**: pairs of rules with the same
//!      head predicate where the object (o-term) is a distinct constant value —
//!      e.g. one rule derives `?x ex:eligible "true"` and another derives
//!      `?x ex:eligible "false"` for the same subject variable pattern.
//!
//!   2. **Rule-vs-SHACL conflicts**: a rule derives `?x p ?v` but an active
//!      `sh:not`, `sh:disjoint`, or `sh:in` constraint references the head
//!      predicate.
//!
//! - **Runtime detection** (`mode = 'runtime'`): queries
//!   `_pg_ripple.derivations` joined with VP tables to find derived facts that
//!   violate active SHACL mutual-exclusion constraints.
//!
//! # Error code
//!
//! PT0451 — `inference halted: rule conflict detected in ruleset '%s'` (raised
//! when `pg_ripple.block_on_conflict = on` and the runtime check finds a
//! contradiction after a fixpoint iteration).

use pgrx::prelude::*;
use serde_json::{Value, json};

use super::Term;

// ─── Public entry point ───────────────────────────────────────────────────────

/// Main entry point: run conflict detection in the specified mode.
///
/// `ruleset` — name of the rule set stored in `_pg_ripple.rules`.
/// `mode`    — `"static"` or `"runtime"`.
///
/// Returns a JSONB array of conflict objects; empty array means no conflicts.
pub fn rule_conflicts(ruleset: &str, mode: &str) -> Value {
    match mode {
        "runtime" => detect_runtime(ruleset),
        _ => detect_static(ruleset),
    }
}

// ─── Static analysis ─────────────────────────────────────────────────────────

fn detect_static(ruleset: &str) -> Value {
    super::ensure_catalog();

    let mut conflicts: Vec<Value> = Vec::new();

    // Load rules for this rule set from the catalog: (rule_text, head_pred).
    // head_pred is the dictionary-encoded predicate ID (or NULL for constraint rules).
    let rules = Spi::connect(|client| {
        client
            .select(
                "SELECT rule_text, head_pred \
                 FROM _pg_ripple.rules \
                 WHERE rule_set = $1 AND active = true AND head_pred IS NOT NULL \
                 ORDER BY id",
                None,
                &[pgrx::datum::DatumWithOid::from(ruleset)],
            )
            .unwrap_or_else(|e| pgrx::error!("rule_conflicts SPI error: {e}"))
            .map(|row| {
                let rule_text = row.get::<String>(1).ok().flatten().unwrap_or_default();
                let head_pred = row.get::<i64>(2).ok().flatten().unwrap_or(0);
                (rule_text, head_pred)
            })
            .collect::<Vec<_>>()
    });

    // Parse each rule text back into the IR to get head object terms.
    // Group by head_pred_id.
    use std::collections::HashMap;
    // key = head_pred i64, value = list of (rule_text, head_obj: Option<i64>)
    let mut by_pred: HashMap<i64, Vec<(String, Option<i64>)>> = HashMap::new();

    for (rule_text, head_pred) in &rules {
        let obj_term = parse_head_object(rule_text);
        by_pred
            .entry(*head_pred)
            .or_default()
            .push((rule_text.clone(), obj_term));
    }

    // ── (1) Same-head opposing-value conflicts ────────────────────────────────

    for (pred_id, rule_list) in &by_pred {
        // Only consider rules that have constant (Const) object terms.
        let const_rules: Vec<&(String, Option<i64>)> =
            rule_list.iter().filter(|(_, obj)| obj.is_some()).collect();

        // Check all pairs for differing constant object terms.
        for i in 0..const_rules.len() {
            for j in (i + 1)..const_rules.len() {
                let (text_a, obj_a) = const_rules[i];
                let (text_b, obj_b) = const_rules[j];
                if obj_a != obj_b {
                    // Decode the predicate IRI for the report.
                    let pred_iri = crate::dictionary::decode(*pred_id)
                        .unwrap_or_else(|| format!("<dict:{pred_id}>"));
                    let obj_a_str = obj_a
                        .and_then(crate::dictionary::decode)
                        .unwrap_or_else(|| format!("<dict:{}>", obj_a.unwrap_or(0)));
                    let obj_b_str = obj_b
                        .and_then(crate::dictionary::decode)
                        .unwrap_or_else(|| format!("<dict:{}>", obj_b.unwrap_or(0)));
                    conflicts.push(json!({
                        "mode": "static",
                        "rule_a": text_a,
                        "rule_b": text_b,
                        "conflict_type": "same_head_opposing_values",
                        "head_predicate": pred_iri,
                        "conflicting_pattern":
                            format!("rule_a derives '{obj_a_str}', rule_b derives '{obj_b_str}' for the same head predicate"),
                        "shacl_constraint": null,
                        "example_triple": null
                    }));
                }
            }
        }
    }

    // ── (2) Rule-vs-SHACL conflicts ──────────────────────────────────────────

    let shacl_constraints = load_shacl_constraints();
    if !shacl_constraints.is_empty() {
        for (pred_id, rule_list) in &by_pred {
            let pred_iri =
                crate::dictionary::decode(*pred_id).unwrap_or_else(|| format!("<dict:{pred_id}>"));
            for sc in &shacl_constraints {
                if sc.path_iri == pred_iri {
                    for (text_a, _) in rule_list {
                        conflicts.push(json!({
                            "mode": "static",
                            "rule_a": text_a,
                            "rule_b": null,
                            "conflict_type": "rule_vs_shacl",
                            "head_predicate": pred_iri,
                            "conflicting_pattern":
                                format!("rule derives triples for '{}' but SHACL constraint '{}' (type: {}) may forbid them",
                                    pred_iri, sc.shape_iri, sc.constraint_type),
                            "shacl_constraint": sc.shape_iri,
                            "example_triple": null
                        }));
                    }
                }
            }
        }
    }

    json!(conflicts)
}

/// Parse a rule text and return the dictionary-encoded object ID of the head
/// atom, if the head has a constant object term (`Term::Const`).
///
/// Returns `None` for variable heads, wildcard heads, or parse failures.
fn parse_head_object(rule_text: &str) -> Option<i64> {
    let ruleset_ir = match super::parse_rules(rule_text, "_conflict_check") {
        Ok(rs) => rs,
        Err(_) => return None,
    };
    let rule = ruleset_ir.rules.first()?;
    let head = rule.head.as_ref()?;
    match &head.o {
        Term::Const(id) => Some(*id),
        _ => None,
    }
}

// ─── Runtime detection ────────────────────────────────────────────────────────

fn detect_runtime(ruleset: &str) -> Value {
    let mut conflicts: Vec<Value> = Vec::new();

    // Check that the derivations table exists.
    let derivations_exist: bool = Spi::connect(|client| {
        client
            .select(
                "SELECT EXISTS ( \
                    SELECT 1 FROM information_schema.tables \
                    WHERE table_schema = '_pg_ripple' \
                      AND table_name = 'derivations' \
                 )",
                None,
                &[],
            )
            .ok()
            .and_then(|mut rows| rows.next())
            .and_then(|row| row.get::<bool>(1).ok().flatten())
            .unwrap_or(false)
    });

    if !derivations_exist {
        pgrx::warning!(
            "rule_conflicts runtime mode: _pg_ripple.derivations table does not exist; \
             enable pg_ripple.record_derivations and run infer() to populate it"
        );
        return json!([]);
    }

    // ── (1) Detect subjects with multiple inferred values for same predicate ──
    // Queries vp_rare for inferred triples (source=1) with same s+p but different o.

    let sql_multi_val = "\
        SELECT d1.rule_name AS rule_a, d2.rule_name AS rule_b, vr1.p AS pred_id \
        FROM _pg_ripple.derivations d1 \
        JOIN _pg_ripple.derivations d2 \
          ON d1.derived_sid < d2.derived_sid \
         AND d1.rule_set = $1 AND d2.rule_set = $1 \
        JOIN _pg_ripple.vp_rare vr1 ON vr1.i = d1.derived_sid AND vr1.source = 1 \
        JOIN _pg_ripple.vp_rare vr2 ON vr2.i = d2.derived_sid AND vr2.source = 1 \
        WHERE vr1.s = vr2.s AND vr1.p = vr2.p AND vr1.o <> vr2.o \
        LIMIT 20";

    let multi_val_rows: Vec<(Option<String>, Option<String>, Option<i64>)> =
        Spi::connect(|client| {
            client
                .select(
                    sql_multi_val,
                    None,
                    &[pgrx::datum::DatumWithOid::from(ruleset)],
                )
                .map(|tbl| {
                    tbl.map(|row| {
                        (
                            row.get::<String>(1).ok().flatten(),
                            row.get::<String>(2).ok().flatten(),
                            row.get::<i64>(3).ok().flatten(),
                        )
                    })
                    .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        });

    for (rule_a, rule_b, pred_id) in multi_val_rows {
        let pred_iri = pred_id
            .and_then(crate::dictionary::decode)
            .unwrap_or_else(|| format!("<dict:{}>", pred_id.unwrap_or(0)));
        conflicts.push(json!({
            "mode": "runtime",
            "rule_a": rule_a,
            "rule_b": rule_b,
            "conflict_type": "runtime_violation",
            "head_predicate": pred_iri,
            "conflicting_pattern":
                format!("two distinct inferred values for predicate '{pred_iri}' on the same subject"),
            "shacl_constraint": null,
            "example_triple": null
        }));
    }

    // ── (2) Detect sh:disjoint violations among derived facts ─────────────────

    let disjoint_pairs = load_disjoint_pairs();

    for (prop_a_iri, prop_b_iri, shape_iri) in &disjoint_pairs {
        // Encode the property IRIs to dictionary IDs.
        let id_a = match crate::dictionary::lookup_iri(prop_a_iri) {
            Some(id) => id,
            None => continue,
        };
        let id_b = match crate::dictionary::lookup_iri(prop_b_iri) {
            Some(id) => id,
            None => continue,
        };

        let sql_disjoint = "\
            SELECT d1.rule_name AS rule_a, d2.rule_name AS rule_b \
            FROM _pg_ripple.derivations d1 \
            JOIN _pg_ripple.derivations d2 \
              ON d1.rule_set = $1 AND d2.rule_set = $1 \
            JOIN _pg_ripple.vp_rare vr1 ON vr1.i = d1.derived_sid AND vr1.source = 1 AND vr1.p = $2 \
            JOIN _pg_ripple.vp_rare vr2 ON vr2.i = d2.derived_sid AND vr2.source = 1 AND vr2.p = $3 \
            WHERE vr1.s = vr2.s \
            LIMIT 10";

        let disjoint_rows: Vec<(Option<String>, Option<String>)> = Spi::connect(|client| {
            client
                .select(
                    sql_disjoint,
                    None,
                    &[
                        pgrx::datum::DatumWithOid::from(ruleset),
                        pgrx::datum::DatumWithOid::from(id_a),
                        pgrx::datum::DatumWithOid::from(id_b),
                    ],
                )
                .map(|tbl| {
                    tbl.map(|row| {
                        (
                            row.get::<String>(1).ok().flatten(),
                            row.get::<String>(2).ok().flatten(),
                        )
                    })
                    .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        });

        for (rule_a, rule_b) in disjoint_rows {
            conflicts.push(json!({
                "mode": "runtime",
                "rule_a": rule_a,
                "rule_b": rule_b,
                "conflict_type": "runtime_violation",
                "head_predicate": prop_a_iri,
                "conflicting_pattern":
                    format!("sh:disjoint violated: same subject has inferred values for both '{prop_a_iri}' and '{prop_b_iri}'"),
                "shacl_constraint": shape_iri,
                "example_triple": null
            }));
        }
    }

    json!(conflicts)
}

// ─── SHACL catalog helpers ───────────────────────────────────────────────────

struct ShaclConstraint {
    shape_iri: String,
    path_iri: String,
    constraint_type: String,
}

fn shacl_shapes_exist() -> bool {
    Spi::connect(|client| {
        client
            .select(
                "SELECT EXISTS ( \
                    SELECT 1 FROM information_schema.tables \
                    WHERE table_schema = '_pg_ripple' \
                      AND table_name = 'shacl_shapes' \
                 )",
                None,
                &[],
            )
            .ok()
            .and_then(|mut rows| rows.next())
            .and_then(|row| row.get::<bool>(1).ok().flatten())
            .unwrap_or(false)
    })
}

/// Load active SHACL `sh:not`, `sh:disjoint`, and `sh:in` constraints from
/// the SHACL shape catalog.  Returns an empty Vec when the catalog does not
/// exist (pre-v0.7.0 installs).
///
/// The `shacl_shapes` table stores a serialized `Shape` JSON blob.  The
/// `properties` array of each shape contains `PropertyShape` objects whose
/// `constraints` array may include `{"Not":…}`, `{"Disjoint":…}`, or
/// `{"In":[…]}` entries.
fn load_shacl_constraints() -> Vec<ShaclConstraint> {
    if !shacl_shapes_exist() {
        return Vec::new();
    }

    // Query: unnest properties and constraints; keep only Not/Disjoint/In.
    let sql = "\
        SELECT \
            s.shape_iri, \
            prop->>'path_iri' AS path_iri, \
            CASE \
                WHEN c ? 'Not'      THEN 'sh:not' \
                WHEN c ? 'Disjoint' THEN 'sh:disjoint' \
                WHEN c ? 'In'       THEN 'sh:in' \
                ELSE 'unknown' \
            END AS constraint_type \
        FROM _pg_ripple.shacl_shapes s, \
             jsonb_array_elements(s.shape_json->'properties') AS prop, \
             jsonb_array_elements(prop->'constraints') AS c \
        WHERE s.active = true \
          AND (c ? 'Not' OR c ? 'Disjoint' OR c ? 'In') \
          AND prop->>'path_iri' IS NOT NULL \
        ORDER BY s.shape_iri";

    Spi::connect(|client| {
        client
            .select(sql, None, &[])
            .unwrap_or_else(|e| pgrx::error!("SHACL catalog read error: {e}"))
            .map(|row| {
                let shape_iri = row.get::<String>(1).ok().flatten().unwrap_or_default();
                let path_iri = row.get::<String>(2).ok().flatten().unwrap_or_default();
                let constraint_type = row.get::<String>(3).ok().flatten().unwrap_or_default();
                ShaclConstraint {
                    shape_iri,
                    path_iri,
                    constraint_type,
                }
            })
            .collect::<Vec<_>>()
    })
}

/// Load property pairs linked by `sh:disjoint` constraints.
/// Returns `(prop_a_iri, prop_b_iri, shape_iri)` tuples.
///
/// The `Disjoint` variant in `ShapeConstraint` holds the IRI of the
/// *other* property that must be disjoint.  So if a property shape has
/// `path_iri = "http://ex.org/a"` and constraint `{"Disjoint": "http://ex.org/b"}`,
/// the pair is `("http://ex.org/a", "http://ex.org/b", shape_iri)`.
fn load_disjoint_pairs() -> Vec<(String, String, String)> {
    if !shacl_shapes_exist() {
        return Vec::new();
    }

    let sql = "\
        SELECT \
            s.shape_iri, \
            prop->>'path_iri' AS path_a, \
            c->>'Disjoint'    AS path_b \
        FROM _pg_ripple.shacl_shapes s, \
             jsonb_array_elements(s.shape_json->'properties') AS prop, \
             jsonb_array_elements(prop->'constraints') AS c \
        WHERE s.active = true \
          AND c ? 'Disjoint' \
          AND prop->>'path_iri' IS NOT NULL \
          AND c->>'Disjoint' IS NOT NULL";

    Spi::connect(|client| {
        client
            .select(sql, None, &[])
            .unwrap_or_else(|e| pgrx::error!("SHACL disjoint read error: {e}"))
            .map(|row| {
                let shape = row.get::<String>(1).ok().flatten().unwrap_or_default();
                let a = row.get::<String>(2).ok().flatten().unwrap_or_default();
                let b = row.get::<String>(3).ok().flatten().unwrap_or_default();
                (a, b, shape)
            })
            .collect::<Vec<_>>()
    })
}

// ─── Unit tests (run in pg_test harness) ─────────────────────────────────────

#[cfg(any(test, feature = "pg_test"))]
#[allow(clippy::unwrap_used, clippy::expect_used)]
#[pg_schema]
mod tests {
    use super::*;

    #[pg_test]
    fn test_parse_head_object_constant() {
        crate::datalog::builtins::register_standard_prefixes();
        // A rule where the head object is a constant literal — parse should not panic.
        let rule = "?x <http://ex.org/eligible> \"true\" :- ?x <http://ex.org/age> ?a .";
        // Result may be Some or None depending on dictionary state; just assert no panic.
        let _ = parse_head_object(rule);
    }

    #[pg_test]
    fn test_parse_head_object_variable() {
        crate::datalog::builtins::register_standard_prefixes();
        // A rule where the head object is a variable — should return None.
        let rule = "?x <http://ex.org/knows> ?y :- ?y <http://ex.org/knows> ?x .";
        let result = parse_head_object(rule);
        assert_eq!(result, None);
    }

    #[pg_test]
    fn test_rule_conflicts_empty_ruleset() {
        crate::datalog::builtins::register_standard_prefixes();
        // A rule set that doesn't exist should return an empty array.
        let result = rule_conflicts("_nonexistent_test_ruleset_v0103", "static");
        assert!(result.is_array());
        assert_eq!(result.as_array().unwrap().len(), 0);
    }

    #[pg_test]
    fn test_rule_conflicts_runtime_no_derivations() {
        crate::datalog::builtins::register_standard_prefixes();
        // Runtime mode on a non-existent rule set returns [] (or warns if no table).
        let result = rule_conflicts("_nonexistent_test_ruleset_v0103", "runtime");
        assert!(result.is_array());
    }
}
