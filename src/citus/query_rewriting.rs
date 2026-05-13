//! Citus BRIN summarize, worker endpoint detection, and service annotations.
//! (extracted from citus/mod.rs in v0.114.0)

#![allow(clippy::too_many_arguments, unused_imports)]
use pgrx::prelude::*;

use super::is_citus_loaded;

/// Issue `brin_summarize_new_values` on every shard of a VP main-partition
/// table (CITUS-37).
///
/// After a HTAP merge cycle the BRIN indexes on Citus worker shards may be
/// stale.  This function uses `run_command_on_shards` to invoke
/// `brin_summarize_new_values` on every worker shard, keeping first-scan
/// performance consistent.
///
/// Returns the number of shards updated (0 when Citus is not installed or the
/// table is not distributed).
pub fn brin_summarize_vp_shards_impl(pred_id: i64) -> i64 {
    if !is_citus_loaded() {
        // Non-Citus path: find all BRIN indexes on the VP main table and summarize.
        // Uses the pg_catalog to avoid erroring when the main table does not exist.
        return local_brin_summarize(pred_id);
    }

    let main_table = format!("_pg_ripple.vp_{pred_id}_main");

    // Check whether the main table is distributed.
    let is_distributed = Spi::get_one_with_args::<bool>(
        "SELECT EXISTS( \
             SELECT 1 FROM pg_dist_partition \
             WHERE logicalrelid = $1::regclass \
         )",
        &[main_table.as_str().into()],
    )
    .unwrap_or(Some(false))
    .unwrap_or(false);

    if !is_distributed {
        // Table exists but is not distributed; run locally.
        return local_brin_summarize(pred_id);
    }

    // run_command_on_shards returns a table with a `success` column.
    let sql = format!(
        "SELECT count(*)::bigint \
         FROM run_command_on_shards( \
             '{main_table}', \
             $$SELECT brin_summarize_new_values('%s')$$ \
         ) WHERE success"
    );

    let shards = Spi::get_one::<i64>(&sql).unwrap_or(Some(0)).unwrap_or(0);
    if shards > 0 {
        crate::stats::increment_citus_brin_summarise_completed(shards);
    }
    shards
}

/// Summarize all BRIN indexes on `_pg_ripple.vp_{pred_id}_main` locally.
///
/// Returns 0 when the main table does not exist or has no BRIN indexes.
/// Uses the `pg_catalog` to enumerate indexes safely, so this never errors.
fn local_brin_summarize(pred_id: i64) -> i64 {
    // Enumerate BRIN indexes on the VP main table and call
    // brin_summarize_new_values(index_oid) on each.
    let sql = format!(
        "SELECT COALESCE(SUM(brin_summarize_new_values(i.indexrelid)::bigint), 0) \
         FROM pg_index i \
         JOIN pg_class t  ON t.oid  = i.indrelid \
         JOIN pg_namespace n ON n.oid = t.relnamespace \
         JOIN pg_class ix ON ix.oid  = i.indexrelid \
         JOIN pg_am    a  ON a.oid   = ix.relam \
         WHERE n.nspname = '_pg_ripple' \
           AND t.relname  = 'vp_{pred_id}_main' \
           AND a.amname   = 'brin'"
    );
    Spi::get_one::<i64>(&sql).unwrap_or(Some(0)).unwrap_or(0)
}

// ─── v0.66.0: CITUS-04 SQL API — per-predicate BRIN summarise ────────────────

/// Call `brin_summarize_new_values` on all promoted VP main-partition tables.
///
/// This function should be called after an HTAP merge cycle to keep BRIN
/// indexes on worker shards current.  For non-Citus deployments it falls back
/// to local `brin_summarize_new_values`.
///
/// Returns the total number of shards (or local invocations) updated.
///
/// ```sql
/// SELECT pg_ripple.citus_brin_summarise_all();
/// ```
#[pg_extern(schema = "pg_ripple")]
pub fn citus_brin_summarise_all() -> i64 {
    let pred_ids: Vec<i64> = Spi::connect(|c| {
        match c.select(
            "SELECT id FROM _pg_ripple.predicates WHERE table_oid IS NOT NULL",
            None,
            &[],
        ) {
            Ok(rows) => rows
                .filter_map(|row| row.get::<i64>(1).ok().flatten())
                .collect(),
            Err(e) => {
                pgrx::warning!("citus_brin_summarise_all scan error: {e}");
                Vec::new()
            }
        }
    });

    let mut total = 0i64;
    for pred_id in pred_ids {
        total += brin_summarize_vp_shards_impl(pred_id);
    }
    total
}

// ─── Citus SERVICE shard pruning (v0.68.0 CITUS-SVC-01) ──────────────────────

