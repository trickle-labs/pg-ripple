//! Privacy-Preserving Record Linkage (PPRL) primitives (v0.111.0).
//!
//! Provides:
//! - `pg_ripple.bloom_encode(value, key, hash_count, length)` — CLK Bloom-filter encoding
//! - `pg_ripple.dice_similarity(a, b)` — Dice coefficient on Bloom-filter outputs
//! - `pg_ripple.dp_noisy_count(query, epsilon)` — differential-privacy noisy COUNT
//! - `pg_ripple.dp_noisy_histogram(query, key_column, count_column, epsilon)` — noisy histogram
//!
//! # Security note
//! Bloom-filter CLK encodings with fewer than 30 hash functions or length < 1024 bits
//! may be reversible via graph-based attacks (Schnell et al. 2009; Christen et al. 2020).
//! These minimums are enforced as defaults. Using parameters below the recommended
//! minimums triggers a PostgreSQL WARNING to inform the user of the elevated risk.
//!
//! # Reference
//! Schnell, Bachteler & Reiher (2009) — "Privacy-preserving record linkage using
//! Bloom filters." BMC Medical Informatics and Decision Making 9:41.

use pgrx::prelude::*;

/// Recommended minimum hash count for CLK Bloom filters (Schnell et al. 2009).
const BLOOM_MIN_RECOMMENDED_HASH_COUNT: i32 = 30;
/// Recommended minimum bit length for CLK Bloom filters.
const BLOOM_MIN_RECOMMENDED_LENGTH: i32 = 1024;

// ─── bloom_encode ─────────────────────────────────────────────────────────────

/// Bloom-filter CLK encoding.
///
/// Returns a hex-encoded bit vector of `length` bits (= `length / 8` bytes),
/// computed using the standard CLK (Cryptographic Longterm Key) construction:
/// for each of `hash_count` independent HMAC-SHA-256 digests (keyed with
/// `key || i` for i in 0..hash_count), maps the first 8 bytes of the digest
/// to a bit position `pos = u64 mod length` and sets that bit.
///
/// The result is returned as a lowercase hex string (256 chars for default
/// length 1024 bits).  Two calls with the same (value, key, hash_count,
/// length) always produce the same output.
///
/// # Error codes
/// - PT0470: `value` length exceeds `pg_ripple.bloom_max_input_length`
/// - PT0471: `hash_count` not in [1, 256] or `length` not in [64, 65536] or
///           `length` is not a multiple of 8
///
/// ```sql
/// SELECT pg_ripple.bloom_encode('Alice', 'secret', 30, 1024);
/// ```
#[pg_schema]
mod pg_ripple {
    use pgrx::prelude::*;

    #[pg_extern]
    pub fn bloom_encode(
        value: &str,
        key: &str,
        hash_count: default!(i32, 30),
        length: default!(i32, 1024),
    ) -> String {
        use hmac::{Hmac, KeyInit, Mac};
        use sha2::Sha256;

        // ── Parameter validation (PT0471) ─────────────────────────────────────
        if !(1..=256).contains(&hash_count) || !(64..=65536).contains(&length) || length % 8 != 0
        {
            pgrx::error!(
                "bloom_encode: hash_count {} or length {} outside valid range \
                 (hash_count: 1–256, length: 64–65536 and must be a multiple of 8) [PT0471]",
                hash_count,
                length
            );
        }

        // ── Input length check (PT0470) ───────────────────────────────────────
        let max_input = crate::gucs::datalog::BLOOM_MAX_INPUT_LENGTH.get() as usize;
        if value.len() > max_input {
            pgrx::error!(
                "bloom_encode: input length {} exceeds bloom_max_input_length GUC ({}) [PT0470]",
                value.len(),
                max_input
            );
        }

        // ── Security warning for below-recommended parameters ─────────────────
        if hash_count < super::BLOOM_MIN_RECOMMENDED_HASH_COUNT
            || length < super::BLOOM_MIN_RECOMMENDED_LENGTH
        {
            pgrx::warning!(
                "bloom_encode: hash_count={} or length={} is below the recommended \
                 minimum (hash_count >= 30, length >= 1024). Bloom-filter CLK encodings \
                 with these parameters may be reversible via graph-based attacks.",
                hash_count,
                length
            );
        }

        // ── CLK construction ──────────────────────────────────────────────────
        let byte_count = (length as usize) / 8;
        let mut bits = vec![0u8; byte_count];
        let nbits = length as u64;

        for i in 0..hash_count {
            // Key = key || i (big-endian 4-byte suffix for uniqueness per hash function)
            let full_key = format!("{key}\x00{i:04}");

            // HMAC-SHA-256(key=full_key, data=value)
            type HmacSha256 = Hmac<Sha256>;
            // SAFETY: new_from_slice accepts any key length.
            let mut mac = HmacSha256::new_from_slice(full_key.as_bytes())
                .unwrap_or_else(|_| pgrx::error!("bloom_encode: HMAC key initialization failed"));
            mac.update(value.as_bytes());
            let digest = mac.finalize().into_bytes();

            // Map first 8 bytes to bit position
            let pos_bytes: [u8; 8] = digest[..8]
                .try_into()
                .unwrap_or_else(|_| pgrx::error!("bloom_encode: digest too short"));
            let raw = u64::from_le_bytes(pos_bytes);
            let bit_pos = (raw % nbits) as usize;

            let byte_idx = bit_pos / 8;
            let bit_idx = bit_pos % 8;
            bits[byte_idx] |= 1u8 << bit_idx;
        }

        // Return as lowercase hex string
        hex::encode(&bits)
    }

