//! ExtVP semi-join tables and JSON-LD framing views.
//! (extracted from views/mod.rs in v0.114.0)

use pgrx::prelude::*;


use super::{
    PGTRICKLE_HINT, predicate_table_expr, validate_name,
};

/// Create an ExtVP semi-join stream table for two frequently co-joined predicates.
///
/// The stream table pre-computes: subjects that appear in BOTH `pred1_iri` triples
/// and `pred2_iri` triples.  The SPARQL→SQL translator automatically uses these
/// tables for star-pattern optimisation when both predicates appear in the same
/// query.
///
/// Returns the number of rows in the stream table after the first refresh.
pub(crate) fn create_extvp(name: &str, pred1_iri: &str, pred2_iri: &str, schedule: &str) -> i64 {
    if !crate::has_pg_trickle() {
        pgrx::error!(
            "pg_trickle is not installed — ExtVP requires pg_trickle; hint: {}",
            PGTRICKLE_HINT
        );
    }
    if let Err(e) = validate_name(name) {
        pgrx::error!("invalid ExtVP name: {e}");
    }

    let (pred1_id, tbl1) = predicate_table_expr(pred1_iri)
        .unwrap_or_else(|e| pgrx::error!("create_extvp pred1 error: {e}"));
    let (pred2_id, tbl2) = predicate_table_expr(pred2_iri)
        .unwrap_or_else(|e| pgrx::error!("create_extvp pred2 error: {e}"));

    // Semi-join SQL: subjects that have triples for both predicates.
    let extvp_sql = format!(
        "SELECT p1.s, p1.o AS o1, p2.o AS o2 \
         FROM {tbl1} p1 \
         WHERE EXISTS (SELECT 1 FROM {tbl2} p2 WHERE p2.s = p1.s)"
    );

    let escaped_name = name.replace('\'', "''");
    let escaped_schedule = schedule.replace('\'', "''");
    let escaped_sql = extvp_sql.replace('\'', "''");
    let stream_table = format!("_pg_ripple.extvp_{name}");
    let escaped_stream_table = stream_table.replace('\'', "''");

    // Register in catalog.
    // REDUNDANT-01: pred1_iri/pred2_iri TEXT dropped; use pred1_id/pred2_id only.
    Spi::run(&format!(
        "INSERT INTO _pg_ripple.extvp_tables \
         (name, pred1_id, pred2_id, generated_sql, schedule, stream_table) \
         VALUES ('{escaped_name}', \
                 {pred1_id}, {pred2_id}, '{escaped_sql}', \
                 '{escaped_schedule}', '{escaped_stream_table}') \
         ON CONFLICT (name) DO UPDATE \
         SET pred1_id = EXCLUDED.pred1_id, \
             pred2_id = EXCLUDED.pred2_id, \
             generated_sql = EXCLUDED.generated_sql, \
             schedule = EXCLUDED.schedule, \
             stream_table = EXCLUDED.stream_table"
    ))
    .unwrap_or_else(|e| pgrx::error!("failed to register ExtVP: {e}"));

    // Create the pg_trickle stream table.
    // IDEMPOTENT-02 (issue #83): drop any pre-existing stream table so that a
    // repeated call replaces the view cleanly instead of erroring.
    let _ = Spi::run(&format!(
        "SELECT pgtrickle.drop_stream_table(name => '{escaped_stream_table}')"
    ));
    let pgt_sql = format!(
        "SELECT pgtrickle.create_stream_table(\
            name => '{escaped_stream_table}', \
            query => $__extvp_q${extvp_sql}$__extvp_q$, \
            schedule => '{escaped_schedule}'\
        )"
    );
    Spi::run(&pgt_sql).unwrap_or_else(|e| pgrx::error!("failed to create ExtVP stream table: {e}"));

    // Return the initial row count from the stream table.
    Spi::get_one::<i64>(&format!("SELECT COUNT(*)::bigint FROM {stream_table}"))
        .unwrap_or(Some(0))
        .unwrap_or(0)
}

