//! SPARQL EXPLAIN and plan cache monitoring (M15-13, v0.96.0).
//! Moved from execute/mod.rs lines 1394-1506.

use pgrx::prelude::*;
use spargebra::SparqlParser;

use super::super::plan_cache;
use super::super::sqlgen;

// ─── explain_sparql ───────────────────────────────────────────────────────────

/// Explain a SPARQL query with flexible format options.
///
/// - `format = 'sql'`: return the generated SQL without executing it.
/// - `format = 'text'` (default): run `EXPLAIN (ANALYZE, FORMAT TEXT)`.
/// - `format = 'json'`: run `EXPLAIN (ANALYZE, FORMAT JSON)`.
/// - `format = 'sparql_algebra'`: return the spargebra algebra tree via `Debug`.
/// - `format = 'sparql_algebra_optimised'` (O13-03, v0.86.0): run sparopt algebra
///   optimiser and return the post-optimisation algebra tree.
/// - `format = 'sparql_algebra_optimized'` (OBS-04, v0.92.0): en_US alias for
///   `sparql_algebra_optimised`; both spellings are accepted and produce identical output.
pub(crate) fn explain_sparql(query_text: &str, format: &str) -> String {
    use spargebra::Query;

    let query = SparqlParser::new()
        .parse_query(query_text)
        .unwrap_or_else(|e| pgrx::error!("SPARQL parse error: {e}"));

    if format == "sparql_algebra" {
        return std::format!("{query:#?}");
    }

    // O13-03 (v0.86.0): post-sparopt algebra tree.
    // OBS-04 (v0.92.0): accept both en_GB (`algebra_optimised`) and en_US (`algebra_optimized`).
    if format == "sparql_algebra_optimised"
        || format == "algebra_optimised"
        || format == "sparql_algebra_optimized"
        || format == "algebra_optimized"
    {
        let optimised = crate::sparql::plan::optimise_query_algebra(&query);
        return std::format!("{optimised:#?}");
    }

    let inner_sql = match &query {
        Query::Select { pattern, .. } => {
            let trans = sqlgen::translate_select(pattern, None);
            trans.sql
        }
        Query::Ask { pattern, .. } => sqlgen::translate_ask(pattern),
        Query::Construct { pattern, .. } => {
            let trans = sqlgen::translate_select(pattern, None);
            trans.sql
        }
        Query::Describe { .. } => {
            return std::format!("DESCRIBE query algebra:\n{query:#?}");
        }
    };

    if format == "sql" {
        return inner_sql;
    }

    let explain_format = if format == "json" { "JSON" } else { "TEXT" };
    let explain_sql = std::format!("EXPLAIN (ANALYZE, FORMAT {explain_format}) {inner_sql}");

    let plan: String = Spi::connect(|client| {
        let rows = client
            .select(&explain_sql, None, &[])
            .unwrap_or_else(|e| pgrx::error!("explain_sparql EXPLAIN SPI error: {e}"));
        let lines: Vec<String> = rows
            .filter_map(|row| row.get::<String>(1).ok().flatten())
            .collect();
        lines.join("\n")
    });

    std::format!("-- Generated SQL --\n{inner_sql}\n\n-- EXPLAIN ({explain_format}) --\n{plan}")
}

// ─── Plan cache monitoring ────────────────────────────────────────────────────

/// Return SPARQL plan cache statistics as JSONB.
pub(crate) fn plan_cache_stats() -> pgrx::JsonB {
    let (hits, misses, size, cap) = plan_cache::stats();
    let total = hits + misses;
    let hit_rate = if total > 0 {
        hits as f64 / total as f64
    } else {
        0.0_f64
    };
    let mut obj = serde_json::Map::new();
    obj.insert(
        "hits".to_owned(),
        serde_json::Value::Number(serde_json::Number::from(hits)),
    );
    obj.insert(
        "misses".to_owned(),
        serde_json::Value::Number(serde_json::Number::from(misses)),
    );
    obj.insert(
        "size".to_owned(),
        serde_json::Value::Number(serde_json::Number::from(size as u64)),
    );
    obj.insert(
        "capacity".to_owned(),
        serde_json::Value::Number(serde_json::Number::from(cap as u64)),
    );
    let hit_rate_rounded = (hit_rate * 10000.0).round() / 10000.0;
    if let Some(n) = serde_json::Number::from_f64(hit_rate_rounded) {
        obj.insert("hit_rate".to_owned(), serde_json::Value::Number(n));
    } else {
        obj.insert(
            "hit_rate".to_owned(),
            serde_json::Value::Number(serde_json::Number::from(0)),
        );
    }
    pgrx::JsonB(serde_json::Value::Object(obj))
}

/// Evict all cached SPARQL plans and reset hit/miss counters.
pub(crate) fn plan_cache_reset() {
    plan_cache::reset();
}