/// Return `true` if `endpoint_url` matches a Citus worker node hostname.
///
/// Compares the host portion of `endpoint_url` against entries in
/// `pg_dist_node` (if Citus is installed).  Returns `false` when Citus is not
/// installed or the endpoint is not a Citus worker.
pub fn is_citus_worker_endpoint(endpoint_url: &str) -> bool {
    if !is_citus_loaded() {
        return false;
    }
    // Extract host from URL (simple prefix match against pg_dist_node.nodename).
    let host = extract_url_host(endpoint_url);
    if host.is_empty() {
        return false;
    }
    Spi::get_one_with_args::<bool>(
        "SELECT EXISTS ( \
             SELECT 1 FROM pg_dist_node WHERE nodename = $1 \
         )",
        &[host.into()],
    )
    .unwrap_or(Some(false))
    .unwrap_or(false)
}

/// Return a SQL WHERE-clause fragment that adds Citus shard pruning for
/// a federation subquery that targets a Citus worker.
///
/// When `pg_ripple.citus_service_pruning = on` and the endpoint is a Citus
/// worker node, returns a SQL comment annotation
/// `/* citus_pruning: worker=<host> */` and records the worker host for
/// shard-constraint injection at query plan time.
///
/// When the preconditions are not met, returns `None`.
///
/// This is the entry point for the SPARQL translator's SERVICE handler.
pub fn citus_service_shard_annotation(endpoint_url: &str) -> Option<String> {
    if !crate::gucs::storage::CITUS_SERVICE_PRUNING.get() {
        return None;
    }
    if !is_citus_worker_endpoint(endpoint_url) {
        return None;
    }
    let host = extract_url_host(endpoint_url);
    // Return a SQL comment annotation.  The translator embeds this in the
    // generated VALUES subquery so that EXPLAIN output reflects the pruning.
    Some(format!("/* citus_pruning: worker={host} */"))
}

/// Extract the hostname from a URL string.
///
/// Handles the following forms (CITUS-URL-01, v0.72.0):
/// - Normal host:   `http://worker1.internal/db`     → `worker1.internal`
/// - IPv6 literal: `http://[::1]:5432/db`            → `[::1]`
/// - IDN:           `http://xn--bcher-kva.example.com/db` → `xn--bcher-kva.example.com`
/// - Port-only:     `http://host:5432`               → `host`
/// - Malformed:     `not-a-url`                      → `""` (empty)
fn extract_url_host(url: &str) -> String {
    // Strip scheme (http:// or https://).
    let rest = if let Some(r) = url.strip_prefix("https://") {
        r
    } else if let Some(r) = url.strip_prefix("http://") {
        r
    } else {
        // Not a valid http/https URL — return empty to signal failure.
        return String::new();
    };
    // IPv6 literal: starts with '['.
    if rest.starts_with('[') {
        // Find the closing ']'.
        if let Some(close) = rest.find(']') {
            let candidate = &rest[..=close];
            // A valid IPv6 literal cannot contain '/'; if it does the input is
            // malformed (e.g. a second URL scheme embedded inside brackets).
            if candidate.contains('/') {
                return String::new();
            }
            return candidate.to_owned();
        }
        // Malformed IPv6 literal — return empty.
        return String::new();
    }
    // Normal host: take up to the first '/', ':', or '?'.
    let end = rest.find(['/', ':', '?']).unwrap_or(rest.len());
    rest[..end].to_owned()
}

#[cfg(any(test, feature = "pg_test"))]
#[cfg(test)]
mod url_parsing_tests {
    use super::extract_url_host;

    #[test]
    fn test_normal_host() {
        assert_eq!(
            extract_url_host("http://worker1.internal/db"),
            "worker1.internal"
        );
    }

    #[test]
    fn test_ipv6_literal() {
        assert_eq!(extract_url_host("http://[::1]:5432/db"), "[::1]");
    }

    #[test]
    fn test_idn_host() {
        assert_eq!(
            extract_url_host("http://xn--bcher-kva.example.com/db"),
            "xn--bcher-kva.example.com"
        );
    }

    #[test]
    fn test_port_only_no_path() {
        assert_eq!(extract_url_host("http://host:5432"), "host");
    }

    #[test]
    fn test_malformed_url() {
        // Not an http:// URL — must return empty string, not panic.
        assert_eq!(extract_url_host("not-a-url"), "");
    }

    #[test]
    fn test_https_scheme() {
        assert_eq!(
            extract_url_host("https://secure.worker.local/sparql"),
            "secure.worker.local"
        );
    }

    #[test]
    fn test_ipv6_malformed_no_close_bracket() {
        // Malformed IPv6 literal — must return empty, not panic.
        assert_eq!(extract_url_host("http://[::1"), "");
    }
}
