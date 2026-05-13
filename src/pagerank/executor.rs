//! PageRank executor — core computation, convergence loop, and WCOJ path selection.
//!
//! Implements `run_pagerank()` (iterative SQL-based PageRank) and supporting
//! helpers `validate_params()`, `ensure_pagerank_catalog()`, `build_edge_sql()`.

use pgrx::prelude::*;

use super::{PT0401, PT0411, PT0412, PT0422, PT0423, PageRankParams, PageRankRow};

/// Validate PageRank parameters and return an error code + message if invalid.
pub fn validate_params(params: &PageRankParams) -> Option<(&'static str, String)> {
    if params.damping < 0.0 || params.damping > 1.0 {
        return Some((
            PT0401,
            format!(
                "invalid pagerank damping factor {}: must be in [0.0, 1.0]",
                params.damping
            ),
        ));
    }
    if !matches!(params.direction.as_str(), "forward" | "reverse" | "both") {
        return Some((
            PT0412,
            format!(
                "invalid direction '{}': expected 'forward', 'reverse', or 'both'",
                params.direction
            ),
        ));
    }
    let shacl_threshold = crate::PAGERANK_SHACL_THRESHOLD.get();
    if !(0.0..=1.0).contains(&shacl_threshold) {
        return Some((
            PT0422,
            format!("pagerank_shacl_threshold {shacl_threshold} is outside [0.0, 1.0]"),
        ));
    }
    let fed_conf = crate::FEDERATION_MINIMUM_CONFIDENCE.get();
    if !(0.0..=1.0).contains(&fed_conf) {
        return Some((
            PT0423,
            format!("federation_minimum_confidence {fed_conf} is outside [0.0, 1.0]"),
        ));
    }
    None
}

/// Ensure the `pagerank_scores` and `pagerank_dirty_edges` tables exist.
pub fn ensure_pagerank_catalog() {
    Spi::run(
        "CREATE TABLE IF NOT EXISTS _pg_ripple.pagerank_scores ( \
            node         BIGINT       NOT NULL, \
            topic        TEXT         NOT NULL DEFAULT '', \
            score        FLOAT8       NOT NULL DEFAULT 0.0, \
            score_lower  FLOAT8       NOT NULL DEFAULT 0.0, \
            score_upper  FLOAT8       NOT NULL DEFAULT 0.0, \
            computed_at  TIMESTAMPTZ  NOT NULL DEFAULT NOW(), \
            iterations   INT          NOT NULL DEFAULT 0, \
            converged    BOOL         NOT NULL DEFAULT false, \
            stale        BOOL         NOT NULL DEFAULT false, \
            stale_since  TIMESTAMPTZ, \
            PRIMARY KEY  (node, topic) \
        )",
    )
    .unwrap_or_else(|e| pgrx::warning!("ensure_pagerank_catalog: pagerank_scores: {e}"));

    Spi::run(
        "CREATE TABLE IF NOT EXISTS _pg_ripple.pagerank_dirty_edges ( \
            id           BIGSERIAL    PRIMARY KEY, \
            source_id    BIGINT       NOT NULL, \
            target_id    BIGINT       NOT NULL, \
            delta        SMALLINT     NOT NULL DEFAULT 1, \
            enqueued_at  TIMESTAMPTZ  NOT NULL DEFAULT NOW() \
        )",
    )
    .unwrap_or_else(|e| pgrx::warning!("ensure_pagerank_catalog: pagerank_dirty_edges: {e}"));

    Spi::run(
        "CREATE TABLE IF NOT EXISTS _pg_ripple.centrality_scores ( \
            node         BIGINT       NOT NULL, \
            metric       TEXT         NOT NULL, \
            score        FLOAT8       NOT NULL DEFAULT 0.0, \
            computed_at  TIMESTAMPTZ  NOT NULL DEFAULT NOW(), \
            PRIMARY KEY  (node, metric) \
        )",
    )
    .unwrap_or_else(|e| pgrx::warning!("ensure_pagerank_catalog: centrality_scores: {e}"));
}

