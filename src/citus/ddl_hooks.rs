//! Citus VP table distribution helpers: `distribute_vp_delta`.
//! (extracted from citus/mod.rs in v0.114.0)

#![allow(clippy::too_many_arguments, unused_imports)]
use pgrx::prelude::*;

pub fn distribute_vp_delta(pred_id: i64, colocate_with: &str) {
    let delta = format!("_pg_ripple.vp_{pred_id}_delta");

    // Step 1: REPLICA IDENTITY FULL (must come before create_distributed_table).
    Spi::run_with_args(&format!("ALTER TABLE {delta} REPLICA IDENTITY FULL"), &[])
        .unwrap_or_else(|e| pgrx::warning!("REPLICA IDENTITY FULL {delta}: {e}"));

    // Step 2: Distribute the table on column `s` (subject).
    Spi::run_with_args(
        &format!(
            "SELECT create_distributed_table( \
                 '{delta}', 's', colocate_with => '{colocate_with}' \
             )"
        ),
        &[],
    )
    .unwrap_or_else(|e| pgrx::warning!("create_distributed_table {delta}: {e}"));

    // Query the shard count so listeners can enumerate worker-level shard tables.
    let shard_count: i64 = Spi::get_one_with_args::<i64>(
        "SELECT count(*)::bigint FROM pg_dist_shard WHERE logicalrelid = $1::regclass",
        &[delta.as_str().into()],
    )
    .unwrap_or(Some(0))
    .unwrap_or(0);

    // Notify pg-trickle and other listeners that a VP table has been promoted
    // to distributed.  The payload follows the agreed contract (C-4):
    //   table             — fully-qualified logical table name
    //   shard_count       — number of shards created by Citus (for slot setup)
    //   shard_table_prefix — prefix used by Citus for physical shard tables
    //   predicate_id      — pg_ripple predicate integer ID
    //
    // pg-trickle uses `shard_count` and `shard_table_prefix` to enumerate
    // per-worker shard names when creating logical replication slots without
    // querying `pg_dist_shard` directly.
    let shard_table_prefix = format!("{delta}_");
    let payload = format!(
        "{{\"table\":\"{delta}\",\"shard_count\":{shard_count},\
          \"shard_table_prefix\":\"{shard_table_prefix}\",\"predicate_id\":{pred_id}}}",
    );
    Spi::run_with_args(
        &format!("SELECT pg_notify('pg_ripple.vp_promoted', '{payload}')"),
        &[],
    )
    .unwrap_or_else(|e| pgrx::warning!("pg_notify vp_promoted: {e}"));

    // Step 3: Distribute the tombstones table co-located with delta so that
    // the HTAP query path `(main EXCEPT tombstones) UNION ALL delta` stays
    // shard-local and is not re-routed through the coordinator.
    //
    // Tombstones have `s BIGINT` (added since v0.6.0), making co-location
    // straightforward.  `colocate_with` uses the delta table name so Citus
    // assigns tombstones to the same colocation group (same shard count and
    // distribution).
    let tombs = format!("_pg_ripple.vp_{pred_id}_tombstones");
    Spi::run_with_args(
        &format!(
            "SELECT create_distributed_table( \
                 '{tombs}', 's', colocate_with => '{delta}' \
             )"
        ),
        &[],
    )
    .unwrap_or_else(|e| pgrx::warning!("create_distributed_table {tombs}: {e}"));
}

