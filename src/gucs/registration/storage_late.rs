//! GUC registrations for v0.81.0+ features (split from storage.rs v0.122.0 H17-02).

#[allow(unused_imports)]
use crate::gucs::*;
use pgrx::guc::{GucContext, GucFlags};
use pgrx::prelude::*;

/// Register GUCs added in v0.81.0 and later versions.
pub(super) fn register_late() {
    // ── v0.81.0 GUCs ──────────────────────────────────────────────────────────

    pgrx::GucRegistry::define_bool_guc(
        c"pg_ripple.strict_dictionary",
        c"When on, decode() returns an error for missing dictionary IDs instead of \
      the _unknown_<id> placeholder string. Useful for strict data-quality contexts. \
      Default off. (v0.81.0 DICT-STRICT-01)",
        c"",
        &crate::gucs::storage::STRICT_DICTIONARY,
        GucContext::Userset,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_bool_guc(
        c"pg_ripple.strict_sparql_filters",
        c"When on, unknown built-in function names in FILTER expressions raise \
      ERROR (PT422) rather than evaluating to UNDEF. Default off. \
      (v0.81.0 FILTER-STRICT-01)",
        c"",
        &crate::gucs::sparql::STRICT_SPARQL_FILTERS,
        GucContext::Userset,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.cdc_slot_idle_timeout_seconds",
        c"Seconds of no LSN advance before the CDC slot cleanup worker drops an \
      orphaned replication slot. Default: 3600. (v0.81.0 CDC-SLOT-01)",
        c"",
        &crate::gucs::storage::CDC_SLOT_IDLE_TIMEOUT_SECONDS,
        60,
        86400,
        GucContext::Suset,
        GucFlags::default(),
    );

    // ── v0.82.0 GUCs ──────────────────────────────────────────────────────────

    pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.plan_cache_capacity",
        c"Maximum number of cached SPARQL-to-SQL plan translations (default: 1024, range: 64–65536). \
      Replaces the hardcoded constant in plan_cache.rs. (v0.82.0 CACHE-CAP-01)",
        c"",
        &crate::gucs::sparql::PLAN_CACHE_CAPACITY,
        64,
        65536,
        GucContext::Userset,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.merge_lock_timeout_ms",
        c"Milliseconds to wait for the merge fence lock before skipping this cycle \
      (default: 5000, range: 100–60000). (v0.82.0 MERGE-LOCK-GUC-01)",
        c"",
        &crate::gucs::storage::MERGE_LOCK_TIMEOUT_MS,
        100,
        60000,
        GucContext::Sighup,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.merge_heartbeat_interval_seconds",
        c"Seconds between merge worker heartbeat log lines (default: 60, range: 10–3600). \
      (v0.82.0 MERGE-HBEAT-01)",
        c"",
        &crate::gucs::storage::MERGE_HEARTBEAT_INTERVAL_SECONDS,
        10,
        3600,
        GucContext::Sighup,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.stats_scan_limit",
        c"Maximum number of VP tables scanned per graph_stats() call \
      (default: 1000, range: 1–100000). (v0.82.0 STATS-DOC-01)",
        c"",
        &crate::gucs::storage::STATS_SCAN_LIMIT,
        1,
        100_000,
        GucContext::Userset,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.stats_refresh_interval_seconds",
        c"Seconds between background refreshes of predicate_stats_cache \
      (default: 300, range: 10–86400). (v0.82.0 STATS-CACHE-01)",
        c"",
        &crate::gucs::storage::STATS_REFRESH_INTERVAL_SECONDS,
        10,
        86400,
        GucContext::Sighup,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.vacuum_dict_batch_size",
        c"Number of predicates processed per batch in vacuum_dictionary() \
      (default: 200, range: 10–10000). (v0.82.0 VACUUM-DICT-BATCH-01)",
        c"",
        &crate::gucs::storage::VACUUM_DICT_BATCH_SIZE,
        10,
        10000,
        GucContext::Userset,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.all_nodes_predicate_limit",
        c"Maximum number of predicates in a wildcard property-path UNION ALL expansion \
      (default: 500, range: 10–50000). Excess predicates sorted by triple count and truncated. \
      (v0.82.0 PROPPATH-UNBOUNDED-01)",
        c"",
        &crate::gucs::sparql::ALL_NODES_PREDICATE_LIMIT,
        10,
        50000,
        GucContext::Userset,
        GucFlags::default(),
    );

    // GUC-BOUNDS-01 (v0.82.0): merge_batch_size — controls merge worker batch size.
    pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.merge_batch_size",
        c"Maximum rows processed per merge worker INSERT…SELECT batch (default: 1000000, \
          range: 100–100,000,000). (v0.82.0 GUC-BOUNDS-01)",
        c"",
        &crate::gucs::storage::MERGE_BATCH_SIZE,
        100,
        100_000_000,
        GucContext::Userset,
        GucFlags::default(),
    );

    // MERGE-BACKOFF-01 (v0.83.0): merge worker exponential backoff cap.
    pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.merge_max_backoff_secs",
        c"Maximum backoff duration in seconds for the merge worker exponential backoff \
      after consecutive errors (default: same as merge_interval_secs=60, range: 10–3600). \
      First error doubles the wait; each subsequent error doubles again; capped here. \
      (v0.83.0 MERGE-BACKOFF-01)",
        c"",
        &crate::gucs::storage::MERGE_MAX_BACKOFF_SECS,
        10,
        3600,
        GucContext::Sighup,
        GucFlags::default(),
    );

    // PGC_POSTMASTER GUCs can only be registered during shared_preload_libraries
    // loading.  `process_shared_preload_libraries_in_progress` is the correct
    // flag — `IsPostmasterEnvironment` is true in every server process and
    // cannot be used to distinguish this case.
    // SAFETY: `process_shared_preload_libraries_in_progress` is a stable PostgreSQL
    // global set by the postmaster; reading it is safe from any GUC registration context.
    if unsafe { pg_sys::process_shared_preload_libraries_in_progress } {
        pgrx::GucRegistry::define_int_guc(
            c"pg_ripple.dictionary_cache_size",
            c"Shared-memory encode-cache capacity in entries (default: 4096; startup only; range: 1024–1,073,741,824)",
            c"",
            &DICTIONARY_CACHE_SIZE,
            1024,
            1_073_741_824,
            GucContext::Postmaster,
            GucFlags::default(),
        );

        pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.cache_budget",
        c"Shared-memory budget cap in MB; bulk loads throttle when >90% utilised (default: 64; startup only)",
        c"",
        &CACHE_BUDGET_MB,
        0,
        65536,
        GucContext::Postmaster,
        GucFlags::default(),
    );

        pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.merge_workers",
        c"Number of parallel background merge worker processes (default: 1, max: 16; startup only). \
          Each worker handles a round-robin subset of VP table predicates (v0.42.0)",
        c"",
        &MERGE_WORKERS,
        1,
        16,
        GucContext::Postmaster,
        GucFlags::default(),
    );

        pgrx::GucRegistry::define_int_guc(
            c"pg_ripple.audit_retention",
            c"Retention period in days for _pg_ripple.event_audit rows (v0.78.0). \
          0 disables automatic pruning.",
            c"",
            &crate::gucs::observability::AUDIT_RETENTION_DAYS,
            0,
            3650,
            GucContext::Suset,
            GucFlags::default(),
        );
    }

    // C13-11 (v0.85.0): DESCRIBE CBD recursion depth cap.
    // NOTE: Userset GUCs must be registered unconditionally, not inside the
    // process_shared_preload_libraries_in_progress block (which only runs at
    // postmaster startup). Placing Userset GUCs in that block would cause
    // "unrecognized configuration parameter" errors in regular sessions.
    pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.describe_max_depth",
        c"Maximum recursion depth for DESCRIBE CBD traversal. \
          Prevents runaway traversal on cyclic or deep graphs (v0.85.0 C13-11). \
          Default: 16. Range: 1–256.",
        c"",
        &crate::gucs::storage::DESCRIBE_MAX_DEPTH,
        1,
        256,
        GucContext::Userset,
        GucFlags::default(),
    );

    // CDC-01 (v0.91.0): CDC LSN watermark batching GUCs.
    pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.cdc_watermark_batch_size",
        c"Number of CDC events to accumulate before flushing the LSN watermark \
          to reduce per-event write amplification (v0.91.0 CDC-01). Default: 100. Range: 1–10000.",
        c"",
        &crate::gucs::storage::CDC_WATERMARK_BATCH_SIZE,
        1,
        10_000,
        GucContext::Suset,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.cdc_watermark_flush_interval_ms",
        c"Maximum milliseconds between LSN watermark flushes regardless of batch size \
          (v0.91.0 CDC-01). Ensures progress even during low-volume CDC streams. Default: 50. Range: 1–60000.",
        c"",
        &crate::gucs::storage::CDC_WATERMARK_FLUSH_INTERVAL_MS,
        1,
        60_000,
        GucContext::Suset,
        GucFlags::default(),
    );

    // H15-03 (v0.94.0): bounded bidi relay channel.
    pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.bidi_relay_max_inflight",
        c"Maximum number of in-flight bidi relay operations per process. \
          When this limit is reached, new relay calls are dropped and \
          pg_ripple_bidi_relay_dropped_total is incremented (H15-03 v0.94.0). \
          Default: 1000. Range: 1–100000.",
        c"",
        &crate::gucs::storage::BIDI_RELAY_MAX_INFLIGHT,
        1,
        100_000,
        GucContext::Suset,
        GucFlags::default(),
    );

    // H16-05 (v0.113.0): bulk load UNNEST-array batch INSERT path.
    pgrx::GucRegistry::define_bool_guc(
        c"pg_ripple.bulk_load_use_copy",
        c"When on (default), bulk loaders use UNNEST-array batch INSERTs for \
          triple insertion instead of per-row VALUES INSERTs. Delivers 5-10x \
          throughput improvement for large loads. (H16-05 v0.113.0). Default: on.",
        c"",
        &crate::gucs::storage::BULK_LOAD_USE_COPY,
        GucContext::Userset,
        GucFlags::default(),
    );

    // M15-07 (v0.95.0): scheduled VACUUM ANALYZE on dictionary after bulk encode.
    pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.dict_vacuum_threshold",
        c"Minimum number of new dictionary terms inserted in a batch before \
          VACUUM ANALYZE _pg_ripple.dictionary is run automatically. \
          Set to 0 to disable. (M15-07 v0.95.0). Default: 10000. Range: 0–10000000.",
        c"",
        &crate::gucs::storage::DICT_VACUUM_THRESHOLD,
        0,
        10_000_000,
        GucContext::Userset,
        GucFlags::default(),
    );
    // v0.106.0: temporal fact store GUCs.
    pgrx::GucRegistry::define_string_guc(
        c"pg_ripple.temporal_data_model",
        c"Default data model for pg_ripple.mark_temporal() when data_model is omitted. \
          One of 'snapshot' (at most one open interval per (s,p,o,g); asserting a new \
          value closes the previous one) or 'versioned' (every assertion creates a new row). \
          Default: 'snapshot'. (v0.106.0)",
        c"",
        &crate::gucs::storage::TEMPORAL_DATA_MODEL,
        GucContext::Userset,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_bool_guc(
        c"pg_ripple.enable_temporal_operators",
        c"Gate for temporal parse extensions (AFTER, BEFORE, DURING, WITHIN, SEQUENCE, \
          CONSECUTIVE) in Datalog rules. Must be on to use temporal operators. \
          Default: off. (v0.106.0 + v0.107.0)",
        c"",
        &crate::gucs::storage::ENABLE_TEMPORAL_OPERATORS,
        GucContext::Userset,
        GucFlags::default(),
    );

    // v0.107.0: temporal CDC integration GUC.
    pgrx::GucRegistry::define_bool_guc(
        c"pg_ripple.temporal_cdc_enabled",
        c"When on (default), insert_triple() for a temporal predicate automatically \
          records a row in temporal_facts with valid_from = transaction_timestamp(). \
          Set off to disable automatic CDC wiring (for historical data loading). \
          (v0.107.0)",
        c"",
        &crate::gucs::storage::TEMPORAL_CDC_ENABLED,
        GucContext::Userset,
        GucFlags::default(),
    );

    // P6 (v0.113.0): logical apply worker batch watermark GUCs.
    pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.replication_batch_size",
        c"Number of CDC / logical-apply events to accumulate before flushing \
          the LSN watermark. Reduces per-event write amplification. \
          Default: 100. Range: 1-100000. (P6 v0.113.0)",
        c"",
        &crate::gucs::storage::REPLICATION_BATCH_SIZE,
        1,
        100_000,
        GucContext::Suset,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.replication_batch_interval_ms",
        c"Maximum milliseconds between LSN watermark flushes in the logical apply \
          worker, regardless of replication_batch_size. Ensures forward progress \
          during low-volume streams. Default: 500. Range: 1-60000. (P6 v0.113.0)",
        c"",
        &crate::gucs::storage::REPLICATION_BATCH_INTERVAL_MS,
        1,
        60_000,
        GucContext::Suset,
        GucFlags::default(),
    );

    // ── v0.116.0 GUCs ────────────────────────────────────────────────────────

    // M16-01: ER monitoring retention GUC.
    pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.er_monitoring_retention_days",
        c"Retention window in days for ER monitoring tables \
          (er_unresolved_entities, er_cluster_sizes, er_resolution_dashboard). \
          Rows older than this are pruned by the merge background worker. \
          Default: 30. Range: 1-3650. (M16-01 v0.116.0)",
        c"",
        &crate::gucs::storage::ER_MONITORING_RETENTION_DAYS,
        1,
        3650,
        GucContext::Suset,
        GucFlags::default(),
    );

    // M16-11: Bidi relay drop policy GUC.
    pgrx::GucRegistry::define_string_guc(
        c"pg_ripple.bidi_relay_drop_policy",
        c"Overflow drop policy for the bidi relay channel: 'newest' (default) or \
          'oldest'. Both values drop the incoming event when the inflight limit is \
          reached. (M16-11 v0.116.0)",
        c"",
        &crate::gucs::storage::BIDI_RELAY_DROP_POLICY,
        GucContext::Userset,
        GucFlags::default(),
    );

    // ── v0.118.0 GUCs ────────────────────────────────────────────────────────

    // Feature 2: Privacy budget reset interval GUC.
    pgrx::GucRegistry::define_string_guc(
        c"pg_ripple.privacy_budget_reset_interval",
        c"Automatic reset interval for differential-privacy budget rows in \
          _pg_ripple.privacy_budget. When now() - last_reset_at exceeds this \
          interval, budget_spent is reset to 0 on the next DP function call. \
          Default: '1 day'. (Feature 2 v0.118.0)",
        c"",
        &crate::gucs::storage::PRIVACY_BUDGET_RESET_INTERVAL,
        GucContext::Suset,
        GucFlags::default(),
    );

    // ── v0.125.0 GUCs — temporal graph snapshots (FEAT-02) ───────────────────

    pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.snapshot_retention_days",
        c"Retention period in days for _pg_ripple.graph_snapshots rows. \
          Snapshots whose expires_at <= now() are purged by the background merge \
          worker on each tick. Set to 0 to disable automatic GC. \
          Default: 30. Range: 0-3650. (FEAT-02 v0.125.0)",
        c"",
        &crate::gucs::storage::SNAPSHOT_RETENTION_DAYS,
        0,
        3650,
        GucContext::Suset,
        GucFlags::default(),
    );

    // ── v0.128.0 GUCs — JSON mapping relational writeback (JSON-WRITEBACK-01) ─

    pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.json_writeback_batch_size",
        c"Number of _pg_ripple.json_writeback_queue rows processed per \
          background merge-worker tick. Set to 0 to disable automatic background \
          draining. Default: 100. Range: 0-10000. (JSON-WRITEBACK-01 v0.128.0)",
        c"",
        &crate::gucs::storage::JSON_WRITEBACK_BATCH_SIZE,
        0,
        10_000,
        GucContext::Suset,
        GucFlags::default(),
    );
}
