//! Natural-language inference explanation HTTP handlers (v0.101.0).
//!
//! POST /explain  — generate a NL explanation of why a fact was derived
//! GET  /explain  — convenience alias (parameters via query string)

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use serde::Deserialize;

use super::sparql_handlers::json_response_http;
use crate::common::{AppState, check_auth, redacted_error};

fn json_response(status: StatusCode, body: serde_json::Value) -> Response {
    json_response_http(status, body)
}

// ─── Request / response types ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ExplainBody {
    pub subject: String,
    pub predicate: String,
    pub object: String,
    #[serde(default = "default_format")]
    pub format: String,
}

#[derive(Debug, Deserialize)]
pub struct ExplainQuery {
    pub subject: Option<String>,
    pub predicate: Option<String>,
    pub object: Option<String>,
    #[serde(default = "default_format")]
    pub format: String,
}

fn default_format() -> String {
    "text".to_owned()
}

// ─── POST /explain ────────────────────────────────────────────────────────────

pub(crate) async fn explain_post(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Body,
) -> Response {
    if let Err(r) = check_auth(&state, &headers) {
        return r;
    }

    let body_bytes = match axum::body::to_bytes(body, 1024 * 1024).await {
        Ok(b) => b,
        Err(_) => {
            return json_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                serde_json::json!({"error": "request_too_large", "detail": "request body too large"}),
            );
        }
    };

    let req: ExplainBody = match serde_json::from_slice(&body_bytes) {
        Ok(r) => r,
        Err(e) => {
            return redacted_error(
                "invalid_request",
                &format!("invalid JSON body: {e}"),
                StatusCode::BAD_REQUEST,
            );
        }
    };

    explain_inner(
        &state,
        &req.subject,
        &req.predicate,
        &req.object,
        &req.format,
    )
    .await
}

// ─── GET /explain ─────────────────────────────────────────────────────────────

pub(crate) async fn explain_get(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(params): Query<ExplainQuery>,
) -> Response {
    if let Err(r) = check_auth(&state, &headers) {
        return r;
    }

    let subject = match params.subject {
        Some(s) => s,
        None => {
            return json_response(
                StatusCode::BAD_REQUEST,
                serde_json::json!({"error": "missing_param", "detail": "subject is required"}),
            );
        }
    };
    let predicate = match params.predicate {
        Some(p) => p,
        None => {
            return json_response(
                StatusCode::BAD_REQUEST,
                serde_json::json!({"error": "missing_param", "detail": "predicate is required"}),
            );
        }
    };
    let object = match params.object {
        Some(o) => o,
        None => {
            return json_response(
                StatusCode::BAD_REQUEST,
                serde_json::json!({"error": "missing_param", "detail": "object is required"}),
            );
        }
    };

    explain_inner(&state, &subject, &predicate, &object, &params.format).await
}

// ─── Shared implementation ────────────────────────────────────────────────────

async fn explain_inner(
    state: &Arc<AppState>,
    subject: &str,
    predicate: &str,
    object: &str,
    format: &str,
) -> Response {
    let client = match state.pool.get().await {
        Ok(c) => c,
        Err(e) => {
            return redacted_error(
                "pool_error",
                &format!("connection pool error: {e}"),
                StatusCode::SERVICE_UNAVAILABLE,
            );
        }
    };

    let format_val = format.to_owned();
    let subject_val = subject.to_owned();
    let predicate_val = predicate.to_owned();
    let object_val = object.to_owned();

    let rows = match client
        .query(
            "SELECT pg_ripple.explain_inference($1, $2, $3, $4)",
            &[&subject_val, &predicate_val, &object_val, &format_val],
        )
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return redacted_error(
                "query_error",
                &format!("explain_inference error: {e}"),
                StatusCode::INTERNAL_SERVER_ERROR,
            );
        }
    };

    let explanation: Option<String> = rows.first().and_then(|row| row.get::<_, Option<String>>(0));

    json_response(
        StatusCode::OK,
        serde_json::json!({
            "explanation": explanation,
            "cached": false
        }),
    )
}
