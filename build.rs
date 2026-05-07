/// Parse a version string from `.versions.toml` for the given key.
///
/// Handles lines of the form: `key = "value"` (with optional surrounding whitespace).
/// Returns `None` if the key is not found or the line is malformed.
fn parse_versions_toml(contents: &str, key: &str) -> Option<String> {
    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with(key) {
            if let Some(rest) = line.split_once('=') {
                let val = rest.1.trim().trim_matches('"');
                if !val.is_empty() {
                    return Some(val.to_string());
                }
            }
        }
    }
    None
}

fn main() {
    // pgrx macros (pg_shmem_init!, etc.) emit cfg(feature = "pgNN") checks for
    // all supported PostgreSQL versions.  We only enable pg18, but Rust 2024's
    // check-cfg linting requires the other values to be declared as expected.
    for ver in ["pg13", "pg14", "pg15", "pg16", "pg17"] {
        println!("cargo::rustc-check-cfg=cfg(feature, values(\"{ver}\"))");
    }

    // DEP-VER-BUILD-01: Read .versions.toml and emit cargo:rustc-env variables
    // for each tracked extension version.  This makes the compile-time constants
    // in src/lib.rs (PG_TRICKLE_TESTED_VERSION, PG_TIDE_TESTED_VERSION) the
    // single source of truth — they cannot drift from .versions.toml because
    // they are injected at compile time rather than written by hand.
    //
    // Rebuild whenever .versions.toml changes.
    println!("cargo::rerun-if-changed=.versions.toml");
    let versions_path = std::path::Path::new(
        &std::env::var("CARGO_MANIFEST_DIR").expect("build.rs: CARGO_MANIFEST_DIR not set"),
    )
    .join(".versions.toml");
    let versions_contents = std::fs::read_to_string(&versions_path)
        .unwrap_or_else(|e| panic!("build.rs: cannot read .versions.toml: {e}"));
    for key in ["pg_trickle", "pg_tide"] {
        let val = parse_versions_toml(&versions_contents, key)
            .unwrap_or_else(|| panic!("build.rs: key '{key}' not found in .versions.toml"));
        // "pg_trickle" → "PG_TRICKLE_TESTED_VERSION", "pg_tide" → "PG_TIDE_TESTED_VERSION"
        let suffix = key.strip_prefix("pg_").unwrap_or(key).to_uppercase();
        let env_key = format!("PG_{suffix}_TESTED_VERSION");
        println!("cargo::rustc-env={env_key}={val}");
    }
    // BUILD-TIME-FIELD-01 (v0.83.0): emit an RFC-3339 build timestamp so that
    // the /health endpoint can report a real timestamp instead of the package
    // version string.  `cargo:rerun-if-env-changed` is omitted intentionally:
    // we want a fresh timestamp on every build.
    let ts = std::env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .map(|epoch| {
            // Minimal RFC-3339 formatter without external deps.
            let secs = epoch;
            let days = secs / 86400;
            let rem = secs % 86400;
            let h = rem / 3600;
            let m = (rem % 3600) / 60;
            let s = rem % 60;
            // Convert days since epoch to Y-M-D (Gregorian proleptic)
            let z = days + 719468;
            let era = z / 146097;
            let doe = z % 146097;
            let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
            let y = yoe + era * 400;
            let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
            let mp = (5 * doy + 2) / 153;
            let d = doy - (153 * mp + 2) / 5 + 1;
            let mo = if mp < 10 { mp + 3 } else { mp - 9 };
            let y = if mo <= 2 { y + 1 } else { y };
            format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
        })
        .unwrap_or_else(|| {
            // Fallback: use the Cargo package version string with a note.
            format!("build-version={}", env!("CARGO_PKG_VERSION"))
        });
    println!("cargo::rustc-env=BUILD_TIMESTAMP={ts}");
}
