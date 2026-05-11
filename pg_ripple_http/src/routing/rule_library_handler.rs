//! Rule library HTTP handlers (v0.104.0).
//!
//! `GET /rule-libraries`
//!
//! Returns the list of installed rule libraries as a JSON array by calling
//! `pg_ripple.list_rule_libraries()`.

use crate::common::{AppState, check_auth, redacted_error};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use std::sync::Arc;
use std::time::Instant;

use super::datalog_handlers::json_response;

// ─── Handler ─────────────────────────────────────────────────────────────────

/// `GET /rule-libraries`
///
/// Returns a JSON array of installed rule library objects.  Each element has:
/// `name`, `version`, `installed_at`, `description`, `license_iri`.
///
/// An empty array is returned when no libraries are installed.
pub async fn list_rule_libraries(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
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

    let rows = match client
        .query(
            "SELECT name, version, \
                 to_char(installed_at, 'YYYY-MM-DD HH24:MI:SS') AS installed_at, \
                 coalesce(description, '') AS description, \
                 coalesce(license_iri, '') AS license_iri \
             FROM _pg_ripple.rule_libraries \
             ORDER BY name",
            &[],
        )
        .await
    {
        Ok(r) => r,
        Err(e) => {
            state.metrics.record_error();
            // Table may not exist on older versions — return empty array.
            let msg = e.to_string();
            if msg.contains("does not exist") {
                state.metrics.record_datalog_query(start.elapsed());
                return json_response(StatusCode::OK, serde_json::json!([]));
            }
            return redacted_error("database_error", &msg, StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let libs: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| {
            let name: String = row.get(0);
            let version: String = row.get(1);
            let installed_at: String = row.get(2);
            let description: String = row.get(3);
            let license_iri: String = row.get(4);
            serde_json::json!({
                "name": name,
                "version": version,
                "installed_at": installed_at,
                "description": description,
                "license_iri": license_iri,
            })
        })
        .collect();

    state.metrics.record_datalog_query(start.elapsed());
    json_response(StatusCode::OK, serde_json::Value::Array(libs))
}
