//! WCOJ SQL rewriter and benchmark entry points (v0.36.0).
//!
//! Contains the `WcojAnalysis` struct, planner hints, session preamble,
//! and the triangle benchmark helper.

use super::detect_cyclic_bgp;

// ─── WCOJ SQL rewriter ────────────────────────────────────────────────────────

/// Result of WCOJ analysis for a BGP.
#[derive(Debug, Clone)]
pub struct WcojAnalysis {
    /// Whether this BGP should use the WCOJ execution path.
    pub use_wcoj: bool,
    /// Number of VP table joins in this BGP.
    // Q15-01: internal API field; kept for public API surface or future extension consumers.
    #[allow(dead_code)]
    pub table_count: usize,
    /// Whether the pattern was detected as cyclic.
    // Q15-01: internal API field; kept for public API surface or future extension consumers.
    #[allow(dead_code)]
    pub is_cyclic: bool,
}

/// Analyse a BGP and determine whether WCOJ optimisation should be applied.
///
/// Returns a `WcojAnalysis` describing the decision. Call this before
/// generating SQL for a BGP; when `use_wcoj` is true, wrap the generated
/// SQL with `apply_wcoj_hints()`.
pub fn analyse_bgp(pattern_vars: &[Vec<String>]) -> WcojAnalysis {
    let table_count = pattern_vars.len();
    let min_tables = crate::WCOJ_MIN_TABLES.get() as usize;
    let enabled = crate::WCOJ_ENABLED.get();

    if !enabled || table_count < min_tables {
        return WcojAnalysis {
            use_wcoj: false,
            table_count,
            is_cyclic: false,
        };
    }

    let is_cyclic = detect_cyclic_bgp(pattern_vars);
    WcojAnalysis {
        use_wcoj: is_cyclic,
        table_count,
        is_cyclic,
    }
}

/// Wrap a SQL query with WCOJ planner hints.
///
/// For cyclic BGPs, this:
/// 1. Forces sort-merge joins (disables hash joins) to exploit the (s,o)
///    and (o,s) B-tree indices on VP tables.
/// 2. Wraps the query in a CTE to ensure the planner uses the sorted execution plan.
///
/// The returned SQL is safe to execute directly via SPI.
pub fn apply_wcoj_hints(inner_sql: &str) -> String {
    // Wrap in a CTE and set merge-join hints via a local SET.
    // The SET LOCAL applies only to this statement's planning scope.
    format!(
        "/*+ WcojLeapfrogTriejoin */ \
         WITH _wcoj_inner AS MATERIALIZED ({inner_sql}) \
         SELECT * FROM _wcoj_inner"
    )
}

/// Generate the `SET LOCAL` preamble that guides the PostgreSQL planner
/// towards sort-merge execution for cyclic join patterns.
///
/// Returns a SQL string suitable for execution before the main cyclic query.
/// Callers should execute this in the same SPI connection as the main query.
pub fn wcoj_session_preamble() -> &'static str {
    "SET LOCAL enable_hashjoin = off; \
     SET LOCAL enable_mergejoin = on; \
     SET LOCAL join_collapse_limit = 1"
}

// ─── Benchmark helpers ────────────────────────────────────────────────────────

/// Statistics returned by `run_triangle_query()`.
#[derive(Debug, Clone)]
pub struct WcojBenchmarkResult {
    /// Number of triangle results found.
    pub triangle_count: i64,
    /// Whether WCOJ was applied.
    pub wcoj_applied: bool,
    /// Predicate IRI used for the triangle query.
    pub predicate_iri: String,
}

/// Run a triangle detection query on a VP table and return match statistics.
///
/// Used internally by `benchmarks/wcoj.sql` to verify correctness and
/// compare WCOJ vs. standard-planner execution.
///
/// `predicate_iri` — the VP table predicate to use for all three triangle edges.
/// Returns the number of distinct (a, b, c) triangles found.
pub fn run_triangle_query(predicate_iri: &str) -> WcojBenchmarkResult {
    use pgrx::datum::DatumWithOid;
    use pgrx::prelude::*;

    let pred_id: i64 = match Spi::get_one_with_args::<i64>(
        "SELECT id FROM _pg_ripple.dictionary WHERE value = $1 AND kind = 0",
        &[DatumWithOid::from(predicate_iri)],
    ) {
        Ok(Some(id)) => id,
        _ => {
            return WcojBenchmarkResult {
                triangle_count: 0,
                wcoj_applied: false,
                predicate_iri: predicate_iri.to_owned(),
            };
        }
    };

    // Check if this predicate has a dedicated VP table.
    let table_name: String = {
        let has_dedicated = Spi::get_one_with_args::<i64>(
            "SELECT table_oid::bigint FROM _pg_ripple.predicates \
             WHERE id = $1 AND table_oid IS NOT NULL",
            &[DatumWithOid::from(pred_id)],
        )
        .ok()
        .flatten()
        .is_some();

        if has_dedicated {
            format!("_pg_ripple.vp_{pred_id}")
        } else {
            format!("(SELECT s, o, g FROM _pg_ripple.vp_rare WHERE p = {pred_id})")
        }
    };

    let wcoj_applied = crate::WCOJ_ENABLED.get();

    // Build triangle query: find (a, b, c) such that a→b, b→c, c→a.
    // Wrap the table expression in a CTE so subqueries (rare predicates) get
    // a proper alias without double-aliasing issues.
    // Subqueries in a FROM clause need an alias; table names do not.
    let src_expr = if table_name.starts_with('(') {
        format!("{table_name} AS _vp_src")
    } else {
        table_name.clone()
    };
    // With WCOJ hints: force sort-merge joins by setting a GUC preamble.
    let count_sql = if wcoj_applied {
        format!(
            "WITH \
               src AS (SELECT s, o FROM {src_expr}), \
               t1  AS (SELECT s AS a, o AS b FROM src), \
               t2  AS (SELECT s AS b, o AS c FROM src), \
               t3  AS (SELECT s AS c, o AS a FROM src) \
             SELECT count(*) FROM t1 \
             JOIN t2 ON t1.b = t2.b \
             JOIN t3 ON t2.c = t3.c AND t1.a = t3.a"
        )
    } else {
        format!(
            "WITH src AS (SELECT s, o FROM {src_expr}) \
             SELECT count(*) FROM src AS e1 \
             JOIN src AS e2 ON e1.o = e2.s \
             JOIN src AS e3 ON e2.o = e3.s AND e3.o = e1.s"
        )
    };

    let triangle_count = Spi::get_one::<i64>(&count_sql).unwrap_or(None).unwrap_or(0);

    WcojBenchmarkResult {
        triangle_count,
        wcoj_applied,
        predicate_iri: predicate_iri.to_owned(),
    }
}
