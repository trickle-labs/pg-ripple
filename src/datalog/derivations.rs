//! Proof-tree justification infrastructure (v0.100.0 PROOF-TREE-01).
//!
//! When `pg_ripple.record_derivations = on`, the semi-naive inference engine
//! records, for every newly derived fact:
//!
//! - `derived_sid`     — the statement ID of the inferred triple
//! - `rule_name`       — the Datalog rule text that produced it
//! - `rule_set`        — the rule set name
//! - `antecedent_sids` — SIDs of the body-atom triples that fired the rule
//!
//! The public `justify()` SQL function walks this provenance graph recursively
//! and returns a human-readable JSONB proof tree.

use pgrx::datum::DatumWithOid;
use pgrx::prelude::*;

// ─── Derivation recording ─────────────────────────────────────────────────────

/// Record derivation provenance for a single rule invocation.
///
/// Called during the semi-naive fixpoint, BEFORE delta tables are dropped,
/// so the delta tables can be used to filter to newly-derived triples only.
///
/// `delta_table_fn` maps a predicate ID to the name of its delta temp table
/// (e.g. `pred_id ↦ "_dl_delta_{pred_id}"`).  The delta table is used as the
/// filter so we record derivations only for triples that were newly derived in
/// this inference run, not for pre-existing base facts that happen to match the
/// rule head.
pub fn record_rule_derivations_with_delta<F>(rule: &super::Rule, rule_set: &str, delta_table_fn: &F)
where
    F: Fn(i64) -> Option<String>,
{
    if !crate::RECORD_DERIVATIONS.get() {
        return;
    }
    let Some(sql) = compile_antecedent_insert_via_delta(rule, rule_set, delta_table_fn) else {
        return;
    };
    if let Err(e) = Spi::run_with_args(&sql, &[]) {
        pgrx::warning!(
            "derivation record error for rule '{}': {e}",
            rule.rule_text
        );
    }
}

/// Kept for backward-compatibility; delegates to the delta-table variant with
/// an always-None mapper (effectively a no-op without delta context).
#[allow(dead_code)]
pub fn record_rule_derivations(rule: &super::Rule, rule_set: &str) {
    record_rule_derivations_with_delta(rule, rule_set, &|_| None);
}

