//! Federation credential status HTTP handler (v0.126.0 FEAT-03).
//!
//! `GET /federation/{endpoint}/auth-status`
//!
//! Returns the credential age and auth type for a registered federation
//! endpoint. Requires write-level authentication. Never returns plaintext
//! tokens.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;

use super::sparql_handlers::json_response_http;
use crate::common::{AppState, check_auth_write, redacted_error};

fn json_response(status: StatusCode, body: serde_json::Value) -> Response {
    json_response_http(status, body)
}

/// `GET /federation/{endpoint}/auth-status`
///
/// Returns `{"endpoint_iri": "...", "auth_type": "...", "token_age_days": 0.0,
/// "last_used_at": "..." | null}` for the given endpoint.
///
/// `{endpoint}` is URL-encoded.  Returns 404 when no credential is registered.
pub(crate) async fn federation_auth_status(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(endpoint_encoded): Path<String>,
) -> Response {
    if let Err(r) = check_auth_write(&state, &headers) {
        return r;
    }

    // URL-decode the endpoint IRI from the path parameter.
    let endpoint_iri = percent_encoding::percent_decode_str(&endpoint_encoded)
        .decode_utf8()
        .map(|s| s.into_owned())
        .unwrap_or_else(|_| endpoint_encoded.clone());

    let client = match state.pool.get().await {
        Ok(c) => c,
        Err(e) => {
            return redacted_error(
                "db_pool_error",
                &e.to_string(),
                StatusCode::SERVICE_UNAVAILABLE,
            );
        }
    };

    let row = client
        .query_opt(
            "SELECT \
                 endpoint_iri, \
                 auth_type, \
                 EXTRACT(EPOCH FROM (now() - created_at)) / 86400.0 AS token_age_days, \
                 last_used_at::text \
             FROM _pg_ripple.federation_credentials \
             WHERE endpoint_iri = $1",
            &[&endpoint_iri],
        )
        .await;

    match row {
        Ok(Some(r)) => {
            let iri: String = r.try_get(0).unwrap_or_default();
            let auth_type: String = r.try_get(1).unwrap_or_default();
            let age: f64 = r.try_get(2).unwrap_or(0.0);
            let last_used: Option<String> = r.try_get(3).unwrap_or(None);
            json_response(
                StatusCode::OK,
                serde_json::json!({
                    "endpoint_iri": iri,
                    "auth_type": auth_type,
                    "token_age_days": age,
                    "last_used_at": last_used
                }),
            )
        }
        Ok(None) => json_response(
            StatusCode::NOT_FOUND,
            serde_json::json!({
                "error": format!("no credential registered for endpoint '{endpoint_iri}'")
            }),
        ),
        Err(e) => redacted_error(
            "federation_auth_status_error",
            &e.to_string(),
            StatusCode::INTERNAL_SERVER_ERROR,
        ),
    }
}
