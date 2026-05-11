//! Rule authoring HTTP handlers (v0.105.0).
//!
//! `POST /rules/draft` — translate a natural-language description to Datalog
//!   candidate rules via `pg_ripple.draft_rule_from_nl()`.
//!
//! `POST /rules/validate` — statically analyse a Datalog rule via
//!   `pg_ripple.validate_rule()`.

use crate::common::{AppState, check_auth, redacted_error};
use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use serde::Deserialize;
use std::sync::Arc;
use std::time::Instant;

use super::datalog_handlers::{classify_pg_error, json_response};

// ─── Request / response shapes ───────────────────────────────────────────────

/// Request body for `POST /rules/draft`.
#[derive(Deserialize)]
pub(crate) struct DraftRuleRequest {
    /// Natural-language description of the rule to draft.
    pub description: String,
    /// Number of candidate rules to return (1–10, default: 3).
    #[serde(default = "default_candidates")]
    pub candidates: i32,
}

fn default_candidates() -> i32 {
    3
}

/// Request body for `POST /rules/validate`.
#[derive(Deserialize)]
pub(crate) struct ValidateRuleRequest {
    /// The Datalog rule text to validate.
    pub rule: String,
}

// ─── POST /rules/draft ───────────────────────────────────────────────────────

/// `POST /rules/draft`
///
/// Request body:
/// ```json
/// {"description": "Flag suppliers that share a VAT number as duplicates", "candidates": 3}
/// ```
///
/// Response: JSON array of `{"rank": 1, "rule": "...", "explanation": "..."}`.
pub async fn draft_rules_post(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<DraftRuleRequest>,
) -> Response {
    if let Err(r) = check_auth(&state, &headers) {
        return r;
    }

    // Validate candidates range before hitting the DB.
    if !(1..=10).contains(&body.candidates) {
        return redacted_error(
            "bad_request",
            &format!(
                "PT0457: candidates must be between 1 and 10, got {}",
                body.candidates
            ),
            StatusCode::BAD_REQUEST,
        );
    }

    if body.description.trim().is_empty() {
        return redacted_error(
            "bad_request",
            "description must not be empty",
            StatusCode::BAD_REQUEST,
        );
    }

    let description = body.description.clone();
    let candidates = body.candidates;

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

    let rows = match client
        .query(
            "SELECT rank, rule, explanation \
             FROM pg_ripple.draft_rule_from_nl($1, $2)",
            &[&description, &candidates],
        )
        .await
    {
        Ok(r) => r,
        Err(e) => {
            state.metrics.record_error();
            let msg = e.to_string();
            let (kind, status) = classify_pg_error(&msg);
            return redacted_error(kind, &msg, status);
        }
    };

    let result: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| {
            let rank: i32 = row.get(0);
            let rule: String = row.get(1);
            let explanation: String = row.get(2);
            serde_json::json!({
                "rank": rank,
                "rule": rule,
                "explanation": explanation,
            })
        })
        .collect();

    state.metrics.record_datalog_query(start.elapsed());
    json_response(StatusCode::OK, serde_json::Value::Array(result))
}

// ─── POST /rules/validate ────────────────────────────────────────────────────

/// `POST /rules/validate`
///
/// Request body:
/// ```json
/// {"rule": "?x <http://example.org/knows> ?y :- ?x <http://example.org/knows> ?y ."}
/// ```
///
/// Response:
/// ```json
/// {"valid": true}
/// // or
/// {"valid": false, "errors": [...], "warnings": [...]}
/// ```
pub async fn validate_rule_post(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<ValidateRuleRequest>,
) -> Response {
    if let Err(r) = check_auth(&state, &headers) {
        return r;
    }

    if body.rule.trim().is_empty() {
        return redacted_error(
            "bad_request",
            "rule must not be empty",
            StatusCode::BAD_REQUEST,
        );
    }

    let rule = body.rule.clone();

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

    let rows = match client
        .query("SELECT pg_ripple.validate_rule($1)", &[&rule])
        .await
    {
        Ok(r) => r,
        Err(e) => {
            state.metrics.record_error();
            let msg = e.to_string();
            let (kind, status) = classify_pg_error(&msg);
            return redacted_error(kind, &msg, status);
        }
    };

    let result = rows
        .first()
        .and_then(|row| row.try_get::<_, serde_json::Value>(0).ok())
        .unwrap_or_else(|| serde_json::json!({"valid": false, "errors": ["no result"]}));

    state.metrics.record_datalog_query(start.elapsed());
    json_response(StatusCode::OK, result)
}
