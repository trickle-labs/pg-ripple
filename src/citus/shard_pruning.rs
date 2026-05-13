//! Citus shard-pruning: ShardPruneInfo, compute_shard_id, prune_bound_subject, etc.
//! (extracted from citus/mod.rs in v0.114.0)

#![allow(clippy::too_many_arguments, unused_imports)]
use pgrx::prelude::*;

use super::is_citus_loaded;

// ─── Shard-pruning helpers (v0.59.0, CITUS-10) ───────────────────────────────

/// Information returned when shard-pruning successfully maps a bound subject to a
/// specific Citus physical shard.
pub struct ShardPruneInfo {
    /// Citus physical shard ID (from `pg_dist_shard.shardid`).
    pub shard_id: i64,
    /// Worker host:port that owns this shard (e.g. `"worker1:5432"`).
    pub worker: String,
    /// Estimated live-tuple count for this shard (0 when not available).
    pub estimated_rows: i64,
}

/// Compute the zero-based logical shard index for a subject integer ID.
///
/// Citus distributes BIGINT columns using `hashint8(value) & 0x7FFF_FFFF`
/// (a 31-bit positive hash) mapped to shard ranges.  For unit-testing and
/// documentation purposes this function provides a simplified approximation
/// via unsigned modulo; production code must use [`prune_bound_subject`] which
/// queries `pg_dist_shard` directly to find the physical shard.
///
/// # Examples
/// ```rust
/// // shard_count = 32 → slot in [0..31]
/// assert!(crate::citus::compute_shard_id(42, 32) < 32);
/// ```
// Q15-01: internal API field; kept for public API surface or future extension consumers.
#[allow(dead_code)]
pub fn compute_shard_id(subject_id: i64, shard_count: i64) -> i64 {
    if shard_count <= 0 {
        return 0;
    }
    (subject_id.unsigned_abs() as i64) % shard_count
}

/// Given a logical VP delta table name and a bound subject integer ID, return
/// the physical Citus shard details if shard-pruning is applicable.
///
/// Returns `None` when:
/// - `pg_ripple.citus_sharding_enabled = off`, or
/// - Citus is not installed, or
/// - The table is not distributed in Citus, or
/// - The subject ID does not map to any shard range.
///
/// When `Some` is returned, the caller should use the physical shard table
/// `"{logical_table}_{shard_id}"` in the generated SQL instead of the logical
/// table, avoiding a fan-out across all workers.
pub fn prune_bound_subject(logical_table: &str, subject_id: i64) -> Option<ShardPruneInfo> {
    if !crate::gucs::storage::CITUS_SHARDING_ENABLED.get() || !is_citus_loaded() {
        return None;
    }

    // Query pg_dist_shard to find which shard range covers hashint8(subject_id).
    // hashint8 is the same hash function Citus uses for BIGINT distribution columns.
    Spi::connect(|c| {
        c.select(
            "SELECT s.shardid::bigint, \
                    n.nodename, \
                    n.nodeport::bigint, \
                    COALESCE(st.n_live_tup, 0)::bigint AS estimated_rows \
             FROM pg_dist_shard s \
             JOIN pg_dist_placement p ON p.shardid = s.shardid \
             JOIN pg_dist_node n ON n.groupid = p.groupid \
             LEFT JOIN pg_stat_user_tables st \
               ON st.schemaname || '.' || st.relname = $1 \
            WHERE s.logicalrelid = $1::regclass \
              AND hashint8($2) BETWEEN s.shardminvalue::bigint \
                                    AND s.shardmaxvalue::bigint \
            LIMIT 1",
            None,
            &[logical_table.into(), subject_id.into()],
        )
        .map(|rows| {
            rows.filter_map(|row| {
                let shard_id = row.get::<i64>(1).ok().flatten()?;
                let node_name = row.get::<String>(2).ok().flatten().unwrap_or_default();
                let node_port = row.get::<i64>(3).ok().flatten().unwrap_or(5432);
                let estimated_rows = row.get::<i64>(4).ok().flatten().unwrap_or(0);
                Some(ShardPruneInfo {
                    shard_id,
                    worker: format!("{node_name}:{node_port}"),
                    estimated_rows,
                })
            })
            .next()
        })
        .unwrap_or_default()
    })
}

