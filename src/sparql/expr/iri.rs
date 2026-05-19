//! IRI, blank-node, language, and datatype SPARQL function translation (H17-02, v0.122.0).
//!
//! Handles: `IRI`/`URI`, `BNODE`, `LANG`, `DATATYPE`, and XSD constructor cast functions.

use std::collections::HashMap;

use spargebra::algebra::{Expression, Function};

use super::super::sqlgen::Ctx;
use super::cast::{xsd_cast_datatype, xsd_cast_sql};
use super::{decode_lexical_sql, translate_arg_value};

fn encode_iri(sql: String) -> String {
    format!("pg_ripple.encode_term({sql}, 0::int2)")
}

fn encode_literal(sql: String) -> String {
    format!("pg_ripple.encode_term({sql}, 2::int2)")
}

/// Translate an IRI/BNODE/LANG/DATATYPE SPARQL built-in function in value context.
///
/// Returns `Some(sql)` for handled functions and `None` for all others.
pub(super) fn translate(
    func: &Function,
    args: &[Expression],
    bindings: &HashMap<String, String>,
    ctx: &mut Ctx,
    is_numeric: &mut bool,
) -> Option<String> {
    let _ = is_numeric; // None of these set is_numeric = true directly.
    match func {
        // ── IRI / URI ────────────────────────────────────────────────────────
        Function::Iri => {
            if let Some(Expression::NamedNode(nn)) = args.first() {
                let iri = nn.as_str();
                if let Some(id) = ctx.encode_iri(iri) {
                    return Some(id.to_string());
                }
                let iri_esc = iri.replace('\'', "''");
                return Some(format!("pg_ripple.encode_term('{iri_esc}', 0::int2)"));
            }
            let col = translate_arg_value(args.first()?, bindings, ctx)?;
            let text = decode_lexical_sql(&col);
            if let Some(base) = &ctx.base_iri.clone() {
                let base_escaped = base.replace('\'', "''");
                Some(encode_iri(format!(
                    "(CASE WHEN ({text}) ~ '^[A-Za-z][A-Za-z0-9+\\-.]*:' \
                          THEN ({text}) \
                          ELSE '{base_escaped}' || ({text}) \
                     END)"
                )))
            } else {
                Some(encode_iri(text))
            }
        }

        // ── BNODE ───────────────────────────────────────────────────────────
        Function::BNode => {
            if args.is_empty() {
                Some("pg_ripple.encode_term('_:b' || gen_random_uuid()::text, 1::int2)".to_owned())
            } else {
                let col = translate_arg_value(args.first()?, bindings, ctx)?;
                let text = decode_lexical_sql(&col);
                Some(format!("pg_ripple.encode_term('_:' || {text}, 1::int2)"))
            }
        }

        // ── LANG ────────────────────────────────────────────────────────────
        Function::Lang => {
            let col = translate_arg_value(args.first()?, bindings, ctx)?;
            Some(encode_literal(format!(
                "COALESCE(\
                    (SELECT d.lang FROM _pg_ripple.dictionary d WHERE d.id = {col} AND d.kind = 4),\
                    '')"
            )))
        }

        // ── DATATYPE ─────────────────────────────────────────────────────────
        Function::Datatype => {
            if let Some(Expression::Variable(v)) = args.first()
                && ctx.is_raw_double_var(v.as_str())
            {
                return Some(encode_iri(
                    "'http://www.w3.org/2001/XMLSchema#double'".to_owned(),
                ));
            }
            let col = translate_arg_value(args.first()?, bindings, ctx)?;
            Some(encode_iri(format!(
                "CASE \
                   WHEN {col} IS NULL THEN NULL \
                   WHEN {col} < 0 THEN \
                     CASE (({col} & 9151314442816847872::bigint) >> 56) \
                       WHEN 0 THEN 'http://www.w3.org/2001/XMLSchema#integer' \
                       WHEN 1 THEN 'http://www.w3.org/2001/XMLSchema#boolean' \
                       WHEN 2 THEN 'http://www.w3.org/2001/XMLSchema#dateTime' \
                       WHEN 3 THEN 'http://www.w3.org/2001/XMLSchema#date' \
                       ELSE 'http://www.w3.org/2001/XMLSchema#integer' \
                     END \
                   ELSE (\
                     SELECT CASE d.kind \
                       WHEN 3 THEN d.datatype \
                       WHEN 2 THEN 'http://www.w3.org/2001/XMLSchema#string' \
                       WHEN 4 THEN 'http://www.w3.org/1999/02/22-rdf-syntax-ns#langString' \
                       ELSE NULL \
                     END \
                     FROM _pg_ripple.dictionary d WHERE d.id = {col}\
                   )\
                 END"
            )))
        }

        _ => None,
    }
}

/// Translate an XSD constructor custom function (e.g. `xsd:integer(?v)`) in value context.
///
/// Returns `Some(sql)` when `iri` matches an XSD type cast IRI and `None` otherwise.
pub(super) fn translate_xsd_cast(
    iri: &str,
    args: &[Expression],
    bindings: &HashMap<String, String>,
    ctx: &mut Ctx,
) -> Option<String> {
    let dt = xsd_cast_datatype(iri)?;
    let arg_col = translate_arg_value(args.first()?, bindings, ctx)?;
    Some(xsd_cast_sql(&arg_col, dt))
}
