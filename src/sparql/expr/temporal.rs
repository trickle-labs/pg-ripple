//! Temporal, similarity, and Allen's-interval custom function translation (H17-02, v0.122.0).
//!
//! Handles: `pg:temporal_window`, all seven Allen's interval relations (`pg:before`,
//! `pg:meets`, `pg:overlaps`, `pg:during`, `pg:finishes`, `pg:starts`, `pg:equals`),
//! `pg:similar`, `pg:confidence`, `pg:fuzzy_match`, `pg:token_set_ratio`,
//! `pg:trigram_similarity`, `pg:levenshtein`, `pg:levenshtein_less_equal`,
//! `pg:soundex`, `pg:metaphone`, `pg:jaro_winkler`, `pg:dice_similarity`.

use std::collections::HashMap;

use spargebra::algebra::Expression;

use super::super::sqlgen::Ctx;
use super::{
    PG_ALLEN_BEFORE_IRI, PG_ALLEN_DURING_IRI, PG_ALLEN_EQUALS_IRI, PG_ALLEN_FINISHES_IRI,
    PG_ALLEN_MEETS_IRI, PG_ALLEN_OVERLAPS_IRI, PG_ALLEN_STARTS_IRI, PG_CONFIDENCE_IRI,
    PG_DICE_SIMILARITY_IRI, PG_FUZZY_MATCH_IRI, PG_JARO_WINKLER_IRI, PG_LEVENSHTEIN_IRI,
    PG_LEVENSHTEIN_LESS_EQUAL_IRI, PG_METAPHONE_IRI, PG_SIMILAR_IRI, PG_SOUNDEX_IRI,
    PG_TEMPORAL_WINDOW_IRI, PG_TOKEN_SET_RATIO_IRI, PG_TRIGRAM_SIMILARITY_IRI, translate_arg_text,
    translate_arg_value,
};

/// Returns `true` when the `fuzzystrmatch` extension is installed.
///
/// Checked by looking for `levenshtein` in `pg_proc`.  The result is
/// evaluated at SPARQL translation time so that we can emit `NULL` literals
/// instead of references to fuzzystrmatch functions that PostgreSQL would
/// reject when the extension is absent.
pub(super) fn fuzzystrmatch_available() -> bool {
    pgrx::Spi::get_one::<bool>("SELECT EXISTS(SELECT 1 FROM pg_proc WHERE proname = 'levenshtein')")
        .unwrap_or(None)
        .unwrap_or(false)
}

