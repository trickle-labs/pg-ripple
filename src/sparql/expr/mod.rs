//! SPARQL 1.1 built-in function translation (v0.21.0).
//!
//! This module implements the full SPARQL 1.1 function surface as defined in
//! <https://www.w3.org/TR/sparql11-query/#SparqlOps>.
//!
//! # Two translation contexts
//!
//! Every function is reachable in two positions:
//!
//! 1. **FILTER boolean context** — the function result is used as a filter
//!    predicate; returns `Option<String>` containing a SQL boolean expression.
//! 2. **Value context** (BIND / SELECT expression) — the result is stored as a
//!    variable binding; returns `Option<String>` containing a SQL expression
//!    that evaluates to a `BIGINT` dictionary ID.  For numeric-returning
//!    functions, the caller must mark the resulting variable as `raw_numeric`.
//!
//! # SQL helper conventions
//!
//! - `decode_text!(col)` → a SQL expression that decodes a dictionary ID `col`
//!   to its raw lexical text value.  Works for both inline-encoded (negative)
//!   and dictionary-resident (positive) IDs.
//! - String-valued functions in value context wrap their result in
//!   `pg_ripple.encode_term(computed_text, kind)` so the output is always a
//!   `BIGINT` dictionary ID that the normal decode pipeline can handle.
//! - Numeric-valued functions (STRLEN, ABS, CEIL, FLOOR, ROUND, RAND, YEAR, …)
//!   return a raw SQL integer or float expression; the caller marks the bound
//!   variable as `raw_numeric`.

// v0.90.0 CQ-02: pre-emptive split sub-modules
pub mod aggregates;
pub(super) mod cast;
pub mod filters;
pub(super) mod functions;

pub(crate) use functions::{is_numeric_function, translate_function_value};

use std::collections::HashMap;

use spargebra::algebra::{Expression, Function};

use super::sqlgen::Ctx;

// ─── SQL helper ───────────────────────────────────────────────────────────────

/// Build a SQL expression that decodes a dictionary column `col` to its raw
/// lexical value (text string, without N-Triples formatting).
///
/// `pg_ripple.decode_id()` handles both inline IDs (bit 63 = 1, returned as
/// negative i64) and dictionary-resident IDs.  For inline values the extension
/// already stores the canonical N-Triples representation; we strip the quotes
/// and datatype annotation to get back the lexical value.
///
/// For simplicity we use a CASE expression: inline IDs (< 0) go through the
/// extension's decode function; positive IDs use a correlated subquery that
/// avoids a function call overhead.
pub(super) fn decode_lexical_sql(col: &str) -> String {
    format!(
        "CASE WHEN {col} < 0 THEN \
              regexp_replace(pg_ripple.decode_id({col}), \
                  '\"(.*?)\"(\\^\\^<[^>]+>|@\\S+)?$', '\\1') \
         ELSE (SELECT d.value FROM _pg_ripple.dictionary d WHERE d.id = {col}) \
         END"
    )
}

/// Build a SQL boolean expression: TRUE when the dictionary entry for `col`
/// has the given `kind` value.  Inline IDs (< 0) are always typed literals.
pub(super) fn kind_check_sql(col: &str, kind: i16) -> String {
    // Inline IDs are never IRI or blank node — they're always typed literals.
    match kind {
        0 /* IRI */ => format!(
            "({col} IS NOT NULL AND {col} > 0 AND \
             EXISTS(SELECT 1 FROM _pg_ripple.dictionary d WHERE d.id = {col} AND d.kind = 0))"
        ),
        1 /* blank */ => format!(
            "({col} IS NOT NULL AND {col} > 0 AND \
             EXISTS(SELECT 1 FROM _pg_ripple.dictionary d WHERE d.id = {col} AND d.kind = 1))"
        ),
        _ => format!(
            "({col} IS NOT NULL AND \
             ({col} < 0 OR EXISTS(SELECT 1 FROM _pg_ripple.dictionary d WHERE d.id = {col} AND d.kind = {kind})))"
        ),
    }
}

// ─── PostGIS availability probe ──────────────────────────────────────────────

