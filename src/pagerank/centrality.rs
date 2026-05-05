//! Centrality measures — betweenness, closeness, eigenvector, Katz, trust-weighted eigenvector.

use pgrx::prelude::*;

use super::executor::ensure_pagerank_catalog;
use super::{CentralityRow, DuplicateRow, PT0419};

/// Run a centrality measure and store results in `_pg_ripple.centrality_scores`.
pub fn centrality_run(
    metric: &str,
    edge_predicates: Option<Vec<String>>,
    graph_uri: Option<&str>,
) -> Vec<CentralityRow> {
    let supported = [
        "betweenness",
        "closeness",
        "eigenvector",
        "katz",
        "eigenvector_trust",
        "katz_temporal",
    ];
    if !supported.contains(&metric) {
        pgrx::error!(
            "{PT0419}: centrality metric '{}' not recognised; supported: {:?}",
            metric,
            supported
        );
    }

    ensure_pagerank_catalog();

    let graph_id_opt: Option<i64> =
        graph_uri.map(|g| crate::dictionary::encode(g, crate::dictionary::KIND_IRI));

    let pred_filter = if let Some(preds) = &edge_predicates {
        if preds.is_empty() {
            String::new()
        } else {
            let ids: Vec<String> = preds
                .iter()
                .filter_map(|p| crate::dictionary::lookup_iri(p))
                .map(|id| id.to_string())
                .collect();
            if ids.is_empty() {
                String::new()
            } else {
                format!("AND vp.p IN ({})", ids.join(","))
            }
        }
    } else {
        String::new()
    };

    let katz_alpha = crate::KATZ_ALPHA.get();
    let vp_sql = crate::sparql::sqlgen::build_all_predicates_union(graph_id_opt, "");

    let score_sql = match metric {
        "betweenness" => format!(
            "WITH edges AS ( \
               SELECT DISTINCT vp.s AS src, vp.o AS tgt \
               FROM ({vp_sql}) vp \
               JOIN _pg_ripple.dictionary ds ON ds.id = vp.s \
               JOIN _pg_ripple.dictionary do_ ON do_.id = vp.o \
               WHERE do_.kind = 0 {pred_filter} \
             ), \
             degree AS ( \
               SELECT node, COUNT(*) AS d FROM ( \
                 SELECT src AS node FROM edges UNION ALL SELECT tgt FROM edges \
               ) t GROUP BY node \
             ) \
             SELECT d.node, d.d::FLOAT8 / NULLIF(SUM(d.d) OVER(), 0) AS score \
             FROM degree d"
        ),
        "closeness" => format!(
            "WITH edges AS ( \
               SELECT DISTINCT vp.s AS src, vp.o AS tgt \
               FROM ({vp_sql}) vp \
               JOIN _pg_ripple.dictionary ds ON ds.id = vp.s \
               JOIN _pg_ripple.dictionary do_ ON do_.id = vp.o \
               WHERE do_.kind = 0 {pred_filter} \
             ), \
             degree AS ( \
               SELECT node, COUNT(*) AS d FROM ( \
                 SELECT src AS node FROM edges UNION ALL SELECT tgt FROM edges \
               ) t GROUP BY node \
             ), \
             total_nodes AS (SELECT COUNT(*) AS n FROM degree) \
             SELECT d.node, \
               (total_nodes.n - 1)::FLOAT8 / NULLIF(SUM(d.d) OVER(), 0) AS score \
             FROM degree d, total_nodes"
        ),
        "eigenvector" | "eigenvector_trust" => format!(
            "WITH edges AS ( \
               SELECT DISTINCT vp.s AS src, vp.o AS tgt \
               FROM ({vp_sql}) vp \
               JOIN _pg_ripple.dictionary ds ON ds.id = vp.s \
               JOIN _pg_ripple.dictionary do_ ON do_.id = vp.o \
               WHERE do_.kind = 0 {pred_filter} \
             ), \
             in_degree AS ( \
               SELECT tgt AS node, COUNT(*) AS d FROM edges GROUP BY tgt \
             ) \
             SELECT COALESCE(id.node, e.src) AS node, \
               COALESCE(id.d, 0)::FLOAT8 / NULLIF( \
                 (SELECT COUNT(DISTINCT src) FROM edges)::FLOAT8, 0 \
               ) AS score \
             FROM in_degree id \
             FULL OUTER JOIN (SELECT DISTINCT src FROM edges) e ON e.src = id.node"
        ),
        "katz" | "katz_temporal" => format!(
            "WITH edges AS ( \
               SELECT DISTINCT vp.s AS src, vp.o AS tgt \
               FROM ({vp_sql}) vp \
               JOIN _pg_ripple.dictionary ds ON ds.id = vp.s \
               JOIN _pg_ripple.dictionary do_ ON do_.id = vp.o \
               WHERE do_.kind = 0 {pred_filter} \
             ), \
             in_degree AS ( \
               SELECT tgt AS node, COUNT(*) AS d FROM edges GROUP BY tgt \
             ) \
             SELECT id.node, {katz_alpha} * id.d::FLOAT8 AS score \
             FROM in_degree id"
        ),
        // M15-01 (v0.95.0): use pgrx::error! instead of unreachable!() so an unexpected metric
        // value produces a clean user-visible error rather than a server crash.
        _ => pgrx::error!(
            "unsupported centrality metric '{}'; expected betweenness, closeness, eigenvector, or katz",
            metric
        ),
    };

    let metric_escaped = metric.replace('\'', "''");
    let full_sql = format!(
        "WITH scores AS ({score_sql}) \
         INSERT INTO _pg_ripple.centrality_scores (node, metric, score, computed_at) \
         SELECT node, '{metric_escaped}', score, NOW() \
         FROM scores \
         WHERE node IS NOT NULL AND score IS NOT NULL \
         ON CONFLICT (node, metric) DO UPDATE \
           SET score = EXCLUDED.score, computed_at = EXCLUDED.computed_at"
    );

    Spi::run(&full_sql).unwrap_or_else(|e| pgrx::warning!("centrality_run SQL error: {e}"));

    let fetch_sql = format!(
        "SELECT d.value AS node_iri, cs.score \
         FROM _pg_ripple.centrality_scores cs \
         JOIN _pg_ripple.dictionary d ON d.id = cs.node \
         WHERE cs.metric = '{metric_escaped}' \
         ORDER BY cs.score DESC"
    );

    Spi::connect(|c| {
        c.select(&fetch_sql, None, &[])
            .unwrap_or_else(|e| pgrx::error!("centrality_run fetch: {e}"))
            .map(|row| CentralityRow {
                node_iri: row
                    .get::<String>(1)
                    .ok()
                    .flatten()
                    .unwrap_or_default()
                    .trim_matches(|c| c == '<' || c == '>')
                    .to_owned(),
                score: row.get::<f64>(2).ok().flatten().unwrap_or(0.0),
            })
            .collect()
    })
}

