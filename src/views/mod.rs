//! Incremental SPARQL Views, Datalog Views, and Extended VP (ExtVP) — v0.11.0.
//!
//!
//! All three features are soft-dependent on the pg_trickle extension.
//! Functions that require pg_trickle call [`crate::has_pg_trickle`] at call
//! time and raise a descriptive error when it is absent.
//!
//! # Public SQL functions
//!
//! - `pg_ripple.pg_trickle_available()` — check whether pg_trickle is installed
//! - `pg_ripple.create_sparql_view(name, sparql, schedule, decode, immediate)` — create an always-fresh SPARQL result table
//! - `pg_ripple.drop_sparql_view(name)` — drop a SPARQL view
//! - `pg_ripple.list_sparql_views()` — list all registered SPARQL views
//! - `pg_ripple.create_datalog_view(name, rules, goal, schedule, decode, immediate)` — create a Datalog-backed live view
//! - `pg_ripple.create_datalog_view(name, rule_set, goal, schedule, decode, immediate)` — same using a named rule set
//! - `pg_ripple.drop_datalog_view(name)` — drop a Datalog view
//! - `pg_ripple.list_datalog_views()` — list all registered Datalog views
//! - `pg_ripple.create_extvp(name, pred1_iri, pred2_iri, schedule)` — create an ExtVP semi-join stream table
//! - `pg_ripple.drop_extvp(name)` — drop an ExtVP table
//! - `pg_ripple.list_extvp()` — list all registered ExtVP tables
//!
//! v0.18.0 — SPARQL CONSTRUCT, DESCRIBE & ASK Views
//! - `pg_ripple.create_construct_view(name, sparql, schedule, decode)` — CONSTRUCT stream table
//! - `pg_ripple.drop_construct_view(name)` — drop a CONSTRUCT view
//! - `pg_ripple.list_construct_views()` — list CONSTRUCT views
//! - `pg_ripple.create_describe_view(name, sparql, schedule, decode)` — DESCRIBE stream table
//! - `pg_ripple.drop_describe_view(name)` — drop a DESCRIBE view
//! - `pg_ripple.list_describe_views()` — list DESCRIBE views
//! - `pg_ripple.create_ask_view(name, sparql, schedule)` — ASK stream table
//! - `pg_ripple.drop_ask_view(name)` — drop an ASK view
//! - `pg_ripple.list_ask_views()` — list ASK views

// v0.90.0 CQ-02: pre-emptive split sub-modules

use pgrx::datum::DatumWithOid;
use pgrx::prelude::*;
use spargebra::SparqlParser;

use crate::dictionary;
use crate::sparql::sqlgen;

// ─── pg_trickle install hint ─────────────────────────────────────────────────

pub(super) const PGTRICKLE_HINT: &str = "Install pg_trickle: https://github.com/trickle-labs/pg-trickle — \
     then run: CREATE EXTENSION pg_trickle";

// ─── pg_tide install hint (TIDE-3, v0.93.0) ──────────────────────────────────

/// Error hint for relay-dependent operations when pg_tide is not installed.
///
/// pg_tide (trickle-labs/pg-tide ≥ 0.1.0) contains the relay, outbox, and inbox
/// subsystem that was extracted from pg_trickle v0.46.0.  Add this hint to any
/// `pgrx::error!()` call in relay-dependent code paths.
// Q15-01: internal API field; kept for public API surface or future extension consumers.
#[allow(dead_code)]
pub(super) const PGTIDE_HINT: &str = "pg_tide extension is not installed; \
     install pg_tide \u{2265}0.1.0 from https://github.com/trickle-labs/pg-tide \
     then run: CREATE EXTENSION pg_tide";

// ─── SPARQL SQL generation for views ─────────────────────────────────────────

