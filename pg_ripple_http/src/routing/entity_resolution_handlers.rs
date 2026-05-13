//! Entity-resolution HTTP handlers (v0.115.0 M16-02).
//!
//! POST /entity-resolution/resolve              — run the NS-RL pipeline
//! POST /entity-resolution/evaluate             — evaluate resolution quality
//! POST /entity-resolution/monitoring/enable    — enable ER monitoring
//! POST /entity-resolution/monitoring/disable   — disable ER monitoring

use std::sync::Arc;
use std::time::Instant;

use axum::body::Body;
use axum::extract::State;
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
pub struct ResolveEntitiesBody {
    pub source_graph: String,
    pub target_graph: String,
    /// Optional JSON options forwarded to `resolve_entities()`.
    pub options: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct EvaluateResolutionBody {
    pub gold_graph: String,
    pub pipeline_options: Option<serde_json::Value>,
}

// ── POST /entity-resolution/resolve ──────────────────────────────────────────

/// Run the NS-RL five-stage entity-resolution pipeline and return a summary
/// of `owl:sameAs` assertions created.
pub(crate) async fn entity_resolution_resolve(
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
    let req: ResolveEntitiesBody = match serde_json::from_slice(&bytes) {
        Ok(r) => r,
        Err(e) => {
            return json_response(
                StatusCode::BAD_REQUEST,
                serde_json::json!({"error": "invalid_json", "detail": format!("{e}")}),
            );
        }
    };
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
    let start = Instant::now();
    let options_json = req
        .options
        .map(|v| v.to_string())
        .unwrap_or_else(|| "null".to_owned());
    let row = match client
        .query_one(
            "SELECT pg_ripple.resolve_entities($1, $2, $3::json)",
            &[&req.source_graph, &req.target_graph, &options_json],
        )
        .await
    {
        Ok(r) => r,
        Err(e) => {
            state.metrics.record_error();
            return redacted_error(
                "entity_resolution_error",
                &e.to_string(),
                StatusCode::INTERNAL_SERVER_ERROR,
            );
        }
    };
    let elapsed = start.elapsed();
    // M16-03: record owl:sameAs assertion metrics.
    state
        .metrics
        .record_er_stage_duration("canonicalization", elapsed);
    let result: serde_json::Value = row.get::<_, serde_json::Value>(0);
    json_response(StatusCode::OK, result)
}

// ── POST /entity-resolution/evaluate ─────────────────────────────────────────

/// Evaluate entity-resolution quality against a gold-standard graph.
pub(crate) async fn entity_resolution_evaluate(
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
    let req: EvaluateResolutionBody = match serde_json::from_slice(&bytes) {
        Ok(r) => r,
        Err(e) => {
            return json_response(
                StatusCode::BAD_REQUEST,
                serde_json::json!({"error": "invalid_json", "detail": format!("{e}")}),
            );
        }
    };
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
    let opts_json = req
        .pipeline_options
        .map(|v| v.to_string())
        .unwrap_or_else(|| "{}".to_owned());
    let row = match client
        .query_one(
            "SELECT pg_ripple.evaluate_resolution($1, $2::jsonb)",
            &[&req.gold_graph, &opts_json],
        )
        .await
    {
        Ok(r) => r,
        Err(e) => {
            state.metrics.record_error();
            return redacted_error(
                "evaluate_resolution_error",
                &e.to_string(),
                StatusCode::INTERNAL_SERVER_ERROR,
            );
        }
    };
    let result: serde_json::Value = row.get::<_, serde_json::Value>(0);
    json_response(StatusCode::OK, result)
}

// ── POST /entity-resolution/monitoring/enable ─────────────────────────────────

/// Enable ER monitoring (populates `er_unresolved_entities`, `er_cluster_sizes`).
pub(crate) async fn entity_resolution_monitoring_enable(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
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
    match client
        .execute("SELECT pg_ripple.enable_er_monitoring()", &[])
        .await
    {
        Ok(_) => json_response(
            StatusCode::OK,
            serde_json::json!({"status": "monitoring_enabled"}),
        ),
        Err(e) => {
            state.metrics.record_error();
            redacted_error(
                "er_monitoring_error",
                &e.to_string(),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        }
    }
}

// ── POST /entity-resolution/monitoring/disable ────────────────────────────────

/// Disable ER monitoring.
pub(crate) async fn entity_resolution_monitoring_disable(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
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
    match client
        .execute("SELECT pg_ripple.disable_er_monitoring()", &[])
        .await
    {
        Ok(_) => json_response(
            StatusCode::OK,
            serde_json::json!({"status": "monitoring_disabled"}),
        ),
        Err(e) => {
            state.metrics.record_error();
            redacted_error(
                "er_monitoring_error",
                &e.to_string(),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        }
    }
}
