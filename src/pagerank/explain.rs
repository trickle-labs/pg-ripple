//! PageRank score explanation trees — `explain_pagerank()`.

use pgrx::prelude::*;

use super::{ExplainPageRankRow, PT0413};

/// Explain the top-K incoming contributors to a node's PageRank score.
pub fn explain_pagerank(node_iri: &str, top_k: i32) -> Vec<ExplainPageRankRow> {
    if top_k <= 0 {
        pgrx::error!("{PT0413}: explain_pagerank top_k must be positive");
    }

    let trimmed = node_iri
        .trim()
        .trim_matches(|c| c == '<' || c == '>')
        .to_owned();
    let node_id = match crate::dictionary::lookup_iri(&trimmed)
        .or_else(|| crate::dictionary::lookup_iri(node_iri))
    {
        Some(id) => id,
        None => {
            pgrx::warning!("{PT0413}: explain_pagerank: unknown node '{node_iri}'");
            return vec![];
        }
    };

    let score_exists: bool = Spi::get_one_with_args::<bool>(
        "SELECT EXISTS(SELECT 1 FROM _pg_ripple.pagerank_scores WHERE node = $1 AND topic = '')",
        &[pgrx::datum::DatumWithOid::from(node_id)],
    )
    .unwrap_or(None)
    .unwrap_or(false);

    if !score_exists {
        pgrx::warning!("{PT0413}: explain_pagerank: node '{node_iri}' has no pagerank score");
        return vec![];
    }

    let sql = format!(
        "SELECT \
           1 AS depth, \
           d.value AS contributor_iri, \
           ps_src.score * COALESCE( \
             (SELECT weight FROM ( \
               SELECT SUM(1.0) AS weight, s AS source_id \
               FROM _pg_ripple.vp_rare \
               WHERE o = $1 \
               GROUP BY s \
             ) t WHERE source_id = ps_src.node), 1.0 \
           ) AS contribution, \
           d.value || ' -> ' || $2::TEXT AS path \
         FROM _pg_ripple.pagerank_scores ps_src \
         JOIN _pg_ripple.dictionary d ON d.id = ps_src.node \
         WHERE EXISTS ( \
           SELECT 1 FROM _pg_ripple.vp_rare vp \
           WHERE vp.s = ps_src.node AND vp.o = $1 \
         ) AND ps_src.topic = '' \
         ORDER BY contribution DESC \
         LIMIT {top_k}"
    );

    Spi::connect(|c| {
        c.select(
            &sql,
            None,
            &[
                pgrx::datum::DatumWithOid::from(node_id),
                pgrx::datum::DatumWithOid::from(node_iri),
            ],
        )
        .unwrap_or_else(|e| pgrx::error!("explain_pagerank SQL: {e}"))
        .map(|row| ExplainPageRankRow {
            depth: row.get::<i32>(1).ok().flatten().unwrap_or(1),
            contributor_iri: row
                .get::<String>(2)
                .ok()
                .flatten()
                .unwrap_or_default()
                .trim_matches(|c| c == '<' || c == '>')
                .to_owned(),
            contribution: row.get::<f64>(3).ok().flatten().unwrap_or(0.0),
            path: row.get::<String>(4).ok().flatten().unwrap_or_default(),
        })
        .collect()
    })
}