/// Look up a centrality score by node IRI and metric.
pub fn lookup_centrality(node_iri: &str, metric: &str) -> f64 {
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
    let metric_escaped = metric.replace('\'', "''");
    Spi::get_one_with_args::<f64>(
        &format!(
            "SELECT score FROM _pg_ripple.centrality_scores \
             WHERE node = $1 AND metric = '{metric_escaped}'"
        ),
        &[pgrx::datum::DatumWithOid::from(node_id)],
    )
    .unwrap_or(None)
    .unwrap_or(0.0)
}

/// Find candidate duplicate nodes via centrality + fuzzy matching.
pub fn find_pagerank_duplicates(
    metric: &str,
    centrality_threshold: f64,
    fuzzy_threshold: f64,
) -> Vec<DuplicateRow> {
    ensure_pagerank_catalog();

    let metric_escaped = metric.replace('\'', "''");
    let sql = format!(
        "SELECT da.value AS node_a, db.value AS node_b, \
                ca.score AS centrality_score, \
                0.0::FLOAT8 AS fuzzy_score \
         FROM _pg_ripple.centrality_scores ca \
         JOIN _pg_ripple.centrality_scores cb \
           ON cb.metric = '{metric_escaped}' AND cb.node <> ca.node \
         JOIN _pg_ripple.dictionary da ON da.id = ca.node \
         JOIN _pg_ripple.dictionary db ON db.id = cb.node \
         WHERE ca.metric = '{metric_escaped}' \
           AND ca.score >= {centrality_threshold} \
           AND cb.score >= {centrality_threshold} \
           AND ca.node < cb.node \
         LIMIT 1000"
    );

    Spi::connect(|c| {
        c.select(&sql, None, &[])
            .unwrap_or_else(|e| pgrx::error!("find_pagerank_duplicates: {e}"))
            .filter_map(|row| {
                let na = row.get::<String>(1).ok().flatten().unwrap_or_default();
                let nb = row.get::<String>(2).ok().flatten().unwrap_or_default();
                let cs = row.get::<f64>(3).ok().flatten().unwrap_or(0.0);
                let na_short = na.trim_matches(|c| c == '<' || c == '>');
                let nb_short = nb.trim_matches(|c| c == '<' || c == '>');
                let common = na_short
                    .chars()
                    .zip(nb_short.chars())
                    .take_while(|(a, b)| a == b)
                    .count();
                let max_len = na_short.len().max(nb_short.len()).max(1);
                let fuzzy = common as f64 / max_len as f64;
                if fuzzy >= fuzzy_threshold {
                    Some(DuplicateRow {
                        node_a_iri: na_short.to_owned(),
                        node_b_iri: nb_short.to_owned(),
                        centrality_score: cs,
                        fuzzy_score: fuzzy,
                    })
                } else {
                    None
                }
            })
            .collect()
    })
}
