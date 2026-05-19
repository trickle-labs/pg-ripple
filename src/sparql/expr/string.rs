//! String SPARQL built-in function translation (H17-02, v0.122.0).
//!
//! Handles: `STR`, `STRLEN`, `SUBSTR`, `UCASE`, `LCASE`, `CONCAT`, `REPLACE`,
//! `ENCODE_FOR_URI`, `STRLANG`, `STRDT`, `STRBEFORE`, `STRAFTER`,
//! `MD5`, `SHA1`, `SHA256`, `SHA384`, `SHA512`, `UUID`, `STRUUID`.

use std::collections::HashMap;

use spargebra::algebra::{Expression, Function};

use super::super::sqlgen::Ctx;
use super::{decode_lexical_sql, encode_preserving_lang, translate_arg_text, translate_arg_value};

fn encode_literal(sql: String) -> String {
    format!("pg_ripple.encode_term({sql}, 2::int2)")
}

/// Translate a string SPARQL built-in function in value context.
///
/// Returns `Some(sql)` for handled functions and `None` for all others.
/// Sets `is_numeric = true` for `STRLEN`.
pub(super) fn translate(
    func: &Function,
    args: &[Expression],
    bindings: &HashMap<String, String>,
    ctx: &mut Ctx,
    is_numeric: &mut bool,
) -> Option<String> {
    match func {
        // ── STR ──────────────────────────────────────────────────────────────
        Function::Str => {
            let col = translate_arg_value(args.first()?, bindings, ctx)?;
            let text = decode_lexical_sql(&col);
            Some(encode_literal(text))
        }

        // ── STRLEN ───────────────────────────────────────────────────────────
        Function::StrLen => {
            *is_numeric = true;
            if let Some(Expression::Variable(v)) = args.first()
                && ctx.is_raw_text_var(v.as_str())
            {
                let col = bindings.get(v.as_str())?;
                return Some(format!("length({col})"));
            }
            if let Some(Expression::FunctionCall(Function::Str, str_inner)) = args.first()
                && let Some(Expression::Variable(v)) = str_inner.first()
                && ctx.is_raw_iri_var(v.as_str())
            {
                let col = bindings.get(v.as_str())?;
                return Some(format!("length({col})"));
            }
            let col = translate_arg_value(args.first()?, bindings, ctx)?;
            let text = decode_lexical_sql(&col);
            Some(format!("length({text})"))
        }

        // ── SUBSTR ───────────────────────────────────────────────────────────
        Function::SubStr => {
            let str_col = translate_arg_value(args.first()?, bindings, ctx)?;
            let str_text = decode_lexical_sql(&str_col);
            let start = translate_arg_value(args.get(1)?, bindings, ctx)?;
            let start_text = decode_lexical_sql(&start);
            let new_lex = if let Some(len_arg) = args.get(2) {
                let len = translate_arg_value(len_arg, bindings, ctx)?;
                let len_text = decode_lexical_sql(&len);
                format!("substr({str_text}, ({start_text})::int, ({len_text})::int)")
            } else {
                format!("substr({str_text}, ({start_text})::int)")
            };
            Some(encode_preserving_lang(&str_col, &new_lex))
        }

        // ── UCASE / LCASE ────────────────────────────────────────────────────
        Function::UCase => {
            let col = translate_arg_value(args.first()?, bindings, ctx)?;
            let text = decode_lexical_sql(&col);
            Some(encode_preserving_lang(&col, &format!("UPPER({text})")))
        }
        Function::LCase => {
            let col = translate_arg_value(args.first()?, bindings, ctx)?;
            let text = decode_lexical_sql(&col);
            Some(encode_preserving_lang(&col, &format!("LOWER({text})")))
        }

        // ── CONCAT ───────────────────────────────────────────────────────────
        Function::Concat => {
            if args.is_empty() {
                return Some(encode_literal("''".to_owned()));
            }
            let cols: Vec<String> = args
                .iter()
                .filter_map(|a| translate_arg_value(a, bindings, ctx))
                .collect();

            fn string_guard_sql(col: &str) -> String {
                format!(
                    "CASE WHEN ({col}) IS NULL THEN NULL \
                     WHEN ({col}) < 0 THEN NULL \
                     WHEN EXISTS(SELECT 1 FROM _pg_ripple.dictionary d WHERE d.id = ({col}) AND d.kind IN (2, 4)) THEN ({col}) \
                     WHEN EXISTS(SELECT 1 FROM _pg_ripple.dictionary d WHERE d.id = ({col}) AND d.kind = 3 AND d.datatype = 'http://www.w3.org/2001/XMLSchema#string') THEN ({col}) \
                     ELSE NULL END"
                )
            }

            let guarded_cols: Vec<String> = cols.iter().map(|c| string_guard_sql(c)).collect();
            let all_valid = guarded_cols
                .iter()
                .map(|g| format!("({g}) IS NOT NULL"))
                .collect::<Vec<_>>()
                .join(" AND ");

            let parts: Vec<String> = cols.iter().map(|col| decode_lexical_sql(col)).collect();
            if parts.is_empty() {
                return None;
            }
            let concat_expr = parts.join(" || ");

            if cols.len() == 1 {
                let g = string_guard_sql(&cols[0]);
                Some(format!(
                    "CASE WHEN ({g}) IS NULL THEN NULL ELSE {} END",
                    encode_preserving_lang(&cols[0], &concat_expr)
                ))
            } else {
                let first_col = &cols[0];
                let same_lang_check = cols[1..]
                    .iter()
                    .map(|c| format!(
                        "EXISTS(SELECT 1 FROM _pg_ripple.dictionary a \
                                JOIN _pg_ripple.dictionary b ON a.lang = b.lang \
                                WHERE a.id = {first_col} AND a.kind = 4 AND b.id = {c} AND b.kind = 4)"
                    ))
                    .collect::<Vec<_>>()
                    .join(" AND ");
                Some(format!(
                    "CASE WHEN NOT ({all_valid}) THEN NULL \
                       WHEN {first_col} > 0 \
                         AND EXISTS(SELECT 1 FROM _pg_ripple.dictionary d WHERE d.id = {first_col} AND d.kind = 4) \
                         AND {same_lang_check} \
                       THEN (SELECT pg_ripple.encode_lang_literal({concat_expr}, d.lang) \
                             FROM _pg_ripple.dictionary d WHERE d.id = {first_col}) \
                       ELSE pg_ripple.encode_term({concat_expr}, 2::int2) \
                     END"
                ))
            }
        }

        // ── REPLACE ──────────────────────────────────────────────────────────
        Function::Replace => {
            let str_col = translate_arg_value(args.first()?, bindings, ctx)?;
            let str_text = decode_lexical_sql(&str_col);
            let pattern = translate_arg_text(args.get(1)?, bindings, ctx)?;
            let replacement = {
                let repl_arg = args.get(2)?;
                if let Expression::Literal(lit) = repl_arg {
                    let raw = lit.value();
                    let pg_raw = raw.replace("$0", "\\&");
                    let pg_raw = (1..=9usize).fold(pg_raw, |s, n| {
                        s.replace(&format!("${n}"), &format!("\\{n}"))
                    });
                    format!("'{}'", pg_raw.replace('\'', "''"))
                } else {
                    translate_arg_text(repl_arg, bindings, ctx)?
                }
            };
            let flags = args
                .get(3)
                .and_then(|f| {
                    if let Expression::Literal(l) = f {
                        Some(l.value().to_owned())
                    } else {
                        None
                    }
                })
                .unwrap_or_default();
            let new_lex = if flags.is_empty() {
                format!("regexp_replace({str_text}, {pattern}, {replacement}, 'g')")
            } else {
                let pg_flags = format!("'g{flags}'");
                format!("regexp_replace({str_text}, {pattern}, {replacement}, {pg_flags})")
            };
            let result = encode_preserving_lang(&str_col, &new_lex);
            Some(format!(
                "CASE WHEN {str_col} < 0 THEN NULL ELSE {result} END"
            ))
        }

        // ── ENCODE_FOR_URI ───────────────────────────────────────────────────
        Function::EncodeForUri => {
            let col = translate_arg_value(args.first()?, bindings, ctx)?;
            let text = decode_lexical_sql(&col);
            Some(encode_literal(format!(
                "(SELECT string_agg(\
                    CASE WHEN chr ~ '^[A-Za-z0-9\\-_.~]$' THEN chr \
                         ELSE regexp_replace(\
                             upper(encode(convert_to(chr, 'UTF8'), 'hex')), \
                             '(..)', '%\\1', 'g') \
                    END, \
                    '' ORDER BY pos) \
                 FROM regexp_split_to_table({text}, '') WITH ORDINALITY AS t(chr, pos))"
            )))
        }

        // ── STRLANG ──────────────────────────────────────────────────────────
        Function::StrLang => {
            let lang_col = translate_arg_value(args.get(1)?, bindings, ctx)?;
            let lang_text = decode_lexical_sql(&lang_col);
            if let Some(Expression::FunctionCall(Function::Str, str_args)) = args.first() {
                let inner_col = translate_arg_value(str_args.first()?, bindings, ctx)?;
                let str_text = decode_lexical_sql(&inner_col);
                return Some(format!(
                    "pg_ripple.encode_lang_literal({str_text}, {lang_text})"
                ));
            }
            let str_col = translate_arg_value(args.first()?, bindings, ctx)?;
            let str_text = decode_lexical_sql(&str_col);
            Some(format!(
                "CASE \
                   WHEN {str_col} < 0 THEN NULL \
                   WHEN NOT EXISTS(SELECT 1 FROM _pg_ripple.dictionary _dc WHERE _dc.id = {str_col} \
                       AND (_dc.kind = 2 OR (_dc.kind = 3 AND _dc.datatype = \
                           'http://www.w3.org/2001/XMLSchema#string'))) THEN NULL \
                   ELSE pg_ripple.encode_lang_literal({str_text}, {lang_text}) \
                 END"
            ))
        }

        // ── STRDT ────────────────────────────────────────────────────────────
        Function::StrDt => {
            let dt_arg = args.get(1)?;
            let dt_text = match dt_arg {
                Expression::NamedNode(nn) => format!("'{}'", nn.as_str().replace('\'', "''")),
                _ => {
                    let dt_col = translate_arg_value(dt_arg, bindings, ctx)?;
                    decode_lexical_sql(&dt_col)
                }
            };
            if let Some(Expression::FunctionCall(Function::Str, str_args)) = args.first() {
                let inner_col = translate_arg_value(str_args.first()?, bindings, ctx)?;
                let str_text = decode_lexical_sql(&inner_col);
                return Some(format!(
                    "pg_ripple.encode_typed_literal({str_text}, {dt_text})"
                ));
            }
            let str_col = translate_arg_value(args.first()?, bindings, ctx)?;
            let str_text = decode_lexical_sql(&str_col);
            Some(format!(
                "CASE \
                   WHEN {str_col} < 0 THEN NULL \
                   WHEN NOT EXISTS(SELECT 1 FROM _pg_ripple.dictionary _dc WHERE _dc.id = {str_col} \
                       AND (_dc.kind = 2 OR (_dc.kind = 3 AND _dc.datatype = \
                           'http://www.w3.org/2001/XMLSchema#string'))) THEN NULL \
                   ELSE pg_ripple.encode_typed_literal({str_text}, {dt_text}) \
                 END"
            ))
        }

        // ── STRBEFORE ────────────────────────────────────────────────────────
        Function::StrBefore => {
            let str_col = translate_arg_value(args.first()?, bindings, ctx)?;
            let str_text = decode_lexical_sql(&str_col);
            let needle_expr = args.get(1)?;

            let (needle_sql, compat_sql, empty_result_sql) =
                if let Expression::Literal(lit) = needle_expr {
                    let val = lit.value().replace('\'', "''");
                    let needle_sql = format!("'{val}'");
                    if let Some(lang) = lit.language() {
                        let lang_lower = lang.to_lowercase();
                        let compat = format!(
                            "{str_col} > 0 AND EXISTS(\
                             SELECT 1 FROM _pg_ripple.dictionary _dn \
                             WHERE _dn.id = {str_col} AND _dn.kind = 4 \
                             AND LOWER(_dn.lang) = '{lang_lower}')"
                        );
                        let empty_res = format!(
                            "(SELECT pg_ripple.encode_lang_literal('', _dl.lang) \
                              FROM _pg_ripple.dictionary _dl WHERE _dl.id = {str_col})"
                        );
                        (needle_sql, compat, empty_res)
                    } else {
                        let compat = "TRUE".to_string();
                        let empty_res = encode_preserving_lang(&str_col, "''");
                        (needle_sql, compat, empty_res)
                    }
                } else {
                    let t = translate_arg_text(needle_expr, bindings, ctx)?;
                    let compat = "TRUE".to_string();
                    let empty_res = encode_preserving_lang(&str_col, "''");
                    (t, compat, empty_res)
                };

            let found_expr = encode_preserving_lang(
                &str_col,
                &format!("left({str_text}, strpos({str_text}, {needle_sql}) - 1)"),
            );
            Some(format!(
                "CASE \
                   WHEN {str_col} < 0 THEN NULL \
                   WHEN NOT ({compat_sql}) THEN NULL \
                   WHEN {needle_sql} = '' THEN {empty_result_sql} \
                   WHEN strpos({str_text}, {needle_sql}) > 0 THEN {found_expr} \
                   ELSE pg_ripple.encode_term('', 2::int2) \
                 END"
            ))
        }

        // ── STRAFTER ─────────────────────────────────────────────────────────
        Function::StrAfter => {
            let str_col = translate_arg_value(args.first()?, bindings, ctx)?;
            let str_text = decode_lexical_sql(&str_col);
            let needle_expr = args.get(1)?;

            let (needle_sql, compat_sql, empty_result_sql) =
                if let Expression::Literal(lit) = needle_expr {
                    let val = lit.value().replace('\'', "''");
                    let needle_sql = format!("'{val}'");
                    if let Some(lang) = lit.language() {
                        let lang_lower = lang.to_lowercase();
                        let compat = format!(
                            "{str_col} > 0 AND EXISTS(\
                             SELECT 1 FROM _pg_ripple.dictionary _dn \
                             WHERE _dn.id = {str_col} AND _dn.kind = 4 \
                             AND LOWER(_dn.lang) = '{lang_lower}')"
                        );
                        let empty_res = encode_preserving_lang(&str_col, &str_text);
                        (needle_sql, compat, empty_res)
                    } else {
                        let compat = "TRUE".to_string();
                        let empty_res = encode_preserving_lang(&str_col, &str_text);
                        (needle_sql, compat, empty_res)
                    }
                } else {
                    let t = translate_arg_text(needle_expr, bindings, ctx)?;
                    let compat = "TRUE".to_string();
                    let empty_res = encode_preserving_lang(&str_col, &str_text);
                    (t, compat, empty_res)
                };

            let found_expr = encode_preserving_lang(
                &str_col,
                &format!(
                    "right({str_text}, length({str_text}) - strpos({str_text}, {needle_sql}) - length({needle_sql}) + 1)"
                ),
            );
            Some(format!(
                "CASE \
                   WHEN {str_col} < 0 THEN NULL \
                   WHEN NOT ({compat_sql}) THEN NULL \
                   WHEN {needle_sql} = '' THEN {empty_result_sql} \
                   WHEN strpos({str_text}, {needle_sql}) > 0 THEN {found_expr} \
                   ELSE pg_ripple.encode_term('', 2::int2) \
                 END"
            ))
        }

        // ── MD5 / SHA hash functions ──────────────────────────────────────────
        Function::Md5 => {
            let col = translate_arg_value(args.first()?, bindings, ctx)?;
            let text = decode_lexical_sql(&col);
            Some(encode_literal(format!("md5({text})")))
        }
        Function::Sha1 => {
            let col = translate_arg_value(args.first()?, bindings, ctx)?;
            let text = decode_lexical_sql(&col);
            Some(encode_literal(format!(
                "encode(digest(({text})::bytea, 'sha1'), 'hex')"
            )))
        }
        Function::Sha256 => {
            let col = translate_arg_value(args.first()?, bindings, ctx)?;
            let text = decode_lexical_sql(&col);
            Some(encode_literal(format!(
                "encode(sha256(({text})::bytea), 'hex')"
            )))
        }
        Function::Sha384 => {
            let col = translate_arg_value(args.first()?, bindings, ctx)?;
            let text = decode_lexical_sql(&col);
            Some(encode_literal(format!(
                "encode(digest(({text})::bytea, 'sha384'), 'hex')"
            )))
        }
        Function::Sha512 => {
            let col = translate_arg_value(args.first()?, bindings, ctx)?;
            let text = decode_lexical_sql(&col);
            Some(encode_literal(format!(
                "encode(digest(({text})::bytea, 'sha512'), 'hex')"
            )))
        }

        // ── UUID / STRUUID ────────────────────────────────────────────────────
        Function::Uuid => Some("('urn:uuid:' || gen_random_uuid()::text)".to_owned()),
        Function::StrUuid => Some("gen_random_uuid()::text".to_owned()),

        _ => None,
    }
}
