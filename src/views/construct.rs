//! SPARQL CONSTRUCT views: compile_construct_for_view, create/drop/list_construct_view.
//! (extracted from views/mod.rs in v0.114.0)

use pgrx::prelude::*;
use spargebra::SparqlParser;

use crate::dictionary;
use crate::sparql::sqlgen;

use super::{
    PGTRICKLE_HINT,
    remap_view_columns, validate_name,
};

/// Compile a SPARQL CONSTRUCT query to a SQL SELECT for a stream table.
///
/// Returns `(sql, template_count)` where `sql` projects `(s BIGINT, p BIGINT, o BIGINT, g BIGINT)`.
/// Each template triple becomes one row in the UNION ALL.
///
/// Constant IRI/literal terms in the template are dictionary-encoded at view-creation time
/// (integer literals in SQL — no string comparisons at refresh time).
///
/// Errors:
/// - Blank node in template (not expressible as a stable BIGINT)
/// - Variable in template that is not bound in the WHERE pattern
fn compile_construct_for_view(query_text: &str) -> Result<(String, usize, usize), String> {
    let query = SparqlParser::new()
        .parse_query(query_text)
        .map_err(|e| format!("SPARQL parse error: {e}"))?;

    let (template, pattern) = match query {
        spargebra::Query::Construct {
            template, pattern, ..
        } => (template, pattern),
        _ => return Err("sparql must be a CONSTRUCT query".to_owned()),
    };

    let trans = sqlgen::translate_select(&pattern, None);
    let where_sql = trans.sql;
    let variables = trans.variables;
    let var_set: std::collections::HashSet<&str> = variables.iter().map(|s| s.as_str()).collect();

    // Validate template: no blank nodes; all variables bound.
    for triple in &template {
        match &triple.subject {
            spargebra::term::TermPattern::BlankNode(_) => {
                return Err(
                    "CONSTRUCT template contains a blank node subject; replace blank \
                     nodes with IRIs or use skolemisation before registering as a view"
                        .to_owned(),
                );
            }
            spargebra::term::TermPattern::Variable(v) if !var_set.contains(v.as_str()) => {
                return Err(format!(
                    "variable ?{} appears in the CONSTRUCT template but is not bound \
                     by the WHERE pattern",
                    v.as_str()
                ));
            }
            _ => {}
        }
        match &triple.predicate {
            spargebra::term::NamedNodePattern::Variable(v) if !var_set.contains(v.as_str()) => {
                return Err(format!(
                    "variable ?{} appears in the CONSTRUCT template but is not bound \
                     by the WHERE pattern",
                    v.as_str()
                ));
            }
            _ => {}
        }
        match &triple.object {
            spargebra::term::TermPattern::BlankNode(_) => {
                return Err(
                    "CONSTRUCT template contains a blank node object; replace blank \
                     nodes with IRIs or use skolemisation before registering as a view"
                        .to_owned(),
                );
            }
            spargebra::term::TermPattern::Variable(v) if !var_set.contains(v.as_str()) => {
                return Err(format!(
                    "variable ?{} appears in the CONSTRUCT template but is not bound \
                     by the WHERE pattern",
                    v.as_str()
                ));
            }
            _ => {}
        }
    }

    let template_count = template.len();
    if template_count == 0 {
        return Err("CONSTRUCT template is empty".to_owned());
    }

    // Remap WHERE SQL column aliases from `_v_{var}` to `{var}`.
    let clean_where_sql = remap_view_columns(&where_sql, &variables);

    let var_col = |v: &str| -> String {
        // Column name in the remapped WHERE SQL.
        format!("_construct_inner_.{v}")
    };

    let resolve_subject = |tp: &spargebra::term::TermPattern| -> Result<String, String> {
        match tp {
            spargebra::term::TermPattern::NamedNode(nn) => {
                let id = dictionary::encode(nn.as_str(), dictionary::KIND_IRI);
                Ok(format!("{id}::bigint"))
            }
            spargebra::term::TermPattern::Variable(v) => Ok(var_col(v.as_str())),
            _ => Err(
                "internal: blank node or RDF-star subject reached resolver — please report"
                    .to_owned(),
            ),
        }
    };

    let resolve_predicate = |np: &spargebra::term::NamedNodePattern| -> Result<String, String> {
        match np {
            spargebra::term::NamedNodePattern::NamedNode(nn) => {
                let id = dictionary::encode(nn.as_str(), dictionary::KIND_IRI);
                Ok(format!("{id}::bigint"))
            }
            spargebra::term::NamedNodePattern::Variable(v) => Ok(var_col(v.as_str())),
        }
    };

    let resolve_object = |tp: &spargebra::term::TermPattern| -> Result<String, String> {
        match tp {
            spargebra::term::TermPattern::NamedNode(nn) => {
                let id = dictionary::encode(nn.as_str(), dictionary::KIND_IRI);
                Ok(format!("{id}::bigint"))
            }
            spargebra::term::TermPattern::Literal(lit) => {
                let id = if let Some(lang) = lit.language() {
                    dictionary::encode_lang_literal(lit.value(), lang)
                } else {
                    dictionary::encode_typed_literal(lit.value(), lit.datatype().as_str())
                };
                Ok(format!("{id}::bigint"))
            }
            spargebra::term::TermPattern::Variable(v) => Ok(var_col(v.as_str())),
            spargebra::term::TermPattern::BlankNode(_) => {
                Err("internal: blank node object reached resolver — please report".to_owned())
            }
            spargebra::term::TermPattern::Triple(_) => {
                Err("CONSTRUCT template contains an RDF-star quoted triple; \
                     RDF-star template terms are not supported in views"
                    .to_owned())
            }
        }
    };

    // Build UNION ALL of per-template-triple SELECTs.
    let mut parts: Vec<String> = Vec::with_capacity(template_count);
    for triple in &template {
        let s_expr = resolve_subject(&triple.subject)?;
        let p_expr = resolve_predicate(&triple.predicate)?;
        let o_expr = resolve_object(&triple.object)?;
        parts.push(format!(
            "SELECT {s_expr} AS s, {p_expr} AS p, {o_expr} AS o, 0::bigint AS g \
             FROM ({clean_where_sql}) AS _construct_inner_"
        ));
    }

    let union_sql = parts.join("\nUNION ALL\n");
    Ok((union_sql, template_count, variables.len()))
}

