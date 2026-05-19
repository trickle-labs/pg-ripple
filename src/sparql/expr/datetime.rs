//! Datetime SPARQL built-in function translation (H17-02, v0.122.0).
//!
//! Handles: `NOW`, `YEAR`, `MONTH`, `DAY`, `HOURS`, `MINUTES`, `SECONDS`,
//! `TIMEZONE`, `TZ`.

use std::collections::HashMap;

use spargebra::algebra::{Expression, Function};

use super::super::sqlgen::Ctx;
use super::{decode_lexical_sql, translate_arg_value};

fn encode_literal(sql: String) -> String {
    format!("pg_ripple.encode_term({sql}, 2::int2)")
}

/// Translate a datetime SPARQL built-in function in value context.
///
/// Returns `Some(sql)` for handled functions and `None` for all others.
/// Sets `is_numeric = true` for `YEAR`, `MONTH`, `DAY`, `HOURS`, `MINUTES`.
pub(super) fn translate(
    func: &Function,
    args: &[Expression],
    bindings: &HashMap<String, String>,
    ctx: &mut Ctx,
    is_numeric: &mut bool,
) -> Option<String> {
    match func {
        Function::Now => {
            // NOW() → encode current timestamp as xsd:dateTime typed literal.
            Some(
                "pg_ripple.encode_typed_literal(\
                    to_char(now(), 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"'), \
                    'http://www.w3.org/2001/XMLSchema#dateTime')"
                    .to_string(),
            )
        }
        Function::Year => {
            *is_numeric = true;
            let col = translate_arg_value(args.first()?, bindings, ctx)?;
            let text = decode_lexical_sql(&col);
            Some(format!("(substring({text} FROM '^(\\d{{4}})-'))::bigint"))
        }
        Function::Month => {
            *is_numeric = true;
            let col = translate_arg_value(args.first()?, bindings, ctx)?;
            let text = decode_lexical_sql(&col);
            Some(format!(
                "(substring({text} FROM '^\\d{{4}}-(\\d{{2}})-'))::bigint"
            ))
        }
        Function::Day => {
            *is_numeric = true;
            let col = translate_arg_value(args.first()?, bindings, ctx)?;
            let text = decode_lexical_sql(&col);
            Some(format!(
                "(substring({text} FROM '^\\d{{4}}-\\d{{2}}-(\\d{{2}})T'))::bigint"
            ))
        }
        Function::Hours => {
            *is_numeric = true;
            let col = translate_arg_value(args.first()?, bindings, ctx)?;
            let text = decode_lexical_sql(&col);
            Some(format!("(substring({text} FROM 'T(\\d{{2}}):'))::bigint"))
        }
        Function::Minutes => {
            *is_numeric = true;
            let col = translate_arg_value(args.first()?, bindings, ctx)?;
            let text = decode_lexical_sql(&col);
            Some(format!(
                "(substring({text} FROM 'T\\d{{2}}:(\\d{{2}}):'))::bigint"
            ))
        }
        Function::Seconds => {
            // SPARQL spec: SECONDS returns xsd:decimal (not xsd:integer).
            let col = translate_arg_value(args.first()?, bindings, ctx)?;
            let text = decode_lexical_sql(&col);
            Some(format!(
                "pg_ripple.encode_typed_literal(\
                    RTRIM(RTRIM((COALESCE(substring({text} FROM 'T\\d{{2}}:\\d{{2}}:(\\d+(?:\\.\\d+)?)'), '0'))::numeric::text, '0'), '.'), \
                    'http://www.w3.org/2001/XMLSchema#decimal')"
            ))
        }
        Function::Timezone => {
            let col = translate_arg_value(args.first()?, bindings, ctx)?;
            let text = decode_lexical_sql(&col);
            let tz_expr = format!(
                "CASE \
                   WHEN ({text}) LIKE '%Z' THEN 'PT0S' \
                   WHEN ({text}) ~ '[+-]\\d{{2}}:\\d{{2}}$' THEN (\
                     WITH tz AS (SELECT substring(({text}) from '[+-]\\d{{2}}:\\d{{2}}$') AS t) \
                     SELECT CASE \
                       WHEN t = '+00:00' OR t = '-00:00' THEN 'PT0S' \
                       WHEN left(t,1) = '-' THEN '-PT' || ltrim(substring(t from 2 for 2),'0') || 'H' \
                       ELSE 'PT' || ltrim(substring(t from 2 for 2),'0') || 'H' \
                     END FROM tz) \
                   ELSE NULL \
                 END"
            );
            Some(format!(
                "pg_ripple.encode_typed_literal(\
                    ({tz_expr}), \
                    'http://www.w3.org/2001/XMLSchema#dayTimeDuration')"
            ))
        }
        Function::Tz => {
            let col = translate_arg_value(args.first()?, bindings, ctx)?;
            let text = decode_lexical_sql(&col);
            Some(encode_literal(format!(
                "CASE WHEN ({text}) LIKE '%Z' THEN 'Z' \
                      WHEN ({text}) ~ '[+-]\\d{{2}}:\\d{{2}}$' \
                           THEN regexp_replace({text}, '.*(([+-]\\d{{2}}:\\d{{2}}))$', '\\1') \
                      ELSE '' END"
            )))
        }
        _ => None,
    }
}
