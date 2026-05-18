//! Privacy-Preserving Record Linkage (PPRL) HTTP handlers (v0.115.0 M16-02).
//!
//! POST /pprl/bloom_encode     — encode a value as a Bloom-filter bit-string
//! POST /pprl/dice_similarity  — compute Dice-coefficient similarity between two bit-strings

use std::sync::Arc;

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
pub struct BloomEncodeBody {
    pub value: String,
    pub key: String,
    #[serde(default = "default_hash_count")]
    pub hash_count: i32,
    #[serde(default = "default_length")]
    pub length: i32,
}

fn default_hash_count() -> i32 {
    30
}
fn default_length() -> i32 {
    1024
}

#[derive(Debug, Deserialize)]
pub struct DiceSimilarityBody {
    pub a: String,
    pub b: String,
}

// ── POST /pprl/bloom_encode ───────────────────────────────────────────────────

/// Encode a value as a Bloom-filter bit-string using HMAC-SHA256.
///
/// Protected by `check_auth_write` because the key material is confidential.
pub(crate) async fn pprl_bloom_encode(
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
    let req: BloomEncodeBody = match serde_json::from_slice(&bytes) {
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
    let row = match client
        .query_one(
            "SELECT pg_ripple.bloom_encode($1, $2, $3, $4)",
            &[&req.value, &req.key, &req.hash_count, &req.length],
        )
        .await
    {
        Ok(r) => r,
        Err(e) => {
            state.metrics.record_error();
            return redacted_error(
                "bloom_encode_error",
                &e.to_string(),
                StatusCode::INTERNAL_SERVER_ERROR,
            );
        }
    };
    // M16-03: increment Bloom encode counter.
    state.metrics.record_pprl_bloom_encode();
    let encoded: String = row.get(0);
    json_response(
        StatusCode::OK,
        serde_json::json!({
            "encoded": encoded,
            "hash_count": req.hash_count,
            "length": req.length,
        }),
    )
}

// ── POST /pprl/dice_similarity ────────────────────────────────────────────────

/// Compute the Dice-coefficient similarity between two Bloom-filter bit-strings.
pub(crate) async fn pprl_dice_similarity(
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
    let req: DiceSimilarityBody = match serde_json::from_slice(&bytes) {
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
    let row = match client
        .query_one(
            "SELECT pg_ripple.dice_similarity($1, $2)",
            &[&req.a, &req.b],
        )
        .await
    {
        Ok(r) => r,
        Err(e) => {
            state.metrics.record_error();
            return redacted_error(
                "dice_similarity_error",
                &e.to_string(),
                StatusCode::INTERNAL_SERVER_ERROR,
            );
        }
    };
    let similarity: f64 = row.get(0);
    json_response(
        StatusCode::OK,
        serde_json::json!({ "similarity": similarity }),
    )
}