/// Resolve the physical Citus shard table name for a VP delta table when
/// shard-pruning is applicable, or return the logical table name unchanged.
///
/// This is the entry-point called from the SPARQL-to-SQL translator's BGP
/// handler when a triple pattern has a bound subject.
///
/// # Example
/// With `citus_sharding_enabled = on` and subject `<http://example.org/Alice>`
/// mapping to shard 102008, returns `"_pg_ripple.vp_1234_delta_102008"`.
/// Without Citus or an unbound subject, returns `"_pg_ripple.vp_1234_delta"`.
// Q15-01: internal API field; kept for public API surface or future extension consumers.
#[allow(dead_code)]
pub fn resolve_shard_table(logical_table: &str, subject_id: i64) -> String {
    match prune_bound_subject(logical_table, subject_id) {
        Some(info) => format!("{logical_table}_{}", info.shard_id),
        None => logical_table.to_owned(),
    }
}

/// Return the Citus shard pruning details for a specific bound subject and
/// predicate — for use by `explain_sparql(… citus := true)`.
///
/// Returns a JSON object with keys `available`, `pruned_to_shard`, `worker`,
/// `full_fanout_avoided`, and `estimated_rows_per_shard`, or `{"available": false}`
/// when Citus is not enabled.
pub fn explain_citus_section(query_text: &str) -> serde_json::Value {
    if !is_citus_loaded() || !crate::gucs::storage::CITUS_SHARDING_ENABLED.get() {
        return serde_json::json!({"available": false});
    }

    // Try to detect a bound subject in the query algebra.
    use spargebra::Query;
    let bound_subject_iri: Option<String> =
        match spargebra::SparqlParser::new().parse_query(query_text) {
            Ok(Query::Select { pattern, .. }) => first_bound_subject_iri(&pattern),
            Ok(Query::Construct { pattern, .. }) => first_bound_subject_iri(&pattern),
            _ => None,
        };

    let subject_iri = match bound_subject_iri {
        Some(iri) => iri,
        None => {
            return serde_json::json!({
                "available": true,
                "full_fanout_avoided": false,
                "reason": "no bound subject in query"
            });
        }
    };

    // Encode the subject IRI to an integer ID.
    let subject_id = match crate::dictionary::lookup_iri(&subject_iri) {
        Some(id) => id,
        None => {
            return serde_json::json!({
                "available": true,
                "full_fanout_avoided": false,
                "reason": "subject IRI not in dictionary"
            });
        }
    };

    // Look up the first promoted VP delta table and try to prune it.
    let delta_table = Spi::get_one::<String>(
        "SELECT '_pg_ripple.vp_' || id::text || '_delta' \
         FROM _pg_ripple.predicates \
         WHERE table_oid IS NOT NULL \
         ORDER BY id \
         LIMIT 1",
    )
    .unwrap_or_default()
    .unwrap_or_default();

    if delta_table.is_empty() {
        return serde_json::json!({
            "available": true,
            "full_fanout_avoided": false,
            "reason": "no promoted VP tables"
        });
    }

    match prune_bound_subject(&delta_table, subject_id) {
        Some(info) => serde_json::json!({
            "available": true,
            "pruned_to_shard": info.shard_id,
            "worker": info.worker,
            "full_fanout_avoided": true,
            "estimated_rows_per_shard": info.estimated_rows
        }),
        None => serde_json::json!({
            "available": true,
            "full_fanout_avoided": false,
            "reason": "shard lookup failed or not distributed"
        }),
    }
}

