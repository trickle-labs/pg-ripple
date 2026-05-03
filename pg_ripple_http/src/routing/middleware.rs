//! HTTP middleware composition for pg_ripple_http (HTTP-03, v0.91.0).
//!
//! Extracts CORS, rate-limiting (governor), and tracing middleware from
//! `main.rs` to a dedicated module so that `build_router()` and `main()` stay
//! focused on their own concerns.

use std::sync::Arc;

use axum::Router;
use axum::http::HeaderValue;
use tower_governor::{GovernorLayer, governor::GovernorConfigBuilder};
use tower_http::cors::{AllowOrigin, CorsLayer};

/// Apply the standard pg_ripple_http middleware stack to a router.
///
/// Layers applied (outer → inner):
/// 1. Optional per-IP rate-limiting via `tower_governor` (when `rate_limit > 0`).
/// 2. CORS policy from `cors_origins` env-var.
///
/// `build_router()` in `routing/mod.rs` passes the already-constructed CORS layer
/// here so that the permissive-CORS warning can be logged once at startup in
/// `main()` before `apply_middleware()` is called.
pub fn apply_rate_limit(app: Router, rate_limit: u32) -> Router {
    if rate_limit == 0 {
        return app;
    }
    let governor_conf = match GovernorConfigBuilder::default()
        .per_second(rate_limit as u64)
        .burst_size(rate_limit)
        .finish()
    {
        Some(c) => c,
        None => {
            tracing::error!("invalid governor rate-limit configuration");
            std::process::exit(1);
        }
    };
    app.layer(GovernorLayer::new(Arc::new(governor_conf)))
}

/// Build the CORS layer from a comma-separated list of allowed origin strings.
///
/// - `"*"` — wildcard (permissive); logs a warning. Returns `CorsLayer::permissive()`.
/// - `""` — empty string; no cross-origin access. Returns `CorsLayer::new()`.
/// - `"https://a.example,https://b.example"` — explicit allowlist.
pub fn build_cors_layer(cors_origins: &str) -> CorsLayer {
    if cors_origins == "*" {
        tracing::warn!(
            "CORS is permissive (*). Set PG_RIPPLE_HTTP_CORS_ORIGINS to a comma-separated list \
             of allowed origins for production use."
        );
        CorsLayer::permissive()
    } else if cors_origins.is_empty() {
        CorsLayer::new()
    } else {
        let origins: Vec<HeaderValue> = cors_origins
            .split(',')
            .filter_map(|o| o.trim().parse().ok())
            .collect();
        CorsLayer::new().allow_origin(AllowOrigin::list(origins))
    }
}