/// Generate the SQL INSERT into `_pg_ripple.derivations` for one rule.
///
/// Returns `None` for rules we cannot handle (recursive, variable predicates,
/// no head, complex head expressions that would require more than simple joins).
///
/// Superseded by [`compile_antecedent_insert_via_delta`]; retained for reference.
#[allow(dead_code)]
fn compile_antecedent_insert(rule: &super::Rule, rule_set: &str) -> Option<String> {
    use super::{BodyLiteral, Term};

    let head = rule.head.as_ref()?;

    // Head predicate must be a constant.
    let head_pred = match &head.p {
        Term::Const(id) => *id,
        _ => return None,
    };

    // Skip recursive rules — antecedent tracking for recursive rules would
    // require unwinding the CTE-based evaluation, which is out of scope here.
    let is_recursive = rule.body.iter().any(|lit| {
        if let BodyLiteral::Positive(atom) = lit {
            matches!(&atom.p, Term::Const(p) if *p == head_pred)
        } else {
            false
        }
    });
    if is_recursive {
        // Fall back to recording with empty antecedent_sids.
        return record_recursive_rule_stub(rule, rule_set);
    }

    // Collect positive body atoms (skip negated / arithmetic / compare literals).
    let pos_atoms: Vec<&super::Atom> = rule
        .body
        .iter()
        .filter_map(|lit| {
            if let BodyLiteral::Positive(a) = lit {
                Some(a)
            } else {
                None
            }
        })
        .collect();

    if pos_atoms.is_empty() {
        return None;
    }

    // All body atom predicates must be constants.
    for atom in &pos_atoms {
        if !matches!(atom.p, Term::Const(_)) {
            return None;
        }
    }

    // Build FROM/JOIN clauses and track variable bindings.
    // var_map: variable name → SQL expression that produced it.
    let mut var_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut from_join_parts: Vec<String> = Vec::new();
    let mut bid_columns: Vec<String> = Vec::new();

    for (idx, atom) in pos_atoms.iter().enumerate() {
        let pred_id = match &atom.p {
            Term::Const(id) => *id,
            _ => return None,
        };

        let alias = format!("b{idx}");
        // Include `i` column for SID capture.
        let table_expr =
            format!("(SELECT s, o, g, i FROM _pg_ripple.vp_rare WHERE p = {pred_id}) AS {alias}");

        bid_columns.push(format!("{alias}.i"));

        let mut join_conds: Vec<String> = Vec::new();

        // Subject
        match &atom.s {
            Term::Var(v) => {
                if let Some(existing) = var_map.get(v.as_str()) {
                    join_conds.push(format!("{alias}.s = {existing}"));
                } else {
                    var_map.insert(v.clone(), format!("{alias}.s"));
                }
            }
            Term::Const(id) => {
                join_conds.push(format!("{alias}.s = {id}"));
            }
            _ => {}
        }

        // Object
        match &atom.o {
            Term::Var(v) => {
                if let Some(existing) = var_map.get(v.as_str()) {
                    join_conds.push(format!("{alias}.o = {existing}"));
                } else {
                    var_map.insert(v.clone(), format!("{alias}.o"));
                }
            }
            Term::Const(id) => {
                join_conds.push(format!("{alias}.o = {id}"));
            }
            _ => {}
        }

        // Graph (optional — we allow any graph for the body scan)
        match &atom.g {
            Term::Var(v) => {
                if let Some(existing) = var_map.get(v.as_str()) {
                    join_conds.push(format!("{alias}.g = {existing}"));
                } else {
                    var_map.insert(v.clone(), format!("{alias}.g"));
                }
            }
            Term::Const(id) => {
                join_conds.push(format!("{alias}.g = {id}"));
            }
            _ => {}
        }

        if idx == 0 {
            from_join_parts.push(format!("FROM {table_expr}"));
        } else if join_conds.is_empty() {
            from_join_parts.push(format!("CROSS JOIN {table_expr}"));
        } else {
            from_join_parts.push(format!("JOIN {table_expr} ON {}", join_conds.join(" AND ")));
        }
    }

    // Resolve head subject and object from var_map.
    let head_s_sql = match &head.s {
        Term::Var(v) => var_map.get(v.as_str()).cloned()?,
        Term::Const(id) => id.to_string(),
        _ => return None,
    };
    let head_o_sql = match &head.o {
        Term::Var(v) => var_map.get(v.as_str()).cloned()?,
        Term::Const(id) => id.to_string(),
        _ => return None,
    };
    let head_g_sql = match &head.g {
        Term::Var(v) => var_map
            .get(v.as_str())
            .cloned()
            .unwrap_or_else(|| "0".to_owned()),
        Term::Const(id) => id.to_string(),
        Term::DefaultGraph => "0".to_owned(),
        Term::Wildcard => "0".to_owned(),
    };

    let from_join_sql = from_join_parts.join("\n  ");
    let antecedent_array = if bid_columns.is_empty() {
        "ARRAY[]::BIGINT[]".to_owned()
    } else {
        format!("ARRAY[{}]::BIGINT[]", bid_columns.join(", "))
    };

    // Escape single quotes in rule text / rule_set for embedding in SQL.
    let rule_name_esc = rule.rule_text.replace('\'', "''");
    let rule_set_esc = rule_set.replace('\'', "''");

    // Build the INSERT … SELECT.
    // The inner SELECT re-runs the rule body and joins with vp_rare (source=1
    // = inferred) on the head to obtain derived_sid.
    let sql = format!(
        "INSERT INTO _pg_ripple.derivations \
           (derived_sid, rule_name, rule_set, antecedent_sids) \
         SELECT \
           vr_head.i, \
           '{rule_name_esc}'::text, \
           '{rule_set_esc}'::text, \
           {antecedent_array} \
         {from_join_sql} \
         JOIN (SELECT i, s, o, g FROM _pg_ripple.vp_rare \
               WHERE p = {head_pred} AND source = 1) vr_head \
           ON vr_head.s = {head_s_sql} \
          AND vr_head.o = {head_o_sql} \
          AND vr_head.g = {head_g_sql} \
         ON CONFLICT (derived_sid, rule_name) DO NOTHING"
    );

    Some(sql)
}

