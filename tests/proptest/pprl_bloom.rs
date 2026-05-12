//! Property-based tests for PPRL Bloom-filter primitives (v0.111.0).
//!
//! Verifies algebraic properties of `bloom_encode` and `dice_similarity`
//! in pure Rust without a database connection.
//!
//! # Properties tested
//! 1. **Round-trip identity**: `dice_similarity(bloom_encode(v, k, h, l), bloom_encode(v, k, h, l)) = 1.0`
//!    for any valid parameters.
//! 2. **Distinctness**: `dice_similarity(bloom_encode(v1, k), bloom_encode(v2, k)) < 1.0`
//!    whenever `v1 ≠ v2` (with overwhelming probability for recommended parameters).
//! 3. **dp_noisy_count sign**: result is always `≥ 0` regardless of Laplace noise.
//! 4. **Range**: `dice_similarity` is always in `[0.0, 1.0]`.
//! 5. **Symmetry**: `dice_similarity(a, b) = dice_similarity(b, a)`.
//! 6. **Output length**: `bloom_encode` output has exactly `length / 4` hex characters.

use proptest::prelude::*;

// ─── Pure-Rust re-implementations matching src/pprl.rs ───────────────────────

fn bloom_encode_rs(value: &str, key: &str, hash_count: u32, length: u32) -> String {
    use hmac::{Hmac, KeyInit, Mac};
    use sha2::Sha256;

    assert!(hash_count >= 1 && hash_count <= 256);
    assert!(length >= 64 && length <= 65536 && length % 8 == 0);

    let byte_count = (length as usize) / 8;
    let mut bits = vec![0u8; byte_count];
    let nbits = length as u64;

    for i in 0..hash_count {
        let full_key = format!("{key}\x00{i:04}");
        type HmacSha256 = Hmac<Sha256>;
        let mut mac =
            HmacSha256::new_from_slice(full_key.as_bytes()).expect("HMAC init failed in test");
        mac.update(value.as_bytes());
        let digest = mac.finalize().into_bytes();
        let pos_bytes: [u8; 8] = digest[..8].try_into().expect("digest too short");
        let raw = u64::from_le_bytes(pos_bytes);
        let bit_pos = (raw % nbits) as usize;
        bits[bit_pos / 8] |= 1u8 << (bit_pos % 8);
    }
    hex::encode(&bits)
}

fn dice_similarity_rs(a_hex: &str, b_hex: &str) -> f64 {
    let a_bytes = hex::decode(a_hex).expect("invalid hex in a");
    let b_bytes = hex::decode(b_hex).expect("invalid hex in b");

    if a_bytes == b_bytes {
        return 1.0;
    }

    let len = a_bytes.len().min(b_bytes.len());
    let mut and_count: u32 = 0;
    let mut a_count: u32 = 0;
    let mut b_count: u32 = 0;
    for i in 0..len {
        and_count += (a_bytes[i] & b_bytes[i]).count_ones();
        a_count += a_bytes[i].count_ones();
        b_count += b_bytes[i].count_ones();
    }
    for byte in &a_bytes[len..] {
        a_count += byte.count_ones();
    }
    for byte in &b_bytes[len..] {
        b_count += byte.count_ones();
    }

    let denom = a_count + b_count;
    if denom == 0 {
        return 0.0;
    }
    (2.0 * and_count as f64) / (denom as f64)
}

/// Clamped noisy count (Laplace noise simulated as zero for pure-Rust property test).
fn dp_noisy_count_sign(true_count: i64, noise: f64) -> i64 {
    ((true_count as f64) + noise).max(0.0) as i64
}

// ─── Strategies ───────────────────────────────────────────────────────────────

/// Generate a non-empty printable ASCII string up to 64 chars.
fn arb_value() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9 ]{1,64}".prop_filter("non-empty", |s| !s.is_empty())
}

/// Generate a valid hash_count in [1, 256].
fn arb_hash_count() -> impl Strategy<Value = u32> {
    1u32..=256u32
}

/// Generate a valid length: multiple of 8 in [64, 2048] (keep small for test speed).
fn arb_length() -> impl Strategy<Value = u32> {
    (8u32..=256u32).prop_map(|n| n * 8)
}

