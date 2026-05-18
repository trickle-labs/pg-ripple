//! Differential-privacy HTTP handlers (v0.115.0 M16-02, v0.118.0 Feature 2).
//!
//! POST /dp/noisy_count      — differentially-private count query
//! POST /dp/noisy_histogram  — differentially-private histogram query
//! GET  /dp/budget/{dataset}/{principal} — privacy budget status (v0.118.0)

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use serde::Deserialize;

use super::sparql_handlers::json_response_http;
use crate::common::{AppState, check_auth_write, redacted_error};

fn json_response(status: StatusCode, body: serde_json::Value) -> Response {
    json_response_http(status, body)
}

// ── Request types ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct NoisyCountBody {
    /// A SQL query whose result set will be noisy-counted.
    pub query: String,
    /// Differential privacy epsilon parameter (0 < epsilon ≤ 10).
    #[serde(default = "default_epsilon")]
    pub epsilon: f64,
}

#[derive(Debug, Deserialize)]
pub struct NoisyHistogramBody {
    pub query: String,
    pub key_column: String,
    pub count_column: String,
    #[serde(default = "default_epsilon")]
    pub epsilon: f64,
}

fn default_epsilon() -> f64 {
    1.0
}

// ── POST /dp/noisy_count ──────────────────────────────────────────────────────

/// Returns a differentially-private noisy count for the result set of the
/// given query.
pub(crate) async fn dp_noisy_count(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Body,
) -> Response {
    if let Err(r) = check_auth_write(&state, &headers) {
        return r;
    }
    let bytes = match axum::body::to_bytes(body, 1024 * 1024).await {
        Ok(b) => b,
        Err(_) => {
            return json_response(
                StatusCode::BAD_REQUEST,
                serde_json::json!({"error": "read_error"}),
            );
        }
    };
    let req: NoisyCountBody = match serde_json::from_slice(&bytes) {
        Ok(r) => r,
        Err(e) => {
            return json_response(
                StatusCode::BAD_REQUEST,
                serde_json::json!({"error": "invalid_json", "detail": format!("{e}")}),
            );
        }
    };
    if req.epsilon <= 0.0 || req.epsilon > 10.0 {
        return json_response(
            StatusCode::BAD_REQUEST,
            serde_json::json!({"error": "invalid_epsilon", "detail": "epsilon must be in (0, 10]"}),
        );
    }
    let client = match state.pool.get().await {
        Ok(c) => c,
        Err(e) => {
            state.metrics.record_error();
            return redacted_error(
                "db_pool_error",
                &e.to_string(),
                StatusCode::SERVICE_UNAVAILABLE,
            );
        }
    };
    let row = match client
        .query_one(
            "SELECT pg_ripple.dp_noisy_count($1, $2)",
            &[&req.query, &req.epsilon],
        )
        .await
    {
        Ok(r) => r,
        Err(e) => {
            state.metrics.record_error();
            return redacted_error(
                "dp_noisy_count_error",
                &e.to_string(),
                StatusCode::INTERNAL_SERVER_ERROR,
            );
        }
    };
    let noisy_count: i64 = row.get(0);
    json_response(
        StatusCode::OK,
        serde_json::json!({ "noisy_count": noisy_count, "epsilon": req.epsilon }),
    )
}

// ── POST /dp/noisy_histogram ──────────────────────────────────────────────────