/// Delta-table variant of [`compile_antecedent_insert`].
///
/// Uses the delta temp-table returned by `delta_table_fn(head_pred)` — which
/// contains only the (s, o, g) rows newly derived in this inference run — to
/// join against `vp_rare` and obtain the correct SIDs.  This is required
/// because derived triples are stored with `source = 0` (DEFAULT), not
/// `source = 1`, so the `source = 1` filter in `compile_antecedent_insert`
/// would always return zero rows.
///
/// Returns `None` when:
/// - `delta_table_fn` returns `None` for the head predicate (no delta context)
/// - the rule cannot be translated (recursive head, variable predicates, etc.)
fn compile_antecedent_insert_via_delta<F>(
    rule: &super::Rule,
    rule_set: &str,
    delta_table_fn: &F,
) -> Option<String>
where
    F: Fn(i64) -> Option<String>,
{
    use super::{BodyLiteral, Term};

    let head = rule.head.as_ref()?;

    // Head predicate must be a constant.
    let head_pred = match &head.p {
        Term::Const(id) => *id,
        _ => return None,
    };

    // Delta table must be available for this predicate.
    let delta_table = delta_table_fn(head_pred)?;

    // Skip recursive rules — use delta-aware stub instead.
    let is_recursive = rule.body.iter().any(|lit| {
        if let BodyLiteral::Positive(atom) = lit {
            matches!(&atom.p, Term::Const(p) if *p == head_pred)
        } else {
            false
        }
    });
    if is_recursive {
        return record_recursive_rule_stub_via_delta(rule, rule_set, &delta_table);
    }

    // Collect positive body atoms.
    let pos_atoms: Vec<&super::Atom> = rule
        .body
        .iter()
        .filter_map(|lit| {
            if let BodyLiteral::Positive(a) = lit {
                Some(a)
            } else {
                None
            }
        })
        .collect();

    if pos_atoms.is_empty() {
        return None;
    }

    // All body atom predicates must be constants.
    for atom in &pos_atoms {
        if !matches!(atom.p, Term::Const(_)) {
            return None;
        }
    }

    let mut var_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut from_join_parts: Vec<String> = Vec::new();
    let mut bid_columns: Vec<String> = Vec::new();

    for (idx, atom) in pos_atoms.iter().enumerate() {
        let pred_id = match &atom.p {
            Term::Const(id) => *id,
            _ => return None,
        };
        let alias = format!("b{idx}");
        let table_expr = format!(
            "(SELECT s, o, g, i FROM _pg_ripple.vp_rare WHERE p = {pred_id}) AS {alias}"
        );
        bid_columns.push(format!("{alias}.i"));

        let mut join_conds: Vec<String> = Vec::new();

        match &atom.s {
            Term::Var(v) => {
                if let Some(existing) = var_map.get(v.as_str()) {
                    join_conds.push(format!("{alias}.s = {existing}"));
                } else {
                    var_map.insert(v.clone(), format!("{alias}.s"));
                }
            }
            Term::Const(id) => join_conds.push(format!("{alias}.s = {id}")),
            _ => {}
        }

        match &atom.o {
            Term::Var(v) => {
                if let Some(existing) = var_map.get(v.as_str()) {
                    join_conds.push(format!("{alias}.o = {existing}"));
                } else {
                    var_map.insert(v.clone(), format!("{alias}.o"));
                }
            }
            Term::Const(id) => join_conds.push(format!("{alias}.o = {id}")),
            _ => {}
        }

        match &atom.g {
            Term::Var(v) => {
                if let Some(existing) = var_map.get(v.as_str()) {
                    join_conds.push(format!("{alias}.g = {existing}"));
                } else {
                    var_map.insert(v.clone(), format!("{alias}.g"));
                }
            }
            Term::Const(id) => join_conds.push(format!("{alias}.g = {id}")),
            _ => {}
        }

        if idx == 0 {
            from_join_parts.push(format!("FROM {table_expr}"));
        } else if join_conds.is_empty() {
            from_join_parts.push(format!("CROSS JOIN {table_expr}"));
        } else {
            from_join_parts.push(format!("JOIN {table_expr} ON {}", join_conds.join(" AND ")));
        }
    }

    let head_s_sql = match &head.s {
        Term::Var(v) => var_map.get(v.as_str()).cloned()?,
        Term::Const(id) => id.to_string(),
        _ => return None,
    };
    let head_o_sql = match &head.o {
        Term::Var(v) => var_map.get(v.as_str()).cloned()?,
        Term::Const(id) => id.to_string(),
        _ => return None,
    };
    let head_g_sql = match &head.g {
        Term::Var(v) => var_map
            .get(v.as_str())
            .cloned()
            .unwrap_or_else(|| "0".to_owned()),
        Term::Const(id) => id.to_string(),
        Term::DefaultGraph => "0".to_owned(),
        Term::Wildcard => "0".to_owned(),
    };

    let from_join_sql = from_join_parts.join("\n  ");
    let antecedent_array = if bid_columns.is_empty() {
        "ARRAY[]::BIGINT[]".to_owned()
    } else {
        format!("ARRAY[{}]::BIGINT[]", bid_columns.join(", "))
    };

    let rule_name_esc = rule.rule_text.replace('\'', "''");
    let rule_set_esc = rule_set.replace('\'', "''");

    // Join delta temp-table (s,o,g) against vp_rare to get SIDs for
    // newly-derived head triples.
    let sql = format!(
        "INSERT INTO _pg_ripple.derivations \
           (derived_sid, rule_name, rule_set, antecedent_sids) \
         SELECT \
           vr_head.i, \
           '{rule_name_esc}'::text, \
           '{rule_set_esc}'::text, \
           {antecedent_array} \
         {from_join_sql} \
         JOIN (SELECT vr.i, vr.s, vr.o, vr.g \
               FROM {delta_table} dt \
               JOIN _pg_ripple.vp_rare vr \
                 ON vr.p = {head_pred} AND vr.s = dt.s AND vr.o = dt.o AND vr.g = dt.g \
              ) vr_head \
           ON vr_head.s = {head_s_sql} \
          AND vr_head.o = {head_o_sql} \
          AND vr_head.g = {head_g_sql} \
         ON CONFLICT (derived_sid, rule_name) DO NOTHING"
    );

    Some(sql)
}