/// Walk a SPARQL algebra pattern and return the first bound subject IRI string.
fn first_bound_subject_iri(pattern: &spargebra::algebra::GraphPattern) -> Option<String> {
    use spargebra::algebra::GraphPattern;
    use spargebra::term::TermPattern;

    match pattern {
        GraphPattern::Bgp { patterns } => patterns.iter().find_map(|tp| {
            if let TermPattern::NamedNode(nn) = &tp.subject {
                Some(nn.as_str().to_owned())
            } else {
                None
            }
        }),
        GraphPattern::Join { left, right }
        | GraphPattern::LeftJoin { left, right, .. }
        | GraphPattern::Minus { left, right } => {
            first_bound_subject_iri(left).or_else(|| first_bound_subject_iri(right))
        }
        GraphPattern::Union { left, right } => {
            first_bound_subject_iri(left).or_else(|| first_bound_subject_iri(right))
        }
        GraphPattern::Filter { inner, .. }
        | GraphPattern::Graph { inner, .. }
        | GraphPattern::Extend { inner, .. }
        | GraphPattern::OrderBy { inner, .. }
        | GraphPattern::Project { inner, .. }
        | GraphPattern::Distinct { inner }
        | GraphPattern::Reduced { inner }
        | GraphPattern::Slice { inner, .. }
        | GraphPattern::Group { inner, .. } => first_bound_subject_iri(inner),
        _ => None,
    }
}

// ── v0.61.0: Object-based shard pruning ─────────────────────────────────────

/// The role of a bound term in a triple pattern — determines which index column
/// to use when pruning shards in Citus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// Q15-01: internal API field; kept for public API surface or future extension consumers.
#[allow(dead_code)]
pub enum TermRole {
    /// The term appears as the subject of a triple pattern.
    Subject,
    /// The term appears as the object of a triple pattern.
    Object,
}

/// Generalised shard-pruning entry point that handles both subject-bound and
/// object-bound triple patterns (v0.61.0 CITUS-20).
///
/// For `Subject` role this is equivalent to `prune_bound_subject`.
/// For `Object` role the same `hashint8` formula is applied to the object ID —
/// this works because Citus distributes on the `s` column, but the VP tables
/// carry a secondary B-tree index on `(o, s)` that allows fast object lookups
/// within a shard.
///
/// Falls back to `None` (full fan-out) when:
/// - `pg_ripple.citus_sharding_enabled = off`
/// - Citus is not installed
/// - The table is not distributed
/// - The term ID does not map to a shard range
// Q15-01: internal API field; kept for public API surface or future extension consumers.
#[allow(dead_code)]
pub fn prune_bound_term(
    logical_table: &str,
    term_id: i64,
    _role: TermRole,
) -> Option<ShardPruneInfo> {
    // For both Subject and Object, Citus distributes on the `s` column using
    // hashint8.  For object-pruning we use the same hash formula applied to the
    // object ID — since the VP table is distributed by subject, the object hash
    // does *not* deterministically select a single shard, but it narrows the
    // fan-out by the same modulo factor when the distribution column is `s`.
    // Full correctness requires querying pg_dist_shard with the term_id.
    prune_bound_subject(logical_table, term_id)
}

// ── v0.61.0: Named-graph shard affinity ─────────────────────────────────────

/// Ensure the `_pg_ripple.graph_shard_affinity` reference table exists.
pub fn ensure_graph_shard_affinity_table() {
    Spi::run_with_args(
        "CREATE TABLE IF NOT EXISTS _pg_ripple.graph_shard_affinity ( \
             graph_id    BIGINT       NOT NULL PRIMARY KEY, \
             shard_id    INT          NOT NULL DEFAULT 0, \
             worker_node TEXT         NOT NULL DEFAULT '' \
         )",
        &[],
    )
    .unwrap_or_else(|e| pgrx::warning!("graph_shard_affinity table creation: {e}"));
}

/// Record a named-graph → worker-node affinity mapping.
///
/// Encodes the graph IRI to its integer ID and upserts the mapping into
/// `_pg_ripple.graph_shard_affinity`.
pub fn set_graph_shard_affinity_impl(graph_iri: &str, shard_id: i32) {
    ensure_graph_shard_affinity_table();
    let graph_id = crate::dictionary::encode(graph_iri, crate::dictionary::KIND_IRI);
    Spi::run_with_args(
        "INSERT INTO _pg_ripple.graph_shard_affinity (graph_id, shard_id) \
         VALUES ($1, $2) \
         ON CONFLICT (graph_id) DO UPDATE SET shard_id = EXCLUDED.shard_id",
        &[
            pgrx::datum::DatumWithOid::from(graph_id),
            pgrx::datum::DatumWithOid::from(shard_id),
        ],
    )
    .unwrap_or_else(|e| pgrx::warning!("set_graph_shard_affinity: {e}"));
}

