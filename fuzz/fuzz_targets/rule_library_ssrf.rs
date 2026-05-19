//! Fuzz target for the `subscribe_rule_library()` SSRF URL validation path
//! and the rule-library NDJSON stream parser (H17-01 / SEC-H-01, v0.121.0).
//!
//! Two invariants are tested:
//!
//! 1. **SSRF blocker never panics**: The `is_blocked_host()` and
//!    `is_private_ip()` SSRF validation logic must handle any arbitrary URL
//!    string without panicking — including IPv6-mapped addresses
//!    (`::ffff:192.168.x.x`), decimal-encoded IPs, CGNAT ranges
//!    (`100.64.0.0/10`), multicast (`224.0.0.0/4`), and other special ranges.
//!
//! 2. **NDJSON stream parser never panics**: The newline-delimited JSON parser
//!    that processes rule library streams from remote endpoints must handle
//!    arbitrary byte sequences without panicking.
//!
//! Neither invariant requires the actual PostgreSQL SPI — both are pure Rust
//! string-parsing paths that can be exercised without a live database.
//!
//! # Running locally
//!
//! ```sh
//! cargo install cargo-fuzz
//! cargo fuzz run rule_library_ssrf -- -max_total_time=600
//! ```

#![no_main]

use libfuzzer_sys::fuzz_target;

// ── Inline SSRF validation logic (mirrors src/sparql/federation/policy.rs) ───
//
// We cannot import pgrx-dependent code directly; this is a faithful copy of
// the validation logic sufficient to exercise the blocking paths.

fn is_blocked_host(host: &str) -> bool {
    // loopback
    if host == "localhost"
        || host == "127.0.0.1"
        || host == "::1"
        || host.starts_with("127.")
    {
        return true;
    }
    // link-local IPv4: 169.254.x.x
    if host.starts_with("169.254.") {
        return true;
    }
    // link-local IPv6: fe80::
    if host.to_lowercase().starts_with("fe80") {
        return true;
    }
    // Unique-local IPv6: fc00::/7
    let h_lower = host.to_lowercase();
    if h_lower.starts_with("fc") || h_lower.starts_with("fd") {
        return true;
    }
    // RFC-1918: 10.x.x.x
    if host.starts_with("10.") {
        return true;
    }
    // RFC-1918: 172.16.x.x – 172.31.x.x
    if host
        .strip_prefix("172.")
        .and_then(|rest| rest.split('.').next())
        .and_then(|s| s.parse::<u8>().ok())
        .is_some_and(|second| (16..=31).contains(&second))
    {
        return true;
    }
    // RFC-1918: 192.168.x.x
    if host.starts_with("192.168.") {
        return true;
    }
    // CGNAT: 100.64.0.0/10 (RFC 6598, v0.121.0 SEC-M-03)
    if host
        .strip_prefix("100.")
        .and_then(|rest| rest.split('.').next())
        .and_then(|s| s.parse::<u8>().ok())
        .is_some_and(|second| (64..=127).contains(&second))
    {
        return true;
    }
    // Multicast: 224.0.0.0/4 (v0.121.0 SEC-M-03)
    if host
        .split('.')
        .next()
        .and_then(|s| s.parse::<u8>().ok())
        .is_some_and(|first| (224..=239).contains(&first))
    {
        return true;
    }
    // This-network: 0.0.0.0/8 (v0.121.0 SEC-M-03)
    if host.starts_with("0.") {
        return true;
    }
    // IPv4-mapped IPv6: ::ffff: prefix (v0.121.0 SEC-M-03)
    if h_lower.starts_with("::ffff:") {
        let ipv4_part = &h_lower[7..];
        return is_blocked_host(ipv4_part);
    }
    false
}

/// Extract the hostname from an HTTP/HTTPS URL, matching the logic in
/// `src/sparql/federation/policy.rs::extract_host()`.
fn extract_host(url: &str) -> Option<String> {
    let after_scheme = url.split_once("://").map(|(_, rest)| rest)?;
    let authority = after_scheme.split('/').next().unwrap_or(after_scheme);
    let host_port = if let Some((_, hp)) = authority.split_once('@') {
        hp
    } else {
        authority
    };
    if host_port.starts_with('[') {
        return host_port
            .split_once(']')
            .map(|(h, _)| h.trim_start_matches('[').to_string());
    }
    let host = host_port.split(':').next().unwrap_or(host_port);
    if host.is_empty() { None } else { Some(host.to_string()) }
}

// ── NDJSON stream parser ──────────────────────────────────────────────────────
//
// Mirrors the minimal NDJSON parsing logic in the rule-library stream handler.

fn parse_ndjson_line(line: &str) -> Option<serde_json::Value> {
    serde_json::from_str(line).ok()
}

// ── Fuzz entry point ──────────────────────────────────────────────────────────

fuzz_target!(|data: &[u8]| {
    let text = match std::str::from_utf8(data) {
        Ok(s) => s,
        Err(_) => return,
    };

    // Invariant 1: SSRF blocker never panics for any URL string.
    if let Some(host) = extract_host(text) {
        let _blocked = is_blocked_host(&host);
    }
    // Also call is_blocked_host directly on the raw text (simulates
    // decimal-encoded or unusual inputs reaching the check).
    let _direct = is_blocked_host(text);

    // Invariant 2: NDJSON stream parser never panics for any line.
    for line in text.lines() {
        let _parsed = parse_ndjson_line(line);
    }
});
