//! Temporal graph snapshots — v0.125.0 (FEAT-02).
//!
//! `graph_at(graph_iri, snapshot_time)` — materialise a named-graph snapshot
//! from `_pg_ripple.temporal_facts` and register it in
//! `_pg_ripple.graph_snapshots`.
//!
//! `graph_diff(graph_iri, from_ts, to_ts)` — return the triple-level delta
//! between two temporal views of a named graph.
//!
//! `prune_expired_snapshots()` — housekeeping helper invoked from the merge
//! background worker on each tick.
//!
//! `graph_snapshots_count()` — return the live snapshot count for the HTTP
//! companion Prometheus gauge.
//!
//! The `_pg_ripple.graph_snapshots` catalog table and `snapshot_id_seq`
//! sequence are created at `CREATE EXTENSION` time by the
//! `v0125_graph_snapshots` `extension_sql!` block in `src/schema/tables.rs`.

use pgrx::prelude::*;

// ─── Schema initialisation (idempotent fallback) ─────────────────────────────

/// Create `_pg_ripple.graph_snapshots` catalog and `snapshot_id_seq`.
///
/// Called from `initialize_schema()` (idempotent via `IF NOT EXISTS`).
pub fn initialize_graph_snapshots_schema() {
    Spi::run_with_args(
        "CREATE SEQUENCE IF NOT EXISTS _pg_ripple.snapshot_id_seq",
        &[],
    )
    .unwrap_or_else(|e| pgrx::warning!("snapshot_id_seq creation: {e}"));

    Spi::run_with_args(
        "CREATE TABLE IF NOT EXISTS _pg_ripple.graph_snapshots ( \
             snapshot_id  BIGINT      NOT NULL DEFAULT nextval('_pg_ripple.snapshot_id_seq') \
                          PRIMARY KEY, \
             graph_iri    TEXT        NOT NULL, \
             snapshot_iri TEXT        NOT NULL UNIQUE, \
             captured_at  TIMESTAMPTZ NOT NULL, \
             triple_count BIGINT, \
             expires_at   TIMESTAMPTZ \
         )",
        &[],
    )
    .unwrap_or_else(|e| pgrx::warning!("graph_snapshots creation: {e}"));

    Spi::run_with_args(
        "CREATE INDEX IF NOT EXISTS idx_graph_snapshots_graph_iri \
         ON _pg_ripple.graph_snapshots (graph_iri, captured_at DESC)",
        &[],
    )
    .unwrap_or_else(|e| pgrx::warning!("graph_snapshots graph_iri index: {e}"));

    Spi::run_with_args(
        "CREATE INDEX IF NOT EXISTS idx_graph_snapshots_expires_at \
         ON _pg_ripple.graph_snapshots (expires_at) \
         WHERE expires_at IS NOT NULL",
        &[],
    )
    .unwrap_or_else(|e| pgrx::warning!("graph_snapshots expires_at index: {e}"));
}

// ─── SQL functions ────────────────────────────────────────────────────────────

/// `pg_ripple.graph_at(graph_iri, snapshot_time)` — materialise a named-graph
/// snapshot from `_pg_ripple.temporal_facts` at the given timestamp and register
/// it in `_pg_ripple.graph_snapshots`.
///
/// Returns the snapshot IRI (a `urn:snapshot:…` string) that can be used
/// directly in `GRAPH <snapshot_iri> { … }` SPARQL queries.
///
/// The snapshot captures all temporal facts for `graph_iri` whose validity
/// interval contains `snapshot_time` (i.e. `valid_from <= snapshot_time AND
/// (valid_to IS NULL OR valid_to > snapshot_time)`).
///
/// The `expires_at` timestamp is set to `snapshot_time +
/// pg_ripple.snapshot_retention_days` days; set `snapshot_retention_days = 0`
/// to keep snapshots indefinitely.
#[pg_extern(schema = "pg_ripple")]
pub fn graph_at(graph_iri: &str, snapshot_time: pgrx::datum::TimestampWithTimeZone) -> String {
    let g_id = crate::dictionary::encode(graph_iri, 0);

    // Build a deterministic snapshot IRI from the graph IRI + timestamp.
    let ts_str: String = Spi::get_one_with_args::<String>(
        "SELECT to_char($1 AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')",
        &[pgrx::datum::DatumWithOid::from(snapshot_time)],
    )
    .unwrap_or(None)
    .unwrap_or_else(|| "unknown".to_owned());

    // Sanitise graph_iri for inclusion in the URN.
    let iri_slug: String = graph_iri
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let snapshot_iri = format!("urn:snapshot:{iri_slug}:{ts_str}");

    // Count the temporal facts valid at snapshot_time for this graph.
    let triple_count: i64 = Spi::get_one_with_args::<i64>(
        "SELECT COUNT(*)::bigint FROM _pg_ripple.temporal_facts \
         WHERE g = $1 AND valid_from <= $2 \
           AND (valid_to IS NULL OR valid_to > $2)",
        &[
            pgrx::datum::DatumWithOid::from(g_id),
            pgrx::datum::DatumWithOid::from(snapshot_time),
        ],
    )
    .unwrap_or(None)
    .unwrap_or(0);

    // Compute expires_at based on the retention GUC.
    // $3 is snapshot_time (captured_at); expires_at = snapshot_time + N days.
    let retention_days = crate::gucs::storage::SNAPSHOT_RETENTION_DAYS.get();
    let expires_at_expr = if retention_days > 0 {
        format!("$3 + interval '{retention_days} days'")
    } else {
        "NULL::TIMESTAMPTZ".to_owned()
    };

    let insert_sql = format!(
        "INSERT INTO _pg_ripple.graph_snapshots \
           (graph_iri, snapshot_iri, captured_at, triple_count, expires_at) \
         VALUES ($1, $2, $3, $4, {expires_at_expr}) \
         ON CONFLICT (snapshot_iri) DO UPDATE \
           SET triple_count = EXCLUDED.triple_count, \
               captured_at  = EXCLUDED.captured_at, \
               expires_at   = EXCLUDED.expires_at \
         RETURNING snapshot_iri"
    );

    let returned: Option<String> = Spi::get_one_with_args::<String>(
        &insert_sql,
        &[
            pgrx::datum::DatumWithOid::from(graph_iri),
            pgrx::datum::DatumWithOid::from(snapshot_iri.as_str()),
            pgrx::datum::DatumWithOid::from(snapshot_time),
            pgrx::datum::DatumWithOid::from(triple_count),
        ],
    )
    .unwrap_or(None);

    returned.unwrap_or(snapshot_iri)
}