/// Build a SQL expression that applies `new_lexical_sql` to the lexical value of
/// `col`, preserving any language tag from the input.
///
/// If `col` is a dictionary-resident lang-tagged literal (kind=4), the result
/// is re-encoded with `pg_ripple.encode_lang_literal(new_lexical_sql, lang)`.
/// Otherwise (plain literal, typed literal, inline) the result is encoded as a
/// plain literal (kind=2) with `pg_ripple.encode_term(new_lexical_sql, 2)`.
pub(super) fn encode_preserving_lang(col: &str, new_lexical_sql: &str) -> String {
    format!(
        "CASE \
          WHEN {col} > 0 AND EXISTS(\
              SELECT 1 FROM _pg_ripple.dictionary d WHERE d.id = {col} AND d.kind = 4) \
          THEN (SELECT pg_ripple.encode_lang_literal({new_lexical_sql}, d.lang) \
                FROM _pg_ripple.dictionary d WHERE d.id = {col}) \
          ELSE pg_ripple.encode_term({new_lexical_sql}, 2::int2) \
        END"
    )
}

/// Returns `true` when PostGIS is installed in the current database.
///
/// Checked by looking for `st_geomfromtext` in `pg_proc`.  The result is
/// evaluated **at SPARQL translation time** so that we can emit plain `false`
/// or `NULL` SQL literals instead of references to PostGIS functions that
/// PostgreSQL would reject at catalog-resolution time.
fn postgis_available() -> bool {
    pgrx::Spi::get_one::<bool>(
        "SELECT EXISTS(SELECT 1 FROM pg_proc WHERE proname = 'st_geomfromtext')",
    )
    .unwrap_or(None)
    .unwrap_or(false)
}

// ─── Function name rendering (for error messages) ────────────────────────────

pub(super) fn function_name(func: &Function) -> &'static str {
    match func {
        Function::Str => "STR",
        Function::Lang => "LANG",
        Function::LangMatches => "LANGMATCHES",
        Function::Datatype => "DATATYPE",
        Function::Iri => "IRI",
        Function::BNode => "BNODE",
        Function::Rand => "RAND",
        Function::Abs => "ABS",
        Function::Ceil => "CEIL",
        Function::Floor => "FLOOR",
        Function::Round => "ROUND",
        Function::Concat => "CONCAT",
        Function::SubStr => "SUBSTR",
        Function::StrLen => "STRLEN",
        Function::Replace => "REPLACE",
        Function::UCase => "UCASE",
        Function::LCase => "LCASE",
        Function::EncodeForUri => "ENCODE_FOR_URI",
        Function::Contains => "CONTAINS",
        Function::StrStarts => "STRSTARTS",
        Function::StrEnds => "STRENDS",
        Function::StrBefore => "STRBEFORE",
        Function::StrAfter => "STRAFTER",
        Function::Year => "YEAR",
        Function::Month => "MONTH",
        Function::Day => "DAY",
        Function::Hours => "HOURS",
        Function::Minutes => "MINUTES",
        Function::Seconds => "SECONDS",
        Function::Timezone => "TIMEZONE",
        Function::Tz => "TZ",
        Function::Now => "NOW",
        Function::Uuid => "UUID",
        Function::StrUuid => "STRUUID",
        Function::Md5 => "MD5",
        Function::Sha1 => "SHA1",
        Function::Sha256 => "SHA256",
        Function::Sha384 => "SHA384",
        Function::Sha512 => "SHA512",
        Function::StrLang => "STRLANG",
        Function::StrDt => "STRDT",
        Function::IsIri => "isIRI",
        Function::IsBlank => "isBLANK",
        Function::IsLiteral => "isLITERAL",
        Function::IsNumeric => "isNUMERIC",
        Function::Regex => "REGEX",
        Function::Custom(_) => "custom function",
        #[allow(unreachable_patterns)]
        _ => "unknown function",
    }
}

// ─── FILTER boolean context ───────────────────────────────────────────────────

