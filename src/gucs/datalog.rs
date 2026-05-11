//! GUC parameters for the Datalog reasoning engine (inference, aggregation,
//! semi-naive evaluation, DRed, parallel strata, WFS, lattice, tabling).

// ─── v0.10.0 Datalog GUCs ─────────────────────────────────────────────────────

/// GUC: Datalog inference execution mode.
pub static INFERENCE_MODE: pgrx::GucSetting<Option<std::ffi::CString>> =
    pgrx::GucSetting::<Option<std::ffi::CString>>::new(None);

/// GUC: Datalog constraint enforcement mode.
pub static ENFORCE_CONSTRAINTS: pgrx::GucSetting<Option<std::ffi::CString>> =
    pgrx::GucSetting::<Option<std::ffi::CString>>::new(None);

/// GUC: graph scope for unscoped body atoms. Default is 'all' (match any graph).
/// Set to 'default' to restrict unscoped atoms to g = 0 only.
pub static RULE_GRAPH_SCOPE: pgrx::GucSetting<Option<std::ffi::CString>> =
    pgrx::GucSetting::<Option<std::ffi::CString>>::new(None);

// ─── v0.29.0 Datalog GUCs ─────────────────────────────────────────────────────

/// GUC: master switch for magic sets goal-directed inference (v0.29.0).
pub static MAGIC_SETS: pgrx::GucSetting<bool> = pgrx::GucSetting::<bool>::new(true);

/// GUC: when `true` (default), sort Datalog rule body atoms by ascending
/// estimated VP-table cardinality before SQL compilation (v0.29.0).
pub static DATALOG_COST_REORDER: pgrx::GucSetting<bool> = pgrx::GucSetting::<bool>::new(true);

/// GUC: minimum VP-table row count for negated body atoms to use anti-join (v0.29.0).
pub static DATALOG_ANTIJOIN_THRESHOLD: pgrx::GucSetting<i32> = pgrx::GucSetting::<i32>::new(1000);

/// GUC: minimum semi-naive delta temp-table row count before creating a B-tree index (v0.29.0).
pub static DELTA_INDEX_THRESHOLD: pgrx::GucSetting<i32> = pgrx::GucSetting::<i32>::new(500);

// ─── v0.30.0 Datalog GUCs ─────────────────────────────────────────────────────

/// GUC: master switch for the Datalog rule plan cache (v0.30.0).
pub static RULE_PLAN_CACHE: pgrx::GucSetting<bool> = pgrx::GucSetting::<bool>::new(true);

/// GUC: maximum number of rule sets whose compiled SQL is kept in the plan cache (v0.30.0).
pub static RULE_PLAN_CACHE_SIZE: pgrx::GucSetting<i32> = pgrx::GucSetting::<i32>::new(64);

// ─── v0.31.0 Datalog GUCs ─────────────────────────────────────────────────────

/// GUC: master switch for `owl:sameAs` entity canonicalization (v0.31.0).
pub static SAMEAS_REASONING: pgrx::GucSetting<bool> = pgrx::GucSetting::<bool>::new(true);

/// GUC: master switch for demand transformation (v0.31.0).
pub static DEMAND_TRANSFORM: pgrx::GucSetting<bool> = pgrx::GucSetting::<bool>::new(true);

// ─── v0.32.0 Datalog GUCs ─────────────────────────────────────────────────────

/// GUC: safety cap on alternating fixpoint rounds for well-founded semantics (v0.32.0).
pub static WFS_MAX_ITERATIONS: pgrx::GucSetting<i32> = pgrx::GucSetting::<i32>::new(100);

/// GUC: master switch for the Datalog / SPARQL tabling cache (v0.32.0).
pub static TABLING: pgrx::GucSetting<bool> = pgrx::GucSetting::<bool>::new(true);

/// GUC: TTL in seconds for tabling cache entries (v0.32.0).
pub static TABLING_TTL: pgrx::GucSetting<i32> = pgrx::GucSetting::<i32>::new(300);

// ─── v0.34.0 Datalog GUCs ─────────────────────────────────────────────────────

/// GUC: maximum depth for bounded-depth Datalog fixpoint termination (v0.34.0).
pub static DATALOG_MAX_DEPTH: pgrx::GucSetting<i32> = pgrx::GucSetting::<i32>::new(0);

/// GUC: master switch for the Delete-Rederive (DRed) algorithm (v0.34.0).
pub static DRED_ENABLED: pgrx::GucSetting<bool> = pgrx::GucSetting::<bool>::new(true);