/// Translate a temporal / similarity / Allen's-interval custom function IRI in value context.
///
/// Returns `Some(sql)` for recognised IRIs and `None` for all others.
/// Sets `is_numeric = true` for functions that return raw numeric values.
pub(super) fn translate_custom(
    iri: &str,
    args: &[Expression],
    bindings: &HashMap<String, String>,
    ctx: &mut Ctx,
    is_numeric: &mut bool,
) -> Option<String> {
    match iri {
        // ── pg:similar(?entity, "text", k) — pgvector cosine distance ──
        // Returns cosine distance as xsd:double (0 = identical, 2 = opposite).
        // When pgvector is absent or disabled, emits NULL::float8.
        PG_SIMILAR_IRI => {
            *is_numeric = true;
            let entity_col = translate_arg_value(args.first()?, bindings, ctx)?;
            let query_text = args
                .get(1)
                .and_then(|e| {
                    if let Expression::Literal(lit) = e {
                        Some(lit.value().to_owned())
                    } else {
                        None
                    }
                })
                .unwrap_or_default();
            let k = args
                .get(2)
                .and_then(|e| {
                    if let Expression::Literal(lit) = e {
                        lit.value().parse::<i64>().ok()
                    } else {
                        None
                    }
                })
                .unwrap_or(10);
            Some(crate::sparql::embedding::sql_for_pg_similar(
                &entity_col,
                &query_text,
                k,
            ))
        }

        // ── v0.87.0: pg:confidence(?s, ?p, ?o) ────────────────────────────
        // Returns the highest confidence score across all models for a triple,
        // or 1.0 if no confidence row exists (explicit facts are always confident).
        PG_CONFIDENCE_IRI => {
            *is_numeric = true;
            let s_sql = args
                .first()
                .and_then(|a| translate_arg_value(a, bindings, ctx));
            let p_sql = args
                .get(1)
                .and_then(|a| translate_arg_value(a, bindings, ctx));
            let o_sql = args
                .get(2)
                .and_then(|a| translate_arg_value(a, bindings, ctx));

            match (s_sql, p_sql, o_sql) {
                (None, None, None) => {
                    pgrx::error!(
                        "pg:confidence() requires at least one bound argument \
                         to prevent a full confidence table scan (PT0304)"
                    );
                }
                (s_opt, p_opt, o_opt) => {
                    let p_cond = match &p_opt {
                        Some(p) => {
                            format!(
                                "COALESCE((\
                                   SELECT MAX(c.confidence) \
                                   FROM _pg_ripple.vp_rare vp \
                                   JOIN _pg_ripple.confidence c \
                                     ON c.statement_id = vp.i \
                                   WHERE vp.p = ({p}) \
                                   {s_filter} {o_filter} \
                                   LIMIT 1\
                                 ), 1.0)",
                                s_filter = s_opt
                                    .as_ref()
                                    .map(|s| format!("AND vp.s = ({s})"))
                                    .unwrap_or_default(),
                                o_filter = o_opt
                                    .as_ref()
                                    .map(|o| format!("AND vp.o = ({o})"))
                                    .unwrap_or_default(),
                            )
                        }
                        None => {
                            format!(
                                "COALESCE((\
                                   SELECT MAX(c.confidence) \
                                   FROM _pg_ripple.vp_rare vp \
                                   JOIN _pg_ripple.confidence c \
                                     ON c.statement_id = vp.i \
                                   WHERE 1=1 \
                                   {s_filter} {o_filter} \
                                   LIMIT 1\
                                 ), 1.0)",
                                s_filter = s_opt
                                    .as_ref()
                                    .map(|s| format!("AND vp.s = ({s})"))
                                    .unwrap_or_default(),
                                o_filter = o_opt
                                    .as_ref()
                                    .map(|o| format!("AND vp.o = ({o})"))
                                    .unwrap_or_default(),
                            )
                        }
                    };
                    Some(p_cond)
                }
            }
        }

        // ── v0.87.0: pg:fuzzy_match(a, b) — trigram similarity ─────────
        PG_FUZZY_MATCH_IRI => {
            *is_numeric = true;
            let a_text = translate_arg_text(args.first()?, bindings, ctx)?;
            let b_text = translate_arg_text(args.get(1)?, bindings, ctx)?;
            Some(format!("pg_ripple._fuzzy_match_guard({a_text}, {b_text})"))
        }

        // ── v0.87.0: pg:token_set_ratio(a, b) — word-set similarity ────
        PG_TOKEN_SET_RATIO_IRI => {
            *is_numeric = true;
            let a_text = translate_arg_text(args.first()?, bindings, ctx)?;
            let b_text = translate_arg_text(args.get(1)?, bindings, ctx)?;
            Some(format!(
                "pg_ripple._token_set_ratio_guard({a_text}, {b_text})"
            ))
        }

        // ── v0.106.0: pg:temporal_window(?subject, ?predicate, ?start, ?end) ──
        PG_TEMPORAL_WINDOW_IRI => {
            let s_sql = translate_arg_value(args.first()?, bindings, ctx)?;
            let p_sql = translate_arg_value(args.get(1)?, bindings, ctx)?;
            let start_sql = translate_arg_text(args.get(2)?, bindings, ctx)?;
            let end_sql = translate_arg_text(args.get(3)?, bindings, ctx)?;
            Some(format!(
                "EXISTS( \
                   SELECT 1 FROM _pg_ripple.temporal_facts tf \
                   WHERE tf.s = ({s_sql}) \
                     AND tf.p = ({p_sql}) \
                     AND tstzrange(tf.valid_from, tf.valid_to, '[)') \
                       && tstzrange(({start_sql})::timestamptz, \
                                    ({end_sql})::timestamptz, '[)') \
                 )"
            ))
        }

        // ── v0.109.0: pg:trigram_similarity(a, b) — alias for pg:fuzzy_match ──
        PG_TRIGRAM_SIMILARITY_IRI => {
            *is_numeric = true;
            let a_text = translate_arg_text(args.first()?, bindings, ctx)?;
            let b_text = translate_arg_text(args.get(1)?, bindings, ctx)?;
            Some(format!("pg_ripple._fuzzy_match_guard({a_text}, {b_text})"))
        }

        // ── v0.109.0: pg:levenshtein(a, b) — edit distance ────────────
        PG_LEVENSHTEIN_IRI => {
            *is_numeric = true;
            if !fuzzystrmatch_available() {
                return Some("NULL::integer".to_string());
            }
            let a_text = translate_arg_text(args.first()?, bindings, ctx)?;
            let b_text = translate_arg_text(args.get(1)?, bindings, ctx)?;
            Some(format!("levenshtein({a_text}, {b_text})"))
        }

        // ── v0.109.0: pg:levenshtein_less_equal(a, b, max) ──────────────
        PG_LEVENSHTEIN_LESS_EQUAL_IRI => {
            *is_numeric = true;
            if !fuzzystrmatch_available() {
                return Some("NULL::integer".to_string());
            }
            let a_text = translate_arg_text(args.first()?, bindings, ctx)?;
            let b_text = translate_arg_text(args.get(1)?, bindings, ctx)?;
            let max_sql = args.get(2).and_then(|e| {
                if let Expression::Literal(lit) = e {
                    lit.value().parse::<i64>().ok().map(|n| n.to_string())
                } else {
                    translate_arg_text(e, bindings, ctx)
                }
            })?;
            Some(format!(
                "levenshtein_less_equal({a_text}, {b_text}, ({max_sql})::integer)"
            ))
        }

        // ── v0.109.0: pg:soundex(s) — phonetic code ───────────────────
        PG_SOUNDEX_IRI => {
            if !fuzzystrmatch_available() {
                return Some("NULL::bigint".to_string());
            }
            let s_text = translate_arg_text(args.first()?, bindings, ctx)?;
            Some(format!("pg_ripple.encode_term(soundex({s_text}), 2::int2)"))
        }

        // ── v0.109.0: pg:metaphone(s, maxlen) — phonetic code ─────────
        PG_METAPHONE_IRI => {
            if !fuzzystrmatch_available() {
                return Some("NULL::bigint".to_string());
            }
            let s_text = translate_arg_text(args.first()?, bindings, ctx)?;
            let maxlen_sql = args.get(1).and_then(|e| {
                if let Expression::Literal(lit) = e {
                    lit.value().parse::<i64>().ok().map(|n| n.to_string())
                } else {
                    translate_arg_text(e, bindings, ctx)
                }
            })?;
            Some(format!(
                "pg_ripple.encode_term(metaphone({s_text}, ({maxlen_sql})::integer), 2::int2)"
            ))
        }

        // ── v0.109.0: pg:jaro_winkler(a, b) — Jaro-Winkler distance ──
        PG_JARO_WINKLER_IRI => {
            *is_numeric = true;
            if !fuzzystrmatch_available() {
                return Some("NULL::float8".to_string());
            }
            let a_text = translate_arg_text(args.first()?, bindings, ctx)?;
            let b_text = translate_arg_text(args.get(1)?, bindings, ctx)?;
            Some(format!("jarowinkler({a_text}, {b_text})"))
        }

        // ── v0.111.0: pg:dice_similarity(a, b) — Bloom-filter Dice coefficient ──
        PG_DICE_SIMILARITY_IRI => {
            *is_numeric = true;
            let a_text = translate_arg_text(args.first()?, bindings, ctx)?;
            let b_text = translate_arg_text(args.get(1)?, bindings, ctx)?;
            Some(format!("pg_ripple.dice_similarity({a_text}, {b_text})"))
        }

        // ── v0.118.0: Allen's interval relation SPARQL FILTER functions ──
        PG_ALLEN_BEFORE_IRI => {
            // A is before B: a_end < b_start
            let a_start = translate_arg_text(args.first()?, bindings, ctx)?;
            let a_end = translate_arg_text(args.get(1)?, bindings, ctx)?;
            let b_start = translate_arg_text(args.get(2)?, bindings, ctx)?;
            let _b_end = translate_arg_text(args.get(3)?, bindings, ctx)?;
            Some(format!(
                "({a_start}::timestamptz < {b_start}::timestamptz \
                 AND {a_end}::timestamptz <= {b_start}::timestamptz)"
            ))
        }

        PG_ALLEN_MEETS_IRI => {
            // A meets B: a_end = b_start
            let _a_start = translate_arg_text(args.first()?, bindings, ctx)?;
            let a_end = translate_arg_text(args.get(1)?, bindings, ctx)?;
            let b_start = translate_arg_text(args.get(2)?, bindings, ctx)?;
            let _b_end = translate_arg_text(args.get(3)?, bindings, ctx)?;
            Some(format!("{a_end}::timestamptz = {b_start}::timestamptz"))
        }

        PG_ALLEN_OVERLAPS_IRI => {
            // A overlaps B: a_start < b_start AND a_end > b_start AND a_end < b_end
            let a_start = translate_arg_text(args.first()?, bindings, ctx)?;
            let a_end = translate_arg_text(args.get(1)?, bindings, ctx)?;
            let b_start = translate_arg_text(args.get(2)?, bindings, ctx)?;
            let b_end = translate_arg_text(args.get(3)?, bindings, ctx)?;
            Some(format!(
                "({a_start}::timestamptz < {b_start}::timestamptz \
                 AND {a_end}::timestamptz > {b_start}::timestamptz \
                 AND {a_end}::timestamptz < {b_end}::timestamptz)"
            ))
        }

        PG_ALLEN_DURING_IRI => {
            // A is during B: a_start > b_start AND a_end < b_end
            let a_start = translate_arg_text(args.first()?, bindings, ctx)?;
            let a_end = translate_arg_text(args.get(1)?, bindings, ctx)?;
            let b_start = translate_arg_text(args.get(2)?, bindings, ctx)?;
            let b_end = translate_arg_text(args.get(3)?, bindings, ctx)?;
            Some(format!(
                "({a_start}::timestamptz > {b_start}::timestamptz \
                 AND {a_end}::timestamptz < {b_end}::timestamptz)"
            ))
        }

        PG_ALLEN_FINISHES_IRI => {
            // A finishes B: a_start > b_start AND a_end = b_end
            let a_start = translate_arg_text(args.first()?, bindings, ctx)?;
            let a_end = translate_arg_text(args.get(1)?, bindings, ctx)?;
            let b_start = translate_arg_text(args.get(2)?, bindings, ctx)?;
            let b_end = translate_arg_text(args.get(3)?, bindings, ctx)?;
            Some(format!(
                "({a_start}::timestamptz > {b_start}::timestamptz \
                 AND {a_end}::timestamptz = {b_end}::timestamptz)"
            ))
        }

        PG_ALLEN_STARTS_IRI => {
            // A starts B: a_start = b_start AND a_end < b_end
            let a_start = translate_arg_text(args.first()?, bindings, ctx)?;
            let a_end = translate_arg_text(args.get(1)?, bindings, ctx)?;
            let b_start = translate_arg_text(args.get(2)?, bindings, ctx)?;
            let b_end = translate_arg_text(args.get(3)?, bindings, ctx)?;
            Some(format!(
                "({a_start}::timestamptz = {b_start}::timestamptz \
                 AND {a_end}::timestamptz < {b_end}::timestamptz)"
            ))
        }

        PG_ALLEN_EQUALS_IRI => {
            // A equals B: a_start = b_start AND a_end = b_end
            let a_start = translate_arg_text(args.first()?, bindings, ctx)?;
            let a_end = translate_arg_text(args.get(1)?, bindings, ctx)?;
            let b_start = translate_arg_text(args.get(2)?, bindings, ctx)?;
            let b_end = translate_arg_text(args.get(3)?, bindings, ctx)?;
            Some(format!(
                "({a_start}::timestamptz = {b_start}::timestamptz \
                 AND {a_end}::timestamptz = {b_end}::timestamptz)"
            ))
        }

        _ => None,
    }
}