/// Compile a SPARQL SELECT query to a SQL SELECT suitable for a stream table.
///
/// Returns `(sql, variables)` where `sql` projects each SPARQL variable as
/// `{col} AS {varname}` (plain name, not `_v_{varname}`).  Column names are
/// safe SPARQL variable names and therefore valid SQL identifiers.
///
/// The stream table always stores raw `BIGINT` dictionary IDs.
/// When the caller requests `decode = true`, a thin `_{name}_decoded` companion
/// VIEW is created on top (the same pattern used by `create_construct_view`).
/// This keeps the pg_trickle IVM delta path working on integer columns while
/// still offering a convenient TEXT-decoded surface.
pub(super) fn compile_sparql_for_view(query_text: &str) -> Result<(String, Vec<String>), String> {
    let query = SparqlParser::new()
        .parse_query(query_text)
        .map_err(|e| format!("SPARQL parse error: {e}"))?;

    let pattern = match query {
        spargebra::Query::Select { pattern, .. } => pattern,
        _ => return Err("only SELECT queries can be compiled to views".to_owned()),
    };

    let trans = sqlgen::translate_select(&pattern, None);

    // The standard translation uses `_v_{var}` column aliases.  Re-map them to
    // plain variable names so the stream table schema is readable.
    let clean_sql = remap_view_columns(&trans.sql, &trans.variables);

    Ok((clean_sql, trans.variables))
}

/// Re-map `_v_{var}` column aliases in a translated SQL to plain `{var}`.
///
/// The standard SPARQL translator emits `... AS _v_{var}` to avoid name
/// collisions.  For views we want clean column names.
pub(super) fn remap_view_columns(sql: &str, variables: &[String]) -> String {
    let mut result = sql.to_owned();
    for v in variables {
        let old = format!("AS _v_{v}");
        let new = format!("AS {v}");
        result = result.replace(&old, &new);
    }
    result
}

// ─── Stream-table name validation ────────────────────────────────────────────

/// Validate a user-supplied view/table name: ASCII alphanumeric + underscore, ≤ 63 chars.
/// Returns an error string if invalid.
pub(super) fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.len() > 63 {
        return Err("view name must be 1–63 characters".to_owned());
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(
            "view name must contain only ASCII letters, digits, and underscores".to_owned(),
        );
    }
    Ok(())
}

// ─── Resolve a predicate IRI to its VP table name ────────────────────────────

/// Look up a predicate IRI and return the VP table name (`_pg_ripple.vp_{id}`)
/// or `_pg_ripple.vp_rare` with the predicate ID filter if rare.
///
/// Returns `Err` if the IRI is not in the dictionary or has no triples.
pub(super) fn predicate_table_expr(pred_iri: &str) -> Result<(i64, String), String> {
    let pred_id = dictionary::lookup_iri(pred_iri)
        .ok_or_else(|| format!("predicate IRI not found in dictionary: {pred_iri}"))?;
    let table_expr = match Spi::get_one_with_args::<i64>(
        "SELECT table_oid::bigint FROM _pg_ripple.predicates WHERE id = $1",
        &[DatumWithOid::from(pred_id)],
    ) {
        Ok(Some(_)) => format!("_pg_ripple.vp_{pred_id}"),
        Ok(None) => format!("(SELECT s, o, g FROM _pg_ripple.vp_rare WHERE p = {pred_id})"),
        Err(_) => {
            return Err(format!(
                "predicate not found in predicate catalog: {pred_iri}"
            ));
        }
    };
    Ok((pred_id, table_expr))
}

// ─── Goal predicate extraction from SPARQL (issue #89, v0.112.0) ─────────────

