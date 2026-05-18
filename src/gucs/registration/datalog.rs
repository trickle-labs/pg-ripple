//! GUC registration for Datalog inference and reasoning (Q13-01, v0.84.0).
//! Split from registration.rs for navigability.

// A16-CQ: unused_imports here is intentional for test/cfg-gated code paths.
#[allow(unused_imports)]
use crate::gucs::*;
use pgrx::guc::{GucContext, GucFlags};
// A16-CQ: unused_imports here is intentional for test/cfg-gated code paths.
#[allow(unused_imports)]
use pgrx::prelude::*;

unsafe extern "C-unwind" fn check_inference_mode(
    newval: *mut *mut std::ffi::c_char,
    _extra: *mut *mut std::ffi::c_void,
    _source: pgrx::pg_sys::GucSource::Type,
) -> bool {
    if newval.is_null() {
        return true;
    }
    // SAFETY: newval is a GUC check-hook argument; the pointer is valid for
    // the duration of this call and the string has at least a NUL terminator.
    let s = unsafe {
        if (*newval).is_null() {
            return true;
        }
        std::ffi::CStr::from_ptr(*newval).to_str().unwrap_or("")
    };
    matches!(s, "off" | "on_demand" | "materialized" | "incremental_rdfs")
}

/// Validate `enforce_constraints`: `off`, `warn`, or `error`.
unsafe extern "C-unwind" fn check_enforce_constraints(
    newval: *mut *mut std::ffi::c_char,
    _extra: *mut *mut std::ffi::c_void,
    _source: pgrx::pg_sys::GucSource::Type,
) -> bool {
    if newval.is_null() {
        return true;
    }
    // SAFETY: newval is a GUC check-hook argument; the pointer is valid for
    // the duration of this call and the string has at least a NUL terminator.
    let s = unsafe {
        if (*newval).is_null() {
            return true;
        }
        std::ffi::CStr::from_ptr(*newval).to_str().unwrap_or("")
    };
    matches!(s, "off" | "warn" | "error")
}

/// Validate `rule_graph_scope`: `default` or `all`.
unsafe extern "C-unwind" fn check_rule_graph_scope(
    newval: *mut *mut std::ffi::c_char,
    _extra: *mut *mut std::ffi::c_void,
    _source: pgrx::pg_sys::GucSource::Type,
) -> bool {
    if newval.is_null() {
        return true;
    }
    // SAFETY: newval is a GUC check-hook argument; the pointer is valid for
    // the duration of this call and the string has at least a NUL terminator.
    let s = unsafe {
        if (*newval).is_null() {
            return true;
        }
        std::ffi::CStr::from_ptr(*newval).to_str().unwrap_or("")
    };
    matches!(s, "default" | "all")
}

/// Validate `shacl_mode`: `off`, `sync`, or `async`.
unsafe extern "C-unwind" fn check_shacl_mode(
    newval: *mut *mut std::ffi::c_char,
    _extra: *mut *mut std::ffi::c_void,
    _source: pgrx::pg_sys::GucSource::Type,
) -> bool {
    if newval.is_null() {
        return true;
    }
    // SAFETY: newval is a GUC check-hook argument; the pointer is valid for
    // the duration of this call and the string has at least a NUL terminator.
    let s = unsafe {
        if (*newval).is_null() {
            return true;
        }
        std::ffi::CStr::from_ptr(*newval).to_str().unwrap_or("")
    };
    matches!(s, "off" | "sync" | "async")
}

/// Validate `strict_goal_validation`: `off`, `warn`, or `error`.
unsafe extern "C-unwind" fn check_strict_goal_validation(
    newval: *mut *mut std::ffi::c_char,
    _extra: *mut *mut std::ffi::c_void,
    _source: pgrx::pg_sys::GucSource::Type,
) -> bool {
    if newval.is_null() {
        return true;
    }
    // SAFETY: newval is a GUC check-hook argument; the pointer is valid for
    // the duration of this call and the string has at least a NUL terminator.
    let s = unsafe {
        if (*newval).is_null() {
            return true;
        }
        std::ffi::CStr::from_ptr(*newval).to_str().unwrap_or("")
    };
    matches!(s, "off" | "warn" | "error")
}

