//! Property-based tests for the Bayesian confidence update algebra (v0.108.0 BAYES-01).
//!
//! Verifies three algebraic properties of the Bayesian update formula used in
//! `pg_ripple.update_confidence()`:
//!
//! 1. **Monotone property**: a likelihood ratio > 1.0 always increases confidence;
//!    < 1.0 always decreases it (neutral LR = 1.0 leaves it unchanged).
//!
//! 2. **Bayesian consistency**: applying a sequence of independent evidence updates
//!    equals applying all evidence jointly (sequential Bayesian updates commute when
//!    evidence is conditionally independent).
//!
//! 3. **Clamping property**: the posterior is always in [0.001, 0.999] regardless of
//!    inputs (never absolute certainty or impossibility).
//!
//! All properties are tested against a pure-Rust reference oracle implementing the
//! same Bayesian formula. No database connection is required.
//!
//! # Running
//!
//! ```sh
//! cargo test --test proptest_suite bayesian
//! PROPTEST_CASES=50000 cargo test --test proptest_suite bayesian
//! ```

use proptest::prelude::*;

// ─── Reference oracle (mirrors src/uncertain_knowledge_api/bayesian.rs) ───────

/// Bayesian update in odds form.
///
/// posterior = (λ · p₀) / (λ · p₀ + (1 − p₀))
///
/// Clamped to [0.001, 0.999] — never absolute certainty.
fn bayesian_update(prior: f64, likelihood_ratio: f64) -> f64 {
    let numerator = likelihood_ratio * prior;
    let denominator = numerator + (1.0 - prior);
    let posterior = if denominator == 0.0 {
        prior
    } else {
        numerator / denominator
    };
    posterior.clamp(0.001, 0.999)
}

/// Noisy-OR update (v0.87 combiner, used when strategy = 'noisy-or').
fn noisy_or_update(prior: f64, likelihood_ratio: f64) -> f64 {
    let weight = 1.0 - 1.0 / (1.0 + likelihood_ratio);
    let posterior = 1.0 - (1.0 - prior) * (1.0 - weight);
    posterior.clamp(0.001, 0.999)
}

// ─── Strategy generators ──────────────────────────────────────────────────────

/// Confidence value in (0.001, 0.999) — the valid post-clamp range.
fn arb_prior() -> impl Strategy<Value = f64> {
    (1u64..=999u64).prop_map(|n| n as f64 / 1000.0)
}

/// Positive likelihood ratio in (0.0, 100.0].
fn arb_lr_positive() -> impl Strategy<Value = f64> {
    (1u64..=10_000u64).prop_map(|n| n as f64 / 100.0)
}

/// Likelihood ratio strictly > 1.0.
fn arb_lr_gt1() -> impl Strategy<Value = f64> {
    (101u64..=10_000u64).prop_map(|n| n as f64 / 100.0)
}

/// Likelihood ratio strictly < 1.0.
fn arb_lr_lt1() -> impl Strategy<Value = f64> {
    (1u64..=99u64).prop_map(|n| n as f64 / 100.0)
}

/// "Safe" prior in [0.1, 0.9] — far enough from clamp boundaries that a
/// sequence of up to 5 updates with `arb_safe_lr` cannot hit [0.001, 0.999].
fn arb_safe_prior() -> impl Strategy<Value = f64> {
    (100u64..=900u64).prop_map(|n| n as f64 / 1000.0)
}

/// "Safe" likelihood ratio in [0.5, 2.0].  With up to 5 such updates from a
/// safe prior the posterior stays strictly inside the clamp range, preserving
/// the sequential = joint and order-independence algebraic identities.
fn arb_safe_lr() -> impl Strategy<Value = f64> {
    (50u64..=200u64).prop_map(|n| n as f64 / 100.0)
}

/// A small sequence of 2–5 independent likelihood ratios, all in [0.5, 2.0].
fn arb_lr_sequence() -> impl Strategy<Value = Vec<f64>> {
    prop::collection::vec(arb_safe_lr(), 2..=5)
}

const EPSILON: f64 = 1e-10;

// ─── Properties ──────────────────────────────────────────────────────────────