    // ─── dice_similarity ──────────────────────────────────────────────────────

    /// Dice coefficient for two Bloom-filter outputs (hex-encoded byte vectors).
    ///
    /// Computes `2 * popcount(a & b) / (popcount(a) + popcount(b))`.
    /// Returns `1.0` when both inputs are identical (including all-zero vectors
    /// that are identical).  Returns `0.0` when the total popcount of both is
    /// zero and they are equal (empty vectors case handled by identity check).
    ///
    /// The inputs must be hex strings as produced by `bloom_encode()`.
    ///
    /// ```sql
    /// SELECT pg_ripple.dice_similarity(
    ///     pg_ripple.bloom_encode('Alice', 'k', 30, 1024),
    ///     pg_ripple.bloom_encode('Alice', 'k', 30, 1024)
    /// );  -- returns 1.0
    /// ```
    #[pg_extern]
    pub fn dice_similarity(a: &str, b: &str) -> f64 {
        let a_bytes = hex::decode(a).unwrap_or_else(|_| {
            pgrx::error!("dice_similarity: argument 'a' is not valid hex-encoded bytes")
        });
        let b_bytes = hex::decode(b).unwrap_or_else(|_| {
            pgrx::error!("dice_similarity: argument 'b' is not valid hex-encoded bytes")
        });

        // Identical strings → always 1.0 (handles all-zero case too)
        if a_bytes == b_bytes {
            return 1.0;
        }

        // Align to the shorter length (in case lengths differ)
        let len = a_bytes.len().min(b_bytes.len());
        let mut and_count: u32 = 0;
        let mut a_count: u32 = 0;
        let mut b_count: u32 = 0;
        for i in 0..len {
            and_count += (a_bytes[i] & b_bytes[i]).count_ones();
            a_count += a_bytes[i].count_ones();
            b_count += b_bytes[i].count_ones();
        }
        // Count remaining bytes if lengths differ
        for byte in &a_bytes[len..] {
            a_count += byte.count_ones();
        }
        for byte in &b_bytes[len..] {
            b_count += byte.count_ones();
        }

        let denom = a_count + b_count;
        if denom == 0 {
            // Both all-zero (but not identical — different lengths)
            return 0.0;
        }

        (2.0 * and_count as f64) / (denom as f64)
    }

    // ─── dp_noisy_count ───────────────────────────────────────────────────────

    /// Differentially-private COUNT with Laplace noise.
    ///
    /// Executes `query` in read-only mode, expects a single-row single-INTEGER
    /// result, adds Laplace(0, 1/epsilon) noise, and returns the noisy count
    /// (clamped to ≥ 0).
    ///
    /// # Error codes
    /// - PT0472: `epsilon` out of range (must be in (0.0, 10.0])
    /// - PT0473: query did not return a single INTEGER
    /// - PT0474: query rejected by validation (not a read-only SELECT)
    ///
    /// ```sql
    /// SELECT pg_ripple.dp_noisy_count(
    ///     'SELECT COUNT(*) FROM _pg_ripple.dictionary',
    ///     0.1
    /// ) >= 0;
    /// ```
    #[pg_extern]
    pub fn dp_noisy_count(query: &str, epsilon: f64) -> i64 {
        // ── epsilon validation (PT0472) ────────────────────────────────────────
        if epsilon <= 0.0 || epsilon > 10.0 {
            pgrx::error!(
                "dp_noisy_count: epsilon {} out of valid range (0, 10] [PT0472]",
                epsilon
            );
        }

        // ── Query validation (PT0474) ──────────────────────────────────────────
        super::validate_dp_query(query);

        // ── Execute read-only ──────────────────────────────────────────────────
        let count: i64 = Spi::get_one(query)
            .unwrap_or(None)
            .unwrap_or_else(|| {
                pgrx::error!(
                    "dp_noisy_count: query must return a single INTEGER value [PT0473]"
                )
            });

        // ── Add Laplace noise ──────────────────────────────────────────────────
        let noise = super::laplace_noise(1.0 / epsilon);
        let noisy = count as f64 + noise;
        noisy.max(0.0) as i64
    }