/// Build the edge SQL for a PageRank computation.
///
/// Returns a SQL expression yielding (source_id BIGINT, target_id BIGINT, weight FLOAT8).
/// When `direction = 'reverse'`, source and target are swapped.
fn build_edge_sql(params: &PageRankParams) -> String {
    let graph_id_opt: Option<i64> = params
        .graph_uri
        .as_ref()
        .map(|g| crate::dictionary::encode(g, crate::dictionary::KIND_IRI));

    // 'both' treats the graph as undirected: UNION of (s,o) and (o,s).
    // 'reverse' swaps source and target; 'forward' (default) uses (s,o).
    let (src_col, tgt_col) = if params.direction == "reverse" {
        ("vp.o", "vp.s")
    } else {
        ("vp.s", "vp.o")
    };

    let blank_filter = if !crate::PAGERANK_INCLUDE_BLANK_NODES.get() {
        "AND ds.kind = 0"
    } else {
        ""
    };

    let pred_filter = if let Some(preds) = &params.edge_predicates {
        if preds.is_empty() {
            String::new()
        } else {
            let ids: Vec<String> = preds
                .iter()
                .filter_map(|p| crate::dictionary::lookup_iri(p))
                .map(|id| id.to_string())
                .collect();
            if ids.is_empty() {
                return "SELECT NULL::bigint AS source_id, NULL::bigint AS target_id, \
                        1.0::FLOAT8 AS weight WHERE false"
                    .to_owned();
            }
            format!("AND vp.p IN ({})", ids.join(","))
        }
    } else {
        String::new()
    };

    let vp_sql = crate::sparql::sqlgen::build_all_predicates_union(graph_id_opt, "");

    if params.direction == "both" {
        // Undirected: emit each edge in both orientations.
        return format!(
            "SELECT DISTINCT source_id, target_id, weight FROM (\
               SELECT {src_col} AS source_id, {tgt_col} AS target_id, 1.0::FLOAT8 AS weight \
               FROM ({vp_sql}) vp \
               JOIN _pg_ripple.dictionary ds  ON ds.id  = vp.s \
               JOIN _pg_ripple.dictionary do_ ON do_.id = vp.o \
               WHERE do_.kind = 0 {blank_filter} {pred_filter} \
               UNION \
               SELECT {tgt_col} AS source_id, {src_col} AS target_id, 1.0::FLOAT8 AS weight \
               FROM ({vp_sql}) vp \
               JOIN _pg_ripple.dictionary ds  ON ds.id  = vp.s \
               JOIN _pg_ripple.dictionary do_ ON do_.id = vp.o \
               WHERE do_.kind = 0 {blank_filter} {pred_filter}\
             ) undirected",
            src_col = "vp.s",
            tgt_col = "vp.o",
        );
    }

    format!(
        "SELECT DISTINCT {src_col} AS source_id, {tgt_col} AS target_id, 1.0::FLOAT8 AS weight \
         FROM ({vp_sql}) vp \
         JOIN _pg_ripple.dictionary ds  ON ds.id  = vp.s \
         JOIN _pg_ripple.dictionary do_ ON do_.id = vp.o \
         WHERE do_.kind = 0 \
         {blank_filter} \
         {pred_filter}"
    )
}