/// Walk a spargebra `GraphPattern` and collect all bound (non-variable) predicate
/// IRI strings from triple pattern predicates.
pub(super) fn collect_sparql_predicates(
    pattern: &spargebra::algebra::GraphPattern,
    out: &mut Vec<String>,
) {
    use spargebra::algebra::GraphPattern;
    use spargebra::term::NamedNodePattern;

    match pattern {
        GraphPattern::Bgp { patterns } => {
            for tp in patterns {
                if let NamedNodePattern::NamedNode(nn) = &tp.predicate {
                    out.push(nn.as_str().to_owned());
                }
            }
        }
        GraphPattern::Join { left, right } => {
            collect_sparql_predicates(left, out);
            collect_sparql_predicates(right, out);
        }
        GraphPattern::LeftJoin { left, right, .. } => {
            collect_sparql_predicates(left, out);
            collect_sparql_predicates(right, out);
        }
        GraphPattern::Filter { inner, .. } => {
            collect_sparql_predicates(inner, out);
        }
        GraphPattern::Union { left, right } => {
            collect_sparql_predicates(left, out);
            collect_sparql_predicates(right, out);
        }
        GraphPattern::Graph { inner, .. } => {
            collect_sparql_predicates(inner, out);
        }
        GraphPattern::Extend { inner, .. } => {
            collect_sparql_predicates(inner, out);
        }
        GraphPattern::Minus { left, right } => {
            collect_sparql_predicates(left, out);
            collect_sparql_predicates(right, out);
        }
        GraphPattern::OrderBy { inner, .. } => {
            collect_sparql_predicates(inner, out);
        }
        GraphPattern::Project { inner, .. } => {
            collect_sparql_predicates(inner, out);
        }
        GraphPattern::Distinct { inner } | GraphPattern::Reduced { inner } => {
            collect_sparql_predicates(inner, out);
        }
        GraphPattern::Slice { inner, .. } => {
            collect_sparql_predicates(inner, out);
        }
        GraphPattern::Group { inner, .. } => {
            collect_sparql_predicates(inner, out);
        }
        GraphPattern::Service { inner, .. } => {
            collect_sparql_predicates(inner, out);
        }
        // Path patterns and table patterns — no BGP predicates to extract.
        _ => {}
    }
}

/// Validate all bound predicates in a SPARQL goal query against known rule head
/// predicates and base VP predicates for the given rule set.
///
/// Called by `create_datalog_view_from_rules` and `create_datalog_view_from_rule_set`
/// after the rule set is loaded / verified.  Only fires when the GUC
/// `pg_ripple.strict_goal_validation` is not `'off'`.
pub(super) fn validate_datalog_view_goal(rule_set: Option<&str>, goal: &str) {
    let mode: String = crate::STRICT_GOAL_VALIDATION
        .get()
        .and_then(|s| s.to_str().ok().map(|s| s.to_lowercase()))
        .unwrap_or_else(|| "warn".to_owned());
    if mode == "off" {
        return;
    }

    // Try to parse as SPARQL SELECT and extract predicates.
    let parsed = match spargebra::SparqlParser::new().parse_query(goal) {
        Ok(q) => q,
        Err(_) => return, // non-SPARQL goal (e.g. triple pattern string) — skip
    };
    let pattern = match parsed {
        spargebra::Query::Select { pattern, .. } => pattern,
        _ => return,
    };

    let mut pred_iris: Vec<String> = Vec::new();
    collect_sparql_predicates(&pattern, &mut pred_iris);
    pred_iris.dedup();

    for iri in pred_iris {
        // Encode the IRI to get the dictionary ID.
        let pred_id = crate::datalog::encode_iri(&iri);
        crate::datalog::validate_goal_predicate(rule_set, pred_id);
    }
}

// ─── Public functions — exposed through lib.rs ────────────────────────────────

// These functions are re-exported in the `pg_ripple` schema module in lib.rs.
// They are `pub(crate)` so that lib.rs can call them from the schema module.

/// Return `true` when the pg_trickle extension is installed in the current database.
pub(crate) fn pg_trickle_available() -> bool {
    crate::has_pg_trickle()
}

// ─── SPARQL Views ─────────────────────────────────────────────────────────────

// ─── Sub-modules (v0.114.0) ──────────────────────────────────────────────────

pub mod construct;
pub mod dependency;
pub mod describe;
pub mod materialise;
pub mod refresh;
pub mod sparql;

// ─── Re-exports for views_api.rs ─────────────────────────────────────────────
pub(crate) use construct::{create_construct_view, drop_construct_view, list_construct_views};
pub(crate) use dependency::{
    create_extvp, create_framing_view, drop_extvp, drop_framing_view, list_extvp,
    list_framing_views,
};
pub(crate) use describe::{create_describe_view, drop_describe_view, list_describe_views};
pub(crate) use materialise::{
    create_datalog_view_from_rule_set, create_datalog_view_from_rules, drop_datalog_view,
    list_datalog_views,
};
pub(crate) use refresh::{create_ask_view, drop_ask_view, list_ask_views};
pub(crate) use sparql::{create_sparql_view, drop_sparql_view, list_sparql_views};
