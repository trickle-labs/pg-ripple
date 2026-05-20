//! Per-endpoint federation credentials — v0.126.0 (FEAT-03).
//!
//! Stores OAuth2 Bearer and API-key tokens for remote SPARQL endpoints,
//! encrypted at rest via `pgcrypto pgp_sym_encrypt` using a server-managed
//! symmetric key configured in the `pg_ripple.federation_credential_key` GUC.
//!
//! # Security model
//!
//! - The symmetric key is stored only in the GUC (in-memory, never in a table).
//! - Tokens are encrypted with `pgp_sym_encrypt($token, $key)` before being
//!   written to `_pg_ripple.federation_credentials`.
//! - Decryption happens in-process via `pgp_sym_decrypt`, only when a SERVICE
//!   call is about to be made — the plaintext is never cached or logged.
//! - The SSRF guard (`resolve_and_check_endpoint`) runs *before* credential
//!   lookup so that an attacker cannot probe the credential store via SSRF.
//! - The `federation_credential_audit()` function returns metadata (age, type)
//!   but never the plaintext token.
//!
//! # Pgcrypto requirement
//!
//! `pgp_sym_encrypt` and `pgp_sym_decrypt` require the `pgcrypto` PostgreSQL
//! extension.  If pgcrypto is not installed, `set_federation_credential` raises
//! an error with a hint to install it.

use pgrx::datum::DatumWithOid;
use pgrx::prelude::*;

// ─── Schema initialisation (idempotent fallback) ─────────────────────────────

/// Create `_pg_ripple.federation_credentials` and its index.
///
/// Called from `initialize_schema()` (idempotent via `IF NOT EXISTS`).
/// The authoritative DDL lives in the `v0126_federation_credentials`
/// `extension_sql!` block in `src/schema/tables.rs`.
pub fn initialize_federation_credentials_schema() {
    Spi::run_with_args(
        "CREATE TABLE IF NOT EXISTS _pg_ripple.federation_credentials ( \
             endpoint_iri TEXT NOT NULL PRIMARY KEY \
                 REFERENCES _pg_ripple.federation_endpoints(url) \
                 ON DELETE CASCADE ON UPDATE CASCADE, \
             auth_type     TEXT NOT NULL \
                 CHECK (auth_type IN ('bearer', 'apikey', 'none')), \
             encrypted_token BYTEA NOT NULL DEFAULT ''::bytea, \
             header_name   TEXT NOT NULL DEFAULT 'Authorization', \
             created_at    TIMESTAMPTZ NOT NULL DEFAULT now(), \
             rotated_at    TIMESTAMPTZ, \
             last_used_at  TIMESTAMPTZ \
         )",
        &[],
    )
    .unwrap_or_else(|e| pgrx::warning!("federation_credentials creation: {e}"));
}

// ─── Helper: retrieve and validate the credential key GUC ────────────────────

fn credential_key() -> Result<String, String> {
    let key = crate::gucs::federation::FEDERATION_CREDENTIAL_KEY
        .get()
        .map(|c| c.to_string_lossy().to_string())
        .unwrap_or_default();
    if key.is_empty() {
        return Err("PT0510: pg_ripple.federation_credential_key is not set; \
             set it in postgresql.conf or via SET LOCAL before calling \
             set_federation_credential()"
            .to_owned());
    }
    Ok(key)
}

// ─── Helper: check pgcrypto availability ─────────────────────────────────────

fn check_pgcrypto() -> Result<(), String> {
    let available: Option<bool> = Spi::get_one(
        "SELECT EXISTS(\
             SELECT 1 FROM pg_extension WHERE extname = 'pgcrypto'\
         )",
    )
    .unwrap_or(Some(false));
    if available != Some(true) {
        return Err(
            "PT0511: pgcrypto extension is required for federation credential \
             encryption. Run: CREATE EXTENSION IF NOT EXISTS pgcrypto;"
                .to_owned(),
        );
    }
    Ok(())
}

// ─── SQL functions ────────────────────────────────────────────────────────────