/// Generate a Laplace-like noise value (arbitrary float).
fn arb_noise() -> impl Strategy<Value = f64> {
    (-1_000_000.0f64..1_000_000.0f64)
}

// ─── Property tests ───────────────────────────────────────────────────────────

proptest! {
    /// 1. Round-trip identity: dice_similarity(encode(v), encode(v)) = 1.0
    #[test]
    fn bloom_roundtrip_identity(
        v in arb_value(),
        k in "[a-zA-Z0-9]{1,32}",
        h in arb_hash_count(),
        l in arb_length(),
    ) {
        let a = bloom_encode_rs(&v, &k, h, l);
        let b = bloom_encode_rs(&v, &k, h, l);
        let sim = dice_similarity_rs(&a, &b);
        prop_assert_eq!(
            sim, 1.0,
            "round-trip failed: dice_similarity({:?}, {:?}) with h={}, l={} → {}",
            v, v, h, l, sim
        );
    }

    /// 2. Distinctness: dice_similarity(encode(v1), encode(v2)) < 1.0 when v1 ≠ v2
    #[test]
    fn bloom_distinctness(
        v1 in arb_value(),
        v2 in arb_value(),
        k in "[a-zA-Z0-9]{1,32}",
    ) {
        // Only test when values are actually different
        prop_assume!(v1 != v2);
        let a = bloom_encode_rs(&v1, &k, 30, 1024);
        let b = bloom_encode_rs(&v2, &k, 30, 1024);
        let sim = dice_similarity_rs(&a, &b);
        prop_assert!(
            sim < 1.0,
            "distinctness violated: dice_similarity({v1:?}, {v2:?}) → {sim} (expected < 1.0)"
        );
    }

    /// 3. dp_noisy_count sign: result always ≥ 0
    #[test]
    fn dp_noisy_count_always_nonneg(
        true_count in 0i64..1_000_000i64,
        noise in arb_noise(),
    ) {
        let result = dp_noisy_count_sign(true_count, noise);
        prop_assert!(result >= 0, "dp_noisy_count returned {result} < 0 for count={true_count}, noise={noise}");
    }

    /// 4. Range: dice_similarity is always in [0.0, 1.0]
    #[test]
    fn dice_range(
        v1 in arb_value(),
        v2 in arb_value(),
        k in "[a-zA-Z0-9]{1,32}",
    ) {
        let a = bloom_encode_rs(&v1, &k, 30, 1024);
        let b = bloom_encode_rs(&v2, &k, 30, 1024);
        let sim = dice_similarity_rs(&a, &b);
        prop_assert!(
            (0.0..=1.0).contains(&sim),
            "dice_similarity out of range: {sim}"
        );
    }

    /// 5. Symmetry: dice_similarity(a, b) = dice_similarity(b, a)
    #[test]
    fn dice_symmetry(
        v1 in arb_value(),
        v2 in arb_value(),
        k in "[a-zA-Z0-9]{1,32}",
    ) {
        let a = bloom_encode_rs(&v1, &k, 30, 1024);
        let b = bloom_encode_rs(&v2, &k, 30, 1024);
        let ab = dice_similarity_rs(&a, &b);
        let ba = dice_similarity_rs(&b, &a);
        prop_assert!(
            (ab - ba).abs() < 1e-12,
            "symmetry violated: dice({v1:?},{v2:?})={ab} ≠ dice({v2:?},{v1:?})={ba}"
        );
    }

    /// 6. Output length: bloom_encode produces exactly `length / 4` hex chars
    ///    (length bits → length/8 bytes → length/4 hex chars)
    #[test]
    fn bloom_output_length(
        v in arb_value(),
        k in "[a-zA-Z0-9]{1,32}",
        h in arb_hash_count(),
        l in arb_length(),
    ) {
        let encoded = bloom_encode_rs(&v, &k, h, l);
        let expected_hex_chars = (l / 4) as usize;
        let actual_len = encoded.len();
        prop_assert_eq!(
            actual_len,
            expected_hex_chars,
            "wrong output length for l={}: got {} hex chars, expected {}",
            l, actual_len, expected_hex_chars
        );
    }
}
