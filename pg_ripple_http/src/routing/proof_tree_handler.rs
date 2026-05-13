//! Proof-tree HTTP handler (v0.115.0 M16-02).
//!
//! GET /proof-tree/:subject/:predicate/:object — backward-chaining proof tree

use std::sync::Arc;
use std::time::Instant;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use percent_encoding::percent_decode_str;

use super::sparql_handlers::json_response_http;
use crate::common::{AppState, check_auth, redacted_error};

fn json_response(status: StatusCode, body: serde_json::Value) -> Response {
    json_response_http(status, body)
}

// ── GET /proof-tree/:subject/:predicate/:object ───────────────────────────────

/// Return the backward-chaining proof tree for a Datalog-derived triple.
///
/// Path parameters are percent-decoded. Returns `null` for base facts or triples
/// with no recorded derivation provenance.
///
/// Requires `pg_ripple.record_derivations = on` to have been set before the
/// `infer()` call that produced the fact.
pub(crate) async fn proof_tree_get(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path((subject, predicate, object)): Path<(String, String, String)>,
) -> Response {
    if let Err(r) = check_auth(&state, &headers) {
        return r;
    }

    // Percent-decode path segments so clients can pass IRIs via URL encoding.
    let subject = percent_decode_str(&subject).decode_utf8_lossy().into_owned();
    let predicate = percent_decode_str(&predicate).decode_utf8_lossy().into_owned();
    let object = percent_decode_str(&object).decode_utf8_lossy().into_owned();

    let client = match state.pool.get().await {
        Ok(c) => c,
        Err(e) => {
            state.metrics.record_error();
            return redacted_error("db_pool_error", &e.to_string(), StatusCode::SERVICE_UNAVAILABLE);
        }
    };

    let start = Instant::now();
    let row = match client
        .query_opt(
            "SELECT pg_ripple.justify($1, $2, $3)",
            &[&subject, &predicate, &object],
        )
        .await
    {
        Ok(r) => r,
        Err(e) => {
            state.metrics.record_error();
            return redacted_error(
                "proof_tree_error",
                &e.to_string(),
                StatusCode::INTERNAL_SERVER_ERROR,
            );
        }
    };

    let elapsed = start.elapsed();
    // M16-03: record proof-tree generation latency.
    state.metrics.record_proof_tree_duration(elapsed);

    let tree: serde_json::Value = row
        .and_then(|r| r.get::<_, Option<serde_json::Value>>(0))
        .unwrap_or(serde_json::Value::Null);

    json_response(
        StatusCode::OK,
        serde_json::json!({
            "subject":   subject,
            "predicate": predicate,
            "object":    object,
            "proof_tree": tree,
        }),
    )
}