/// GUC: maximum number of deleted base triples per DRed transaction (v0.34.0).
pub static DRED_BATCH_SIZE: pgrx::GucSetting<i32> = pgrx::GucSetting::<i32>::new(1000);

// ─── v0.35.0 Datalog GUCs ─────────────────────────────────────────────────────

/// GUC: maximum number of parallel background workers for Datalog stratum evaluation (v0.35.0).
pub static DATALOG_PARALLEL_WORKERS: pgrx::GucSetting<i32> = pgrx::GucSetting::<i32>::new(4);

/// GUC: minimum estimated total row count for a stratum before parallel group
/// analysis is applied (v0.35.0).
pub static DATALOG_PARALLEL_THRESHOLD: pgrx::GucSetting<i32> = pgrx::GucSetting::<i32>::new(10_000);

// ─── v0.36.0 Datalog GUCs ─────────────────────────────────────────────────────

/// GUC: maximum fixpoint iterations for lattice-based Datalog inference (v0.36.0).
pub static LATTICE_MAX_ITERATIONS: pgrx::GucSetting<i32> = pgrx::GucSetting::<i32>::new(1000);

// ─── v0.40.0 Datalog GUCs ─────────────────────────────────────────────────────

/// GUC: maximum derived facts produced by a single `infer()` call (v0.40.0).
pub static DATALOG_MAX_DERIVED: pgrx::GucSetting<i32> = pgrx::GucSetting::<i32>::new(0);

// ─── v0.42.0 Datalog GUCs ─────────────────────────────────────────────────────

/// GUC: maximum `owl:sameAs` equivalence-class size before emitting PT550 WARNING (v0.42.0).
pub static SAMEAS_MAX_CLUSTER_SIZE: pgrx::GucSetting<i32> = pgrx::GucSetting::<i32>::new(100_000);

// ─── v0.57.0 Datalog / OWL profile GUCs ──────────────────────────────────────

/// GUC: active OWL reasoning profile: `'RL'` (default), `'EL'`, `'QL'`, or `'off'` (v0.57.0).
pub static OWL_PROFILE: pgrx::GucSetting<Option<std::ffi::CString>> =
    pgrx::GucSetting::<Option<std::ffi::CString>>::new(None);

/// GUC: enable experimental probabilistic Datalog with rule confidence weights (v0.57.0).
pub static PROBABILISTIC_DATALOG: pgrx::GucSetting<bool> = pgrx::GucSetting::<bool>::new(false);

// ─── v0.62.0 Datalog GUCs ────────────────────────────────────────────────────

/// GUC: when on, wrap Datalog stratum-iteration INSERT…SELECT in
/// `run_command_on_all_nodes` for parallel worker execution (v0.62.0 CITUS-27).
/// Requires `citus_sharding_enabled = on`. Default off.
pub static DATALOG_CITUS_DISPATCH: pgrx::GucSetting<bool> = pgrx::GucSetting::<bool>::new(false);

// ─── v0.83.0 Datalog cost-model GUCs (DL-COST-GUC-01) ────────────────────────

/// GUC: synthetic cardinality divisor applied when a Datalog rule atom has the
/// subject position bound to a constant (v0.83.0 DL-COST-GUC-01).
///
/// A larger value makes single-bound atoms appear cheaper, sorting them earlier
/// in the join order.  Useful on datasets where the subject fanout is very low.
/// Replaces the hardcoded `100` divisor in `src/datalog/compiler.rs`.
pub static DATALOG_COST_BOUND_S_DIVISOR: pgrx::GucSetting<i32> = pgrx::GucSetting::<i32>::new(100);

/// GUC: synthetic cardinality divisor applied when a Datalog rule atom has both
/// the subject and object positions bound to constants (v0.83.0 DL-COST-GUC-01).
///
/// A larger value makes dual-bound atoms appear cheaper relative to other atoms.
/// Replaces the hardcoded `10` divisor in `src/datalog/compiler.rs`.
pub static DATALOG_COST_BOUND_SO_DIVISOR: pgrx::GucSetting<i32> = pgrx::GucSetting::<i32>::new(10);

// ─── v0.87.0 Uncertain Knowledge Engine GUCs ─────────────────────────────────

/// GUC: allow probabilistic evaluation on cyclic rule sets (v0.87.0).
/// Cyclic probabilistic rule sets require approximate evaluation; this must be
/// explicitly enabled. Default off.
pub static PROB_DATALOG_CYCLIC: pgrx::GucSetting<bool> = pgrx::GucSetting::<bool>::new(false);