proptest! {
    // 1. Monotone increase: LR > 1.0 ⟹ posterior > prior (after clamping).
    #[test]
    fn monotone_increase_bayesian(prior in arb_prior(), lr in arb_lr_gt1()) {
        let posterior = bayesian_update(prior, lr);
        prop_assert!(
            posterior >= prior - EPSILON,
            "monotone_increase violated: bayesian_update({prior}, {lr}) = {posterior} < {prior}"
        );
    }

    // 2. Monotone decrease: LR < 1.0 ⟹ posterior < prior (after clamping).
    #[test]
    fn monotone_decrease_bayesian(prior in arb_prior(), lr in arb_lr_lt1()) {
        let posterior = bayesian_update(prior, lr);
        prop_assert!(
            posterior <= prior + EPSILON,
            "monotone_decrease violated: bayesian_update({prior}, {lr}) = {posterior} > {prior}"
        );
    }

    // 3. Neutral LR = 1.0 leaves confidence unchanged.
    #[test]
    fn neutral_lr_bayesian(prior in arb_prior()) {
        let posterior = bayesian_update(prior, 1.0);
        prop_assert!(
            (posterior - prior).abs() < EPSILON,
            "neutral_lr violated: bayesian_update({prior}, 1.0) = {posterior} ≠ {prior}"
        );
    }

    // 4. Bayesian consistency: sequential updates equal joint update.
    //    Applying LR₁ then LR₂ equals applying LR₁ × LR₂ in one step.
    //
    //    This identity holds for the unclamped formula; we restrict inputs to
    //    [0.1, 0.9] × [0.5, 2.0]² so that no intermediate value ever hits the
    //    [0.001, 0.999] clamp boundary, making the property observable.
    #[test]
    fn sequential_equals_joint_bayesian(
        prior in arb_safe_prior(),
        lr1 in arb_safe_lr(),
        lr2 in arb_safe_lr()
    ) {
        let sequential = bayesian_update(bayesian_update(prior, lr1), lr2);
        let joint = bayesian_update(prior, lr1 * lr2);
        prop_assert!(
            (sequential - joint).abs() < 1e-8,
            "sequential_equals_joint violated: \
             sequential({prior}, {lr1}, {lr2}) = {sequential} ≠ joint = {joint}"
        );
    }

    // 5. Clamping: posterior is always in [0.001, 0.999].
    #[test]
    fn posterior_in_range_bayesian(prior in arb_prior(), lr in arb_lr_positive()) {
        let posterior = bayesian_update(prior, lr);
        prop_assert!(
            posterior >= 0.001 && posterior <= 0.999,
            "clamping violated: bayesian_update({prior}, {lr}) = {posterior} out of [0.001, 0.999]"
        );
    }

    // 6. Propagation consistency: applying a sequence of updates is order-independent
    //    when evidence is conditionally independent (commutativity of joint LR product).
    //
    //    Restricted to safe inputs so intermediate clamping cannot break commutativity.
    #[test]
    fn sequence_order_independence_bayesian(
        prior in arb_safe_prior(),
        lrs in arb_lr_sequence()
    ) {
        // Forward order.
        let forward = lrs.iter().fold(prior, |p, &lr| bayesian_update(p, lr));
        // Reverse order.
        let backward = lrs.iter().rev().fold(prior, |p, &lr| bayesian_update(p, lr));
        prop_assert!(
            (forward - backward).abs() < 1e-8,
            "order_independence violated: forward={forward}, backward={backward} for prior={prior}"
        );
    }

    // 7. Noisy-OR monotone increase for LR > 1.0.
    #[test]
    fn noisy_or_monotone_increase(prior in arb_prior(), lr in arb_lr_gt1()) {
        let posterior = noisy_or_update(prior, lr);
        prop_assert!(
            posterior >= prior - EPSILON,
            "noisy_or_monotone_increase violated: noisy_or_update({prior}, {lr}) = {posterior} < {prior}"
        );
    }

    // 8. Noisy-OR posterior in range [0.001, 0.999].
    #[test]
    fn noisy_or_in_range(prior in arb_prior(), lr in arb_lr_positive()) {
        let posterior = noisy_or_update(prior, lr);
        prop_assert!(
            posterior >= 0.001 && posterior <= 0.999,
            "noisy_or_in_range violated: noisy_or_update({prior}, {lr}) = {posterior}"
        );
    }
}