/// Drop an ExtVP table and remove it from the catalog.
pub(crate) fn drop_extvp(name: &str) -> bool {
    if !crate::has_pg_trickle() {
        pgrx::error!(
            "pg_trickle is not installed — ExtVP requires pg_trickle; hint: {}",
            PGTRICKLE_HINT
        );
    }

    let stream_table = format!("_pg_ripple.extvp_{name}");
    let escaped_stream_table = stream_table.replace('\'', "''");

    // Drop the stream table (ignore error if already gone).
    let _ = Spi::run(&format!(
        "SELECT pgtrickle.drop_stream_table(name => '{escaped_stream_table}')"
    ));

    // Remove from catalog.
    Spi::run(&format!(
        "DELETE FROM _pg_ripple.extvp_tables WHERE name = '{}'",
        name.replace('\'', "''")
    ))
    .unwrap_or_else(|e| pgrx::error!("failed to remove ExtVP from catalog: {e}"));

    true
}

/// List all registered ExtVP tables.
///
/// REDUNDANT-01: pred1_iri/pred2_iri TEXT dropped; decode from dictionary for display.
/// Returns a JSONB array of `{name, pred1_iri, pred2_iri, schedule, stream_table, created_at}`.
pub(crate) fn list_extvp() -> pgrx::JsonB {
    Spi::get_one::<pgrx::JsonB>(
        "SELECT COALESCE(json_agg(row_to_json(v))::jsonb, '[]'::jsonb) \
         FROM (SELECT e.name, \
                      (SELECT value FROM _pg_ripple.dictionary WHERE id = e.pred1_id) AS pred1_iri, \
                      (SELECT value FROM _pg_ripple.dictionary WHERE id = e.pred2_id) AS pred2_iri, \
                      e.schedule, e.stream_table, e.created_at \
               FROM _pg_ripple.extvp_tables e ORDER BY e.created_at) v",
    )
    .unwrap_or_else(|e| pgrx::error!("list_extvp SPI error: {e}"))
    .unwrap_or_else(|| pgrx::JsonB(serde_json::Value::Array(vec![])))
}

// ─── Framing views (v0.17.0) ──────────────────────────────────────────────────