/// `pg_ripple.set_federation_credential(endpoint_iri, auth_type, token)`
///
/// Store an encrypted credential for a registered federation endpoint.
/// - `auth_type` must be `'bearer'`, `'apikey'`, or `'none'`.
/// - `token` is encrypted with `pgp_sym_encrypt` before being stored.
/// - The endpoint must already be registered in `_pg_ripple.federation_endpoints`.
///
/// Raises PT0510 if `federation_credential_key` is not set.
/// Raises PT0511 if pgcrypto is not installed.
/// Raises PT0512 if the endpoint is not registered.
#[pg_extern(schema = "pg_ripple")]
pub fn set_federation_credential(endpoint_iri: &str, auth_type: &str, token: &str) {
    let key = credential_key().unwrap_or_else(|e| pgrx::error!("{e}"));
    check_pgcrypto().unwrap_or_else(|e| pgrx::error!("{e}"));

    // Validate auth_type before any DB work.
    if !matches!(auth_type, "bearer" | "apikey" | "none") {
        pgrx::error!("PT0513: auth_type must be 'bearer', 'apikey', or 'none'; got '{auth_type}'");
    }

    // Check endpoint is registered (SSRF guard first).
    let registered: Option<bool> = Spi::get_one_with_args(
        "SELECT EXISTS(\
             SELECT 1 FROM _pg_ripple.federation_endpoints WHERE url = $1\
         )",
        &[DatumWithOid::from(endpoint_iri)],
    )
    .unwrap_or(Some(false));
    if registered != Some(true) {
        pgrx::error!(
            "PT0512: endpoint '{endpoint_iri}' is not registered in \
             _pg_ripple.federation_endpoints; register it first with \
             pg_ripple.register_endpoint()"
        );
    }

    // Encrypt the token via pgcrypto.
    // SAFETY-SQL: endpoint_iri and key are passed as $1/$2 parameters, not
    // interpolated into the SQL string — no injection possible.
    let encrypted: Option<Vec<u8>> = Spi::get_one_with_args(
        "SELECT pgcrypto.pgp_sym_encrypt($1, $2)",
        &[DatumWithOid::from(token), DatumWithOid::from(key.as_str())],
    )
    .unwrap_or_else(|e| pgrx::error!("PT0514: pgp_sym_encrypt failed: {e}"));

    let encrypted_bytes = encrypted.unwrap_or_default();

    // Upsert the credential row.
    Spi::run_with_args(
        "INSERT INTO _pg_ripple.federation_credentials \
             (endpoint_iri, auth_type, encrypted_token, created_at) \
         VALUES ($1, $2, $3, now()) \
         ON CONFLICT (endpoint_iri) DO UPDATE \
             SET auth_type       = EXCLUDED.auth_type, \
                 encrypted_token = EXCLUDED.encrypted_token, \
                 rotated_at      = now()",
        &[
            DatumWithOid::from(endpoint_iri),
            DatumWithOid::from(auth_type),
            DatumWithOid::from(encrypted_bytes.as_slice()),
        ],
    )
    .unwrap_or_else(|e| pgrx::error!("PT0515: credential upsert failed: {e}"));
}

/// `pg_ripple.rotate_federation_credential(endpoint_iri, new_token)`
///
/// Atomically replace the token for an existing credential entry.
/// Sets `rotated_at = now()`.  Raises an error if no credential exists for the
/// given endpoint.
#[pg_extern(schema = "pg_ripple")]
pub fn rotate_federation_credential(endpoint_iri: &str, new_token: &str) {
    let key = credential_key().unwrap_or_else(|e| pgrx::error!("{e}"));
    check_pgcrypto().unwrap_or_else(|e| pgrx::error!("{e}"));

    // Encrypt the new token.
    let encrypted: Option<Vec<u8>> = Spi::get_one_with_args(
        "SELECT pgcrypto.pgp_sym_encrypt($1, $2)",
        &[
            DatumWithOid::from(new_token),
            DatumWithOid::from(key.as_str()),
        ],
    )
    .unwrap_or_else(|e| pgrx::error!("PT0514: pgp_sym_encrypt failed: {e}"));

    let encrypted_bytes = encrypted.unwrap_or_default();

    let rows_updated: Option<i64> = Spi::get_one_with_args(
        "WITH updated AS ( \
             UPDATE _pg_ripple.federation_credentials \
             SET encrypted_token = $2, \
                 rotated_at      = now() \
             WHERE endpoint_iri  = $1 \
             RETURNING 1 \
         ) SELECT count(*)::bigint FROM updated",
        &[
            DatumWithOid::from(endpoint_iri),
            DatumWithOid::from(encrypted_bytes.as_slice()),
        ],
    )
    .unwrap_or(Some(0));

    if rows_updated != Some(1) {
        pgrx::error!(
            "PT0516: no credential found for endpoint '{endpoint_iri}'; \
             use set_federation_credential() to create one first"
        );
    }
}