/// Stub for recursive rules when a delta table is available.
fn record_recursive_rule_stub_via_delta(
    rule: &super::Rule,
    rule_set: &str,
    delta_table: &str,
) -> Option<String> {
    use super::Term;

    let head = rule.head.as_ref()?;
    let head_pred = match &head.p {
        Term::Const(id) => *id,
        _ => return None,
    };

    let rule_name_esc = rule.rule_text.replace('\'', "''");
    let rule_set_esc = rule_set.replace('\'', "''");

    let sql = format!(
        "INSERT INTO _pg_ripple.derivations \
           (derived_sid, rule_name, rule_set, antecedent_sids) \
         SELECT vr.i, '{rule_name_esc}'::text, '{rule_set_esc}'::text, ARRAY[]::BIGINT[] \
         FROM {delta_table} dt \
         JOIN _pg_ripple.vp_rare vr \
           ON vr.p = {head_pred} AND vr.s = dt.s AND vr.o = dt.o AND vr.g = dt.g \
         ON CONFLICT (derived_sid, rule_name) DO NOTHING"
    );

    Some(sql)
}

/// For recursive rules we cannot easily reconstruct antecedents, so we write a
/// stub row with an empty antecedent_sids array.  `justify()` will still show
/// the rule name even without antecedents.
///
/// Superseded by [`record_recursive_rule_stub_via_delta`]; retained for reference.
#[allow(dead_code)]
fn record_recursive_rule_stub(rule: &super::Rule, rule_set: &str) -> Option<String> {
    use super::Term;

    let head = rule.head.as_ref()?;
    let head_pred = match &head.p {
        Term::Const(id) => *id,
        _ => return None,
    };

    let rule_name_esc = rule.rule_text.replace('\'', "''");
    let rule_set_esc = rule_set.replace('\'', "''");

    // Insert stubs for all inferred triples with this head predicate.
    let sql = format!(
        "INSERT INTO _pg_ripple.derivations \
           (derived_sid, rule_name, rule_set, antecedent_sids) \
         SELECT vr.i, '{rule_name_esc}'::text, '{rule_set_esc}'::text, ARRAY[]::BIGINT[] \
         FROM _pg_ripple.vp_rare vr \
         WHERE vr.p = {head_pred} AND vr.source = 1 \
         ON CONFLICT (derived_sid, rule_name) DO NOTHING"
    );

    Some(sql)
}