/// Translate a `FunctionCall` in a FILTER boolean context.
///
/// Returns a SQL boolean expression string, or `None` when the function is not
/// applicable in boolean context (caller should try value context or raise).
pub(super) fn translate_function_filter(
    func: &Function,
    args: &[Expression],
    bindings: &HashMap<String, String>,
    ctx: &mut Ctx,
) -> Option<String> {
    match func {
        // ── Type-testing predicates ─────────────────────────────────────────
        Function::IsIri => {
            // raw_iri_vars (UUID results) are always IRIs — shortcut without dict lookup.
            if let Some(Expression::Variable(v)) = args.first()
                && ctx.is_raw_iri_var(v.as_str())
            {
                let col = bindings.get(v.as_str())?;
                return Some(format!("({col} IS NOT NULL)"));
            }
            let col = translate_arg_value(args.first()?, bindings, ctx)?;
            Some(kind_check_sql(&col, 0))
        }
        Function::IsBlank => {
            let col = translate_arg_value(args.first()?, bindings, ctx)?;
            Some(kind_check_sql(&col, 1))
        }
        Function::IsLiteral => {
            // raw_text_vars (STRUUID, GROUP_CONCAT) are always literals — shortcut.
            // raw_iri_vars (UUID) are NOT literals; skip them.
            if let Some(Expression::Variable(v)) = args.first()
                && ctx.is_raw_text_var(v.as_str())
                && !ctx.is_raw_iri_var(v.as_str())
            {
                let col = bindings.get(v.as_str())?;
                return Some(format!("({col} IS NOT NULL)"));
            }
            let col = translate_arg_value(args.first()?, bindings, ctx)?;
            // Inline IDs (< 0) are always literals.
            // Kind 2 = plain literal, 3 = typed literal, 4 = lang literal.
            Some(format!(
                "({col} IS NOT NULL AND \
                 ({col} < 0 OR EXISTS(SELECT 1 FROM _pg_ripple.dictionary d \
                   WHERE d.id = {col} AND d.kind IN (2,3,4))))"
            ))
        }
        Function::IsNumeric => {
            let col = translate_arg_value(args.first()?, bindings, ctx)?;
            // Inline IDs: bit 63=1 (negative as i64), bits 62-56 = type code.
            // TYPE_INTEGER=0, TYPE_BOOLEAN=1, TYPE_DATETIME=2, TYPE_DATE=3.
            // Only TYPE_INTEGER is numeric; check bits 62-56 are all zero.
            // Mask 0x7F00000000000000 = 9151314442816847872 selects bits 62-56.
            Some(format!(
                "({col} IS NOT NULL AND \
                 (({col} < 0 AND ({col} & 9151314442816847872::bigint) = 0) OR EXISTS(SELECT 1 FROM _pg_ripple.dictionary d \
                   WHERE d.id = {col} AND d.kind = 3 \
                   AND d.datatype IN (\
                     'http://www.w3.org/2001/XMLSchema#integer',\
                     'http://www.w3.org/2001/XMLSchema#long',\
                     'http://www.w3.org/2001/XMLSchema#int',\
                     'http://www.w3.org/2001/XMLSchema#short',\
                     'http://www.w3.org/2001/XMLSchema#byte',\
                     'http://www.w3.org/2001/XMLSchema#decimal',\
                     'http://www.w3.org/2001/XMLSchema#float',\
                     'http://www.w3.org/2001/XMLSchema#double'\
                   ))))"
            ))
        }
        // Note: Function::SameTerm does not exist in spargebra — sameTerm is
        // Expression::SameTerm and handled directly in translate_expr.

        // ── LANGMATCHES ─────────────────────────────────────────────────────
        Function::LangMatches => {
            // LANGMATCHES(?lang, "range"): case-insensitive prefix match.
            // ?lang should be the result of LANG(?x), i.e. a plain-literal ID.
            let lang_col = translate_arg_value(args.first()?, bindings, ctx)?;
            let range_col = translate_arg_value(args.get(1)?, bindings, ctx)?;
            let lang_text = decode_lexical_sql(&lang_col);
            let range_text = decode_lexical_sql(&range_col);
            // SPARQL LANGMATCHES("en", "*") is TRUE for any language.
            // LANGMATCHES("en-GB", "en") is TRUE (prefix).
            Some(format!(
                "(({range_text}) = '*' \
                 OR LOWER({lang_text}) = LOWER({range_text}) \
                 OR LOWER({lang_text}) LIKE (LOWER({range_text}) || '-%'))"
            ))
        }

        // ── String filter functions ─────────────────────────────────────────
        Function::Contains => {
            let hay = translate_arg_text(args.first()?, bindings, ctx)?;
            let needle = translate_arg_text(args.get(1)?, bindings, ctx)?;
            Some(format!("(strpos({hay}, {needle}) > 0)"))
        }
        Function::StrStarts => {
            let s = translate_arg_text(args.first()?, bindings, ctx)?;
            let prefix = translate_arg_text(args.get(1)?, bindings, ctx)?;
            Some(format!("(starts_with({s}, {prefix}))"))
        }
        Function::StrEnds => {
            let s = translate_arg_text(args.first()?, bindings, ctx)?;
            let suffix = translate_arg_text(args.get(1)?, bindings, ctx)?;
            Some(format!("(right({s}, length({suffix})) = {suffix})"))
        }
        Function::StrBefore => {
            // STRBEFORE returns "" if not found — in boolean context treat as
            // IS NOT NULL after comparison; not really a boolean function.
            None
        }
        Function::StrAfter => None,

        Function::Regex => {
            let s = translate_arg_text(args.first()?, bindings, ctx)?;
            let pattern = translate_arg_text(args.get(1)?, bindings, ctx)?;
            let case_insensitive = args
                .get(2)
                .is_some_and(|f| matches!(f, Expression::Literal(fl) if fl.value().contains('i')));
            if case_insensitive {
                Some(format!("({s} ~* {pattern})"))
            } else {
                Some(format!("({s} ~ {pattern})"))
            }
        }

        // ── IF in boolean context ───────────────────────────────────────────
        // Note: IF is Expression::If in spargebra, not Function::If.
        // This arm is unreachable in practice, but kept as a safety fallback.

        // ── GeoSPARQL topological predicates ───────────────────────────────
        // geo:sf* functions are represented as Function::Custom in spargebra.
        // They require PostGIS (ST_GeomFromText). PostGIS availability is
        // probed at translation time so no PostGIS function reference ever
        // appears in the generated SQL when PostGIS is not installed.
        Function::Custom(name) => {
            let iri = name.as_str();
            // Map GeoSPARQL Simple Features topology predicates to PostGIS.
            // Also supports geof:within and geof:intersects (v0.56.0 L-1.1).
            let postgis_fn = match iri {
                "http://www.opengis.net/def/function/geosparql/sfIntersects" => {
                    Some("ST_Intersects")
                }
                "http://www.opengis.net/def/function/geosparql/sfContains" => Some("ST_Contains"),
                "http://www.opengis.net/def/function/geosparql/sfWithin" => Some("ST_Within"),
                "http://www.opengis.net/def/function/geosparql/sfOverlaps" => Some("ST_Overlaps"),
                "http://www.opengis.net/def/function/geosparql/sfTouches" => Some("ST_Touches"),
                "http://www.opengis.net/def/function/geosparql/sfCrosses" => Some("ST_Crosses"),
                "http://www.opengis.net/def/function/geosparql/sfDisjoint" => Some("ST_Disjoint"),
                "http://www.opengis.net/def/function/geosparql/sfEquals" => Some("ST_Equals"),
                "http://www.opengis.net/def/function/geosparql/ehIntersects" => {
                    Some("ST_Intersects")
                }
                "http://www.opengis.net/def/function/geosparql/ehContains" => Some("ST_Contains"),
                "http://www.opengis.net/def/function/geosparql/ehCoveredBy" => Some("ST_CoveredBy"),
                "http://www.opengis.net/def/function/geosparql/ehCovers" => Some("ST_Covers"),
                // v0.56.0 L-1.1: geof:within and geof:intersects as boolean predicates.
                "http://www.opengis.net/def/function/geosparql/within" => Some("ST_Within"),
                "http://www.opengis.net/def/function/geosparql/intersects" => Some("ST_Intersects"),
                _ => None,
            };
            if let Some(pg_fn) = postgis_fn {
                // When PostGIS is absent, emit literal false — no PostGIS
                // function references are included so PostgreSQL catalog
                // resolution succeeds even without the PostGIS extension.
                if !postgis_available() {
                    return Some("false".to_string());
                }
                let a_col = translate_arg_value(args.first()?, bindings, ctx)?;
                let b_col = translate_arg_value(args.get(1)?, bindings, ctx)?;
                let a_wkt = decode_lexical_sql(&a_col);
                let b_wkt = decode_lexical_sql(&b_col);
                Some(format!(
                    "{pg_fn}(\
                        ST_GeomFromText({a_wkt}), \
                        ST_GeomFromText({b_wkt})\
                      )"
                ))
            } else {
                None
            }
        }

        // In filter context, remaining functions are handled by converting to
        // value and comparing non-null. Return None; caller will use value context.
        _ => None,
    }
}