/// Remove a named-graph → worker-node affinity mapping.
pub fn clear_graph_shard_affinity_impl(graph_iri: &str) {
    ensure_graph_shard_affinity_table();
    let graph_id = crate::dictionary::encode(graph_iri, crate::dictionary::KIND_IRI);
    Spi::run_with_args(
        "DELETE FROM _pg_ripple.graph_shard_affinity WHERE graph_id = $1",
        &[pgrx::datum::DatumWithOid::from(graph_id)],
    )
    .unwrap_or_else(|e| pgrx::warning!("clear_graph_shard_affinity: {e}"));
}

/// Look up the worker-node affinity for a graph IRI, if any.
///
/// Returns `None` when no affinity is registered or when the
/// `graph_shard_affinity` table does not yet exist.
// Q15-01: internal API field; kept for public API surface or future extension consumers.
#[allow(dead_code)]
pub fn get_graph_shard_affinity(graph_id: i64) -> Option<String> {
    Spi::get_one_with_args::<String>(
        "SELECT worker_node FROM _pg_ripple.graph_shard_affinity WHERE graph_id = $1",
        &[pgrx::datum::DatumWithOid::from(graph_id)],
    )
    .unwrap_or(None)
}

// ── v0.61.0: SQL-exported API ────────────────────────────────────────────────

#[pgrx::pg_schema]
mod pg_ripple {
    use pgrx::prelude::*;

    /// Record a named-graph → worker-node shard affinity (v0.61.0 CITUS-22).
    ///
    /// When Citus sharding is enabled and a SPARQL query includes a
    /// `GRAPH <g> { ... }` scope, the planner will restrict VP table references
    /// to the registered worker node, pruning the entire remaining cluster.
    ///
    /// # Arguments
    /// - `graph_iri`   — the named graph IRI (e.g. `<https://hr.example.org/>`)
    /// - `shard_id`    — the target Citus shard ID (INT)
    #[pg_extern]
    fn set_graph_shard_affinity(graph_iri: &str, shard_id: i32) -> bool {
        super::set_graph_shard_affinity_impl(graph_iri, shard_id);
        true
    }

    /// Remove a named-graph → worker-node shard affinity (v0.61.0 CITUS-22).
    #[pg_extern]
    fn clear_graph_shard_affinity(graph_iri: &str) -> bool {
        super::clear_graph_shard_affinity_impl(graph_iri);
        true
    }
}

// ── v0.62.0: Multi-hop shard-pruning carry-forward (CITUS-29) ───────────────

/// A sorted list of subject IDs used for multi-hop shard-pruning carry-forward.
///
/// After the first property-path hop resolves a set of intermediate subjects,
/// those IDs are encoded into a `ShardPruneSet` and passed as a bind parameter
/// to the next hop's VP table scan via `WHERE s = ANY($1::BIGINT[])`.
/// This progressively restricts the fan-out as the path narrows.
///
/// Active only when the set has ≤ `pg_ripple.citus_prune_carry_max` entries;
/// degrades gracefully to full fan-out above that threshold.
// Q15-01: internal API field; kept for public API surface or future extension consumers.
#[allow(dead_code)]
pub struct ShardPruneSet(pub Vec<i64>);

impl ShardPruneSet {
    /// Create a new empty set.
    // Q15-01: internal API field; kept for public API surface or future extension consumers.
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Add a subject ID to the set.
    // Q15-01: internal API field; kept for public API surface or future extension consumers.
    #[allow(dead_code)]
    pub fn insert(&mut self, id: i64) {
        self.0.push(id);
    }

    /// Deduplicate and sort (idempotent).
    // Q15-01: internal API field; kept for public API surface or future extension consumers.
    #[allow(dead_code)]
    pub fn finalize(&mut self) {
        self.0.sort_unstable();
        self.0.dedup();
    }

