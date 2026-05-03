//! Datalog REST API — legacy re-export shim (v0.90.0 CQ-05).
//!
//! The implementation has been moved to `routing::datalog_handlers`.
//! This module re-exports the full public API to maintain backward
//! compatibility for any external callers.
pub use crate::routing::datalog_handlers::*;
