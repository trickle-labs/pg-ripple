//! Hypothetical inference HTTP handler (v0.102.0).
//!
//! `POST /hypothetical` — runs what-if inference on a set of hypothetical
//! facts and returns a JSONB diff without touching real VP tables.

use crate::common::{AppState, check_auth, redacted_error};
use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use serde::Deserialize;
use std::sync::Arc;
use std::time::Instant;

use super::datalog_handlers::{classify_pg_error, json_response, read_body};

// ─── Request / Response types ─────────────────────────────────────────────────

/// Request body for `POST /hypothetical`.
#[derive(Deserialize)]
pub(crate) struct HypotheticalRequest {
    /// Hypothetical facts to assert and/or retract.
    pub hypotheses: serde_json::Value,
    /// Name of the Datalog rule set to evaluate. Defaults to `"default"`.
    #[serde(default = "default_rules")]
    pub rules: String,
}

fn default_rules() -> String {
    "default".to_owned()
}

// ─── Handler ──────────────────────────────────────────────────────────────────

/// `POST /hypothetical`
///
/// Request body (JSON):
/// ```json
/// {
///   "hypotheses": {
///     "assert":  [{"s": "<iri>", "p": "<iri>", "o": "<iri-or-literal>"}],
///     "retract": [{"s": "<iri>", "p": "<iri>", "o": "<iri-or-literal>"}]
///   },
///   "rules": "default"
/// }
/// ```
///
/// Calls `pg_ripple.hypothetical_inference($hypotheses, $rules)` and returns
/// the JSONB diff `{"derived": [...], "retracted": [...]}`.
pub async fn hypothetical_post(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Body,
) -> Response {
    if let Err(r) = check_auth(&state, &headers) {
        return r;
    }

    let body_bytes = match read_body(body).await {
        Ok(b) => b,
        Err(r) => return r,
    };

    let req: HypotheticalRequest = match serde_json::from_str(&body_bytes) {
        Ok(r) => r,
        Err(e) => {
            return redacted_error(
                "bad_request",
                &format!("invalid JSON body: {e}"),
                StatusCode::BAD_REQUEST,
            );
        }
    };

    let start = Instant::now();
    let client = match state.pool.get().await {
        Ok(c) => c,
        Err(e) => {
            state.metrics.record_error();
            return redacted_error(
                "service_unavailable",
                &format!("pool error: {e}"),
                StatusCode::SERVICE_UNAVAILABLE,
            );
        }
    };

    let hypotheses_str = req.hypotheses.to_string();
    let rules = req.rules.clone();

    let row = match client
        .query_one(
            "SELECT pg_ripple.hypothetical_inference($1::jsonb, $2)",
            &[&hypotheses_str, &rules],
        )
        .await
    {
        Ok(r) => r,
        Err(e) => {
            state.metrics.record_error();
            let msg = e.to_string();
            let (cat, status) = classify_pg_error(&msg);
            return redacted_error(cat, &msg, status);
        }
    };

    let result: serde_json::Value = row.get(0);
    state.metrics.record_datalog_query(start.elapsed());
    json_response(StatusCode::OK, result)
}
