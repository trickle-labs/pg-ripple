//! Multi-tenant management HTTP handlers (v0.115.0 M16-02).
//!
//! GET    /tenants           — list all tenants with stats
//! POST   /tenants           — create a new tenant
//! GET    /tenants/:name     — get stats for a specific tenant
//! DELETE /tenants/:name     — drop a tenant

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use serde::Deserialize;

use super::sparql_handlers::json_response_http;
use crate::common::{AppState, check_auth, check_auth_write, redacted_error};

fn json_response(status: StatusCode, body: serde_json::Value) -> Response {
    json_response_http(status, body)
}

// ── Request types ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateTenantBody {
    pub name: String,
    pub graph_iri: String,
    #[serde(default)]
    pub quota_triples: i64,
}

// ── Validation helper ─────────────────────────────────────────────────────────

/// A tenant name must be non-empty and consist only of alphanumerics, hyphens,
/// and underscores — identical to the subscription_id validation in routing.
fn is_valid_tenant_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 63
        && name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

// ── GET /tenants ──────────────────────────────────────────────────────────────

/// List all tenants and their statistics.
pub(crate) async fn list_tenants(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Response {
    if let Err(r) = check_auth(&state, &headers) {
        return r;
    }
    let client = match state.pool.get().await {
        Ok(c) => c,
        Err(e) => {
            state.metrics.record_error();
            return redacted_error("db_pool_error", &e.to_string(), StatusCode::SERVICE_UNAVAILABLE);
        }
    };
    let rows = match client
        .query("SELECT tenant_name, graph_iri, quota_triples, triple_count FROM pg_ripple.tenant_stats()", &[])
        .await
    {
        Ok(r) => r,
        Err(e) => {
            state.metrics.record_error();
            return redacted_error("tenant_stats_error", &e.to_string(), StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    let tenants: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| {
            serde_json::json!({
                "name":          row.get::<_, String>(0),
                "graph_iri":     row.get::<_, String>(1),
                "quota_triples": row.get::<_, i64>(2),
                "triple_count":  row.get::<_, i64>(3),
            })
        })
        .collect();
    json_response(StatusCode::OK, serde_json::json!({ "tenants": tenants }))
}

// ── POST /tenants ─────────────────────────────────────────────────────────────

/// Create a new tenant.
pub(crate) async fn create_tenant(
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
            return json_response(StatusCode::BAD_REQUEST, serde_json::json!({"error": "read_error"}));
        }
    };
    let req: CreateTenantBody = match serde_json::from_slice(&bytes) {
        Ok(r) => r,
        Err(e) => {
            return json_response(
                StatusCode::BAD_REQUEST,
                serde_json::json!({"error": "invalid_json", "detail": format!("{e}")}),
            );
        }
    };
    if !is_valid_tenant_name(&req.name) {
        return json_response(
            StatusCode::BAD_REQUEST,
            serde_json::json!({
                "error": "invalid_tenant_name",
                "detail": "name must be 1-63 alphanumeric/hyphen/underscore characters",
            }),
        );
    }
    let client = match state.pool.get().await {
        Ok(c) => c,
        Err(e) => {
            state.metrics.record_error();
            return redacted_error("db_pool_error", &e.to_string(), StatusCode::SERVICE_UNAVAILABLE);
        }
    };
    match client
        .execute(
            "SELECT pg_ripple.create_tenant($1, $2, $3)",
            &[&req.name, &req.graph_iri, &req.quota_triples],
        )
        .await
    {
        Ok(_) => json_response(
            StatusCode::CREATED,
            serde_json::json!({
                "status": "created",
                "name": req.name,
                "graph_iri": req.graph_iri,
                "quota_triples": req.quota_triples,
            }),
        ),
        Err(e) => {
            state.metrics.record_error();
            redacted_error("create_tenant_error", &e.to_string(), StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// ── GET /tenants/:name ────────────────────────────────────────────────────────

/// Get statistics for a specific tenant.
pub(crate) async fn get_tenant(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(name): Path<String>,
) -> Response {
    if let Err(r) = check_auth(&state, &headers) {
        return r;
    }
    if !is_valid_tenant_name(&name) {
        return json_response(
            StatusCode::BAD_REQUEST,
            serde_json::json!({"error": "invalid_tenant_name"}),
        );
    }
    let client = match state.pool.get().await {
        Ok(c) => c,
        Err(e) => {
            state.metrics.record_error();
            return redacted_error("db_pool_error", &e.to_string(), StatusCode::SERVICE_UNAVAILABLE);
        }
    };
    let row = match client
        .query_opt(
            "SELECT tenant_name, graph_iri, quota_triples, triple_count \
             FROM pg_ripple.tenant_stats() WHERE tenant_name = $1",
            &[&name],
        )
        .await
    {
        Ok(Some(r)) => r,
        Ok(None) => {
            return json_response(
                StatusCode::NOT_FOUND,
                serde_json::json!({"error": "not_found", "detail": format!("tenant '{name}' not found")}),
            );
        }
        Err(e) => {
            state.metrics.record_error();
            return redacted_error("tenant_stats_error", &e.to_string(), StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    json_response(
        StatusCode::OK,
        serde_json::json!({
            "name":          row.get::<_, String>(0),
            "graph_iri":     row.get::<_, String>(1),
            "quota_triples": row.get::<_, i64>(2),
            "triple_count":  row.get::<_, i64>(3),
        }),
    )
}

// ── DELETE /tenants/:name ─────────────────────────────────────────────────────

/// Drop a tenant (removes their graph and quota entry).
pub(crate) async fn delete_tenant(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(name): Path<String>,
) -> Response {
    if let Err(r) = check_auth_write(&state, &headers) {
        return r;
    }
    if !is_valid_tenant_name(&name) {
        return json_response(
            StatusCode::BAD_REQUEST,
            serde_json::json!({"error": "invalid_tenant_name"}),
        );
    }
    let client = match state.pool.get().await {
        Ok(c) => c,
        Err(e) => {
            state.metrics.record_error();
            return redacted_error("db_pool_error", &e.to_string(), StatusCode::SERVICE_UNAVAILABLE);
        }
    };
    match client.execute("SELECT pg_ripple.drop_tenant($1)", &[&name]).await {
        Ok(_) => json_response(
            StatusCode::OK,
            serde_json::json!({ "status": "dropped", "name": name }),
        ),
        Err(e) => {
            state.metrics.record_error();
            redacted_error("drop_tenant_error", &e.to_string(), StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
