//! Rule explainability REST handler — v0.110.0.
//!
//! Implements:
//! - `GET /rules/{id}/explain?language=en&format=text`

use std::sync::Arc;
use std::time::Instant;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use serde::Deserialize;

use crate::common::{AppState, check_auth, redacted_error};
use crate::routing::datalog_handlers::json_response;

#[derive(Deserialize)]
pub(crate) struct ExplainParams {
    language: Option<String>,
    format: Option<String>,
}

/// `GET /rules/{id}/explain`
///
/// Returns a plain-English explanation of the Datalog rule with the given
/// numeric ID, using `pg_ripple.explain_rule($1, $2, $3)`.
///
/// Query parameters:
/// - `language` — ISO language code (default `en`)
/// - `format`   — `text` (default) or `markdown`
///
/// Returns `{"rule_id": N, "language": "...", "format": "...", "explanation": "..."}`.
pub async fn explain_rule_get(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(params): Query<ExplainParams>,
) -> Response {
    if let Err(r) = check_auth(&state, &headers) {
        return r;
    }

    let language = params.language.as_deref().unwrap_or("en").to_owned();
    let format = params.format.as_deref().unwrap_or("text").to_owned();

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

    let row = match client
        .query_one(
            "SELECT pg_ripple.explain_rule($1, $2, $3)",
            &[&id, &language, &format],
        )
        .await
    {
        Ok(r) => r,
        Err(e) => {
            state.metrics.record_error();
            let msg = e.to_string();
            let status = if msg.to_lowercase().contains("pt0462")
                || msg.to_lowercase().contains("not found")
            {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            return redacted_error("explain_rule_error", &msg, status);
        }
    };

    let explanation: String = row.get(0);
    state.metrics.record_datalog_query(start.elapsed());
    json_response(
        StatusCode::OK,
        serde_json::json!({
            "rule_id":     id,
            "language":    language,
            "format":      format,
            "explanation": explanation,
        }),
    )
}