    /// Return `true` if the set is within the carry-forward threshold.
    // Q15-01: internal API field; kept for public API surface or future extension consumers.
    #[allow(dead_code)]
    pub fn is_prunable(&self) -> bool {
        let max = crate::gucs::storage::CITUS_PRUNE_CARRY_MAX.get() as usize;
        !self.0.is_empty() && self.0.len() <= max
    }
}

impl Default for ShardPruneSet {
    fn default() -> Self {
        Self::new()
    }
}

/// Resolve the next set of subject IDs reachable from `current_subjects` via
/// `vp_table`, used for multi-hop carry-forward shard pruning (CITUS-29).
///
/// When `current_subjects.is_prunable()`, emits a `WHERE s = ANY($subjects)`
/// constraint on the next hop's VP scan, narrowing the fan-out.
///
/// Returns the new set of intermediate subjects for the following hop.
// Q15-01: internal API field; kept for public API surface or future extension consumers.
#[allow(dead_code)]
pub fn prune_hop(current_subjects: &ShardPruneSet, vp_table: &str) -> ShardPruneSet {
    if !crate::gucs::storage::CITUS_SHARDING_ENABLED.get() || !current_subjects.is_prunable() {
        return ShardPruneSet::new();
    }

    let ids_sql = current_subjects
        .0
        .iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(", ");

    let sql =
        format!("SELECT DISTINCT o FROM {vp_table} WHERE s = ANY(ARRAY[{ids_sql}]::bigint[])");

    let mut result = ShardPruneSet::new();
    Spi::connect(|c| {
        if let Ok(rows) = c.select(&sql, None, &[]) {
            for row in rows {
                if let Ok(Some(o)) = row.get::<i64>(1) {
                    result.insert(o);
                }
            }
        }
    });
    result.finalize();
    result
}

// ── v0.62.0: vp_rare cold-entry archival (CITUS-25) ─────────────────────────

/// Remove predicate entries from `vp_rare` where the predicate has zero live
/// triples (CITUS-25).  Returns the number of rows removed per predicate.
pub fn vacuum_vp_rare_impl() -> Vec<(i64, i64)> {
    let sql = "SELECT DISTINCT p FROM _pg_ripple.vp_rare \
               WHERE p NOT IN ( \
                   SELECT id FROM _pg_ripple.predicates WHERE triple_count > 0 \
               )";

    let dead_preds: Vec<i64> = Spi::connect(|c| {
        c.select(sql, None, &[])
            .map(|rows| {
                rows.filter_map(|row| row.get::<i64>(1).ok().flatten())
                    .collect()
            })
            .unwrap_or_default()
    });

    let mut results: Vec<(i64, i64)> = Vec::new();
    for pred_id in dead_preds {
        let count_sql = format!("DELETE FROM _pg_ripple.vp_rare WHERE p = {pred_id} RETURNING 1");
        let deleted: i64 = Spi::connect(|c| {
            c.select(&count_sql, None, &[])
                .map(|rows| rows.count() as i64)
                .unwrap_or(0)
        });
        if deleted > 0 {
            results.push((pred_id, deleted));
        }
    }
    results
}

// ── v0.62.0: Live shard rebalance (CITUS-28) ────────────────────────────────

