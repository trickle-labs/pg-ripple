//! Citus sharding SQL API: `enable_citus_sharding`, `citus_rebalance`, status.
//! (extracted from citus/mod.rs in v0.114.0)

#![allow(clippy::too_many_arguments, unused_imports)]
use pgrx::prelude::*;

use super::{is_citus_loaded, make_reference_table};
use crate::citus::distribute_vp_delta;

// ─── SQL API ─────────────────────────────────────────────────────────────────

/// Enable Citus sharding for all existing VP tables.
///
/// Iterates over all promoted predicates and distributes their delta tables
/// using `s` (subject ID) as the distribution column.  Also converts the
/// dictionary and predicates catalog to reference tables.
///
/// Requires `pg_ripple.citus_sharding_enabled = on`.
///
/// Returns a summary row for each distributed table.
#[pg_extern(schema = "pg_ripple")]
pub fn enable_citus_sharding() -> TableIterator<
    'static,
    (
        name!(predicate_id, i64),
        name!(table_name, String),
        name!(status, String),
    ),
> {
    if !is_citus_loaded() {
        pgrx::error!("enable_citus_sharding: Citus extension is not installed (PT536)");
    }

    let colocate = if crate::gucs::storage::CITUS_TRICKLE_COMPAT.get() {
        "none"
    } else {
        "default"
    };

    // Convert reference tables (idempotent via Citus's own checks).
    make_reference_table("_pg_ripple.dictionary");
    make_reference_table("_pg_ripple.predicates");
    // `vp_rare` cannot be straightforwardly distributed by `s` because its
    // primary selectivity column is `p`; promote it to a reference table so
    // that every worker has a full copy and coordinator fan-out is avoided.
    make_reference_table("_pg_ripple.vp_rare");

    // Collect predicate IDs that have promoted VP tables.
    let pred_ids: Vec<i64> = Spi::connect(|c| {
        c.select(
            "SELECT id FROM _pg_ripple.predicates WHERE table_oid IS NOT NULL ORDER BY id",
            None,
            &[],
        )
        .map(|rows| {
            rows.filter_map(|row| row.get::<i64>(1).ok().flatten())
                .collect()
        })
        .unwrap_or_default()
    });

    let mut results: Vec<(i64, String, String)> = Vec::new();

    for pred_id in pred_ids {
        let delta_name = format!("_pg_ripple.vp_{pred_id}_delta");

        // Idempotency check: skip tables already registered in pg_dist_partition.
        // `partmethod IS NOT NULL` means Citus already knows this table.
        let already_distributed = Spi::get_one_with_args::<bool>(
            "SELECT EXISTS( \
                 SELECT 1 FROM pg_dist_partition \
                 WHERE logicalrelid = $1::regclass \
             )",
            &[delta_name.as_str().into()],
        )
        .unwrap_or(Some(false))
        .unwrap_or(false);

        if already_distributed {
            results.push((pred_id, delta_name, "skip".to_string()));
        } else {
            distribute_vp_delta(pred_id, colocate);
            results.push((pred_id, delta_name, "distributed".to_string()));
        }
    }

    // Notify merge worker to re-fence.
    let payload = format!("{{\"pid\":{}}}", std::process::id());
    Spi::run_with_args(
        &format!("SELECT pg_notify('pg_ripple.merge_start', '{payload}')"),
        &[],
    )
    .unwrap_or_else(|e| pgrx::warning!("pg_notify merge_start: {e}"));

    TableIterator::new(results)
}

/// Trigger a Citus shard rebalance.
///
/// Acquires the merge fence advisory lock (`0x5052_5000`) in **blocking** mode
/// before calling `citus_rebalance_start()`.  This ensures no merge cycle is
/// running mid-rebalance (the merge worker holds the same lock while merging).
/// The lock is released immediately after `citus_rebalance_start()` returns so
/// the merge worker can resume.
///
/// v0.59.0 (CITUS-11): Emits `pg_ripple.merge_start` NOTIFY before acquiring the
/// fence and `pg_ripple.merge_end` after releasing it so that pg-trickle and
/// monitoring tools can observe rebalance activity.
///
/// Returns the number of rebalanced shard moves.
///
/// Requires Citus to be installed (PT536).
#[pg_extern(schema = "pg_ripple")]
pub fn citus_rebalance() -> i64 {
    if !is_citus_loaded() {
        pgrx::error!("citus_rebalance: Citus extension is not installed (PT536)");
    }

    const FENCE_KEY: i64 = 0x5052_5000_i64; // "PRP\0"
    let pid = std::process::id();

    // Emit merge_start NOTIFY before acquiring the fence (CITUS-11).
    // pg-trickle uses this signal to suspend per-worker slot polling until
    // the rebalance completes.
    let start_payload = format!("{{\"context\":\"rebalance\",\"pid\":{pid}}}");
    Spi::run_with_args(
        &format!("SELECT pg_notify('pg_ripple.merge_start', '{start_payload}')"),
        &[],
    )
    .unwrap_or_else(|e| pgrx::warning!("citus_rebalance: merge_start notify: {e}"));

    // Block until no merge worker is active.  The merge worker holds this
    // session advisory lock while executing a cycle; once we acquire it the
    // worker is idle and we can safely start the Citus rebalance.
    Spi::run_with_args(
        "SELECT pg_advisory_lock($1)",
        &[pgrx::datum::DatumWithOid::from(FENCE_KEY)],
    )
    .unwrap_or_else(|e| pgrx::warning!("citus_rebalance: fence lock failed: {e}"));

    // Start the rebalance (non-blocking Citus API; the rebalancer runs async).
    let moves: i64 = Spi::get_one::<i64>(
        "SELECT COALESCE( \
             (SELECT count(*) FROM citus_rebalance_start()), \
             0 \
         )",
    )
    .unwrap_or_else(|e| {
        pgrx::warning!("citus_rebalance: {e}");
        None
    })
    .unwrap_or(0);

    // Release the fence lock so the merge worker can resume.
    Spi::run_with_args(
        "SELECT pg_advisory_unlock($1)",
        &[pgrx::datum::DatumWithOid::from(FENCE_KEY)],
    )
    .unwrap_or_else(|e| pgrx::warning!("citus_rebalance: fence unlock failed: {e}"));

    // Emit merge_end NOTIFY after releasing the fence (CITUS-11).
    let end_payload = format!("{{\"context\":\"rebalance\",\"pid\":{pid}}}");
    Spi::run_with_args(
        &format!("SELECT pg_notify('pg_ripple.merge_end', '{end_payload}')"),
        &[],
    )
    .unwrap_or_else(|e| pgrx::warning!("citus_rebalance: merge_end notify: {e}"));

    moves
}