/// `pg_ripple.graph_diff(graph_iri, from_ts, to_ts)` — return the delta between
/// two temporal snapshots of a named graph as a set of `(s, p, o, change)` rows.
///
/// `change` is `'added'` for facts that are present at `to_ts` but not at
/// `from_ts`, and `'removed'` for facts present at `from_ts` but not at
/// `to_ts`.  Facts present at both timestamps are not returned.
///
/// All IDs are dictionary-encoded `BIGINT` values.  Callers can decode them
/// with `pg_ripple.decode(id)`.
#[pg_extern(schema = "pg_ripple")]
pub fn graph_diff(
    graph_iri: &str,
    from_ts: pgrx::datum::TimestampWithTimeZone,
    to_ts: pgrx::datum::TimestampWithTimeZone,
) -> TableIterator<
    'static,
    (
        name!(s, i64),
        name!(p, i64),
        name!(o, i64),
        name!(change, String),
    ),
> {
    let g_id = crate::dictionary::encode(graph_iri, 0);

    let rows: Vec<(i64, i64, i64, String)> = Spi::connect(|client| {
        let result = client.select(
            "SELECT s, p, o, change FROM ( \
               SELECT s, p, o, 'added'::text AS change \
               FROM _pg_ripple.temporal_facts \
               WHERE g = $1 AND valid_from <= $3 \
                 AND (valid_to IS NULL OR valid_to > $3) \
               EXCEPT \
               SELECT s, p, o, 'added'::text AS change \
               FROM _pg_ripple.temporal_facts \
               WHERE g = $1 AND valid_from <= $2 \
                 AND (valid_to IS NULL OR valid_to > $2) \
               UNION ALL \
               SELECT s, p, o, 'removed'::text AS change \
               FROM _pg_ripple.temporal_facts \
               WHERE g = $1 AND valid_from <= $2 \
                 AND (valid_to IS NULL OR valid_to > $2) \
               EXCEPT \
               SELECT s, p, o, 'removed'::text AS change \
               FROM _pg_ripple.temporal_facts \
               WHERE g = $1 AND valid_from <= $3 \
                 AND (valid_to IS NULL OR valid_to > $3) \
             ) delta \
             ORDER BY change, s, p, o",
            None,
            &[
                pgrx::datum::DatumWithOid::from(g_id),
                pgrx::datum::DatumWithOid::from(from_ts),
                pgrx::datum::DatumWithOid::from(to_ts),
            ],
        );
        match result {
            Ok(tup_table) => tup_table
                .into_iter()
                .filter_map(|row| {
                    let s: i64 = row.get_by_name::<i64, _>("s").ok()??;
                    let p: i64 = row.get_by_name::<i64, _>("p").ok()??;
                    let o: i64 = row.get_by_name::<i64, _>("o").ok()??;
                    let change: String = row.get_by_name::<String, _>("change").ok()??;
                    Some((s, p, o, change))
                })
                .collect(),
            Err(e) => {
                pgrx::warning!("graph_diff query error: {e}");
                Vec::new()
            }
        }
    });

    TableIterator::new(rows)
}

// ─── Housekeeping ─────────────────────────────────────────────────────────────

/// Prune expired snapshots from `_pg_ripple.graph_snapshots`.
///
/// Called from the merge background worker on each tick (worker_idx == 0 only).
/// Deletes rows where `expires_at <= now()` when
/// `pg_ripple.snapshot_retention_days > 0`.
pub fn prune_expired_snapshots() {
    if crate::gucs::storage::SNAPSHOT_RETENTION_DAYS.get() == 0 {
        return;
    }
    Spi::run_with_args(
        "DELETE FROM _pg_ripple.graph_snapshots \
         WHERE expires_at IS NOT NULL AND expires_at <= now()",
        &[],
    )
    .unwrap_or_else(|e| pgrx::warning!("prune_expired_snapshots: {e}"));
}

// ─── HTTP companion helpers ───────────────────────────────────────────────────

/// Return the current live snapshot count from `_pg_ripple.graph_snapshots`.
///
/// Used by the HTTP companion service to update the
/// `pg_ripple_graph_snapshots_total` Prometheus gauge.
#[pg_extern(schema = "pg_ripple")]
pub fn graph_snapshots_count() -> i64 {
    Spi::get_one::<i64>("SELECT COUNT(*)::bigint FROM _pg_ripple.graph_snapshots")
        .unwrap_or(None)
        .unwrap_or(0)
}