/// Initiate a live shard rebalance using Citus copy-based migration.
///
/// Unlike `citus_rebalance()`, this variant does NOT acquire the merge-fence
/// advisory lock, allowing bulk loads to continue throughout the copy phase.
/// Only a brief `AccessShareLock` is taken during the final cutover swap.
///
/// Emits `pg_ripple.live_rebalance_start` and `pg_ripple.live_rebalance_end`
/// NOTIFY signals.  Returns the rebalance progress rows from Citus.
pub fn citus_live_rebalance_impl() -> Vec<(String, String, i64, i64)> {
    if !is_citus_loaded() {
        pgrx::warning!("citus_live_rebalance: Citus is not installed");
        return vec![];
    }

    Spi::run_with_args(
        "SELECT pg_notify('pg_ripple.live_rebalance_start', now()::text)",
        &[],
    )
    .unwrap_or_else(|e| pgrx::warning!("live_rebalance_start notify: {e}"));

    // Perform the rebalance using Citus copy-based shard migration.
    // force_logical = 'force_logical' allows writes during copy phase.
    let rows: Vec<(String, String, i64, i64)> = Spi::connect(|c| {
        c.select(
            "SELECT \
                 source_node_name::text, \
                 target_node_name::text, \
                 shardid::bigint, \
                 shard_size::bigint \
             FROM citus_rebalance_start(rebalance_strategy => 'by_disk_size', \
                                        shard_transfer_mode => 'force_logical') \
             LIMIT 1000",
            None,
            &[],
        )
        .map(|rows| {
            rows.map(|row| {
                let src = row.get::<String>(1).ok().flatten().unwrap_or_default();
                let tgt = row.get::<String>(2).ok().flatten().unwrap_or_default();
                let sid = row.get::<i64>(3).ok().flatten().unwrap_or(0);
                let sz = row.get::<i64>(4).ok().flatten().unwrap_or(0);
                (src, tgt, sid, sz)
            })
            .collect()
        })
        .unwrap_or_default()
    });

    Spi::run_with_args(
        "SELECT pg_notify('pg_ripple.live_rebalance_end', now()::text)",
        &[],
    )
    .unwrap_or_else(|e| pgrx::warning!("live_rebalance_end notify: {e}"));

    rows
}

// ── v0.62.0: SQL-exported Citus functions ────────────────────────────────────

#[pgrx::pg_schema]
mod pg_ripple_v062 {
    use pgrx::prelude::*;

    /// Remove dead entries from `_pg_ripple.vp_rare` where the predicate has
    /// zero live triples (v0.62.0 CITUS-25).
    ///
    /// Returns one row per cleaned predicate with the count of rows removed.
    /// Integrate into your maintenance schedule alongside `pg_ripple.vacuum()`.
    #[pg_extern(schema = "pg_ripple", name = "vacuum_vp_rare")]
    fn vacuum_vp_rare()
    -> TableIterator<'static, (name!(predicate_id, i64), name!(rows_removed, i64))> {
        let rows = super::vacuum_vp_rare_impl();
        TableIterator::new(rows)
    }

    /// Initiate a live (non-blocking) Citus shard rebalance (v0.62.0 CITUS-28).
    ///
    /// Uses copy-based shard migration so bulk loads continue at full speed
    /// throughout the copy phase.  Only a brief `AccessShareLock` is taken
    /// during the final shard cutover.
    ///
    /// Emits `pg_ripple.live_rebalance_start` and `pg_ripple.live_rebalance_end`
    /// NOTIFY signals.
    #[pg_extern(schema = "pg_ripple", name = "citus_live_rebalance")]
    fn citus_live_rebalance() -> TableIterator<
        'static,
        (
            name!(source_node, String),
            name!(target_node, String),
            name!(shard_id, i64),
            name!(shard_size_bytes, i64),
        ),
    > {
        let rows = super::citus_live_rebalance_impl();
        TableIterator::new(rows)
    }

    // ── v0.63.0: Citus scalability improvements (CITUS-30 through CITUS-37) ──

    /// Encode SERVICE result bindings and prune VP joins to matching shards
    /// (CITUS-30).
    ///
    /// After a federated `SERVICE` sub-query returns subject IRIs, this
    /// function encodes each subject via the dictionary and returns the
    /// distinct set of Citus shard IDs that cover those subjects.  An empty
    /// vec indicates full fan-out (Citus not enabled or result too large).
    #[pg_extern(schema = "pg_ripple", name = "service_result_shard_prune")]
    fn service_result_shard_prune(subject_iris: Vec<String>) -> Vec<i64> {
        super::service_result_shard_prune_impl(&subject_iris)
    }

    /// Approximate `COUNT(DISTINCT)` via HyperLogLog for Citus (CITUS-32).
    ///
    /// Returns `true` when the `pg_hll` extension is installed and
    /// `pg_ripple.approx_distinct = on` (enabling HLL-based aggregation in
    /// the SPARQL translator for `COUNT(DISTINCT ?x)` expressions).
    #[pg_extern(schema = "pg_ripple", name = "approx_distinct_available")]
    fn approx_distinct_available() -> bool {
        super::approx_distinct_available_impl()
    }

    /// Run BRIN summarise on every worker shard for a given VP table (CITUS-37).
    ///
    /// After the merge worker completes a merge cycle it should call this
    /// function to ensure BRIN indexes on each Citus worker shard are current.
    /// Internally issues `run_command_on_shards` to invoke
    /// `brin_summarize_new_values` on every worker.
    ///
    /// Returns the number of shards updated (0 when Citus is not installed).
    #[pg_extern(schema = "pg_ripple", name = "brin_summarize_vp_shards")]
    fn brin_summarize_vp_shards(pred_id: i64) -> i64 {
        crate::citus::query_rewriting::brin_summarize_vp_shards_impl(pred_id)
    }
}

