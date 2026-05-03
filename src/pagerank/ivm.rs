//! PageRank IVM (Incremental View Maintenance) — dirty-edge queue, staleness, score lookups.

use pgrx::prelude::*;

use super::{PageRankParams, QueueStatsRow, run_pagerank};

/// Drain processed entries from the dirty-edges queue.
pub fn vacuum_pagerank_dirty() -> i64 {
    super::executor::ensure_pagerank_catalog();
    Spi::get_one::<i64>(
        "WITH deleted AS (\
           DELETE FROM _pg_ripple.pagerank_dirty_edges \
           WHERE enqueued_at < NOW() - INTERVAL '1 day' \
           RETURNING 1 \
         ) SELECT COUNT(*)::BIGINT FROM deleted",
    )
    .unwrap_or(None)
    .unwrap_or(0)
}

/// Get IVM queue statistics for Prometheus / SQL.
pub fn pagerank_queue_stats() -> QueueStatsRow {
    super::executor::ensure_pagerank_catalog();
    let stats: (i64, f64, Option<pgrx::datum::TimestampWithTimeZone>, f64) = Spi::connect(|c| {
        let result = c
            .select(
                "SELECT COUNT(*)::BIGINT, COALESCE(MAX(ABS(delta::FLOAT8)), 0.0), \
                    MIN(enqueued_at), COUNT(*)::FLOAT8 / 100.0 \
             FROM _pg_ripple.pagerank_dirty_edges",
                None,
                &[],
            )
            .unwrap_or_else(|e| pgrx::error!("pagerank_queue_stats: {e}"))
            .next();
        if let Some(r) = result {
            (
                r.get::<i64>(1).ok().flatten().unwrap_or(0),
                r.get::<f64>(2).ok().flatten().unwrap_or(0.0),
                r.get::<pgrx::datum::TimestampWithTimeZone>(3)
                    .ok()
                    .flatten(),
                r.get::<f64>(4).ok().flatten().unwrap_or(0.0),
            )
        } else {
            (0, 0.0, None, 0.0)
        }
    });

    let warn_threshold = crate::PAGERANK_QUEUE_WARN_THRESHOLD.get();
    if stats.0 > warn_threshold as i64 {
        pgrx::warning!(
            "pg_ripple: pagerank dirty-edges queue depth {} exceeds threshold {}",
            stats.0,
            warn_threshold
        );
    }
    QueueStatsRow {
        queued_edges: stats.0,
        max_delta: stats.1,
        oldest_enqueue: stats.2,
        estimated_drain_seconds: stats.3,
    }
}

/// Look up a PageRank score by node IRI and topic.
pub fn lookup_pagerank(node_iri: &str, topic: &str) -> f64 {
    let trimmed = node_iri
        .trim()
        .trim_matches(|c| c == '<' || c == '>')
        .to_owned();
    let node_id = match crate::dictionary::lookup_iri(&trimmed)
        .or_else(|| crate::dictionary::lookup_iri(node_iri))
    {
        Some(id) => id,
        None => return 0.0,
    };

    if crate::PAGERANK_ON_DEMAND.get() {
        let is_stale_val: bool = Spi::get_one_with_args::<bool>(
            "SELECT COALESCE(stale, true) FROM _pg_ripple.pagerank_scores \
             WHERE node = $1 AND topic = $2",
            &[
                pgrx::datum::DatumWithOid::from(node_id),
                pgrx::datum::DatumWithOid::from(topic),
            ],
        )
        .unwrap_or(None)
        .unwrap_or(true);

        if is_stale_val {
            let _ = run_pagerank(PageRankParams {
                topic: Some(topic.to_owned()),
                ..Default::default()
            });
        }
    }

    Spi::get_one_with_args::<f64>(
        "SELECT score FROM _pg_ripple.pagerank_scores WHERE node = $1 AND topic = $2",
        &[
            pgrx::datum::DatumWithOid::from(node_id),
            pgrx::datum::DatumWithOid::from(topic),
        ],
    )
    .unwrap_or(None)
    .unwrap_or(0.0)
}

/// Check if a node's score is stale (K-hop updated rather than full recompute).
pub fn is_stale(node_iri: &str) -> bool {
    let trimmed = node_iri
        .trim()
        .trim_matches(|c| c == '<' || c == '>')
        .to_owned();
    let node_id = match crate::dictionary::lookup_iri(&trimmed)
        .or_else(|| crate::dictionary::lookup_iri(node_iri))
    {
        Some(id) => id,
        None => return false,
    };
    Spi::get_one_with_args::<bool>(
        "SELECT COALESCE(stale, false) FROM _pg_ripple.pagerank_scores \
         WHERE node = $1 AND topic = ''",
        &[pgrx::datum::DatumWithOid::from(node_id)],
    )
    .unwrap_or(None)
    .unwrap_or(false)
}

/// Get score lower bound for a node.
pub fn pagerank_lower(node_iri: &str) -> f64 {
    let trimmed = node_iri
        .trim()
        .trim_matches(|c| c == '<' || c == '>')
        .to_owned();
    let node_id = match crate::dictionary::lookup_iri(&trimmed)
        .or_else(|| crate::dictionary::lookup_iri(node_iri))
    {
        Some(id) => id,
        None => return 0.0,
    };
    Spi::get_one_with_args::<f64>(
        "SELECT score_lower FROM _pg_ripple.pagerank_scores WHERE node = $1 AND topic = ''",
        &[pgrx::datum::DatumWithOid::from(node_id)],
    )
    .unwrap_or(None)
    .unwrap_or(0.0)
}

/// Get score upper bound for a node.
pub fn pagerank_upper(node_iri: &str) -> f64 {
    let trimmed = node_iri
        .trim()
        .trim_matches(|c| c == '<' || c == '>')
        .to_owned();
    let node_id = match crate::dictionary::lookup_iri(&trimmed)
        .or_else(|| crate::dictionary::lookup_iri(node_iri))
    {
        Some(id) => id,
        None => return 0.0,
    };
    Spi::get_one_with_args::<f64>(
        "SELECT score_upper FROM _pg_ripple.pagerank_scores WHERE node = $1 AND topic = ''",
        &[pgrx::datum::DatumWithOid::from(node_id)],
    )
    .unwrap_or(None)
    .unwrap_or(0.0)
}