// ─── Orphan cleanup (DRed integration) ───────────────────────────────────────

/// Remove derivation rows whose `derived_sid` no longer exists in `vp_rare`
/// or any dedicated VP table.  Called after DRed retraction and optionally
/// exposed via the `vacuum_derivations()` SQL function.
///
/// Returns the number of rows removed.
pub fn vacuum_orphan_derivations() -> i64 {
    // A derived_sid is orphaned when it does not appear in vp_rare (any source)
    // and does not appear in the `i` column of any dedicated VP table.
    // For simplicity we check vp_rare.i only (dedicated VP deltas are mirrored
    // in vp_rare after merge; inferred triples always live in vp_rare).
    let sql = "WITH deleted AS ( \
        DELETE FROM _pg_ripple.derivations d \
        WHERE NOT EXISTS ( \
            SELECT 1 FROM _pg_ripple.vp_rare vr WHERE vr.i = d.derived_sid \
        ) \
        RETURNING 1 \
    ) SELECT COUNT(*)::bigint FROM deleted";

    Spi::get_one::<i64>(sql).unwrap_or(None).unwrap_or(0)
}

// ─── proof-tree builder ───────────────────────────────────────────────────────

/// Look up the dictionary ID for an IRI/literal string.
/// Returns `None` if not found in the dictionary.
fn dict_id_for(value: &str) -> Option<i64> {
    Spi::get_one_with_args::<i64>(
        "SELECT id FROM _pg_ripple.dictionary WHERE value = $1 LIMIT 1",
        &[DatumWithOid::from(value)],
    )
    .ok()
    .flatten()
}

/// Decode a dictionary ID to its human-readable string value.
#[allow(dead_code)]
fn dict_decode(id: i64) -> String {
    Spi::get_one_with_args::<String>(
        "SELECT value FROM _pg_ripple.dictionary WHERE id = $1",
        &[DatumWithOid::from(id)],
    )
    .ok()
    .flatten()
    .unwrap_or_else(|| format!("<id:{id}>"))
}

/// Batch-decode a list of dictionary IDs to their string values.
/// Returns a HashMap from id → value.
fn batch_decode(ids: &[i64]) -> std::collections::HashMap<i64, String> {
    if ids.is_empty() {
        return std::collections::HashMap::new();
    }
    let id_list: String = ids
        .iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT id, value FROM _pg_ripple.dictionary WHERE id = ANY(ARRAY[{id_list}]::BIGINT[])"
    );
    Spi::connect(|client| {
        client
            .select(&sql, None, &[])
            .unwrap_or_else(|e| pgrx::error!("batch decode SPI error: {e}"))
            .map(|row| {
                let id = row.get::<i64>(1).ok().flatten().unwrap_or(0);
                let val = row.get::<String>(2).ok().flatten().unwrap_or_default();
                (id, val)
            })
            .collect::<std::collections::HashMap<_, _>>()
    })
}

/// Look up the `vp_rare.i` (SID) for a triple `(s_id, p_id, o_id)`.
fn sid_for_triple(s_id: i64, p_id: i64, o_id: i64) -> Option<i64> {
    Spi::get_one_with_args::<i64>(
        "SELECT i FROM _pg_ripple.vp_rare WHERE p = $1 AND s = $2 AND o = $3 LIMIT 1",
        &[
            DatumWithOid::from(p_id),
            DatumWithOid::from(s_id),
            DatumWithOid::from(o_id),
        ],
    )
    .ok()
    .flatten()
}

/// Read derivation rows for a given SID.
/// Returns a list of `(rule_name, rule_set, antecedent_sids)`.
fn derivations_for_sid(sid: i64) -> Vec<(String, String, Vec<i64>)> {
    let sql = "SELECT rule_name, rule_set, antecedent_sids \
               FROM _pg_ripple.derivations \
               WHERE derived_sid = $1";
    Spi::connect(|client| {
        client
            .select(sql, None, &[DatumWithOid::from(sid)])
            .unwrap_or_else(|e| pgrx::error!("derivation lookup SPI error: {e}"))
            .map(|row| {
                let rn = row.get::<String>(1).ok().flatten().unwrap_or_default();
                let rs = row.get::<String>(2).ok().flatten().unwrap_or_default();
                let ant: Vec<i64> = row.get::<Vec<i64>>(3).ok().flatten().unwrap_or_default();
                (rn, rs, ant)
            })
            .collect::<Vec<_>>()
    })
}