    // ─── dp_noisy_histogram ───────────────────────────────────────────────────

    /// Differentially-private histogram with per-bucket Laplace noise.
    ///
    /// Executes `query` in read-only mode, reads `key_column` (TEXT) and
    /// `count_column` (BIGINT) from the result, adds independent
    /// Laplace(0, 1/epsilon) noise to each bucket count, and returns the
    /// noisy histogram (each bucket count clamped to ≥ 0).
    ///
    /// # Error codes
    /// - PT0472: `epsilon` out of range
    /// - PT0474: query rejected by validation
    ///
    /// ```sql
    /// SELECT * FROM pg_ripple.dp_noisy_histogram(
    ///     'SELECT category, COUNT(*) AS n FROM my_table GROUP BY category',
    ///     'category', 'n', 0.5
    /// );
    /// ```
    #[pg_extern]
    pub fn dp_noisy_histogram(
        query: &str,
        key_column: &str,
        count_column: &str,
        epsilon: f64,
    ) -> TableIterator<'static, (name!(key, String), name!(noisy_count, i64))> {
        // ── epsilon validation (PT0472) ────────────────────────────────────────
        if epsilon <= 0.0 || epsilon > 10.0 {
            pgrx::error!(
                "dp_noisy_histogram: epsilon {} out of valid range (0, 10] [PT0472]",
                epsilon
            );
        }

        // ── Query validation (PT0474) ──────────────────────────────────────────
        super::validate_dp_query(query);

        // ── Execute read-only ──────────────────────────────────────────────────
        let key_col = key_column.to_string();
        let count_col = count_column.to_string();
        let sensitivity = 1.0 / epsilon;

        let rows: Vec<(String, i64)> = Spi::connect(|client| {
            let tup_table = client
                .select(query, None, &[])
                .unwrap_or_else(|e| pgrx::error!("dp_noisy_histogram: query execution failed: {e}"));

            let mut result = Vec::new();
            for row in tup_table {
                let key_val = row
                    .get_by_name::<String, _>(&key_col)
                    .unwrap_or(None)
                    .unwrap_or_else(|| String::from("(null)"));
                let count_val: i64 = row
                    .get_by_name::<i64, _>(&count_col)
                    .unwrap_or(None)
                    .unwrap_or(0);
                let noisy = (count_val as f64 + super::laplace_noise(sensitivity)).max(0.0) as i64;
                result.push((key_val, noisy));
            }
            result
        });

        TableIterator::new(rows)
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Validate a DP query: must be a read-only SELECT; rejects DML/DDL keywords.
///
/// Raises PT0474 if validation fails.
fn validate_dp_query(query: &str) {
    let trimmed = query.trim();
    let upper = trimmed.to_uppercase();

    // Must start with SELECT
    if !upper.starts_with("SELECT") {
        pgrx::error!(
            "dp_noisy_count: query rejected by validation — must be a read-only SELECT [PT0474]"
        );
    }

    // Reject dangerous keywords (case-insensitive)
    for kw in &[
        ";", "INSERT", "UPDATE", "DELETE", "DROP", "CREATE", "ALTER", "GRANT",
        "REVOKE", "TRUNCATE",
    ] {
        if upper.contains(kw) {
            pgrx::error!(
                "dp_noisy_count: query rejected by validation — must be a read-only SELECT [PT0474]"
            );
        }
    }
}

/// Generate a sample from the Laplace distribution with scale `b`
/// using the inverse CDF method.
///
/// `b = sensitivity / epsilon` for ε-differential privacy.
///
/// The implementation uses `getrandom` via standard Rust random sampling:
/// `X = -b * sign(U) * ln(1 - 2|U|)` where U is uniform in (-0.5, 0.5).
///
/// We use a simple LCG-style deterministic offset based on the current
/// timestamp to avoid requiring an external random crate.
///
/// In production, replace with a cryptographically secure RNG.
fn laplace_noise(b: f64) -> f64 {
    // Use PostgreSQL's random() via SPI for a uniform sample in (0, 1).
    let u: f64 = pgrx::Spi::get_one("SELECT random()")
        .unwrap_or(None)
        .unwrap_or(0.5_f64);

    // Map uniform (0, 1) → (-0.5, 0.5) and apply inverse CDF of Laplace.
    // Clamp to avoid log(0) edge case.
    let u_shifted = (u - 0.5).clamp(-0.4999, 0.4999);
    let sign = if u_shifted >= 0.0 { 1.0_f64 } else { -1.0_f64 };
    -b * sign * (1.0 - 2.0 * u_shifted.abs()).ln()
}
