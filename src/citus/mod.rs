//! Citus horizontal sharding integration (v0.58.0, Feature L-5.4).
//!
//!
//! # Architecture
//!
//! When `pg_ripple.citus_sharding_enabled = on`, VP tables are distributed
//! across Citus worker nodes using `create_distributed_table()`.  The
//! distribution column is `s` (subject ID) to co-locate triples that share a
//! subject on the same shard — this optimises star-pattern joins.
//!
//! Key decisions (v0.58.0):
//! - Dictionary and predicates catalog become Citus **reference tables**.
//! - VP delta tables use `colocate_with = 'none'` when
//!   `pg_ripple.citus_trickle_compat = on` (prevents pg-trickle from issuing
//!   cross-shard deletes during apply).
//! - `REPLICA IDENTITY FULL` is set **before** `create_distributed_table()` so
//!   that the logical replication slot used by pg-trickle captures full row
//!   images from the very first write.
//! - The merge worker fence uses `pg_try_advisory_lock(0x5052_5000)` (session-
//!   level, key = `"PRP\0"`) on the coordinator before executing a merge cycle.
//!   `pg_ripple.citus_rebalance()` acquires the same lock (blocking form) before
//!   calling `citus_rebalance_start()` to prevent split-brain.
//!
//! # Error codes
//!
//! - PT536 — Citus extension is not installed.

// v0.90.0 CQ-02: pre-emptive split sub-modules
// Q15-01: internal API field; kept for public API surface or future extension consumers.
#[allow(dead_code)]
pub mod aggregate;
// Q15-01: internal API field; kept for public API surface or future extension consumers.
#[allow(dead_code)]
pub mod federation;
// Q15-01: internal API field; kept for public API surface or future extension consumers.
#[allow(dead_code)]
pub mod rls;
// Q15-01: internal API field; kept for public API surface or future extension consumers.
#[allow(dead_code)]
pub mod sharding;

use pgrx::prelude::*;

// ─── Citus detection ──────────────────────────────────────────────────────────

/// Return `true` if Citus is installed and accessible.
pub(super) fn is_citus_loaded() -> bool {
    let result = Spi::get_one::<bool>(
        "SELECT EXISTS ( \
             SELECT 1 FROM pg_extension WHERE extname = 'citus' \
         )",
    );
    matches!(result, Ok(Some(true)))
}

// ─── Reference table setup ───────────────────────────────────────────────────

/// Convert the dictionary and predicates catalog to Citus reference tables.
///
/// Reference tables are replicated to every worker node so that dictionary
/// lookups and predicate routing never require cross-shard queries.
///
/// # Errors
/// Raises `PT536` if Citus is not installed.
pub(super) fn make_reference_table(table: &str) {
    let sql = format!("SELECT create_reference_table('{table}')");
    Spi::run_with_args(&sql, &[])
        .unwrap_or_else(|e| pgrx::warning!("make_reference_table {table}: {e}"));
}

// ─── VP table distribution ───────────────────────────────────────────────────

/// Set `REPLICA IDENTITY FULL` on a VP delta table and distribute it.
///
/// This is the **canonical** order: REPLICA IDENTITY must come **before**
/// `create_distributed_table()` so pg-trickle captures full row images from
/// the first logical replication message (C-9 fix).
///
/// # Arguments
/// - `pred_id` — predicate integer ID
/// - `colocate_with` — Citus colocate_with parameter (`'default'` or `'none'`)
// ─── Sub-modules split out in v0.114.0 ──────────────────────────────────────
pub mod ddl_hooks;
pub mod query_rewriting;
pub mod rebalance;
pub mod shard_pruning;

// Re-export key public API
pub use ddl_hooks::distribute_vp_delta;
pub use shard_pruning::compute_shard_id;
pub use query_rewriting::citus_service_shard_annotation;
pub use shard_pruning::explain_citus_section;