/// Create an incrementally-maintained JSON-LD framing view (requires pg_trickle).
///
/// Translates `frame` to a SPARQL CONSTRUCT query using the framing engine,
/// then registers a pg_trickle stream table `pg_ripple.framing_view_{name}`
/// with schema `(subject_id BIGINT, frame_tree JSONB, refreshed_at TIMESTAMPTZ)`.
pub(crate) fn create_framing_view(
    name: &str,
    frame: &serde_json::Value,
    schedule: &str,
    decode: bool,
    output_format: &str,
    immediate: bool,
) {
    if !crate::has_pg_trickle() {
        pgrx::error!(
            "pg_trickle is required for framing views — install pg_trickle and add it to \
             shared_preload_libraries, then retry; hint: {}",
            PGTRICKLE_HINT
        );
    }
    if let Err(e) = validate_name(name) {
        pgrx::error!("invalid framing view name: {e}");
    }

    let construct_query = crate::framing::frame_to_sparql(frame, None)
        .unwrap_or_else(|e| pgrx::error!("frame translation error: {e}"));

    let frame_json = serde_json::to_string(frame).unwrap_or_else(|_| "{}".to_owned());
    // For stream_sql, the frame JSON is embedded in a dollar-quoted SQL literal
    // (used as pg_trickle query body), so we keep the escaped version there.
    let escaped_frame = frame_json.replace('\'', "''");
    let escaped_schedule = schedule.replace('\'', "''");

    // Stream table SQL: run the CONSTRUCT query, embed and compact each root node.
    // Since pg_trickle executes raw SQL, we use the underlying SPARQL execution
    // by calling the pg_ripple function directly.
    let stream_table = format!("pg_ripple.framing_view_{name}");
    let escaped_stream_table = stream_table.replace('\'', "''");

    let stream_sql = format!(
        "SELECT \
            (jsonb_array_elements(r.tree->'@graph'))->>'@id' AS subject_id_text, \
            jsonb_array_elements(r.tree->'@graph') AS frame_tree, \
            now() AS refreshed_at \
         FROM (SELECT pg_ripple.export_jsonld_framed('{escaped_frame}'::jsonb) AS tree) r"
    );

    // SQL-INJ-01 (v0.80.0): parameterised INSERT for framing view catalog entry.
    Spi::run_with_args(
        "INSERT INTO _pg_ripple.framing_views \
         (name, frame, generated_construct, schedule, output_format, decode, created_at) \
         VALUES ($1, $2::jsonb, $3, $4, $5, $6, now()) \
         ON CONFLICT (name) DO UPDATE \
         SET frame = EXCLUDED.frame, \
             generated_construct = EXCLUDED.generated_construct, \
             schedule = EXCLUDED.schedule, \
             output_format = EXCLUDED.output_format, \
             decode = EXCLUDED.decode",
        &[
            pgrx::datum::DatumWithOid::from(name),
            pgrx::datum::DatumWithOid::from(frame_json.as_str()),
            pgrx::datum::DatumWithOid::from(construct_query.as_str()),
            pgrx::datum::DatumWithOid::from(schedule),
            pgrx::datum::DatumWithOid::from(output_format),
            pgrx::datum::DatumWithOid::from(decode),
        ],
    )
    .unwrap_or_else(|e| pgrx::error!("failed to register framing view: {e}"));

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
            query => $__fv_q${stream_sql}$__fv_q$, \
            schedule => '{escaped_schedule}'\
            {refresh_mode_clause}\
        )"
    );
    Spi::run(&pgt_sql)
        .unwrap_or_else(|e| pgrx::error!("failed to create framing view stream table: {e}"));

    // If decode = TRUE, create a thin IRI-decoding view.
    if decode {
        let decode_view = format!("pg_ripple.framing_view_{name}_decoded");
        Spi::run(&format!(
            "CREATE OR REPLACE VIEW {decode_view} AS \
             SELECT pg_ripple.decode_iri(subject_id::bigint) AS subject_iri, \
                    frame_tree, refreshed_at \
             FROM {stream_table}"
        ))
        .unwrap_or_else(|e| pgrx::error!("failed to create decode view: {e}"));
    }
}

/// Drop a framing view stream table and its catalog entry.
pub(crate) fn drop_framing_view(name: &str) -> bool {
    if !crate::has_pg_trickle() {
        pgrx::error!(
            "pg_trickle is not installed — framing views require pg_trickle; hint: {}",
            PGTRICKLE_HINT
        );
    }

    let stream_table = format!("pg_ripple.framing_view_{name}");
    let escaped_stream_table = stream_table.replace('\'', "''");
    let decode_view = format!("pg_ripple.framing_view_{name}_decoded");

    // Drop the decode view (ignore error if absent).
    let _ = Spi::run(&format!("DROP VIEW IF EXISTS {decode_view}"));

    // Drop the stream table (ignore error if already gone).
    let _ = Spi::run(&format!(
        "SELECT pgtrickle.drop_stream_table(name => '{escaped_stream_table}')"
    ));

    // Remove from catalog.
    Spi::run(&format!(
        "DELETE FROM _pg_ripple.framing_views WHERE name = '{}'",
        name.replace('\'', "''")
    ))
    .unwrap_or_else(|e| pgrx::error!("failed to remove framing view from catalog: {e}"));

    true
}

/// List all registered framing views.
///
/// Returns a JSONB array of `{name, frame, schedule, output_format, decode, created_at}`.
pub(crate) fn list_framing_views() -> pgrx::JsonB {
    Spi::get_one::<pgrx::JsonB>(
        "SELECT COALESCE(json_agg(row_to_json(v))::jsonb, '[]'::jsonb) \
         FROM (SELECT name, frame, schedule, output_format, decode, created_at \
               FROM _pg_ripple.framing_views ORDER BY created_at) v",
    )
    .unwrap_or_else(|e| pgrx::error!("list_framing_views SPI error: {e}"))
    .unwrap_or_else(|| pgrx::JsonB(serde_json::Value::Array(vec![])))
}

// ─── CONSTRUCT Views (v0.18.0) ────────────────────────────────────────────────