/// Returns a differentially-private noisy histogram over the given query result.
pub(crate) async fn dp_noisy_histogram(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Body,
) -> Response {
    if let Err(r) = check_auth_write(&state, &headers) {
        return r;
    }
    let bytes = match axum::body::to_bytes(body, 1024 * 1024).await {
        Ok(b) => b,
        Err(_) => {
            return json_response(
                StatusCode::BAD_REQUEST,
                serde_json::json!({"error": "read_error"}),
            );
        }
    };
    let req: NoisyHistogramBody = match serde_json::from_slice(&bytes) {
        Ok(r) => r,
        Err(e) => {
            return json_response(
                StatusCode::BAD_REQUEST,
                serde_json::json!({"error": "invalid_json", "detail": format!("{e}")}),
            );
        }
    };
    if req.epsilon <= 0.0 || req.epsilon > 10.0 {
        return json_response(
            StatusCode::BAD_REQUEST,
            serde_json::json!({"error": "invalid_epsilon", "detail": "epsilon must be in (0, 10]"}),
        );
    }
    let client = match state.pool.get().await {
        Ok(c) => c,
        Err(e) => {
            state.metrics.record_error();
            return redacted_error(
                "db_pool_error",
                &e.to_string(),
                StatusCode::SERVICE_UNAVAILABLE,
            );
        }
    };
    let rows = match client
        .query(
            "SELECT key, noisy_count FROM pg_ripple.dp_noisy_histogram($1, $2, $3, $4)",
            &[&req.query, &req.key_column, &req.count_column, &req.epsilon],
        )
        .await
    {
        Ok(r) => r,
        Err(e) => {
            state.metrics.record_error();
            return redacted_error(
                "dp_noisy_histogram_error",
                &e.to_string(),
                StatusCode::INTERNAL_SERVER_ERROR,
            );
        }
    };
    let histogram: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| {
            serde_json::json!({
                "key": row.get::<_, String>(0),
                "noisy_count": row.get::<_, i64>(1),
            })
        })
        .collect();
    json_response(
        StatusCode::OK,
        serde_json::json!({ "histogram": histogram, "epsilon": req.epsilon }),
    )
}

// ── GET /dp/budget/{dataset}/{principal} ─────────────────────────────────────

/// Returns the current privacy budget status for a (dataset_id, principal) pair.
///
/// Response JSON:
/// ```json
/// {"dataset_id": 42, "principal": "alice",
///  "budget_total": 10.0, "budget_spent": 3.5,
///  "budget_remaining": 6.5, "last_reset_at": "..."}
/// ```
/// Returns 404 when no budget row exists for the given pair.
pub(crate) async fn dp_budget_get(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path((dataset, principal)): Path<(i64, String)>,
) -> Response {
    if let Err(r) = check_auth_write(&state, &headers) {
        return r;
    }
    let client = match state.pool.get().await {
        Ok(c) => c,
        Err(e) => {
            state.metrics.record_error();
            return redacted_error(
                "db_pool_error",
                &e.to_string(),
                StatusCode::SERVICE_UNAVAILABLE,
            );
        }
    };
    let row = match client
        .query_opt(
            "SELECT dataset_id, principal, budget_total, budget_spent, \
             to_char(last_reset_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS last_reset_at \
             FROM _pg_ripple.privacy_budget \
             WHERE dataset_id = $1 AND principal = $2",
            &[&dataset, &principal],
        )
        .await
    {
        Ok(Some(r)) => r,
        Ok(None) => {
            return json_response(
                StatusCode::NOT_FOUND,
                serde_json::json!({
                    "error": "not_found",
                    "detail": format!("no budget row for dataset_id={dataset} principal='{principal}'")
                }),
            );
        }
        Err(e) => {
            state.metrics.record_error();
            return redacted_error(
                "dp_budget_get_error",
                &e.to_string(),
                StatusCode::INTERNAL_SERVER_ERROR,
            );
        }
    };
    let dataset_id: i64 = row.get(0);
    let princ: String = row.get(1);
    let budget_total: f64 = row.get(2);
    let budget_spent: f64 = row.get(3);
    let last_reset_at: String = row.get(4);
    json_response(
        StatusCode::OK,
        serde_json::json!({
            "dataset_id": dataset_id,
            "principal": princ,
            "budget_total": budget_total,
            "budget_spent": budget_spent,
            "budget_remaining": (budget_total - budget_spent).max(0.0),
            "last_reset_at": last_reset_at,
        }),
    )
}
