//! Confidence-aware bulk loader (v0.87.0 LOAD-CONF-01,
//! split from bulk_load/mod.rs in v0.122.0 H17-02).

use crate::dictionary;

/// Ensure `_pg_ripple.confidence` table and its index exist.
///
/// This is idempotent and safe to call on every `load_triples_with_confidence`
/// and `vacuum_confidence` invocation.  On a fresh install (before the v0.87.0
/// migration script has been applied), `CREATE TABLE IF NOT EXISTS` creates the
/// table on-demand; on upgraded instances it is a no-op.
pub fn ensure_confidence_catalog() {
    pgrx::Spi::run(
        "CREATE TABLE IF NOT EXISTS _pg_ripple.confidence ( \
            statement_id BIGINT  NOT NULL, \
            confidence   FLOAT8  NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0), \
            model        TEXT    NOT NULL DEFAULT 'datalog', \
            asserted_at  TIMESTAMPTZ NOT NULL DEFAULT now(), \
            PRIMARY KEY  (statement_id, model) \
        )",
    )
    .unwrap_or_else(|e| pgrx::warning!("confidence catalog creation: {e}"));

    pgrx::Spi::run(
        "CREATE INDEX IF NOT EXISTS confidence_stmt_idx \
         ON _pg_ripple.confidence (statement_id)",
    )
    .unwrap_or_else(|e| pgrx::warning!("confidence_stmt_idx creation: {e}"));
}

/// Load triples with an explicit uniform confidence score.
///
/// After inserting triples, inserts confidence rows with the given confidence
/// score for every newly-inserted SID into `_pg_ripple.confidence (model='explicit')`.
///
/// `format` may be `'ntriples'` (default), `'nquads'`, `'turtle'`, or `'jsonld'`.
/// `graph_uri` routes all triples to a named graph when provided.
///
/// Returns the number of triples loaded.
pub fn load_triples_with_confidence(
    data: &str,
    confidence: f64,
    format: &str,
    graph_uri: Option<&str>,
) -> i64 {
    if confidence.is_nan() {
        pgrx::error!("confidence value is NaN — must be a finite number in [0.0, 1.0] (PT0301)");
    }
    if confidence.is_infinite() {
        pgrx::error!(
            "confidence value is {} — must be a finite number in [0.0, 1.0] (PT0301)",
            if confidence.is_sign_positive() {
                "+Infinity"
            } else {
                "-Infinity"
            }
        );
    }
    if !(0.0..=1.0).contains(&confidence) {
        pgrx::error!(
            "confidence must be in [0.0, 1.0]; got {} (PT0301)",
            confidence
        );
    }

    ensure_confidence_catalog();

    let count = match format.to_ascii_lowercase().as_str() {
        "ntriples" | "nt" => {
            if let Some(g_uri) = graph_uri {
                let g_id = dictionary::encode(g_uri, dictionary::KIND_IRI);
                super::load_ntriples_into_graph(data, g_id)
            } else {
                super::load_ntriples(data, false)
            }
        }
        "nquads" | "nq" => super::load_nquads(data, false),
        "turtle" | "ttl" => {
            if let Some(g_uri) = graph_uri {
                let g_id = dictionary::encode(g_uri, dictionary::KIND_IRI);
                super::load_turtle_into_graph(data, g_id)
            } else {
                super::load_turtle(data, false)
            }
        }
        other => {
            pgrx::error!(
                "unsupported format '{}'; use 'ntriples', 'nquads', 'turtle'",
                other
            );
        }
    };

    if count > 0 {
        let conf_sql = format!(
            "INSERT INTO _pg_ripple.confidence (statement_id, confidence, model) \
             SELECT i, {confidence}::float8, 'explicit' \
             FROM _pg_ripple.vp_rare \
             WHERE source = 0 \
             ORDER BY i DESC LIMIT {count} \
             ON CONFLICT (statement_id, model) DO NOTHING"
        );
        if let Err(e) = pgrx::Spi::run_with_args(&conf_sql, &[]) {
            pgrx::warning!("confidence insert error in load_triples_with_confidence: {e}");
        }

        if let Err(e) = pgrx::Spi::run("ANALYZE _pg_ripple.confidence") {
            pgrx::warning!("load_triples_with_confidence: ANALYZE failed: {e}");
        }
    }

    count
}