/// Return the in-progress shard move plan for the current Citus rebalance job.
///
/// Queries `pg_dist_rebalance_progress` (available in Citus 10+) and returns
/// one row per planned shard move.  Returns an empty set when:
/// - Citus is not installed.
/// - No rebalance job is currently running.
///
/// Columns: `shard_id`, `from_node`, `to_node`, `status`.
///
/// This is the progress-reporting variant introduced in v0.59.0 (CITUS-13).
#[pg_extern(schema = "pg_ripple")]
pub fn citus_rebalance_progress() -> TableIterator<
    'static,
    (
        name!(shard_id, i64),
        name!(from_node, String),
        name!(to_node, String),
        name!(status, String),
    ),
> {
    if !is_citus_loaded() {
        return TableIterator::new(std::iter::empty());
    }

    let rows = Spi::connect(|c| {
        // pg_dist_rebalance_progress is available in Citus 10+.
        // Columns: move_id, sourcename, targetname, progress (0.0–1.0).
        c.select(
            "SELECT \
                 move_id::bigint AS shard_id, \
                 sourcename AS from_node, \
                 targetname AS to_node, \
                 CASE WHEN progress >= 1.0 THEN 'completed' \
                      WHEN progress > 0.0  THEN 'moving' \
                      ELSE                      'pending' \
                 END AS status \
             FROM pg_dist_rebalance_progress \
             ORDER BY move_id",
            None,
            &[],
        )
        .map(|rows| {
            rows.map(|row| {
                let shard_id = row.get::<i64>(1).ok().flatten().unwrap_or(0);
                let from_node = row.get::<String>(2).ok().flatten().unwrap_or_default();
                let to_node = row.get::<String>(3).ok().flatten().unwrap_or_default();
                let status = row.get::<String>(4).ok().flatten().unwrap_or_default();
                (shard_id, from_node, to_node, status)
            })
            .collect::<Vec<_>>()
        })
        .unwrap_or_default()
    });

    TableIterator::new(rows)
}

/// Return a status summary for the Citus cluster as seen by pg_ripple.
///
/// Columns: `node_id`, `node_name`, `shard_count`, `is_active`.
/// Returns an empty set if Citus is not installed.
#[pg_extern(schema = "pg_ripple")]
pub fn citus_cluster_status() -> TableIterator<
    'static,
    (
        name!(node_id, i64),
        name!(node_name, String),
        name!(shard_count, i64),
        name!(is_active, bool),
    ),
> {
    if !is_citus_loaded() {
        return TableIterator::new(std::iter::empty());
    }

    let rows = Spi::connect(|c| {
        c.select(
            "SELECT n.nodeid::bigint, \
                    n.nodename, \
                    count(s.shardid)::bigint AS shard_count, \
                    n.isactive \
             FROM pg_dist_node n \
             LEFT JOIN pg_dist_placement p ON p.groupid = n.groupid \
             LEFT JOIN pg_dist_shard s ON s.shardid = p.shardid \
             GROUP BY n.nodeid, n.nodename, n.isactive \
             ORDER BY n.nodeid",
            None,
            &[],
        )
        .map(|rows| {
            rows.filter_map(|row| {
                let node_id = row.get::<i64>(1).ok().flatten()?;
                let node_name = row.get::<String>(2).ok().flatten().unwrap_or_default();
                let shard_count = row.get::<i64>(3).ok().flatten().unwrap_or(0);
                let is_active = row.get::<bool>(4).ok().flatten().unwrap_or(false);
                Some((node_id, node_name, shard_count, is_active))
            })
            .collect::<Vec<_>>()
        })
        .unwrap_or_default()
    });

    TableIterator::new(rows)
}

/// Return `true` if Citus extension is available.
#[pg_extern(schema = "pg_ripple")]
pub fn citus_available() -> bool {
    is_citus_loaded()
}