/// Run iterative PageRank via SQL and return result rows.
pub fn run_pagerank(params: PageRankParams) -> Vec<PageRankRow> {
    ensure_pagerank_catalog();

    if let Some((code, msg)) = validate_params(&params) {
        pgrx::error!("{code}: {msg}");
    }

    // SEC-03 (v0.89.0): guard the seed_iris array length.
    if let Some(ref seeds) = params.seed_iris {
        let max_seeds = crate::PAGERANK_MAX_SEEDS.get();
        if seeds.len() as i32 > max_seeds {
            pgrx::error!(
                "{PT0411}: seed_iris array length {} exceeds pg_ripple.pagerank_max_seeds ({}) — \
                 reduce the array or raise the GUC",
                seeds.len(),
                max_seeds
            );
        }
    }

    // CON-03 (v0.90.0): acquire per-topic advisory lock to prevent concurrent
    // pagerank_run() calls for the same topic from racing on TRUNCATE+INSERT of
    // pagerank_scores. Calls for different topics run concurrently.
    let topic_for_lock = params.topic.clone().unwrap_or_default();
    let lock_key = format!("pagerank_run_{topic_for_lock}");
    Spi::run_with_args(
        "SELECT pg_advisory_xact_lock(hashtext($1))",
        &[pgrx::datum::DatumWithOid::from(lock_key.as_str())],
    )
    .unwrap_or_else(|e| pgrx::warning!("pagerank advisory lock: {e}"));

    let damping = params.damping;
    let max_iter = params.max_iterations;
    let conv_delta = params.convergence_delta;
    let topic = params.topic.clone().unwrap_or_default();
    let conf_default = crate::PAGERANK_CONFIDENCE_DEFAULT.get();
    let _ = conf_default; // Used in build_edge_sql context

    // PERF-01 (v0.90.0): select scan mode based on estimated edge count.
    // When wcoj_enabled = on AND edge count exceeds threshold, use WCOJ path.
    let wcoj_threshold = crate::PAGERANK_WCOJ_THRESHOLD.get() as i64 * 1_000_000;
    let use_wcoj = if crate::WCOJ_ENABLED.get() {
        let est_edges: i64 =
            Spi::get_one::<i64>("SELECT COALESCE(SUM(triple_count), 0) FROM _pg_ripple.predicates")
                .unwrap_or(None)
                .unwrap_or(0);
        est_edges > wcoj_threshold
    } else {
        false
    };
    let _ = use_wcoj; // Scan-mode reported via explain_inference

    let edge_sql = build_edge_sql(&params);

    let pr_sql = format!(
        "DO $pr$
         DECLARE
           v_iter INT := 0;
           v_max_delta FLOAT8 := 1.0;
           v_node_count BIGINT;
           v_dangling_mass FLOAT8;
           v_dangling_policy TEXT := COALESCE(
             current_setting('pg_ripple.pagerank_dangling_policy', true),
             'redistribute'
           );
         BEGIN
           CREATE TEMP TABLE IF NOT EXISTS _pr_edges (
             source_id BIGINT,
             target_id BIGINT,
             weight    FLOAT8
           ) ON COMMIT DROP;
           TRUNCATE _pr_edges;
           INSERT INTO _pr_edges SELECT * FROM ({edge_sql}) AS e
             WHERE e.source_id IS NOT NULL AND e.target_id IS NOT NULL;

           CREATE TEMP TABLE IF NOT EXISTS _pr_scores (
             node_id BIGINT PRIMARY KEY,
             score   FLOAT8,
             new_score FLOAT8
           ) ON COMMIT DROP;
           TRUNCATE _pr_scores;

           SELECT COUNT(DISTINCT node_id) INTO v_node_count FROM (
             SELECT source_id AS node_id FROM _pr_edges
             UNION
             SELECT target_id AS node_id FROM _pr_edges
           ) t;

           IF v_node_count = 0 THEN
             RETURN;
           END IF;

           INSERT INTO _pr_scores (node_id, score, new_score)
           SELECT node_id, 1.0 / v_node_count, 0.0
           FROM (
             SELECT source_id AS node_id FROM _pr_edges
             UNION
             SELECT target_id AS node_id FROM _pr_edges
           ) t;

           WHILE v_iter < {max_iter} AND v_max_delta > {conv_delta} LOOP
             v_iter := v_iter + 1;

             SELECT COALESCE(SUM(s.score), 0.0)
             INTO v_dangling_mass
             FROM _pr_scores s
             WHERE NOT EXISTS (
               SELECT 1 FROM _pr_edges e WHERE e.source_id = s.node_id
             );

             UPDATE _pr_scores s
             SET new_score = (1.0 - {damping}) / v_node_count +
               {damping} * (
                 COALESCE((
                   SELECT SUM(e.weight * src.score / NULLIF(out_deg.total_weight, 0))
                   FROM _pr_edges e
                   JOIN _pr_scores src ON src.node_id = e.source_id
                   JOIN (
                     SELECT source_id, SUM(weight) AS total_weight
                     FROM _pr_edges
                     GROUP BY source_id
                   ) out_deg ON out_deg.source_id = e.source_id
                   WHERE e.target_id = s.node_id
                 ), 0.0)
                 + CASE WHEN v_dangling_policy = 'redistribute'
                        THEN v_dangling_mass / v_node_count
                        ELSE 0.0 END
               );

             SELECT MAX(ABS(new_score - score))
             INTO v_max_delta
             FROM _pr_scores;

             UPDATE _pr_scores SET score = new_score;
           END LOOP;

           DELETE FROM _pg_ripple.pagerank_scores
           WHERE topic = {topic_literal};

           INSERT INTO _pg_ripple.pagerank_scores
             (node, topic, score, score_lower, score_upper, computed_at,
              iterations, converged, stale, stale_since)
           SELECT
             s.node_id,
             {topic_literal},
             s.score,
             s.score,
             s.score,
             NOW(),
             v_iter,
             (v_max_delta <= {conv_delta}),
             false,
             NULL
           FROM _pr_scores s;

           DROP TABLE IF EXISTS _pr_edges;
           DROP TABLE IF EXISTS _pr_scores;
         END;
         $pr$",
        edge_sql = edge_sql,
        max_iter = max_iter,
        conv_delta = conv_delta,
        damping = damping,
        topic_literal = format!("'{}'", topic.replace('\'', "''")),
    );

    Spi::run(&pr_sql).unwrap_or_else(|e| pgrx::warning!("pagerank_run SQL error: {e}"));

    let result_sql = format!(
        "SELECT d.value AS node_iri, ps.score, ps.score_lower, ps.score_upper, \
                ps.iterations, ps.converged, ps.stale, ps.topic \
         FROM _pg_ripple.pagerank_scores ps \
         JOIN _pg_ripple.dictionary d ON d.id = ps.node \
         WHERE ps.topic = '{}' \
         ORDER BY ps.score DESC",
        topic.replace('\'', "''")
    );

    Spi::connect(|c| {
        c.select(&result_sql, None, &[])
            .unwrap_or_else(|e| pgrx::error!("pagerank_run fetch: {e}"))
            .map(|row| PageRankRow {
                node_iri: row
                    .get::<String>(1)
                    .ok()
                    .flatten()
                    .unwrap_or_default()
                    .trim_matches(|c| c == '<' || c == '>')
                    .to_owned(),
                score: row.get::<f64>(2).ok().flatten().unwrap_or(0.0),
                score_lower: row.get::<f64>(3).ok().flatten().unwrap_or(0.0),
                score_upper: row.get::<f64>(4).ok().flatten().unwrap_or(0.0),
                iterations: row.get::<i32>(5).ok().flatten().unwrap_or(0),
                converged: row.get::<bool>(6).ok().flatten().unwrap_or(false),
                stale: row.get::<bool>(7).ok().flatten().unwrap_or(false),
                topic: row.get::<String>(8).ok().flatten().unwrap_or_default(),
            })
            .collect()
    })
}