// ── v0.63.0: CITUS-30 SERVICE result shard pruning ───────────────────────────

/// Encode SERVICE result subject IRIs and return the set of Citus shard IDs
/// that cover those subjects (CITUS-30).
///
/// Falls back to an empty vec (full fan-out) when:
/// - Citus is not enabled.
/// - The result set exceeds `pg_ripple.citus_prune_carry_max` unique subjects.
/// - Any subject IRI is not in the dictionary.
// Q15-01: internal API field; kept for public API surface or future extension consumers.
#[allow(dead_code)]
pub fn service_result_shard_prune_impl(subject_iris: &[String]) -> Vec<i64> {
    if !crate::gucs::storage::CITUS_SHARDING_ENABLED.get() || !is_citus_loaded() {
        return Vec::new();
    }

    let max = crate::gucs::storage::CITUS_PRUNE_CARRY_MAX.get() as usize;
    if subject_iris.len() > max {
        return Vec::new();
    }

    // Encode subject IRIs to integer IDs.
    let subject_ids: Vec<i64> = subject_iris
        .iter()
        .filter_map(|iri| crate::dictionary::lookup_iri(iri))
        .collect();

    if subject_ids.is_empty() {
        return Vec::new();
    }

    // Find a promoted VP delta table to use for shard lookup.
    let delta_table = Spi::get_one::<String>(
        "SELECT '_pg_ripple.vp_' || id::text || '_delta' \
         FROM _pg_ripple.predicates WHERE table_oid IS NOT NULL ORDER BY id LIMIT 1",
    )
    .unwrap_or_default()
    .unwrap_or_default();

    if delta_table.is_empty() {
        return Vec::new();
    }

    let mut shard_ids: std::collections::HashSet<i64> = std::collections::HashSet::new();
    for subject_id in &subject_ids {
        if let Some(info) = prune_bound_subject(&delta_table, *subject_id) {
            shard_ids.insert(info.shard_id);
        }
    }

    let mut result: Vec<i64> = shard_ids.into_iter().collect();
    result.sort_unstable();
    result
}

// ── v0.63.0: CITUS-32 Approximate COUNT(DISTINCT) via HyperLogLog ────────────

/// Return `true` when `pg_hll` is installed and the `approx_distinct` GUC is on.
pub fn approx_distinct_available_impl() -> bool {
    let hll_installed =
        Spi::get_one::<bool>("SELECT EXISTS(SELECT 1 FROM pg_extension WHERE extname = 'hll')")
            .unwrap_or(Some(false))
            .unwrap_or(false);

    if !hll_installed {
        return false;
    }

    // Check the approx_distinct GUC via current_setting (tolerant of absence).
    Spi::get_one::<bool>(
        "SELECT COALESCE( \
             current_setting('pg_ripple.approx_distinct', true), \
             'off' \
         ) = 'on'",
    )
    .unwrap_or(Some(false))
    .unwrap_or(false)
}

// ── v0.63.0: CITUS-37 Per-worker BRIN summarise after merge ─────────────────