// ─── pg:similar() — pgvector cosine similarity (v0.27.0) ─────────────────────

/// `pg:similar` IRI constant.
pub(super) const PG_SIMILAR_IRI: &str = "http://pg-ripple.org/functions/similar";

// ─── v0.87.0 uncertain-knowledge extension function IRI constants ─────────────

/// `pg:confidence(?s, ?p, ?o)` IRI — returns highest confidence score across models (v0.87.0).
pub(super) const PG_CONFIDENCE_IRI: &str = "http://pg-ripple.org/functions/confidence";

/// `pg:fuzzy_match(a, b)` IRI — trigram similarity via pg_trgm (v0.87.0).
pub(super) const PG_FUZZY_MATCH_IRI: &str = "http://pg-ripple.org/functions/fuzzy_match";

/// `pg:token_set_ratio(a, b)` IRI — word-set similarity via pg_trgm (v0.87.0).
pub(super) const PG_TOKEN_SET_RATIO_IRI: &str = "http://pg-ripple.org/functions/token_set_ratio";

// ─── v0.106.0 temporal extension function ─────────────────────────────────────

/// `pg:temporal_window(?subject, ?predicate, ?start, ?end)` IRI (v0.106.0).
///
/// SPARQL FILTER function that returns `true` when a temporal fact for
/// `(?subject, ?predicate, *)` exists with a validity interval overlapping
/// `[?start, ?end]`.
pub(super) const PG_TEMPORAL_WINDOW_IRI: &str = "http://pg-ripple.org/functions/temporal_window";

