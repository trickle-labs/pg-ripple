//! Property-based tests for the noisy-OR confidence combination algebra
//! used by pg_ripple's probabilistic Datalog engine (CB-01, v0.89.0).
//!
//! The noisy-OR formula is: `noisy_or(a, b) = 1 - (1 - a) * (1 - b)`
//! This is equivalent to the SQL merge rule in `seminaive.rs`:
//!   `1.0 - (1.0 - EXCLUDED.confidence) * (1.0 - confidence.confidence)`
//!
//! We verify six algebraic identities against a reference in-process Rust
//! implementation.  No database connection is required — all tests run in
//! pure Rust.
//!
//! # Identities tested
//! 1. **Commutativity**: `noisy_or(a, b) = noisy_or(b, a)`
//! 2. **Associativity**: `noisy_or(noisy_or(a, b), c) ≈ noisy_or(a, noisy_or(b, c))`
//! 3. **Monotonicity**: `a ≤ b ⟹ noisy_or(a, x) ≤ noisy_or(b, x)`
//! 4. **Idempotence**: `noisy_or(c, c) = 1 - (1-c)²`
//! 5. **Identity element**: `noisy_or(0.0, c) = c`
//! 6. **Absorbing element**: `noisy_or(1.0, c) = 1.0`

use proptest::prelude::*;

/// The noisy-OR confidence combination operator.
///
/// Implements the probabilistic independence assumption:
///   P(A ∨ B) = 1 - P(¬A) * P(¬B)
fn noisy_or(a: f64, b: f64) -> f64 {
    1.0 - (1.0 - a) * (1.0 - b)
}

/// Tolerance for floating-point comparisons.
const EPSILON: f64 = 1e-12;

/// Strategy that generates a float in [0.0, 1.0] (the valid confidence range).
fn arb_confidence() -> impl Strategy<Value = f64> {
    (0u64..=1_000_000u64).prop_map(|n| n as f64 / 1_000_000.0)
}

proptest! {
    /// 1. Commutativity: noisy_or(a, b) == noisy_or(b, a)
    #[test]
    fn commutativity(a in arb_confidence(), b in arb_confidence()) {
        let ab = noisy_or(a, b);
        let ba = noisy_or(b, a);
        prop_assert!(
            (ab - ba).abs() < EPSILON,
            "commutativity violated: noisy_or({a}, {b}) = {ab} ≠ noisy_or({b}, {a}) = {ba}"
        );
    }

    /// 2. Associativity: noisy_or(noisy_or(a, b), c) ≈ noisy_or(a, noisy_or(b, c))
    #[test]
    fn associativity(a in arb_confidence(), b in arb_confidence(), c in arb_confidence()) {
        let left  = noisy_or(noisy_or(a, b), c);
        let right = noisy_or(a, noisy_or(b, c));
        prop_assert!(
            (left - right).abs() < EPSILON,
            "associativity violated: noisy_or(noisy_or({a},{b}),{c}) = {left} ≠ noisy_or({a},noisy_or({b},{c})) = {right}"
        );
    }

    /// 3. Monotonicity: a ≤ b ⟹ noisy_or(a, x) ≤ noisy_or(b, x)
    #[test]
    fn monotonicity(a in arb_confidence(), b in arb_confidence(), x in arb_confidence()) {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        let lo_x = noisy_or(lo, x);
        let hi_x = noisy_or(hi, x);
        prop_assert!(
            lo_x <= hi_x + EPSILON,
            "monotonicity violated: a={lo} ≤ b={hi} but noisy_or(a,{x}) = {lo_x} > noisy_or(b,{x}) = {hi_x}"
        );
    }

    /// 4. Idempotence: noisy_or(c, c) == 1 - (1-c)^2
    #[test]
    fn idempotence(c in arb_confidence()) {
        let result = noisy_or(c, c);
        let expected = 1.0 - (1.0 - c).powi(2);
        prop_assert!(
            (result - expected).abs() < EPSILON,
            "idempotence violated: noisy_or({c},{c}) = {result} ≠ 1-(1-{c})^2 = {expected}"
        );
    }

    /// 5. Identity element: noisy_or(0.0, c) == c
    #[test]
    fn identity_element(c in arb_confidence()) {
        let result = noisy_or(0.0, c);
        prop_assert!(
            (result - c).abs() < EPSILON,
            "identity element violated: noisy_or(0.0, {c}) = {result} ≠ {c}"
        );
    }

    /// 6. Absorbing element: noisy_or(1.0, c) == 1.0
    #[test]
    fn absorbing_element(c in arb_confidence()) {
        let result = noisy_or(1.0, c);
        prop_assert!(
            (result - 1.0).abs() < EPSILON,
            "absorbing element violated: noisy_or(1.0, {c}) = {result} ≠ 1.0"
        );
    }

    /// 7. Output range: noisy_or(a, b) ∈ [0.0, 1.0] for all valid inputs
    #[test]
    fn output_in_range(a in arb_confidence(), b in arb_confidence()) {
        let result = noisy_or(a, b);
        prop_assert!(
            result >= 0.0 && result <= 1.0 + EPSILON,
            "output out of range: noisy_or({a}, {b}) = {result}"
        );
    }
}
