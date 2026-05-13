//! SPARQL DESCRIBE views: compile_describe_for_view, create/drop/list_describe_view.
//! (extracted from views/mod.rs in v0.114.0)

use pgrx::prelude::*;
use spargebra::SparqlParser;

use crate::sparql::sqlgen;

use super::{PGTRICKLE_HINT, remap_view_columns, validate_name};

/// Compile a SPARQL DESCRIBE query to a SQL SELECT for a stream table.
///
/// Returns `(sql, strategy)` where `sql` projects `(s BIGINT, p BIGINT, o BIGINT, g BIGINT)`.
///
/// The SQL uses `_pg_ripple.triples_for_resource(resource_id, symmetric)` helper
/// (created by the migration script) to perform the CBD expansion in SQL.
fn compile_describe_for_view(query_text: &str, strategy: &str) -> Result<String, String> {
    let query = SparqlParser::new()
        .parse_query(query_text)
        .map_err(|e| format!("SPARQL parse error: {e}"))?;

    let pattern = match query {
        spargebra::Query::Describe { pattern, .. } => pattern,
        _ => return Err("sparql must be a DESCRIBE query".to_owned()),
    };

    let trans = sqlgen::translate_select(&pattern, None);
    let where_sql = trans.sql;
    let variables = trans.variables;

    let clean_where_sql = remap_view_columns(&where_sql, &variables);
    let include_incoming = strategy == "scbd";

    // Build a SQL that: for each resource returned by the WHERE pattern,
    // calls the CBD helper to enumerate all triples.
    // The WHERE pattern returns BIGINT IDs for each projected variable.
    // We unnest all variables to get the resource IDs.
    let resource_cols: Vec<String> = variables
        .iter()
        .map(|v| format!("_desc_resources_.{v}"))
        .collect();

    let resource_unions: Vec<String> = resource_cols
        .iter()
        .map(|col| format!("SELECT {col} AS resource_id FROM _desc_resources_"))
        .collect();

    let resource_sql = resource_unions.join("\nUNION\n");

    let sql = format!(
        "SELECT t.s, t.p, t.o, 0::bigint AS g \
         FROM ({clean_where_sql}) AS _desc_resources_ \
         CROSS JOIN LATERAL ( \
           SELECT rs.resource_id FROM ({resource_sql}) rs \
         ) _res_ \
         CROSS JOIN LATERAL _pg_ripple.triples_for_resource(_res_.resource_id, {include_incoming}::boolean) t"
    );

    Ok(sql)
}

