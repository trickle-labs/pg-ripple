//! Property-based tests for guided rule authoring (v0.105.0).
//!
//! Tests the rule validation logic using a pure-Rust reference implementation —
//! no database connection required.
//!
//! # Properties tested
//!
//! 1. **Valid rules pass validation**: a rule where all head variables appear
//!    in the positive body always has no UNBOUND_HEAD_VARIABLE errors.
//!
//! 2. **Unbound head variables are always detected**: deliberately introducing
//!    an unbound head variable must always produce a validation error.
//!
//! 3. **Syntax errors are always detected**: junk strings (no `:-` separator)
//!    always produce a SYNTAX_ERROR.
//!
//! 4. **Unused body variables trigger warnings**: a body variable that does
//!    not appear in the head must be flagged as a warning (but rule is valid).
//!
//! # LLM quality test strategy (ROADMAP, v0.105.0)
//!
//! The roadmap requires a proptest that generates a random knowledge base with
//! a known ground-truth rule, constructs a natural-language description of
//! that rule, calls `draft_rule_from_nl()` with a mock LLM endpoint, and
//! asserts that at least one of the returned candidates is semantically
//! equivalent to the ground-truth rule when evaluated against the same
//! knowledge base.
//!
//! Since `draft_rule_from_nl()` requires a live PostgreSQL instance, the
//! end-to-end test is exercised by the pg_regress suite (RA-10 in
//! `tests/pg_regress/sql/v0105_rule_authoring.sql`).  Here we test the
//! pure-Rust properties of the validation algorithm using a reference
//! implementation that mirrors the logic in `src/rule_authoring.rs`.

use proptest::prelude::*;

// ─── Reference implementation ─────────────────────────────────────────────────
//
// A minimal Datalog rule parser / validator that mirrors the logic in
// `src/rule_authoring.rs` without importing the pgrx extension crate.

/// Result of the pure-Rust rule validation reference implementation.
#[derive(Debug)]
struct ValidationResult {
    valid: bool,
    errors: Vec<&'static str>,
    warnings: Vec<&'static str>,
}

/// Extract the `?name` variable names from a term token like `?x` or `<iri>`.
fn extract_var(term: &str) -> Option<String> {
    let term = term.trim();
    if let Some(v) = term.strip_prefix('?') {
        let v = v.trim_end_matches([',', '.', ')']);
        if !v.is_empty() {
            return Some(v.to_owned());
        }
    }
    None
}

/// Extremely simplified Datalog rule validator (reference implementation).
///
/// Handles the following rule shape only:
/// ```text
/// ?head_s <pred_iri> ?head_o :- ?body_s1 <iri1> ?body_o1 [, ...] [, NOT(?s2 <iri2> ?o2)] .
/// ```
///
/// This is sufficient to verify the properties under test without building a
/// full parser.
fn validate_rule_ref(rule: &str) -> ValidationResult {
    let rule = rule.trim().trim_end_matches('.');

    // Check for neck separator `:-`.
    let Some(neck_pos) = rule.find(":-") else {
        return ValidationResult {
            valid: false,
            errors: vec!["SYNTAX_ERROR"],
            warnings: vec![],
        };
    };

    let head_str = rule[..neck_pos].trim();
    let body_str = rule[neck_pos + 2..].trim();

    // Parse head terms.
    let head_tokens: Vec<&str> = head_str.split_whitespace().collect();
    if head_tokens.len() < 3 {
        return ValidationResult {
            valid: false,
            errors: vec!["SYNTAX_ERROR"],
            warnings: vec![],
        };
    }

    let head_vars: std::collections::HashSet<String> =
        head_tokens.iter().filter_map(|t| extract_var(t)).collect();

    // Parse body literals.
    let mut pos_body_vars: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut negated_only_vars: Vec<String> = Vec::new();
    let mut errors: Vec<&'static str> = Vec::new();
    let mut warnings: Vec<&'static str> = Vec::new();

    for lit in body_str.split(',') {
        let lit = lit.trim().trim_end_matches('.');
        if lit.is_empty() {
            continue;
        }

        if lit.trim_start().starts_with("NOT(") || lit.trim_start().starts_with("not(") {
            let inner = lit
                .trim()
                .strip_prefix("NOT(")
                .or_else(|| lit.trim().strip_prefix("not("))
                .unwrap_or("")
                .trim_end_matches(')');
            for tok in inner.split_whitespace() {
                if let Some(v) = extract_var(tok) {
                    negated_only_vars.push(v);
                }
            }
        } else {
            for tok in lit.split_whitespace() {
                if let Some(v) = extract_var(tok) {
                    pos_body_vars.insert(v);
                }
            }
        }
    }

    // Check unbound head variables.
    for hv in &head_vars {
        if !pos_body_vars.contains(hv.as_str()) {
            errors.push("UNBOUND_HEAD_VARIABLE");
        }
    }

    // Check unsafe negation.
    for nv in &negated_only_vars {
        if !pos_body_vars.contains(nv.as_str()) {
            errors.push("UNSAFE_NEGATION");
        }
    }

    // Check unused body variables.
    for bv in &pos_body_vars {
        if !head_vars.contains(bv.as_str()) {
            warnings.push("UNUSED_BODY_VARIABLE");
        }
    }

    ValidationResult {
        valid: errors.is_empty(),
        errors,
        warnings,
    }
}