/// Get the triple `(p, s, o)` for a given SID from vp_rare.
fn triple_for_sid(sid: i64) -> Option<(i64, i64, i64)> {
    let sql = "SELECT p, s, o FROM _pg_ripple.vp_rare WHERE i = $1 LIMIT 1";
    Spi::connect(|client| {
        client
            .select(sql, None, &[DatumWithOid::from(sid)])
            .unwrap_or_else(|e| pgrx::error!("triple for SID SPI error: {e}"))
            .next()
            .and_then(|row| {
                let p = row.get::<i64>(1).ok().flatten()?;
                let s = row.get::<i64>(2).ok().flatten()?;
                let o = row.get::<i64>(3).ok().flatten()?;
                Some((p, s, o))
            })
    })
}

/// Recursively build the JSONB proof tree for a given SID.
///
/// `visited` guards against cycles in the derivation graph.
/// `depth` prevents stack overflow on pathological derivation chains.
fn build_proof_tree(
    sid: i64,
    visited: &mut std::collections::HashSet<i64>,
    depth: u32,
) -> serde_json::Value {
    const MAX_DEPTH: u32 = 64;

    // Cycle guard.
    if !visited.insert(sid) {
        return serde_json::json!({
            "sid": sid,
            "cycle": true
        });
    }
    if depth >= MAX_DEPTH {
        visited.remove(&sid);
        return serde_json::json!({
            "sid": sid,
            "max_depth_reached": true
        });
    }

    // Look up the triple's human-readable labels.
    let triple_label = if let Some((p_id, s_id, o_id)) = triple_for_sid(sid) {
        let decode_map = batch_decode(&[s_id, p_id, o_id]);
        serde_json::json!({
            "subject":   decode_map.get(&s_id).cloned().unwrap_or_else(|| format!("<id:{s_id}>")),
            "predicate": decode_map.get(&p_id).cloned().unwrap_or_else(|| format!("<id:{p_id}>")),
            "object":    decode_map.get(&o_id).cloned().unwrap_or_else(|| format!("<id:{o_id}>"))
        })
    } else {
        serde_json::json!({ "sid": sid })
    };

    let derivation_rows = derivations_for_sid(sid);

    if derivation_rows.is_empty() {
        // Base fact — no derivation recorded.
        visited.remove(&sid);
        return serde_json::json!({
            "type": "base",
            "sid": sid,
            "triple": triple_label
        });
    }

    // Build one entry per derivation rule (a triple may be derived by multiple rules).
    let mut rules_json: Vec<serde_json::Value> = Vec::new();
    for (rule_name, rule_set, antecedent_sids) in &derivation_rows {
        let mut antecedents_json: Vec<serde_json::Value> = Vec::new();
        for &ant_sid in antecedent_sids {
            antecedents_json.push(build_proof_tree(ant_sid, visited, depth + 1));
        }
        rules_json.push(serde_json::json!({
            "rule": rule_name,
            "rule_set": rule_set,
            "antecedents": antecedents_json
        }));
    }

    visited.remove(&sid);
    serde_json::json!({
        "type": "inferred",
        "sid": sid,
        "triple": triple_label,
        "derivations": rules_json
    })
}

/// Public entry point for the `justify()` SQL function.
///
/// Looks up the SID for the triple `(subject, predicate, object)` and calls
/// `build_proof_tree`.  Returns `None` (SQL NULL) when the triple is not found.
pub fn justify_impl(subject: &str, predicate: &str, object: &str) -> Option<serde_json::Value> {
    let s_id = dict_id_for(subject)?;
    let p_id = dict_id_for(predicate)?;
    let o_id = dict_id_for(object)?;
    let sid = sid_for_triple(s_id, p_id, o_id)?;

    let mut visited = std::collections::HashSet::new();
    let tree = build_proof_tree(sid, &mut visited, 0);
    Some(tree)
}