/// Create a CONSTRUCT view — an incrementally-maintained stream table whose rows
/// reflect the CONSTRUCT template output at all times.
///
/// Requires pg_trickle. Raises a descriptive error when absent.
///
/// Returns the number of template triples registered.
pub(crate) fn create_construct_view(
    name: &str,
    sparql: &str,
    schedule: &str,
    decode: bool,
    immediate: bool,
) -> i64 {
    if let Err(e) = validate_name(name) {
        pgrx::error!("invalid view name: {e}");
    }
    // Validate query form before pg_trickle check so user gets the right error
    // even without pg_trickle installed.
    {
        let q = SparqlParser::new()
            .parse_query(sparql)
            .unwrap_or_else(|e| pgrx::error!("SPARQL parse error: {e}"));
        if !matches!(q, spargebra::Query::Construct { .. }) {
            pgrx::error!("sparql must be a CONSTRUCT query");
        }
    }
    if !crate::has_pg_trickle() {
        pgrx::error!(
            "pg_trickle is not installed — CONSTRUCT views require pg_trickle; hint: {}",
            PGTRICKLE_HINT
        );
    }

    let (view_sql, template_count, _var_count) =
        compile_construct_for_view(sparql).unwrap_or_else(|e| pgrx::error!("{e}"));

    let template_count_i64 = template_count as i64;
    let stream_table = format!("pg_ripple.construct_view_{name}");

    // SQL-INJ-01 (v0.80.0): parameterised INSERT for construct view catalog entry.
    Spi::run_with_args(
        "INSERT INTO _pg_ripple.construct_views \
         (name, sparql, generated_sql, schedule, decode, template_count, stream_table) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) \
         ON CONFLICT (name) DO UPDATE \
         SET sparql = EXCLUDED.sparql, \
             generated_sql = EXCLUDED.generated_sql, \
             schedule = EXCLUDED.schedule, \
             decode = EXCLUDED.decode, \
             template_count = EXCLUDED.template_count, \
             stream_table = EXCLUDED.stream_table",
        &[
            pgrx::datum::DatumWithOid::from(name),
            pgrx::datum::DatumWithOid::from(sparql),
            pgrx::datum::DatumWithOid::from(view_sql.as_str()),
            pgrx::datum::DatumWithOid::from(schedule),
            pgrx::datum::DatumWithOid::from(decode),
            pgrx::datum::DatumWithOid::from(template_count_i64),
            pgrx::datum::DatumWithOid::from(stream_table.as_str()),
        ],
    )
    .unwrap_or_else(|e| pgrx::error!("failed to register CONSTRUCT view: {e}"));

    // Create the pg_trickle stream table.
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
            query => $__cv_q${view_sql}$__cv_q$, \
            schedule => '{escaped_schedule}'\
            {refresh_mode_clause}\
        )"
    );
    Spi::run(&pgt_sql)
        .unwrap_or_else(|e| pgrx::error!("failed to create CONSTRUCT view stream table: {e}"));

    // If decode = TRUE, create a thin decoding view.
    if decode {
        let decode_view = format!("pg_ripple.construct_view_{name}_decoded");
        Spi::run(&format!(
            "CREATE OR REPLACE VIEW {decode_view} AS \
             SELECT \
               (SELECT d.value FROM _pg_ripple.dictionary d WHERE d.id = cv.s) AS s, \
               (SELECT d.value FROM _pg_ripple.dictionary d WHERE d.id = cv.p) AS p, \
               (SELECT d.value FROM _pg_ripple.dictionary d WHERE d.id = cv.o) AS o, \
               cv.g \
             FROM {stream_table} cv"
        ))
        .unwrap_or_else(|e| pgrx::error!("failed to create CONSTRUCT decode view: {e}"));
    }

    template_count_i64
}