/// Translate an argument expression to a SQL text expression.
///
/// For variable arguments: decode the dictionary ID to lexical text.
/// For literal arguments: return the raw SQL string literal.
/// For function calls: try to get a value and decode it.
fn translate_arg_text(
    expr: &Expression,
    bindings: &HashMap<String, String>,
    ctx: &mut Ctx,
) -> Option<String> {
    match expr {
        Expression::Variable(v) => {
            let col = bindings.get(v.as_str())?;
            // raw_text_vars (STRUUID, GROUP_CONCAT) and raw_iri_vars (UUID) hold plain
            // text/IRI strings — return directly without dict-decode roundtrip.
            if ctx.is_raw_text_var(v.as_str()) || ctx.is_raw_iri_var(v.as_str()) {
                return Some(col.clone());
            }
            Some(decode_lexical_sql(col))
        }
        Expression::Literal(lit) => {
            let val = lit.value().replace('\'', "''");
            Some(format!("'{val}'"))
        }
        // STR(?x) shortcut: avoid encode_term → decode roundtrip.
        // STR(?x) in text context = lexical form of ?x = decode_lexical_sql(x_col).
        // This also avoids the PostgreSQL snapshot isolation issue where encode_term
        // inserts a new dict row that the subsequent SELECT can't see in the same stmt.
        // Special case: STR(raw_iri_var) → the IRI text itself (no dict lookup needed).
        Expression::FunctionCall(Function::Str, str_args) => {
            if let Some(Expression::Variable(v)) = str_args.first()
                && ctx.is_raw_iri_var(v.as_str())
            {
                let col = bindings.get(v.as_str())?;
                return Some(col.clone());
            }
            let inner_col = translate_arg_value(str_args.first()?, bindings, ctx)?;
            Some(decode_lexical_sql(&inner_col))
        }
        Expression::FunctionCall(func, args) => {
            let mut is_numeric = false;
            let val_sql = translate_function_value(func, args, bindings, ctx, &mut is_numeric)?;
            // The function returned a dict ID — decode it to text.
            Some(decode_lexical_sql(&val_sql))
        }
        _ => None,
    }
}