/// GUC: maximum semi-naive inference rounds when `prob_datalog_cyclic = on` (v0.87.0).
/// After this limit, the evaluator emits a WARNING and returns the partial result.
pub static PROB_DATALOG_MAX_ITERATIONS: pgrx::GucSetting<i32> = pgrx::GucSetting::<i32>::new(100);

/// GUC: early-exit threshold for cyclic probabilistic Datalog convergence (v0.87.0).
/// Iteration stops when the maximum confidence delta is below this value.
pub static PROB_DATALOG_CONVERGENCE_DELTA: pgrx::GucSetting<f64> =
    pgrx::GucSetting::<f64>::new(0.001);

/// GUC: when on, promote max-iterations-exceeded from WARNING to ERROR (v0.87.0).
pub static PROB_DATALOG_CYCLIC_STRICT: pgrx::GucSetting<bool> =
    pgrx::GucSetting::<bool>::new(false);

/// GUC: default similarity threshold for `pg:fuzzy_match()` and `pg:confPath()` (v0.87.0).
/// DEPRECATED since v0.89.0; use `pg_ripple.fuzzy_match_threshold` (API-01).
pub static DEFAULT_FUZZY_THRESHOLD: pgrx::GucSetting<f64> = pgrx::GucSetting::<f64>::new(0.7);

/// GUC: canonical name for default fuzzy match threshold (API-01, v0.89.0).
/// Supersedes `pg_ripple.default_fuzzy_threshold` (to be removed in v1.0.0).
pub static FUZZY_MATCH_THRESHOLD: pgrx::GucSetting<f64> = pgrx::GucSetting::<f64>::new(0.7);

/// GUC: enable automatic confidence propagation from PROV-O `pg:sourceTrust` predicates (v0.87.0).
pub static PROV_CONFIDENCE: pgrx::GucSetting<bool> = pgrx::GucSetting::<bool>::new(false);

/// GUC: enable confidence annotations in RDF export functions (v0.87.0).
pub static EXPORT_CONFIDENCE: pgrx::GucSetting<bool> = pgrx::GucSetting::<bool>::new(false);

/// GUC: CONSTRUCT writeback confidence propagation mode: `'explicit'` or `'inherit'` (v0.87.0).
pub static CWB_CONFIDENCE_PROPAGATION: pgrx::GucSetting<Option<std::ffi::CString>> =
    pgrx::GucSetting::<Option<std::ffi::CString>>::new(None);

// ─── v0.100.0 Proof Tree / Derivation Recording GUCs ─────────────────────────

/// GUC: when `true`, the semi-naive inference engine records derivation
/// provenance in `_pg_ripple.derivations` for every newly derived fact.
/// Disabled by default because of the storage and performance overhead.
/// Enable before calling `infer()` / `infer_agg()` when you need `justify()`.
/// (v0.100.0 PROOF-TREE-01)
pub static RECORD_DERIVATIONS: pgrx::GucSetting<bool> = pgrx::GucSetting::<bool>::new(false);

// ─── v0.102.0 Hypothetical Inference GUCs ────────────────────────────────────

/// GUC: maximum number of assertions + retractions allowed in a single
/// `hypothetical_inference()` call.  Exceeding this limit raises PT0450.
/// (v0.102.0 HYPO-01)
pub static HYPOTHETICAL_MAX_ASSERTIONS: pgrx::GucSetting<i32> =
    pgrx::GucSetting::<i32>::new(10_000);

// ─── v0.103.0 Conflict Detection GUCs ────────────────────────────────────────

/// GUC: when `true`, static conflict analysis runs automatically at
/// `load_rules()` time and raises a WARNING for each conflict found (not an
/// error — allows loading of rule sets that have known soft conflicts).
/// (v0.103.0 CONFLICT-01)
pub static RULE_CONFLICT_CHECK_ON_LOAD: pgrx::GucSetting<bool> =
    pgrx::GucSetting::<bool>::new(false);

/// GUC: when `true`, the semi-naive inference engine calls
/// `rule_conflicts(ruleset, 'runtime')` after each fixpoint iteration and
/// raises PT0451 if any conflicts are found, halting inference before
/// committing derived facts.  Off by default.
/// (v0.103.0 CONFLICT-02)
pub static BLOCK_ON_CONFLICT: pgrx::GucSetting<bool> = pgrx::GucSetting::<bool>::new(false);

// ─── v0.108.0 Bayesian Confidence Update GUCs ────────────────────────────────