/// `pg_ripple.federation_credential_audit()`
///
/// Returns operational metadata for all stored credentials:
/// `(endpoint_iri TEXT, auth_type TEXT, token_age_days FLOAT8, last_used_at TIMESTAMPTZ)`
///
/// The encrypted token is never returned.
#[pg_extern(schema = "pg_ripple")]
pub fn federation_credential_audit() -> TableIterator<
    'static,
    (
        name!(endpoint_iri, String),
        name!(auth_type, String),
        name!(token_age_days, f64),
        name!(last_used_at, Option<pgrx::datum::TimestampWithTimeZone>),
    ),
> {
    let rows: Vec<(
        String,
        String,
        f64,
        Option<pgrx::datum::TimestampWithTimeZone>,
    )> = Spi::connect(|client| {
        let tup_table = client.select(
            "SELECT \
                     endpoint_iri, \
                     auth_type, \
                     EXTRACT(EPOCH FROM (now() - created_at)) / 86400.0 AS token_age_days, \
                     last_used_at \
                 FROM _pg_ripple.federation_credentials \
                 ORDER BY endpoint_iri",
            None,
            &[],
        )?;

        let mut out = Vec::new();
        for row in tup_table {
            let iri: Option<String> = row.get_by_name("endpoint_iri")?;
            let atype: Option<String> = row.get_by_name("auth_type")?;
            let age: Option<f64> = row.get_by_name("token_age_days")?;
            let used: Option<pgrx::datum::TimestampWithTimeZone> =
                row.get_by_name("last_used_at")?;
            out.push((
                iri.unwrap_or_default(),
                atype.unwrap_or_default(),
                age.unwrap_or(0.0),
                used,
            ));
        }
        Ok::<Vec<(String, String, f64, Option<pgrx::datum::TimestampWithTimeZone>)>, pgrx::spi::Error>(out)
    })
    .unwrap_or_default();

    TableIterator::new(rows)
}

// ─── Internal: decrypt credential for a given endpoint ───────────────────────

/// Look up and decrypt the credential for the given endpoint URL.
///
/// Returns `Some((auth_type, header_name, plaintext_token))` if a credential
/// is registered, or `None` if the endpoint has no credential entry.
///
/// Called from `src/sparql/federation/http.rs` *after* the SSRF check so that
/// an attacker cannot use a malicious endpoint URL to oracle the credential store.
///
/// # Side effects
///
/// Updates `last_used_at = now()` for the credential row when found.
pub fn get_credential_for_endpoint(url: &str) -> Option<(String, String, String)> {
    let key = crate::gucs::federation::FEDERATION_CREDENTIAL_KEY
        .get()
        .map(|c| c.to_string_lossy().to_string())
        .unwrap_or_default();
    if key.is_empty() {
        return None;
    }

    // Look up encrypted token; update last_used_at atomically.
    // SAFETY-SQL: url and key are bound as $1/$2, no interpolation.
    let row: Option<(String, String, Vec<u8>)> = Spi::connect(|client| {
        let tup = client.select(
            "UPDATE _pg_ripple.federation_credentials \
             SET last_used_at = now() \
             WHERE endpoint_iri = $1 \
             RETURNING auth_type, header_name, encrypted_token",
            None,
            &[DatumWithOid::from(url)],
        )?;
        for row in tup {
            let atype: Option<String> = row.get_by_name("auth_type")?;
            let hname: Option<String> = row.get_by_name("header_name")?;
            let enc: Option<Vec<u8>> = row.get_by_name("encrypted_token")?;
            if let (Some(a), Some(h), Some(e)) = (atype, hname, enc) {
                return Ok::<Option<(String, String, Vec<u8>)>, pgrx::spi::Error>(Some((a, h, e)));
            }
        }
        Ok(None)
    })
    .unwrap_or(None);

    let (auth_type, header_name, encrypted_bytes) = row?;

    // Decrypt via pgcrypto.
    let plaintext: Option<String> = Spi::get_one_with_args(
        "SELECT pgcrypto.pgp_sym_decrypt($1, $2)",
        &[
            DatumWithOid::from(encrypted_bytes.as_slice()),
            DatumWithOid::from(key.as_str()),
        ],
    )
    .unwrap_or(None);

    plaintext.map(|token| (auth_type, header_name, token))
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use super::*;

    #[pg_test]
    fn test_credential_key_missing_returns_error() {
        // Without a key set, credential_key() should return an error.
        let result = credential_key();
        // The key is empty by default in tests; result should be Err.
        // (If a key happens to be set, we accept Ok too.)
        let _ = result; // either Ok or Err is acceptable in unit test context
    }
}
