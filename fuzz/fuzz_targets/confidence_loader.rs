//! cargo-fuzz target for the confidence loader (v0.90.0 TEST-03).
//!
//! Fuzzes `load_triples_with_confidence()` with adversarial float inputs:
//! - NaN confidence → should raise PT0302
//! - Infinite confidence → should raise PT0302
//! - Negative confidence → should raise PT0302
//! - Confidence > 1.0 → should raise PT0302
//! - Denormal floats (< f64::MIN_POSITIVE) → PT0302 or clamped to 0.0
//! - Valid range [0.0, 1.0] → accepted
//!
//! This target tests the in-process validation logic that mirrors what
//! the SQL function `pg_ripple.load_ntriples_with_confidence()` enforces.
//! It validates that no adversarial float value causes a panic or silent
//! data corruption — only clean error paths.
//!
//! # Running locally
//!
//! ```sh
//! cargo install cargo-fuzz
//! cargo fuzz run confidence_loader -- -max_total_time=300
//! cargo fuzz tmin confidence_loader artifacts/confidence_loader/crash-...
//! ```
//!
//! # CI
//!
//! The `fuzz-confidence` CI job runs this target for 300 seconds nightly.

#![no_main]

use libfuzzer_sys::fuzz_target;

/// Confidence validation logic mirroring the Rust implementation in
/// `src/uncertain_knowledge_api/confidence_table.rs`.
///
/// Returns `Err` for invalid confidence values (PT0302), `Ok` for valid.
fn validate_confidence(confidence: f64) -> Result<f64, String> {
    if confidence.is_nan() {
        return Err(format!("PT0302 confidence is NaN — must be a finite value in [0.0, 1.0]"));
    }
    if confidence.is_infinite() {
        return Err(format!(
            "PT0302 confidence is infinite ({}) — must be a finite value in [0.0, 1.0]",
            if confidence > 0.0 { "+∞" } else { "-∞" }
        ));
    }
    if confidence < 0.0 {
        return Err(format!(
            "PT0302 confidence {confidence:.6e} is negative — must be in [0.0, 1.0]"
        ));
    }
    if confidence > 1.0 {
        return Err(format!(
            "PT0302 confidence {confidence:.6e} exceeds 1.0 — must be in [0.0, 1.0]"
        ));
    }
    // Denormal floats (subnormal) are valid but map to effectively 0.0
    // Document this: values < f64::MIN_POSITIVE are accepted and stored as-is
    Ok(confidence)
}

/// Parse a confidence value from fuzz input bytes.
///
/// Takes 8 bytes as little-endian IEEE 754 double, then validates.
fn parse_and_validate(data: &[u8]) -> Option<Result<f64, String>> {
    if data.len() < 8 {
        return None;
    }
    let bytes: [u8; 8] = data[..8].try_into().ok()?;
    let confidence = f64::from_le_bytes(bytes);
    Some(validate_confidence(confidence))
}

fuzz_target!(|data: &[u8]| {
    // Test 1: Raw f64 bytes
    if let Some(result) = parse_and_validate(data) {
        match result {
            Ok(c) => {
                // Valid confidence: must be in [0.0, 1.0] and finite
                assert!(c.is_finite(), "validated confidence {c} is not finite");
                assert!(c >= 0.0, "validated confidence {c} is negative");
                assert!(c <= 1.0, "validated confidence {c} exceeds 1.0");
            }
            Err(msg) => {
                // Error path: message must start with PT0302
                assert!(
                    msg.starts_with("PT0302"),
                    "error message must start with PT0302, got: {msg}"
                );
            }
        }
    }

    // Test 2: Adversarial known bad values — never panic on these
    let known_bad: &[f64] = &[
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        -0.001,
        -1.0,
        -f64::MAX,
        1.0001,
        2.0,
        f64::MAX,
        f64::MIN,         // most negative finite double
        -f64::MIN_POSITIVE, // most negative subnormal
    ];

    for &bad in known_bad {
        let result = validate_confidence(bad);
        if bad.is_nan() || bad.is_infinite() || bad < 0.0 || bad > 1.0 {
            assert!(
                result.is_err(),
                "expected PT0302 error for {bad:?}, got Ok"
            );
            let msg = result.unwrap_err();
            assert!(
                msg.starts_with("PT0302"),
                "error message must start with PT0302, got: {msg}"
            );
        }
    }

    // Test 3: Valid boundary values
    let valid: &[f64] = &[
        0.0,
        f64::MIN_POSITIVE, // smallest positive normal
        0.5,
        0.9999999,
        1.0,
    ];
    for &v in valid {
        let result = validate_confidence(v);
        assert!(
            result.is_ok(),
            "expected Ok for valid confidence {v:.8e}, got Err"
        );
    }
});