/// GUC: confidence update strategy for `update_confidence()` (v0.108.0 BAYES-01).
/// `'bayesian'`  — apply Bayes' theorem in odds form (default).
/// `'noisy-or'`  — delegate to the existing v0.87 noisy-OR combiner.
/// `'manual'`    — raise PT0441; caller must set confidence directly via insert_triple().
pub static CONFIDENCE_UPDATE_STRATEGY: pgrx::GucSetting<Option<std::ffi::CString>> =
    pgrx::GucSetting::<Option<std::ffi::CString>>::new(None);

/// GUC: maximum Bayesian cascade depth when propagating a base-fact confidence
/// change downstream through the derivation DAG (v0.108.0 BAYES-02).
/// Facts at depth > max are queued in `_pg_ripple.confidence_stale` for background
/// reprocessing.  Default: 10.
pub static CONFIDENCE_PROPAGATION_MAX_DEPTH: pgrx::GucSetting<i32> =
    pgrx::GucSetting::<i32>::new(10);

/// GUC: polling interval for the confidence reprocessing background worker that
/// drains `_pg_ripple.confidence_stale` (v0.108.0 BAYES-03).
/// Default: `'30 seconds'`.
pub static CONFIDENCE_REPROCESSING_INTERVAL: pgrx::GucSetting<Option<std::ffi::CString>> =
    pgrx::GucSetting::<Option<std::ffi::CString>>::new(None);

/// GUC: retention period for `_pg_ripple.evidence_log` rows (v0.108.0 BAYES-04).
/// Rows older than this are pruned by the background worker.
/// Default: `'1 year'`.
pub static EVIDENCE_LOG_RETENTION: pgrx::GucSetting<Option<std::ffi::CString>> =
    pgrx::GucSetting::<Option<std::ffi::CString>>::new(None);

/// GUC: batch size for `bulk_update_confidence()` — number of evidence rows
/// processed per transaction (v0.108.0 BAYES-05).  Default: 1000.
pub static CONFIDENCE_BATCH_SIZE: pgrx::GucSetting<i32> = pgrx::GucSetting::<i32>::new(1000);

/// GUC: confidence attenuation penalty applied when two conflicting rules are
/// detected.  Attenuation = `1.0 - conflict_severity * penalty` (v0.108.0 BAYES-06).
/// Must be in [0.0, 1.0].  Default: 0.3.
pub static CONFLICT_CONFIDENCE_PENALTY: pgrx::GucSetting<f64> = pgrx::GucSetting::<f64>::new(0.3);

// ─── v0.109.0 NS-RL GUCs ──────────────────────────────────────────────────────

/// GUC: maximum `owl:sameAs` triples a single `resolve_entities()` call may assert (v0.109.0).
/// Calls exceeding this limit raise PT0460.  Default: 1000.
pub static SAMEAS_APPLY_RATE_LIMIT: pgrx::GucSetting<i32> = pgrx::GucSetting::<i32>::new(1_000);

/// GUC: informational bool — `true` when both `pg_trgm` and `fuzzystrmatch` are installed (v0.109.0).
/// Set at CREATE EXTENSION time; read via `current_setting('pg_ripple.string_similarity_extensions_ok')`.
/// Default: `false`.
pub static STRING_SIMILARITY_EXTENSIONS_OK: pgrx::GucSetting<bool> =
    pgrx::GucSetting::<bool>::new(false);

// ─── v0.110.0 NS-RL Evaluation & Explainability GUCs ─────────────────────────

/// GUC: when `true`, any `owl:sameAs` assertion that would exceed
/// `pg_ripple.sameas_max_cluster_size` is logged to `_pg_ripple.sameas_anomaly_log`
/// before PT550 is raised (v0.110.0 ANOMALY-01).  Default: `true`.
pub static RECORD_SAMEAS_ANOMALIES: pgrx::GucSetting<bool> = pgrx::GucSetting::<bool>::new(true);

/// GUC: retention period for `_pg_ripple.sameas_anomaly_log` rows (v0.110.0 ANOMALY-02).
/// Rows older than this are pruned by a background worker pass.
/// Default: `'90 days'` (stored as a string, cast to INTERVAL at runtime).
pub static SAMEAS_ANOMALY_LOG_RETENTION: pgrx::GucSetting<Option<std::ffi::CString>> =
    pgrx::GucSetting::<Option<std::ffi::CString>>::new(None);

/// GUC: TTL for cached `explain_rule()` results in `_pg_ripple.rule_explanations`
/// (v0.110.0 EXPLAIN-01).  Default: `'24 hours'`.
pub static RULE_EXPLANATION_CACHE_TTL: pgrx::GucSetting<Option<std::ffi::CString>> =
    pgrx::GucSetting::<Option<std::ffi::CString>>::new(None);
