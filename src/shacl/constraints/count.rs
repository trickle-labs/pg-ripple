//! sh:minCount, sh:maxCount, and sh:validFor constraint checkers.

use super::{ConstraintArgs, Violation};

/// Check `sh:minCount n` — focus must have at least `n` values along the path.
pub(crate) fn check_min_count(n: i64, args: &ConstraintArgs, violations: &mut Vec<Violation>) {
    if args.count < n {
        let focus_iri =
            crate::dictionary::decode(args.focus).unwrap_or_else(|| format!("_id_{}", args.focus));
        violations.push(Violation {
            focus_node: focus_iri,
            shape_iri: args.shape_iri.to_owned(),
            path: Some(args.path_iri.to_owned()),
            constraint: "sh:minCount".to_owned(),
            message: format!(
                "expected at least {n} value(s) for <{}>, found {}",
                args.path_iri, args.count
            ),
            severity: "Violation".to_owned(),
            sh_value: None,
            sh_source_constraint_component: None,
        });
    }
}

/// Check `sh:maxCount n` — focus must have at most `n` values along the path.
pub(crate) fn check_max_count(n: i64, args: &ConstraintArgs, violations: &mut Vec<Violation>) {
    if args.count > n {
        let focus_iri =
            crate::dictionary::decode(args.focus).unwrap_or_else(|| format!("_id_{}", args.focus));
        violations.push(Violation {
            focus_node: focus_iri,
            shape_iri: args.shape_iri.to_owned(),
            path: Some(args.path_iri.to_owned()),
            constraint: "sh:maxCount".to_owned(),
            message: format!(
                "expected at most {n} value(s) for <{}>, found {}",
                args.path_iri, args.count
            ),
            severity: "Violation".to_owned(),
            sh_value: None,
            sh_source_constraint_component: None,
        });
    }
}

/// Check `sh:validFor "P1Y"^^xsd:duration` (v0.106.0) — no temporal fact for
/// the constrained predicate may have a `valid_to - valid_from` interval
/// exceeding the XSD duration string.
///
/// Queries `_pg_ripple.temporal_facts` for rows where `path_id` is the predicate
/// and the focus node is the subject.  Any fact whose closed interval
/// `(valid_to - valid_from)` exceeds the duration produces a violation.
///
/// Facts with `valid_to IS NULL` (open-ended) are not checked — they have no
/// known duration and are considered compliant until closed.
pub(crate) fn check_valid_for(
    duration_str: &str,
    args: &ConstraintArgs,
    violations: &mut Vec<Violation>,
) {
    // Query temporal_facts for closed intervals exceeding the duration.
    let count: i64 = pgrx::Spi::get_one_with_args::<i64>(
        "SELECT COUNT(*)::bigint \
         FROM _pg_ripple.temporal_facts \
         WHERE s = $1 AND p = $2 \
           AND valid_to IS NOT NULL \
           AND (valid_to - valid_from) > $3::interval",
        &[
            pgrx::datum::DatumWithOid::from(args.focus),
            pgrx::datum::DatumWithOid::from(args.path_id),
            pgrx::datum::DatumWithOid::from(duration_str),
        ],
    )
    .unwrap_or(None)
    .unwrap_or(0);

    if count > 0 {
        let focus_iri =
            crate::dictionary::decode(args.focus).unwrap_or_else(|| format!("_id_{}", args.focus));
        violations.push(Violation {
            focus_node: focus_iri,
            shape_iri: args.shape_iri.to_owned(),
            path: Some(args.path_iri.to_owned()),
            constraint: "sh:validFor".to_owned(),
            message: format!(
                "{count} temporal fact(s) for <{}> exceed the allowed duration of {duration_str}",
                args.path_iri
            ),
            severity: "Violation".to_owned(),
            sh_value: None,
            sh_source_constraint_component: None,
        });
    }
}
