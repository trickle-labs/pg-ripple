//! Deduplication helpers for VP tables (v0.7.0, split from scan.rs v0.122.0).

use pgrx::datum::DatumWithOid;
use pgrx::prelude::*;

use super::super::super::vp_rare_io::get_dedicated_vp_table;
use crate::dictionary;

/// Remove duplicate `(s, o, g)` rows for the predicate identified by `p_iri`.
///
/// Strategy:
/// - **delta table**: DELETE all rows where ctid is not the minimum ctid per (s,o,g).
/// - **main table**: insert tombstone rows for all but the minimum-SID row per (s,o,g),
///   so duplicates are masked at query time and removed on the next merge.
/// - **vp_rare** (if predicate has no dedicated table): DELETE duplicate rows by
///   (p, s, o, g) keeping the minimum ctid.
///
/// Runs ANALYZE on all modified tables afterward.
/// Returns the total count of rows removed.
pub fn deduplicate_predicate(p_iri: &str) -> i64 {
    let p_clean = if p_iri.starts_with('<') && p_iri.ends_with('>') {
        &p_iri[1..p_iri.len() - 1]
    } else {
        p_iri
    };

    let p_id = match dictionary::lookup_iri(p_clean) {
        Some(id) => id,
        None => {
            // Predicate not in dictionary — nothing to deduplicate.
            return 0;
        }
    };

    let mut total_removed: i64 = 0;

    if get_dedicated_vp_table(p_id).is_some() {
        // Dedicated HTAP VP table: handle delta and main separately.
        let delta = format!("_pg_ripple.vp_{p_id}_delta");
        let main = format!("_pg_ripple.vp_{p_id}_main");
        let tombs = format!("_pg_ripple.vp_{p_id}_tombstones");

        // Deduplicate delta: delete all rows keeping the minimum-i (SID) row per (s,o,g).
        let delta_removed = Spi::get_one_with_args::<i64>(
            &format!(
                "WITH keep AS ( \
                     SELECT s, o, g, MIN(i) AS min_i \
                     FROM {delta} \
                     GROUP BY s, o, g \
                     HAVING COUNT(*) > 1 \
                 ), \
                 del AS ( \
                     DELETE FROM {delta} d \
                     USING keep k \
                     WHERE d.s = k.s AND d.o = k.o AND d.g = k.g AND d.i <> k.min_i \
                     RETURNING 1 \
                 ) \
                 SELECT COUNT(*)::BIGINT FROM del"
            ),
            &[],
        )
        .unwrap_or(None)
        .unwrap_or(0);

        total_removed += delta_removed;

        // Deduplicate main: tombstone all but the minimum-SID row per (s,o,g).
        let main_removed = Spi::get_one_with_args::<i64>(
            &format!(
                "WITH ranked AS ( \
                     SELECT s, o, g, i, \
                            ROW_NUMBER() OVER (PARTITION BY s, o, g ORDER BY i ASC) AS rn \
                     FROM {main} \
                 ), \
                 dupes AS (SELECT DISTINCT s, o, g FROM ranked WHERE rn > 1), \
                 ins AS ( \
                     INSERT INTO {tombs} (s, o, g) \
                     SELECT s, o, g FROM dupes \
                     ON CONFLICT DO NOTHING \
                     RETURNING 1 \
                 ) \
                 SELECT COUNT(*)::BIGINT FROM ins"
            ),
            &[],
        )
        .unwrap_or(None)
        .unwrap_or(0);

        total_removed += main_removed;

        // ANALYZE both tables.
        Spi::run_with_args(&format!("ANALYZE {delta}"), &[])
            .unwrap_or_else(|e| pgrx::error!("ANALYZE delta error: {e}"));
        Spi::run_with_args(&format!("ANALYZE {main}"), &[])
            .unwrap_or_else(|e| pgrx::error!("ANALYZE main error: {e}"));
    } else {
        // vp_rare: DELETE duplicate (p, s, o, g) keeping the minimum-SID row.
        let rare_removed = Spi::get_one_with_args::<i64>(
            "WITH del AS ( \
                 DELETE FROM _pg_ripple.vp_rare r \
                 WHERE r.p = $1 \
                   AND r.i NOT IN ( \
                       SELECT MIN(i) FROM _pg_ripple.vp_rare \
                       WHERE p = $1 \
                       GROUP BY p, s, o, g \
                   ) \
                 RETURNING 1 \
             ) \
             SELECT COUNT(*)::BIGINT FROM del",
            &[DatumWithOid::from(p_id)],
        )
        .unwrap_or(None)
        .unwrap_or(0);

        total_removed += rare_removed;

        if rare_removed > 0 {
            Spi::run_with_args("ANALYZE _pg_ripple.vp_rare", &[])
                .unwrap_or_else(|e| pgrx::error!("ANALYZE vp_rare error: {e}"));
        }
    }

    total_removed
}