/// Drop a CONSTRUCT view and its underlying stream table.
pub(crate) fn drop_construct_view(name: &str) {
    if !crate::has_pg_trickle() {
        pgrx::error!(
            "pg_trickle is not installed — CONSTRUCT views require pg_trickle; hint: {}",
            PGTRICKLE_HINT
        );
    }

    let stream_table = format!("pg_ripple.construct_view_{name}");
    let escaped_stream_table = stream_table.replace('\'', "''");
    let decode_view = format!("pg_ripple.construct_view_{name}_decoded");

    let _ = Spi::run(&format!("DROP VIEW IF EXISTS {decode_view}"));
    let _ = Spi::run(&format!(
        "SELECT pgtrickle.drop_stream_table(name => '{escaped_stream_table}')"
    ));

    Spi::run(&format!(
        "DELETE FROM _pg_ripple.construct_views WHERE name = '{}'",
        name.replace('\'', "''")
    ))
    .unwrap_or_else(|e| pgrx::error!("failed to remove CONSTRUCT view from catalog: {e}"));
}

/// List all registered CONSTRUCT views.
pub(crate) fn list_construct_views() -> pgrx::JsonB {
    Spi::get_one::<pgrx::JsonB>(
        "SELECT COALESCE(json_agg(row_to_json(v))::jsonb, '[]'::jsonb) \
         FROM (SELECT name, sparql, generated_sql, schedule, decode, template_count, \
                      stream_table, created_at \
               FROM _pg_ripple.construct_views ORDER BY created_at) v",
    )
    .unwrap_or_else(|e| pgrx::error!("list_construct_views SPI error: {e}"))
    .unwrap_or_else(|| pgrx::JsonB(serde_json::Value::Array(vec![])))
}

// ─── DESCRIBE Views (v0.18.0) ─────────────────────────────────────────────────

