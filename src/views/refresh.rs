//! SPARQL ASK views: compile_ask_for_view, create/drop/list_ask_view.
//! (extracted from views/mod.rs in v0.114.0)

use pgrx::prelude::*;
use spargebra::SparqlParser;

use crate::sparql::sqlgen;

use super::{PGTRICKLE_HINT, validate_name};

/// Compile a SPARQL ASK query to a SQL SELECT for a stream table.
///
/// Returns SQL of the form `SELECT EXISTS(...) AS result, now() AS evaluated_at`.
fn compile_ask_for_view(query_text: &str) -> Result<String, String> {
    let query = SparqlParser::new()
        .parse_query(query_text)
        .map_err(|e| format!("SPARQL parse error: {e}"))?;

    let pattern = match query {
        spargebra::Query::Ask { pattern, .. } => pattern,
        _ => return Err("sparql must be an ASK query".to_owned()),
    };

    let exists_sql = sqlgen::translate_ask(&pattern);
    Ok(format!(
        "SELECT ({exists_sql}) AS result, now() AS evaluated_at"
    ))
}

/// Create an ASK view — an incrementally-maintained single-row stream table
/// whose `result` column flips whenever the underlying pattern's satisfiability changes.
///
/// Requires pg_trickle. Raises a descriptive error when absent.
pub(crate) fn create_ask_view(name: &str, sparql: &str, schedule: &str, immediate: bool) {
    if let Err(e) = validate_name(name) {
        pgrx::error!("invalid view name: {e}");
    }
    // Validate query form before pg_trickle check.
    {
        let q = SparqlParser::new()
            .parse_query(sparql)
            .unwrap_or_else(|e| pgrx::error!("SPARQL parse error: {e}"));
        if !matches!(q, spargebra::Query::Ask { .. }) {
            pgrx::error!("sparql must be an ASK query");
        }
    }
    if !crate::has_pg_trickle() {
        pgrx::error!(
            "pg_trickle is not installed — ASK views require pg_trickle; hint: {}",
            PGTRICKLE_HINT
        );
    }

    let view_sql = compile_ask_for_view(sparql).unwrap_or_else(|e| pgrx::error!("{e}"));

    let escaped_name = name.replace('\'', "''");
    let escaped_sparql = sparql.replace('\'', "''");
    let escaped_sql = view_sql.replace('\'', "''");
    let escaped_schedule = schedule.replace('\'', "''");
    let stream_table = format!("pg_ripple.ask_view_{name}");
    let escaped_stream_table = stream_table.replace('\'', "''");

    // Store in catalog.
    Spi::run(&format!(
        "INSERT INTO _pg_ripple.ask_views \
         (name, sparql, generated_sql, schedule, stream_table) \
         VALUES ('{escaped_name}', '{escaped_sparql}', '{escaped_sql}', \
                 '{escaped_schedule}', '{escaped_stream_table}') \
         ON CONFLICT (name) DO UPDATE \
         SET sparql = EXCLUDED.sparql, \
             generated_sql = EXCLUDED.generated_sql, \
             schedule = EXCLUDED.schedule, \
             stream_table = EXCLUDED.stream_table"
    ))
    .unwrap_or_else(|e| pgrx::error!("failed to register ASK view: {e}"));

    // Create the pg_trickle stream table.
    // IDEMPOTENT-02 (issue #83): drop any pre-existing stream table so that a
    // repeated call replaces the view cleanly instead of erroring.
    let _ = Spi::run(&format!(
        "SELECT pgtrickle.drop_stream_table(name => '{escaped_stream_table}')"
    ));
    let refresh_mode_clause = if immediate {
        ", refresh_mode => 'IMMEDIATE'"
    } else {
        ""
    };
    let pgt_sql = format!(
        "SELECT pgtrickle.create_stream_table(\
            name => '{escaped_stream_table}', \
            query => $__av_q${view_sql}$__av_q$, \
            schedule => '{escaped_schedule}'\
            {refresh_mode_clause}\
        )"
    );
    Spi::run(&pgt_sql)
        .unwrap_or_else(|e| pgrx::error!("failed to create ASK view stream table: {e}"));
}

/// Drop an ASK view and its underlying stream table.
pub(crate) fn drop_ask_view(name: &str) {
    if !crate::has_pg_trickle() {
        pgrx::error!(
            "pg_trickle is not installed — ASK views require pg_trickle; hint: {}",
            PGTRICKLE_HINT
        );
    }

    let stream_table = format!("pg_ripple.ask_view_{name}");
    let escaped_stream_table = stream_table.replace('\'', "''");

    let _ = Spi::run(&format!(
        "SELECT pgtrickle.drop_stream_table(name => '{escaped_stream_table}')"
    ));

    Spi::run(&format!(
        "DELETE FROM _pg_ripple.ask_views WHERE name = '{}'",
        name.replace('\'', "''")
    ))
    .unwrap_or_else(|e| pgrx::error!("failed to remove ASK view from catalog: {e}"));
}

/// List all registered ASK views.
pub(crate) fn list_ask_views() -> pgrx::JsonB {
    Spi::get_one::<pgrx::JsonB>(
        "SELECT COALESCE(json_agg(row_to_json(v))::jsonb, '[]'::jsonb) \
         FROM (SELECT name, sparql, generated_sql, schedule, stream_table, created_at \
               FROM _pg_ripple.ask_views ORDER BY created_at) v",
    )
    .unwrap_or_else(|e| pgrx::error!("list_ask_views SPI error: {e}"))
    .unwrap_or_else(|| pgrx::JsonB(serde_json::Value::Array(vec![])))
}
