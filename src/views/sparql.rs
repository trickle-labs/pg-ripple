//! SPARQL result views: create/drop/list_sparql_view.
//! (extracted from views/mod.rs in v0.114.0)

use pgrx::prelude::*;

use super::{PGTRICKLE_HINT, compile_sparql_for_view, validate_name};

/// Create a named, incrementally-maintained SPARQL result table.
///
/// Requires pg_trickle. Raises an error with an install hint if absent.
///
/// Parameters:
/// - `name` — name for the view (also used as the pg_trickle stream table name under `pg_ripple`)
/// - `sparql` — a SPARQL SELECT query
/// - `schedule` — pg_trickle schedule string, e.g. `'1s'`, `'IMMEDIATE'`, `'30s'`
/// - `decode` — when `false` (recommended), the stream table stores `BIGINT` IDs with a decode view
///   on top; when `true`, the stream table stores decoded `TEXT` values
///
/// Returns the number of projected variables (columns) in the view.
pub(crate) fn create_sparql_view(
    name: &str,
    sparql: &str,
    schedule: &str,
    decode: bool,
    immediate: bool,
) -> i64 {
    if !crate::has_pg_trickle() {
        pgrx::error!(
            "pg_trickle is not installed — SPARQL views require pg_trickle; hint: {}",
            PGTRICKLE_HINT
        );
    }
    if let Err(e) = validate_name(name) {
        pgrx::error!("invalid view name: {e}");
    }

    let (view_sql, variables) = compile_sparql_for_view(sparql)
        .unwrap_or_else(|e| pgrx::error!("SPARQL view compilation failed: {e}"));

    let var_count = variables.len() as i64;
    let variables_json = serde_json::to_string(&variables).unwrap_or_else(|_| "[]".to_owned());

    let stream_table = format!("pg_ripple.{name}");

    // SQL-INJ-01 (v0.80.0): use parameterised INSERT to prevent SQL injection
    // via user-supplied view name, SPARQL text, schedule, or generated SQL.
    Spi::run_with_args(
        "INSERT INTO _pg_ripple.sparql_views \
         (name, sparql, generated_sql, schedule, decode, stream_table, variables) \
         VALUES ($1, $2, $3, $4, $5, $6, $7::jsonb) \
         ON CONFLICT (name) DO UPDATE \
         SET sparql = EXCLUDED.sparql, \
             generated_sql = EXCLUDED.generated_sql, \
             schedule = EXCLUDED.schedule, \
             decode = EXCLUDED.decode, \
             stream_table = EXCLUDED.stream_table, \
             variables = EXCLUDED.variables",
        &[
            pgrx::datum::DatumWithOid::from(name),
            pgrx::datum::DatumWithOid::from(sparql),
            pgrx::datum::DatumWithOid::from(view_sql.as_str()),
            pgrx::datum::DatumWithOid::from(schedule),
            pgrx::datum::DatumWithOid::from(decode),
            pgrx::datum::DatumWithOid::from(stream_table.as_str()),
            pgrx::datum::DatumWithOid::from(variables_json.as_str()),
        ],
    )
    .unwrap_or_else(|e| pgrx::error!("failed to register SPARQL view: {e}"));

    // Create the pg_trickle stream table.  The view SQL is passed via a
    // dollar-quoted literal so the schedule and stream_table name need their
    // own escaping for the function-call argument list.
    // The stream table always stores BIGINT dictionary IDs so that pg_trickle
    // IVM can diff rows via integer comparison (fix for issue #81).
    let escaped_stream_table = stream_table.replace('\'', "''");
    let escaped_schedule = schedule.replace('\'', "''");

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
            query => $__pgrst_q${view_sql}$__pgrst_q$, \
            schedule => '{escaped_schedule}'\
            {refresh_mode_clause}\
        )"
    );
    Spi::run(&pgt_sql)
        .unwrap_or_else(|e| pgrx::error!("failed to create pg_trickle stream table: {e}"));

    // If decode = true, create a thin companion VIEW that decodes BIGINT IDs
    // to TEXT strings.  This mirrors the pattern used by create_construct_view
    // and keeps the stream table columns as BIGINT for IVM correctness.
    if decode {
        let decode_view = format!("pg_ripple.{name}_decoded");
        let inner_alias = "_sv_";
        let decode_cols: Vec<String> = variables
            .iter()
            .map(|v| {
                format!("(SELECT d.value FROM _pg_ripple.dictionary d WHERE d.id = {inner_alias}.{v}) AS {v}")
            })
            .collect();
        Spi::run(&format!(
            "CREATE OR REPLACE VIEW {decode_view} AS \
             SELECT {} FROM {stream_table} {inner_alias}",
            decode_cols.join(", ")
        ))
        .unwrap_or_else(|e| pgrx::error!("failed to create SPARQL decode view: {e}"));
    }

    var_count
}

/// Drop a SPARQL view and its underlying stream table.
pub(crate) fn drop_sparql_view(name: &str) -> bool {
    if !crate::has_pg_trickle() {
        pgrx::error!(
            "pg_trickle is not installed — SPARQL views require pg_trickle; hint: {}",
            PGTRICKLE_HINT
        );
    }

    let stream_table = format!("pg_ripple.{name}");
    let escaped_stream_table = stream_table.replace('\'', "''");
    let decode_view = format!("pg_ripple.{name}_decoded");

    // Drop the companion decode view if it was created (ignore error if absent).
    let _ = Spi::run(&format!("DROP VIEW IF EXISTS {decode_view}"));

    // Drop the stream table (ignore error if already gone).
    let _ = Spi::run(&format!(
        "SELECT pgtrickle.drop_stream_table(name => '{escaped_stream_table}')"
    ));

    // Remove from catalog.
    Spi::run(&format!(
        "DELETE FROM _pg_ripple.sparql_views WHERE name = '{}'",
        name.replace('\'', "''")
    ))
    .unwrap_or_else(|e| pgrx::error!("failed to remove SPARQL view from catalog: {e}"));

    true
}

/// List all registered SPARQL views.
///
/// Returns a JSONB array of `{name, sparql, schedule, decode, stream_table, created_at}` objects.
pub(crate) fn list_sparql_views() -> pgrx::JsonB {
    Spi::get_one::<pgrx::JsonB>(
        "SELECT COALESCE(json_agg(row_to_json(v))::jsonb, '[]'::jsonb) \
         FROM (SELECT name, sparql, schedule, decode, stream_table, variables, created_at \
               FROM _pg_ripple.sparql_views ORDER BY created_at) v",
    )
    .unwrap_or_else(|e| pgrx::error!("list_sparql_views SPI error: {e}"))
    .unwrap_or_else(|| pgrx::JsonB(serde_json::Value::Array(vec![])))
}

// ─── Datalog Views ───────────────────────────────────────────────────────────