/// Create a DESCRIBE view — an incrementally-maintained stream table materialising
/// the CBD of the described resources.
///
/// Requires pg_trickle. Raises a descriptive error when absent.
pub(crate) fn create_describe_view(
    name: &str,
    sparql: &str,
    schedule: &str,
    decode: bool,
    immediate: bool,
) {
    if let Err(e) = validate_name(name) {
        pgrx::error!("invalid view name: {e}");
    }
    // Validate query form before pg_trickle check.
    {
        let q = SparqlParser::new()
            .parse_query(sparql)
            .unwrap_or_else(|e| pgrx::error!("SPARQL parse error: {e}"));
        if !matches!(q, spargebra::Query::Describe { .. }) {
            pgrx::error!("sparql must be a DESCRIBE query");
        }
    }
    if !crate::has_pg_trickle() {
        pgrx::error!(
            "pg_trickle is not installed — DESCRIBE views require pg_trickle; hint: {}",
            PGTRICKLE_HINT
        );
    }

    // Read describe_strategy GUC — use the same logic as one-shot sparql_describe().
    let strategy =
        Spi::get_one::<String>("SELECT current_setting('pg_ripple.describe_strategy', true)")
            .unwrap_or(None)
            .unwrap_or_else(|| "cbd".to_owned());
    let strategy = if strategy.is_empty() {
        "cbd".to_owned()
    } else {
        strategy
    };

    let view_sql =
        compile_describe_for_view(sparql, &strategy).unwrap_or_else(|e| pgrx::error!("{e}"));

    let escaped_name = name.replace('\'', "''");
    let escaped_sparql = sparql.replace('\'', "''");
    let escaped_sql = view_sql.replace('\'', "''");
    let escaped_schedule = schedule.replace('\'', "''");
    let escaped_strategy = strategy.replace('\'', "''");
    let stream_table = format!("pg_ripple.describe_view_{name}");
    let escaped_stream_table = stream_table.replace('\'', "''");

    // Store in catalog.
    Spi::run(&format!(
        "INSERT INTO _pg_ripple.describe_views \
         (name, sparql, generated_sql, schedule, decode, strategy, stream_table) \
         VALUES ('{escaped_name}', '{escaped_sparql}', '{escaped_sql}', \
                 '{escaped_schedule}', {decode}, '{escaped_strategy}', '{escaped_stream_table}') \
         ON CONFLICT (name) DO UPDATE \
         SET sparql = EXCLUDED.sparql, \
             generated_sql = EXCLUDED.generated_sql, \
             schedule = EXCLUDED.schedule, \
             decode = EXCLUDED.decode, \
             strategy = EXCLUDED.strategy, \
             stream_table = EXCLUDED.stream_table"
    ))
    .unwrap_or_else(|e| pgrx::error!("failed to register DESCRIBE view: {e}"));

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
            query => $__dv_q${view_sql}$__dv_q$, \
            schedule => '{escaped_schedule}'\
            {refresh_mode_clause}\
        )"
    );
    Spi::run(&pgt_sql)
        .unwrap_or_else(|e| pgrx::error!("failed to create DESCRIBE view stream table: {e}"));

    // If decode = TRUE, create a thin decoding view.
    if decode {
        let decode_view = format!("pg_ripple.describe_view_{name}_decoded");
        Spi::run(&format!(
            "CREATE OR REPLACE VIEW {decode_view} AS \
             SELECT \
               (SELECT d.value FROM _pg_ripple.dictionary d WHERE d.id = dv.s) AS s, \
               (SELECT d.value FROM _pg_ripple.dictionary d WHERE d.id = dv.p) AS p, \
               (SELECT d.value FROM _pg_ripple.dictionary d WHERE d.id = dv.o) AS o, \
               dv.g \
             FROM {stream_table} dv"
        ))
        .unwrap_or_else(|e| pgrx::error!("failed to create DESCRIBE decode view: {e}"));
    }
}

/// Drop a DESCRIBE view and its underlying stream table.
pub(crate) fn drop_describe_view(name: &str) {
    if !crate::has_pg_trickle() {
        pgrx::error!(
            "pg_trickle is not installed — DESCRIBE views require pg_trickle; hint: {}",
            PGTRICKLE_HINT
        );
    }

    let stream_table = format!("pg_ripple.describe_view_{name}");
    let escaped_stream_table = stream_table.replace('\'', "''");
    let decode_view = format!("pg_ripple.describe_view_{name}_decoded");

    let _ = Spi::run(&format!("DROP VIEW IF EXISTS {decode_view}"));
    let _ = Spi::run(&format!(
        "SELECT pgtrickle.drop_stream_table(name => '{escaped_stream_table}')"
    ));

    Spi::run(&format!(
        "DELETE FROM _pg_ripple.describe_views WHERE name = '{}'",
        name.replace('\'', "''")
    ))
    .unwrap_or_else(|e| pgrx::error!("failed to remove DESCRIBE view from catalog: {e}"));
}

/// List all registered DESCRIBE views.
pub(crate) fn list_describe_views() -> pgrx::JsonB {
    Spi::get_one::<pgrx::JsonB>(
        "SELECT COALESCE(json_agg(row_to_json(v))::jsonb, '[]'::jsonb) \
         FROM (SELECT name, sparql, generated_sql, schedule, decode, strategy, \
                      stream_table, created_at \
               FROM _pg_ripple.describe_views ORDER BY created_at) v",
    )
    .unwrap_or_else(|e| pgrx::error!("list_describe_views SPI error: {e}"))
    .unwrap_or_else(|| pgrx::JsonB(serde_json::Value::Array(vec![])))
}

// ─── ASK Views (v0.18.0) ──────────────────────────────────────────────────────