/// Register all GUCs for this domain.
pub fn register() {
    // ── v0.7.0 GUCs ──────────────────────────────────────────────────────────

    // v0.37.0: validated shacl_mode
    // SAFETY: define_string_guc_with_hooks requires an unsafe block;
    // the hook function pointers are valid extern "C" function pointers.
    unsafe {
        pgrx::GucRegistry::define_string_guc_with_hooks(
        c"pg_ripple.shacl_mode",
        c"SHACL validation mode: 'off' (default), 'sync' (reject violations inline), 'async' (queue for background worker)",
        c"",
        &SHACL_MODE,
        GucContext::Userset,
        GucFlags::default(),
        Some(check_shacl_mode),
        None,
        None,
    );
    }

    // ── v0.79.0 SHACL-SPARQL GUCs ────────────────────────────────────────────

    pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.shacl_rule_max_iterations",
        c"Maximum fixpoint iterations for sh:SPARQLRule evaluation per validation cycle; \
      raises an error when the cap is reached (default: 100, min: 1, max: 10000) (v0.79.0)",
        c"",
        &crate::gucs::shacl::SHACL_RULE_MAX_ITERATIONS,
        1,
        10_000,
        GucContext::Userset,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_bool_guc(
        c"pg_ripple.shacl_rule_cwb",
        c"When on, sh:SPARQLRule rules whose target graph matches a CONSTRUCT writeback \
      pipeline are registered as CWB rules (default: off) (v0.79.0)",
        c"",
        &crate::gucs::shacl::SHACL_RULE_CWB,
        GucContext::Userset,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_bool_guc(
    c"pg_ripple.dedup_on_merge",
    c"When true, the HTAP generation merge deduplicates (s,o,g) rows keeping the lowest SID (default: false)",
    c"",
    &DEDUP_ON_MERGE,
    GucContext::Userset,
    GucFlags::default(),
);

    // ── v0.10.0 GUCs ─────────────────────────────────────────────────────────

    // v0.37.0: Use define_string_guc_with_hooks to validate enum values at SET time.
    // SAFETY: define_string_guc_with_hooks requires an unsafe block;
    // the hook function pointers are valid extern "C" function pointers.
    unsafe {
        pgrx::GucRegistry::define_string_guc_with_hooks(
        c"pg_ripple.inference_mode",
        c"Datalog inference mode: 'off' (default), 'on_demand', 'materialized', 'incremental_rdfs' (v0.56.0)",
        c"",
        &INFERENCE_MODE,
        GucContext::Userset,
        GucFlags::default(),
        Some(check_inference_mode),
        None,
        None,
    );

        pgrx::GucRegistry::define_string_guc_with_hooks(
            c"pg_ripple.enforce_constraints",
            c"Constraint rule enforcement: 'off' (default), 'warn', 'error'",
            c"",
            &ENFORCE_CONSTRAINTS,
            GucContext::Userset,
            GucFlags::default(),
            Some(check_enforce_constraints),
            None,
            None,
        );

        pgrx::GucRegistry::define_string_guc_with_hooks(
            c"pg_ripple.rule_graph_scope",
            c"Graph scope for unscoped Datalog atoms: 'all' (any graph, default) or 'default' (g=0 only)",
            c"",
            &RULE_GRAPH_SCOPE,
            GucContext::Userset,
            GucFlags::default(),
            Some(check_rule_graph_scope),
            None,
            None,
        );
    }

    // ── v0.29.0 GUCs ─────────────────────────────────────────────────────────

    pgrx::GucRegistry::define_bool_guc(
        c"pg_ripple.magic_sets",
        c"When on (default), infer_goal() uses magic sets for goal-directed inference; \
      off falls back to full materialization + filter (v0.29.0)",
        c"",
        &MAGIC_SETS,
        GucContext::Userset,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_bool_guc(
        c"pg_ripple.datalog_cost_reorder",
        c"When on (default), sort Datalog rule body atoms by ascending estimated \
      VP-table cardinality before SQL compilation (v0.29.0)",
        c"",
        &DATALOG_COST_REORDER,
        GucContext::Userset,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.datalog_antijoin_threshold",
        c"Minimum VP-table rows for NOT body atoms to compile to LEFT JOIN IS NULL \
      anti-join form instead of NOT EXISTS (default: 1000, 0=always NOT EXISTS; v0.29.0)",
        c"",
        &DATALOG_ANTIJOIN_THRESHOLD,
        0,
        10_000_000,
        GucContext::Userset,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.delta_index_threshold",
        c"Minimum semi-naive delta-table rows before creating a B-tree index on (s,o) \
      join columns (default: 500, 0=disabled; v0.29.0)",
        c"",
        &DELTA_INDEX_THRESHOLD,
        0,
        10_000_000,
        GucContext::Userset,
        GucFlags::default(),
    );

    // ── v0.30.0 GUCs ─────────────────────────────────────────────────────────

    pgrx::GucRegistry::define_bool_guc(
        c"pg_ripple.rule_plan_cache",
        c"When on (default), cache compiled SQL for each rule set to speed up \
      repeated infer() / infer_agg() calls; invalidated by drop_rules() and \
      load_rules() (v0.30.0)",
        c"",
        &RULE_PLAN_CACHE,
        GucContext::Userset,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.rule_plan_cache_size",
        c"Maximum number of rule sets kept in the plan cache (default: 64, \
      min: 1, max: 4096); oldest entries are evicted on overflow (v0.30.0)",
        c"",
        &RULE_PLAN_CACHE_SIZE,
        1,
        4096,
        GucContext::Userset,
        GucFlags::default(),
    );

    // ── v0.31.0 GUCs ─────────────────────────────────────────────────────────

    pgrx::GucRegistry::define_bool_guc(
        c"pg_ripple.sameas_reasoning",
        c"When on (default), Datalog inference applies an owl:sameAs \
      canonicalization pre-pass so that rules and SPARQL queries referencing \
      non-canonical entities are transparently rewritten to the canonical form \
      (v0.31.0)",
        c"",
        &SAMEAS_REASONING,
        GucContext::Userset,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_bool_guc(
        c"pg_ripple.demand_transform",
        c"When on (default), create_datalog_view() automatically applies demand \
      transformation when multiple goal patterns are specified; infer_demand() \
      always applies demand filtering regardless (v0.31.0)",
        c"",
        &DEMAND_TRANSFORM,
        GucContext::Userset,
        GucFlags::default(),
    );

    // ── v0.32.0 GUCs ─────────────────────────────────────────────────────────

    pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.wfs_max_iterations",
        c"Safety cap on alternating fixpoint rounds per WFS pass (default: 100, \
      min: 1, max: 10000); emits PT520 WARNING if a pass does not converge (v0.32.0)",
        c"",
        &WFS_MAX_ITERATIONS,
        1,
        10_000,
        GucContext::Userset,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_bool_guc(
        c"pg_ripple.tabling",
        c"When on (default), infer_wfs() and SPARQL results are cached in \
      _pg_ripple.tabling_cache and reused on matching subsequent calls; \
      invalidated by drop_rules(), load_rules(), and triple modifications (v0.32.0)",
        c"",
        &TABLING,
        GucContext::Userset,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.tabling_ttl",
        c"TTL in seconds for tabling cache entries (default: 300; set 0 to disable \
      TTL-based expiry) (v0.32.0)",
        c"",
        &TABLING_TTL,
        0,
        86_400,
        GucContext::Userset,
        GucFlags::default(),
    );

    // ── v0.57.0 GUCs — OWL profiles, KGE, multi-tenant, columnar, adaptive index ──

    pgrx::GucRegistry::define_string_guc(
        c"pg_ripple.owl_profile",
        c"Active OWL reasoning profile: 'RL' (default), 'EL', 'QL', or 'off'. (v0.57.0)",
        c"",
        &crate::gucs::datalog::OWL_PROFILE,
        GucContext::Userset,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_bool_guc(
        c"pg_ripple.probabilistic_datalog",
        c"Enable experimental probabilistic Datalog with @weight rule annotations. \
      Preview quality; no stability guarantee. Default off. (v0.57.0)",
        c"",
        &crate::gucs::datalog::PROBABILISTIC_DATALOG,
        GucContext::Userset,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_bool_guc(
        c"pg_ripple.kge_enabled",
        c"Enable the knowledge-graph embedding background worker. Default off. (v0.57.0)",
        c"",
        &crate::gucs::llm::KGE_ENABLED,
        GucContext::Userset,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_string_guc(
        c"pg_ripple.kge_model",
        c"Knowledge-graph embedding model: 'transe' (default) or 'rotate'. (v0.57.0)",
        c"",
        &crate::gucs::llm::KGE_MODEL,
        GucContext::Userset,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.columnar_threshold",
        c"VP table triple count above which HTAP merge converts vp_main to columnar storage. \
      -1 = disabled (default). Requires pg_columnar. (v0.57.0)",
        c"",
        &crate::gucs::storage::COLUMNAR_THRESHOLD,
        -1,
        1_000_000_000,
        GucContext::Userset,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_bool_guc(
        c"pg_ripple.adaptive_indexing_enabled",
        c"Enable adaptive B-tree index creation based on per-predicate query access patterns. \
      Default off. (v0.57.0)",
        c"",
        &crate::gucs::storage::ADAPTIVE_INDEXING_ENABLED,
        GucContext::Userset,
        GucFlags::default(),
    );

    // ── v0.83.0 GUCs ──────────────────────────────────────────────────────────

    // DL-COST-GUC-01 (v0.83.0): Datalog cost-model divisors for rule body ordering.
    pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.datalog_cost_bound_s_divisor",
        c"Synthetic cardinality divisor for Datalog rule atoms with subject bound to a constant \
      (default: 100, range: 1–10000). Larger values push single-bound atoms earlier in the join \
      order. Replaces hardcoded divisor 100 in compiler.rs. (v0.83.0 DL-COST-GUC-01)",
        c"",
        &crate::gucs::datalog::DATALOG_COST_BOUND_S_DIVISOR,
        1,
        10000,
        GucContext::Userset,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.datalog_cost_bound_so_divisor",
        c"Synthetic cardinality divisor for Datalog rule atoms with both subject and object bound \
      to constants (default: 10, range: 1–1000). Larger values push dual-bound atoms earlier. \
      Replaces hardcoded divisor 10 in compiler.rs. (v0.83.0 DL-COST-GUC-01)",
        c"",
        &crate::gucs::datalog::DATALOG_COST_BOUND_SO_DIVISOR,
        1,
        1000,
        GucContext::Userset,
        GucFlags::default(),
    );

    // ── v0.87.0 Uncertain Knowledge Engine GUCs ───────────────────────────────

    pgrx::GucRegistry::define_bool_guc(
        c"pg_ripple.prob_datalog_cyclic",
        c"Allow probabilistic evaluation on cyclic Datalog rule sets. Approximate evaluation; \
      requires explicit opt-in. Default off. (v0.87.0 CONF-CYCLIC-01)",
        c"",
        &crate::gucs::datalog::PROB_DATALOG_CYCLIC,
        GucContext::Userset,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.prob_datalog_max_iterations",
        c"Maximum semi-naive inference rounds when prob_datalog_cyclic = on. \
      After this limit the evaluator emits a WARNING and returns the partial result. \
      Default 100, range 1–10000. (v0.87.0 CONF-CYCLIC-01)",
        c"",
        &crate::gucs::datalog::PROB_DATALOG_MAX_ITERATIONS,
        1,
        10000,
        GucContext::Userset,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_float_guc(
        c"pg_ripple.prob_datalog_convergence_delta",
        c"Early-exit threshold for cyclic probabilistic Datalog: iteration stops when the \
      maximum confidence delta is below this value. Default 0.001. (v0.87.0 CONF-CYCLIC-01)",
        c"",
        &crate::gucs::datalog::PROB_DATALOG_CONVERGENCE_DELTA,
        0.0,
        1.0,
        GucContext::Userset,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_bool_guc(
        c"pg_ripple.prob_datalog_cyclic_strict",
        c"When on, promote max-iterations-exceeded from WARNING to ERROR (PT0307). \
      Default off. (v0.87.0 CONF-CYCLIC-01)",
        c"",
        &crate::gucs::datalog::PROB_DATALOG_CYCLIC_STRICT,
        GucContext::Userset,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_float_guc(
        c"pg_ripple.default_fuzzy_threshold",
        c"Default similarity threshold for pg:fuzzy_match() and pg:confPath(). \
      Default 0.7. DEPRECATED since v0.89.0, use pg_ripple.fuzzy_match_threshold \
      (to be removed in v1.0.0). (v0.87.0 FUZZY-SPARQL-01)",
        c"",
        &crate::gucs::datalog::DEFAULT_FUZZY_THRESHOLD,
        0.0,
        1.0,
        GucContext::Userset,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_float_guc(
        c"pg_ripple.fuzzy_match_threshold",
        c"Default similarity threshold for pg:fuzzy_match() and pg:confPath() when no \
      explicit threshold is provided. Default 0.7. Canonical name (API-01, v0.89.0); \
      supersedes default_fuzzy_threshold. (v0.87.0 FUZZY-SPARQL-01)",
        c"",
        &crate::gucs::datalog::FUZZY_MATCH_THRESHOLD,
        0.0,
        1.0,
        GucContext::Userset,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_bool_guc(
        c"pg_ripple.prov_confidence",
        c"Enable automatic confidence propagation from PROV-O pg:sourceTrust predicates. \
      Default off. (v0.87.0 PROV-CONF-01)",
        c"",
        &crate::gucs::datalog::PROV_CONFIDENCE,
        GucContext::Userset,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_bool_guc(
        c"pg_ripple.export_confidence",
        c"Enable confidence annotations in RDF export functions (RDF* and JSON-LD 1.1). \
      Default off. (v0.87.0 CONF-EXPORT-01)",
        c"",
        &crate::gucs::datalog::EXPORT_CONFIDENCE,
        GucContext::Userset,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_string_guc(
        c"pg_ripple.cwb_confidence_propagation",
        c"CONSTRUCT writeback confidence propagation mode: 'explicit' (default, implicit 1.0) \
      or 'inherit' (propagate source confidence weighted by rule weight). \
      (v0.87.0 CONF-CWB-01)",
        c"",
        &crate::gucs::datalog::CWB_CONFIDENCE_PROPAGATION,
        GucContext::Userset,
        GucFlags::default(),
    );

    // ── v0.100.0 Proof Tree / Derivation Recording GUCs ──────────────────────

    pgrx::GucRegistry::define_bool_guc(
        c"pg_ripple.record_derivations",
        c"When on, the semi-naive inference engine records derivation provenance in \
      _pg_ripple.derivations for every newly derived fact, enabling justify() proof trees. \
      Off by default due to storage and performance overhead. Enable before calling infer() \
      when you need backward-chaining explanations. (v0.100.0 PROOF-TREE-01)",
        c"",
        &crate::gucs::datalog::RECORD_DERIVATIONS,
        GucContext::Userset,
        GucFlags::default(),
    );

    // ── v0.102.0 Hypothetical Inference GUCs ──────────────────────────────────

    pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.hypothetical_max_assertions",
        c"Maximum total number of assert + retract triples in a single \
      hypothetical_inference() call. Exceeding this limit raises PT0450. \
      Default 10000. (v0.102.0 HYPO-01)",
        c"",
        &crate::gucs::datalog::HYPOTHETICAL_MAX_ASSERTIONS,
        1,
        1_000_000,
        GucContext::Userset,
        GucFlags::default(),
    );

    // ── v0.103.0 Conflict Detection GUCs ─────────────────────────────────────

    pgrx::GucRegistry::define_bool_guc(
        c"pg_ripple.rule_conflict_check_on_load",
        c"When on, static conflict analysis runs automatically at load_rules() time \
      and raises a WARNING for each conflict found (not an error). \
      Default off. (v0.103.0 CONFLICT-01)",
        c"",
        &crate::gucs::datalog::RULE_CONFLICT_CHECK_ON_LOAD,
        GucContext::Userset,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_bool_guc(
        c"pg_ripple.block_on_conflict",
        c"When on, the semi-naive inference engine checks for runtime rule conflicts \
      after each fixpoint iteration and raises PT0451 if any are found, halting \
      inference before committing derived facts. Default off. (v0.103.0 CONFLICT-02)",
        c"",
        &crate::gucs::datalog::BLOCK_ON_CONFLICT,
        GucContext::Userset,
        GucFlags::default(),
    );

    // ── v0.108.0 Bayesian Confidence Update GUCs ──────────────────────────────

    pgrx::GucRegistry::define_string_guc(
        c"pg_ripple.confidence_update_strategy",
        c"Confidence update strategy for update_confidence(): 'bayesian' (default), \
      'noisy-or' (delegate to v0.87 noisy-OR combiner), or 'manual' (raise PT0441 — \
      caller must set confidence directly via insert_triple()). (v0.108.0 BAYES-01)",
        c"",
        &crate::gucs::datalog::CONFIDENCE_UPDATE_STRATEGY,
        GucContext::Userset,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.confidence_propagation_max_depth",
        c"Maximum cascade depth when propagating a base-fact confidence change \
      downstream through the derivation DAG. Facts at depth > max are queued in \
      _pg_ripple.confidence_stale for background reprocessing. Default 10. (v0.108.0 BAYES-02)",
        c"",
        &crate::gucs::datalog::CONFIDENCE_PROPAGATION_MAX_DEPTH,
        1,
        1000,
        GucContext::Userset,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_string_guc(
        c"pg_ripple.confidence_reprocessing_interval",
        c"Polling interval for the confidence reprocessing background worker that drains \
      _pg_ripple.confidence_stale. Default '30 seconds'. (v0.108.0 BAYES-03)",
        c"",
        &crate::gucs::datalog::CONFIDENCE_REPROCESSING_INTERVAL,
        GucContext::Sighup,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_string_guc(
        c"pg_ripple.evidence_log_retention",
        c"Retention period for _pg_ripple.evidence_log rows. Rows older than this \
      interval are pruned by the background worker. Default '1 year'. (v0.108.0 BAYES-04)",
        c"",
        &crate::gucs::datalog::EVIDENCE_LOG_RETENTION,
        GucContext::Sighup,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.confidence_batch_size",
        c"Batch size for bulk_update_confidence() — number of evidence rows processed \
      per transaction. Default 1000. (v0.108.0 BAYES-05)",
        c"",
        &crate::gucs::datalog::CONFIDENCE_BATCH_SIZE,
        1,
        1_000_000,
        GucContext::Userset,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_float_guc(
        c"pg_ripple.conflict_confidence_penalty",
        c"Confidence attenuation penalty applied when two conflicting rules are detected. \
      Attenuation = 1.0 - conflict_severity * penalty. Must be in [0.0, 1.0]. \
      Default 0.3. (v0.108.0 BAYES-06)",
        c"",
        &crate::gucs::datalog::CONFLICT_CONFIDENCE_PENALTY,
        0.0,
        1.0,
        GucContext::Userset,
        GucFlags::default(),
    );

    // ── v0.109.0 NS-RL GUCs ───────────────────────────────────────────────────

    pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.sameas_apply_rate_limit",
        c"Maximum owl:sameAs triples a single resolve_entities() call may assert. \
      Calls exceeding this limit raise PT0460. Default 1000. (v0.109.0)",
        c"",
        &crate::gucs::datalog::SAMEAS_APPLY_RATE_LIMIT,
        1,
        10_000_000,
        GucContext::Userset,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_bool_guc(
        c"pg_ripple.string_similarity_extensions_ok",
        c"Informational: true when both pg_trgm and fuzzystrmatch are installed. \
      Set at CREATE EXTENSION time. (v0.109.0)",
        c"",
        &crate::gucs::datalog::STRING_SIMILARITY_EXTENSIONS_OK,
        GucContext::Userset,
        GucFlags::default(),
    );

    // ── v0.110.0 NS-RL Evaluation & Explainability GUCs ──────────────────────

    pgrx::GucRegistry::define_bool_guc(
        c"pg_ripple.record_sameas_anomalies",
        c"When on, any owl:sameAs assertion that would exceed \
      pg_ripple.sameas_max_cluster_size is logged to \
      _pg_ripple.sameas_anomaly_log before PT550 is raised. Default on. (v0.110.0)",
        c"",
        &crate::gucs::datalog::RECORD_SAMEAS_ANOMALIES,
        GucContext::Userset,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_string_guc(
        c"pg_ripple.sameas_anomaly_log_retention",
        c"Retention period for _pg_ripple.sameas_anomaly_log rows. \
      Rows older than this are pruned by the background worker. \
      Default '90 days'. (v0.110.0)",
        c"",
        &crate::gucs::datalog::SAMEAS_ANOMALY_LOG_RETENTION,
        GucContext::Userset,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_string_guc(
        c"pg_ripple.rule_explanation_cache_ttl",
        c"TTL for cached explain_rule() results in _pg_ripple.rule_explanations. \
      Default '24 hours'. (v0.110.0)",
        c"",
        &crate::gucs::datalog::RULE_EXPLANATION_CACHE_TTL,
        GucContext::Userset,
        GucFlags::default(),
    );

    // ── v0.111.0 PPRL GUCs ────────────────────────────────────────────────────

    pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.bloom_max_input_length",
        c"Maximum byte length of the value argument to bloom_encode(). \
      Calls exceeding this limit raise PT0470. Default 4096 bytes. (v0.111.0)",
        c"",
        &crate::gucs::datalog::BLOOM_MAX_INPUT_LENGTH,
        1,
        1_048_576,
        GucContext::Userset,
        GucFlags::default(),
    );

    // ── v0.112.0 Goal Validation GUC (issue #89) ─────────────────────────────

    // SAFETY: check hook is a valid extern "C-unwind" function pointer.
    unsafe {
        pgrx::GucRegistry::define_string_guc_with_hooks(
            c"pg_ripple.strict_goal_validation",
            c"Goal predicate validation mode for infer_goal() and create_datalog_view(): \
          'warn' (default) emits a WARNING when the goal predicate is unknown, \
          'error' raises an ERROR, 'off' disables validation (v0.112.0)",
            c"",
            &crate::gucs::datalog::STRICT_GOAL_VALIDATION,
            GucContext::Userset,
            GucFlags::default(),
            Some(check_strict_goal_validation),
            None,
            None,
        );
    }

    // ── v0.116.0 GUCs ────────────────────────────────────────────────────────

    // M16-07: Proof-tree depth and node guards.
    pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.proof_tree_max_depth",
        c"Maximum depth of the proof tree assembled by justify() and explain_inference(). \
          Beyond this depth the builder inserts a sentinel node and emits PT0480. \
          Default: 64. Range: 1-1024. (M16-07 v0.116.0)",
        c"",
        &crate::gucs::datalog::PROOF_TREE_MAX_DEPTH,
        1,
        1024,
        GucContext::Userset,
        GucFlags::default(),
    );

    pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.proof_tree_max_nodes",
        c"Maximum total node count of the proof tree assembled by justify() and \
          explain_inference(). When exceeded the builder stops and emits PT0481. \
          Default: 10000. Range: 10-10000000. (M16-07 v0.116.0)",
        c"",
        &crate::gucs::datalog::PROOF_TREE_MAX_NODES,
        10,
        10_000_000,
        GucContext::Userset,
        GucFlags::default(),
    );

    // M16-19: Bounded rule explanation LRU cache.
    pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.rule_explanation_cache_max_entries",
        c"Maximum entries in the per-process LRU cache for explain_rule() results. \
          Also used to trim _pg_ripple.rule_explanations to this size on each cache-miss write. \
          Default: 1000. Range: 10-100000. (M16-19 v0.116.0)",
        c"",
        &crate::gucs::datalog::RULE_EXPLANATION_CACHE_MAX_ENTRIES,
        10,
        100_000,
        GucContext::Userset,
        GucFlags::default(),
    );

    // M16-20: Bayesian propagation depth GUC.
    pgrx::GucRegistry::define_int_guc(
        c"pg_ripple.bayesian_propagation_max_depth",
        c"Maximum depth for Bayesian confidence propagation in propagate_downstream(). \
          Chains longer than this are queued in _pg_ripple.confidence_stale for background \
          reprocessing. Default: 10. Range: 1-10000. (M16-20 v0.116.0)",
        c"",
        &crate::gucs::datalog::BAYESIAN_PROPAGATION_MAX_DEPTH,
        1,
        10_000,
        GucContext::Userset,
        GucFlags::default(),
    );
}
