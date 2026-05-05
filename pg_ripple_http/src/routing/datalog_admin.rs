//! Datalog admin & monitoring HTTP handlers (M15-14, v0.96.0).
//! Moved from routing/datalog_handlers.rs lines 857-1232.

use crate::common::{AppState, check_auth, check_auth_write, redacted_error};
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use serde::Deserialize;
use std::sync::Arc;
use std::time::Instant;

use super::datalog_handlers::{classify_pg_error, json_response, read_body}; // functions defined in handlers (pub(crate))

// ─── Phase 4 — Admin & monitoring ─────────────────────────────────────────────

/// `GET /datalog/stats/cache`
///
/// Calls `pg_ripple.rule_plan_cache_stats()`.
pub async fn cache_stats(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
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
        .query_one("SELECT pg_ripple.rule_plan_cache_stats()", &[])
        .await
    {
        Ok(r) => r,
        Err(e) => {
            state.metrics.record_error();
            return redacted_error(
                "datalog_error",
                &format!("rule_plan_cache_stats failed: {e}"),
                StatusCode::INTERNAL_SERVER_ERROR,
            );
        }
    };

    let result: serde_json::Value = row.get(0);
    state.metrics.record_datalog_query(start.elapsed());
    json_response(StatusCode::OK, result)
}

/// `GET /datalog/stats/tabling`
///
/// Calls `pg_ripple.tabling_stats()`.
pub async fn tabling_stats(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
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
        .query_one("SELECT pg_ripple.tabling_stats()", &[])
        .await
    {
        Ok(r) => r,
        Err(e) => {
            state.metrics.record_error();
            return redacted_error(
                "datalog_error",
                &format!("tabling_stats failed: {e}"),
                StatusCode::INTERNAL_SERVER_ERROR,
            );
        }
    };

    let result: serde_json::Value = row.get(0);
    state.metrics.record_datalog_query(start.elapsed());
    json_response(StatusCode::OK, result)
}

/// `GET /datalog/lattices`
///
/// Calls `pg_ripple.list_lattices()`.
pub async fn list_lattices(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
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
        .query_one("SELECT pg_ripple.list_lattices()", &[])
        .await
    {
        Ok(r) => r,
        Err(e) => {
            state.metrics.record_error();
            return redacted_error(
                "datalog_error",
                &format!("list_lattices failed: {e}"),
                StatusCode::INTERNAL_SERVER_ERROR,
            );
        }
    };

    let result: serde_json::Value = row.get(0);
    state.metrics.record_datalog_query(start.elapsed());
    json_response(StatusCode::OK, result)
}

/// `POST /datalog/lattices`
///
/// Body: `{"name": "…", "join_fn": "…", "bottom": "…"}` JSON.
/// Calls `pg_ripple.create_lattice($1, $2, $3)`.
#[derive(Deserialize)]
pub struct CreateLatticeBody {
    pub name: String,
    pub join_fn: String,
    pub bottom: String,
}

pub async fn create_lattice(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Body,
) -> Response {
    if let Err(r) = check_auth_write(&state, &headers) {
        return r;
    }
    let body_str = match read_body(body).await {
        Ok(t) => t,
        Err(r) => return r,
    };
    let req: CreateLatticeBody = match serde_json::from_str(&body_str) {
        Ok(v) => v,
        Err(e) => {
            return json_response(
                StatusCode::BAD_REQUEST,
                serde_json::json!({"error": "invalid_request", "detail": format!("expected {{\"name\", \"join_fn\", \"bottom\"}}: {e}")}),
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

    if let Err(e) = client
        .execute(
            "SELECT pg_ripple.create_lattice($1, $2, $3)",
            &[&req.name, &req.join_fn, &req.bottom],
        )
        .await
    {
        state.metrics.record_error();
        return redacted_error(
            "datalog_error",
            &format!("create_lattice failed: {e}"),
            StatusCode::INTERNAL_SERVER_ERROR,
        );
    }

    state.metrics.record_datalog_query(start.elapsed());
    json_response(
        StatusCode::CREATED,
        serde_json::json!({"created": req.name}),
    )
}

/// `GET /datalog/views`
///
/// Calls `pg_ripple.list_datalog_views()`.
pub async fn list_views(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
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
        .query_one("SELECT pg_ripple.list_datalog_views()", &[])
        .await
    {
        Ok(r) => r,
        Err(e) => {
            state.metrics.record_error();
            return redacted_error(
                "datalog_error",
                &format!("list_datalog_views failed: {e}"),
                StatusCode::INTERNAL_SERVER_ERROR,
            );
        }
    };

    let result: serde_json::Value = row.get(0);
    state.metrics.record_datalog_query(start.elapsed());
    json_response(StatusCode::OK, result)
}

/// `POST /datalog/views`
///
/// Body: JSON object with view definition fields.
/// Calls `pg_ripple.create_datalog_view(name, rules, goal, rule_set, schedule, decode)`.
#[derive(Deserialize)]
pub struct CreateViewBody {
    pub name: String,
    pub rules: Option<String>,
    pub goal: String,
    pub rule_set: Option<String>,
    pub schedule: Option<String>,
    pub decode: Option<bool>,
}

pub async fn create_view(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Body,
) -> Response {
    if let Err(r) = check_auth_write(&state, &headers) {
        return r;
    }
    let body_str = match read_body(body).await {
        Ok(t) => t,
        Err(r) => return r,
    };
    let req: CreateViewBody = match serde_json::from_str(&body_str) {
        Ok(v) => v,
        Err(e) => {
            return json_response(
                StatusCode::BAD_REQUEST,
                serde_json::json!({"error": "invalid_request", "detail": format!("invalid view definition: {e}")}),
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

    if let Err(e) = client
        .execute(
            "SELECT pg_ripple.create_datalog_view($1, $2, $3, $4, $5, $6)",
            &[
                &req.name,
                &req.rules,
                &req.goal,
                &req.rule_set,
                &req.schedule,
                &req.decode,
            ],
        )
        .await
    {
        state.metrics.record_error();
        return redacted_error(
            "datalog_error",
            &format!("create_datalog_view failed: {e}"),
            StatusCode::INTERNAL_SERVER_ERROR,
        );
    }

    state.metrics.record_datalog_query(start.elapsed());
    json_response(
        StatusCode::CREATED,
        serde_json::json!({"created": req.name}),
    )
}

/// `DELETE /datalog/views/{name}`
///
/// Calls `pg_ripple.drop_datalog_view($1)`.
pub async fn drop_view(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(name): Path<String>,
) -> Response {
    if let Err(r) = check_auth_write(&state, &headers) {
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

    if let Err(e) = client
        .execute("SELECT pg_ripple.drop_datalog_view($1)", &[&name])
        .await
    {
        state.metrics.record_error();
        let msg = e.to_string();
        let (cat, status) = classify_pg_error(&msg);
        return redacted_error(cat, &msg, status);
    }

    state.metrics.record_datalog_query(start.elapsed());
    json_response(StatusCode::OK, serde_json::json!({"dropped": name}))
}
