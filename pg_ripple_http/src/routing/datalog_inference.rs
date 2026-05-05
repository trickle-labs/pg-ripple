//! Datalog inference HTTP handlers (M15-14, v0.96.0).
//! Moved from routing/datalog_handlers.rs lines 433-856.

use crate::common::{AppState, check_auth, redacted_error};
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use serde::Deserialize;
use std::sync::Arc;
use std::time::Instant;

use super::datalog_handlers::{
    classify_pg_error, classify_pg_goal_error, json_response, read_body,
};

// ─── Phase 2 — Inference ──────────────────────────────────────────────────────

/// `POST /datalog/infer/{rule_set}`
///
/// Calls `pg_ripple.infer($1)`. Returns `{"derived": N}`.
pub async fn infer(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(rule_set): Path<String>,
) -> Response {
    if let Err(r) = check_auth(&state, &headers) {
        return r;
    }

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
        .query_one("SELECT pg_ripple.infer($1)", &[&rule_set])
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

    let derived: i64 = row.get(0);
    state.metrics.record_datalog_query(start.elapsed());
    json_response(StatusCode::OK, serde_json::json!({"derived": derived}))
}

/// `POST /datalog/infer/{rule_set}/stats`
///
/// Calls `pg_ripple.infer_with_stats($1)`. Returns full stats JSONB.
pub async fn infer_with_stats(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(rule_set): Path<String>,
) -> Response {
    if let Err(r) = check_auth(&state, &headers) {
        return r;
    }

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
        .query_one("SELECT pg_ripple.infer_with_stats($1)", &[&rule_set])
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

/// `POST /datalog/infer/{rule_set}/agg`
///
/// Calls `pg_ripple.infer_agg($1)`. Returns `{"derived": N}`.
pub async fn infer_agg(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(rule_set): Path<String>,
) -> Response {
    if let Err(r) = check_auth(&state, &headers) {
        return r;
    }

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
        .query_one("SELECT pg_ripple.infer_agg($1)", &[&rule_set])
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

    let derived: i64 = row.get(0);
    state.metrics.record_datalog_query(start.elapsed());
    json_response(StatusCode::OK, serde_json::json!({"derived": derived}))
}

/// `POST /datalog/infer/{rule_set}/wfs`
///
/// Calls `pg_ripple.infer_wfs($1)`. Returns `{"derived": N}`.
pub async fn infer_wfs(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(rule_set): Path<String>,
) -> Response {
    if let Err(r) = check_auth(&state, &headers) {
        return r;
    }

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
        .query_one("SELECT pg_ripple.infer_wfs($1)", &[&rule_set])
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

    let derived: i64 = row.get(0);
    state.metrics.record_datalog_query(start.elapsed());
    json_response(StatusCode::OK, serde_json::json!({"derived": derived}))
}

/// `POST /datalog/infer/{rule_set}/demand`
///
/// Body: `{"demands": […]}` JSON. Calls `pg_ripple.infer_demand($1, $2::jsonb)`.
/// Returns full JSONB stats from the extension.
pub async fn infer_demand(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(rule_set): Path<String>,
    body: Body,
) -> Response {
    if let Err(r) = check_auth(&state, &headers) {
        return r;
    }
    let body_str = match read_body(body).await {
        Ok(t) => t,
        Err(r) => return r,
    };
    // Validate that the body is valid JSON.
    let demands_json: serde_json::Value = match serde_json::from_str(&body_str) {
        Ok(v) => v,
        Err(e) => {
            return json_response(
                StatusCode::BAD_REQUEST,
                serde_json::json!({"error": "invalid_request", "detail": format!("invalid JSON body: {e}")}),
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

    let row = match client
        .query_one(
            "SELECT pg_ripple.infer_demand($1, $2::jsonb)",
            &[&rule_set, &demands_json],
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

/// `POST /datalog/infer/{rule_set}/lattice`
///
/// Body: `{"lattice": "min"}` JSON. Calls `pg_ripple.infer_lattice($1, $2)`.
/// Returns `{"derived": N}`.
#[derive(Deserialize)]
pub struct LatticeBody {
    pub lattice: String,
}

pub async fn infer_lattice(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(rule_set): Path<String>,
    body: Body,
) -> Response {
    if let Err(r) = check_auth(&state, &headers) {
        return r;
    }
    let body_str = match read_body(body).await {
        Ok(t) => t,
        Err(r) => return r,
    };
    let req: LatticeBody = match serde_json::from_str(&body_str) {
        Ok(v) => v,
        Err(e) => {
            return json_response(
                StatusCode::BAD_REQUEST,
                serde_json::json!({"error": "invalid_request", "detail": format!("expected {{\"lattice\": \"…\"}}: {e}")}),
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

    let row = match client
        .query_one(
            "SELECT pg_ripple.infer_lattice($1, $2)",
            &[&rule_set, &req.lattice],
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

    let derived: i64 = row.get(0);
    state.metrics.record_datalog_query(start.elapsed());
    json_response(StatusCode::OK, serde_json::json!({"derived": derived}))
}

// ─── Phase 3 — Query & constraints ────────────────────────────────────────────

/// `POST /datalog/query/{rule_set}`
///
/// Body: Datalog goal text (`text/x-datalog` or `text/plain`).
/// Calls `pg_ripple.infer_goal($1, $2)`.
/// Returns `{"derived": N, "iterations": N, "matching": […]}`.
pub async fn query_goal(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(rule_set): Path<String>,
    body: Body,
) -> Response {
    if let Err(r) = check_auth(&state, &headers) {
        return r;
    }
    let goal_text = match read_body(body).await {
        Ok(t) => t,
        Err(r) => return r,
    };
    if goal_text.trim().is_empty() {
        return json_response(
            StatusCode::BAD_REQUEST,
            serde_json::json!({"error": "invalid_request", "detail": "empty goal body"}),
        );
    }

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
            "SELECT pg_ripple.infer_goal($1, $2)",
            &[&rule_set, &goal_text],
        )
        .await
    {
        Ok(r) => r,
        Err(e) => {
            state.metrics.record_error();
            let msg = e.to_string();
            let (cat, status) = classify_pg_goal_error(&msg);
            return redacted_error(cat, &msg, status);
        }
    };

    let result: serde_json::Value = row.get(0);
    state.metrics.record_datalog_query(start.elapsed());
    json_response(StatusCode::OK, result)
}

/// `GET /datalog/constraints`
///
/// Calls `pg_ripple.check_constraints(NULL)`. Returns violation array.
pub async fn check_constraints_all(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Response {
    if let Err(r) = check_auth(&state, &headers) {
        return r;
    }
    check_constraints_inner(&state, None).await
}

/// `GET /datalog/constraints/{rule_set}`
///
/// Calls `pg_ripple.check_constraints($1)`. Returns violation array.
pub async fn check_constraints(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(rule_set): Path<String>,
) -> Response {
    if let Err(r) = check_auth(&state, &headers) {
        return r;
    }
    check_constraints_inner(&state, Some(&rule_set)).await
}

async fn check_constraints_inner(state: &AppState, rule_set: Option<&str>) -> Response {
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
        .query_one("SELECT pg_ripple.check_constraints($1)", &[&rule_set])
        .await
    {
        Ok(r) => r,
        Err(e) => {
            state.metrics.record_error();
            return redacted_error(
                "datalog_error",
                &format!("check_constraints failed: {e}"),
                StatusCode::INTERNAL_SERVER_ERROR,
            );
        }
    };

    let result: serde_json::Value = row.get(0);
    state.metrics.record_datalog_query(start.elapsed());
    json_response(StatusCode::OK, result)
}