/// Translate an argument as a SQL value expression (bigint dictionary ID or raw value).
///
/// Handles the common cases: variable reference, named node, literal.
/// Complex nested expressions (function calls inside function calls) return None;
/// the caller will fall back gracefully.
pub(super) fn translate_arg_value(
    expr: &Expression,
    bindings: &HashMap<String, String>,
    ctx: &mut Ctx,
) -> Option<String> {
    match expr {
        Expression::Variable(v) => Some(bindings.get(v.as_str())?.clone()),
        Expression::NamedNode(nn) => {
            let id = ctx.encode_iri(nn.as_str())?;
            Some(id.to_string())
        }
        Expression::Literal(lit) => {
            let id = ctx.encode_literal(lit);
            Some(id.to_string())
        }
        // Nested function calls: attempt value translation through the function dispatch.
        Expression::FunctionCall(func, args) => {
            let mut is_numeric = false;
            translate_function_value(func, args, bindings, ctx, &mut is_numeric)
        }
        _ => None,
    }
}

/// Translate an argument as a SQL boolean expression (for IF condition).
///
/// Handles the common cases: boolean literals, variable IS NOT NULL check,
/// and comparison expressions.
// Q15-01: internal API field; kept for public API surface or future extension consumers.
#[allow(dead_code)]
pub(super) fn translate_arg_filter(
    expr: &Expression,
    bindings: &HashMap<String, String>,
    ctx: &mut Ctx,
) -> Option<String> {
    match expr {
        Expression::Variable(v) => {
            let col = bindings.get(v.as_str())?;
            Some(format!("({col} IS NOT NULL)"))
        }
        Expression::Literal(lit) => {
            // Boolean literals: "true"^^xsd:boolean or "false"^^xsd:boolean.
            let dt = lit.datatype().as_str();
            if dt == "http://www.w3.org/2001/XMLSchema#boolean" {
                if lit.value() == "true" {
                    return Some("TRUE".to_owned());
                } else {
                    return Some("FALSE".to_owned());
                }
            }
            None
        }
        Expression::Equal(a, b) => {
            let la = translate_arg_value(a, bindings, ctx)?;
            let ra = translate_arg_value(b, bindings, ctx)?;
            Some(format!("({la} = {ra})"))
        }
        Expression::Greater(a, b) => {
            let la = translate_arg_value(a, bindings, ctx)?;
            let ra = translate_arg_value(b, bindings, ctx)?;
            Some(format!("({la} > {ra})"))
        }
        Expression::GreaterOrEqual(a, b) => {
            let la = translate_arg_value(a, bindings, ctx)?;
            let ra = translate_arg_value(b, bindings, ctx)?;
            Some(format!("({la} >= {ra})"))
        }
        Expression::Less(a, b) => {
            let la = translate_arg_value(a, bindings, ctx)?;
            let ra = translate_arg_value(b, bindings, ctx)?;
            Some(format!("({la} < {ra})"))
        }
        Expression::LessOrEqual(a, b) => {
            let la = translate_arg_value(a, bindings, ctx)?;
            let ra = translate_arg_value(b, bindings, ctx)?;
            Some(format!("({la} <= {ra})"))
        }
        Expression::And(a, b) => {
            let la = translate_arg_filter(a, bindings, ctx)?;
            let ra = translate_arg_filter(b, bindings, ctx)?;
            Some(format!("({la} AND {ra})"))
        }
        Expression::Or(a, b) => {
            let la = translate_arg_filter(a, bindings, ctx)?;
            let ra = translate_arg_filter(b, bindings, ctx)?;
            Some(format!("({la} OR {ra})"))
        }
        Expression::Not(inner) => {
            let c = translate_arg_filter(inner, bindings, ctx)?;
            Some(format!("(NOT {c})"))
        }
        Expression::FunctionCall(func, args) => {
            translate_function_filter(func, args, bindings, ctx)
        }
        _ => None,
    }
}
