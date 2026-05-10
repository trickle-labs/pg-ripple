//! Rule conflict detection HTTP handler (v0.103.0).
//!
//! `GET /rule-conflicts/{ruleset}?mode=static|runtime`
//!
//! Calls `pg_ripple.rule_conflicts(ruleset, mode)` and returns the JSONB array
//! of conflict objects (empty array when no conflicts are found).

use crate::common::{AppState, check_auth, redacted_error};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use serde::Deserialize;
use std::sync::Arc;
use std::time::Instant;

use super::datalog_handlers::{classify_pg_error, json_response};

// ─── Query parameters ─────────────────────────────────────────────────────────

/// Query parameters for `GET /rule-conflicts/{ruleset}`.
#[derive(Deserialize, Default)]
pub(crate) struct ConflictQuery {
    /// Detection mode: `"static"` (default) or `"runtime"`.
    #[serde(default = "default_mode")]
    pub mode: String,
}

fn default_mode() -> String {
    "static".to_owned()
}

// ─── Handler ─────────────────────────────────────────────────────────────────

/// `GET /rule-conflicts/{ruleset}?mode=static|runtime`
///
/// Returns a JSON array of conflict objects.  An empty array `[]` means no
/// conflicts were detected.
///
/// Each element has the shape documented in the `pg_ripple.rule_conflicts()`
/// SQL function: `mode`, `rule_a`, `rule_b`, `conflict_type`,
/// `head_predicate`, `conflicting_pattern`, `shacl_constraint`,
/// `example_triple`.
pub async fn rule_conflicts_get(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(ruleset): Path<String>,
    Query(params): Query<ConflictQuery>,
) -> Response {
    if let Err(r) = check_auth(&state, &headers) {
        return r;
    }

    // Validate mode parameter.
    let mode = params.mode.clone();
    if mode != "static" && mode != "runtime" {
        return redacted_error(
            "bad_request",
            "mode must be 'static' or 'runtime'",
            StatusCode::BAD_REQUEST,
        );
    }

    // Validate ruleset name: alphanumeric, hyphens, underscores, dots only.
    if !ruleset.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.') {
        return redacted_error(
            "bad_request",
            "ruleset name contains invalid characters",
            StatusCode::BAD_REQUEST,
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
            "SELECT pg_ripple.rule_conflicts($1, $2)",
            &[&ruleset, &mode],
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