/// Remove duplicate `(s, o, g)` rows across all predicates and `vp_rare`.
///
/// Iterates over all predicate IRIs in `_pg_ripple.predicates` and calls
/// `deduplicate_predicate` for each. Then deduplicates `vp_rare` for any
/// predicates that remain in the rare table.
///
/// Returns the total count of rows removed.
pub fn deduplicate_all() -> i64 {
    // Collect all predicate IRIs from the catalog.
    let pred_iris: Vec<String> = Spi::connect(|c| {
        c.select(
            "SELECT d.value FROM _pg_ripple.predicates p \
             JOIN _pg_ripple.dictionary d ON d.id = p.id",
            None,
            &[],
        )
        .unwrap_or_else(|e| pgrx::error!("deduplicate_all SPI error: {e}"))
        .filter_map(|row| row.get::<&str>(1).ok().flatten().map(|s| s.to_owned()))
        .collect()
    });

    let mut total: i64 = 0;
    for iri in pred_iris {
        total += deduplicate_predicate(&iri);
    }

    // Deduplicate all remaining rare triples in vp_rare.
    let rare_removed = Spi::get_one_with_args::<i64>(
        "WITH del AS ( \
             DELETE FROM _pg_ripple.vp_rare r \
             WHERE r.i NOT IN ( \
                 SELECT MIN(i) FROM _pg_ripple.vp_rare \
                 GROUP BY p, s, o, g \
             ) \
             RETURNING 1 \
         ) \
         SELECT COUNT(*)::BIGINT FROM del",
        &[],
    )
    .unwrap_or(None)
    .unwrap_or(0);

    total += rare_removed;

    if rare_removed > 0 {
        Spi::run_with_args("ANALYZE _pg_ripple.vp_rare", &[])
            .unwrap_or_else(|e| pgrx::error!("ANALYZE vp_rare error: {e}"));
    }

    total
}

/// Look up the statement ID (`i` column) for a given `(s, p, o)` triple.
///
/// Returns `None` if the triple does not exist.
pub fn statement_id_for_triple(s: i64, p: i64, o: i64) -> Option<i64> {
    // Check dedicated VP table first.
    let table_oid = Spi::get_one_with_args::<i64>(
        "SELECT table_oid::bigint FROM _pg_ripple.predicates WHERE id = $1",
        &[DatumWithOid::from(p)],
    )
    .unwrap_or(None);

    if table_oid.is_some() {
        let sql = format!("SELECT i FROM _pg_ripple.vp_{p} WHERE s = {s} AND o = {o} LIMIT 1");
        if let Ok(Some(sid)) = Spi::get_one::<i64>(&sql) {
            return Some(sid);
        }
    }

    // Fall back to vp_rare.
    Spi::get_one_with_args::<i64>(
        &format!("SELECT i FROM _pg_ripple.vp_rare WHERE p = {p} AND s = {s} AND o = {o} LIMIT 1"),
        &[],
    )
    .unwrap_or(None)
}