// ─── Strategies ──────────────────────────────────────────────────────────────

fn arb_iri_suffix() -> impl Strategy<Value = String> {
    "[a-z]{3,8}".prop_map(|s| s)
}

fn arb_var() -> impl Strategy<Value = char> {
    prop_oneof![
        Just('x'),
        Just('y'),
        Just('z'),
        Just('a'),
        Just('b'),
        Just('c'),
    ]
}

proptest! {
    /// Property 1: A well-formed rule (all head vars bound) passes validation.
    #[test]
    fn valid_rule_passes(
        pred1 in arb_iri_suffix(),
        pred2 in arb_iri_suffix(),
        var_s in arb_var(),
        var_o in arb_var(),
    ) {
        let rule = format!(
            "?{var_s} <http://ex.org/{pred1}> ?{var_o} :- \
             ?{var_s} <http://ex.org/{pred2}> ?{var_o} ."
        );
        let result = validate_rule_ref(&rule);
        prop_assert!(
            result.valid,
            "expected valid rule to pass, got errors: {:?}\nrule: {rule}",
            result.errors
        );
    }

    /// Property 2: An unbound head variable always triggers UNBOUND_HEAD_VARIABLE.
    #[test]
    fn unbound_head_variable_is_detected(
        pred1 in arb_iri_suffix(),
        pred2 in arb_iri_suffix(),
    ) {
        // ?z is not in the body.
        let rule = format!(
            "?x <http://ex.org/{pred1}> ?z :- \
             ?x <http://ex.org/{pred2}> ?y ."
        );
        let result = validate_rule_ref(&rule);
        prop_assert!(
            !result.valid,
            "expected validation to fail due to unbound head variable\nrule: {rule}"
        );
        prop_assert!(
            result.errors.contains(&"UNBOUND_HEAD_VARIABLE"),
            "expected UNBOUND_HEAD_VARIABLE error, got: {:?}\nrule: {rule}",
            result.errors
        );
    }

    /// Property 3: A string without `:-` always produces a SYNTAX_ERROR.
    #[test]
    fn syntax_error_rule_fails(suffix in arb_iri_suffix()) {
        let junk = format!("this_is_not_valid_datalog_{suffix}");
        let result = validate_rule_ref(&junk);
        prop_assert!(
            !result.valid,
            "expected invalid input to fail\ninput: {junk}"
        );
        prop_assert!(
            result.errors.contains(&"SYNTAX_ERROR"),
            "expected SYNTAX_ERROR, got: {:?}",
            result.errors
        );
    }

    /// Property 4: A body variable not in the head triggers UNUSED_BODY_VARIABLE;
    /// the rule remains valid.
    #[test]
    fn unused_body_variable_triggers_warning(
        pred1 in arb_iri_suffix(),
        pred2 in arb_iri_suffix(),
        pred3 in arb_iri_suffix(),
    ) {
        let rule = format!(
            "?x <http://ex.org/{pred1}> ?y :- \
             ?x <http://ex.org/{pred2}> ?y, \
             ?y <http://ex.org/{pred3}> ?extra ."
        );
        let result = validate_rule_ref(&rule);
        prop_assert!(
            result.valid,
            "rule should be valid despite unused var\nrule: {rule}\nerrors: {:?}",
            result.errors
        );
        prop_assert!(
            result.warnings.contains(&"UNUSED_BODY_VARIABLE"),
            "expected UNUSED_BODY_VARIABLE warning, got: {:?}\nrule: {rule}",
            result.warnings
        );
    }

    /// Property 5: Swapping body atom order does not change the validation outcome.
    #[test]
    fn body_order_does_not_affect_validation(
        pred1 in arb_iri_suffix(),
        pred2 in arb_iri_suffix(),
        pred3 in arb_iri_suffix(),
    ) {
        let rule_ab = format!(
            "?x <http://ex.org/{pred1}> ?y :- \
             ?x <http://ex.org/{pred2}> ?z, ?z <http://ex.org/{pred3}> ?y ."
        );
        let rule_ba = format!(
            "?x <http://ex.org/{pred1}> ?y :- \
             ?z <http://ex.org/{pred3}> ?y, ?x <http://ex.org/{pred2}> ?z ."
        );
        let result_ab = validate_rule_ref(&rule_ab);
        let result_ba = validate_rule_ref(&rule_ba);
        prop_assert_eq!(
            result_ab.valid,
            result_ba.valid,
            "body order must not affect validation outcome\nAB: {}\nBA: {}",
            rule_ab,
            rule_ba
        );
    }
}
