//! Datalog-native PageRank engine (v0.88.0 / v0.90.0 module split).
//!
//! Implements iterative PageRank using SQL via SPI, leveraging the Datalog^agg
//! infrastructure (aggregation in rule bodies) and subsumptive tabling for
//! convergence-aware early termination.  Magic-sets transformation is applied
//! for goal-directed partial-graph evaluation when a bound node is requested.
//!
//! Key design points:
//! - All scores live in `_pg_ripple.pagerank_scores` (the "PageRank view").
//! - VP table joins use integer IDs — no string comparisons in the hot path.
//! - Confidence weighting is optional (PR-CONF-01) and reads from `_pg_ripple.confidence`.
//! - K-hop incremental refresh is queued via `_pg_ripple.pagerank_dirty_edges` (PR-TRICKLE-01).
//! - Centrality measures share the same score table under different `metric` values (PR-CENTRALITY-01).
//!
//! ## Module structure (v0.90.0 CQ-03)
//! - `executor`   — `run_pagerank()`, convergence loop, WCOJ path selection
//! - `ivm`        — dirty-edge queue, K-hop propagation, staleness management
//! - `sketch`     — Count-Min Sketch top-K
//! - `centrality` — betweenness, closeness, eigenvector, Katz
//! - `export`     — Turtle/JSON-LD/CSV/N-Triples export, IRI serialisation
//! - `explain`    — `explain_pagerank()`, score-explanation trees

#![allow(dead_code)]

pub mod centrality;
pub mod executor;
pub mod explain;
pub mod export;
pub mod ivm;
pub mod sketch;

// Re-export the full public API so callers can use `crate::pagerank::foo()`.
pub use centrality::{centrality_run, find_pagerank_duplicates};
pub use executor::run_pagerank;
pub use explain::explain_pagerank;
pub use export::export_pagerank;
pub use ivm::{pagerank_queue_stats, vacuum_pagerank_dirty};

// ── Error codes ───────────────────────────────────────────────────────────────

pub const PT0401: &str = "PT0401";
pub const PT0402: &str = "PT0402";
pub const PT0403: &str = "PT0403";
pub const PT0404: &str = "PT0404";
pub const PT0406: &str = "PT0406";
pub const PT0408: &str = "PT0408";
pub const PT0409: &str = "PT0409";
pub const PT0411: &str = "PT0411";
pub const PT0412: &str = "PT0412";
pub const PT0413: &str = "PT0413";
pub const PT0414: &str = "PT0414";
pub const PT0415: &str = "PT0415";
pub const PT0417: &str = "PT0417";
pub const PT0419: &str = "PT0419";
pub const PT0420: &str = "PT0420";
pub const PT0421: &str = "PT0421";
pub const PT0422: &str = "PT0422";
pub const PT0423: &str = "PT0423";

// ── Result row types ──────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct PageRankRow {
    pub node_iri: String,
    pub score: f64,
    pub score_lower: f64,
    pub score_upper: f64,
    pub iterations: i32,
    pub converged: bool,
    pub stale: bool,
    pub topic: String,
}

#[derive(Debug)]
pub struct CentralityRow {
    pub node_iri: String,
    pub score: f64,
}

#[derive(Debug)]
pub struct ExplainPageRankRow {
    pub depth: i32,
    pub contributor_iri: String,
    pub contribution: f64,
    pub path: String,
}

#[derive(Debug)]
pub struct QueueStatsRow {
    pub queued_edges: i64,
    pub max_delta: f64,
    pub oldest_enqueue: Option<pgrx::datum::TimestampWithTimeZone>,
    pub estimated_drain_seconds: f64,
}

#[derive(Debug)]
pub struct DuplicateRow {
    pub node_a_iri: String,
    pub node_b_iri: String,
    pub centrality_score: f64,
    pub fuzzy_score: f64,
}

// ── Core PageRank computation parameters ──────────────────────────────────────

/// Parameters for a PageRank run.
pub struct PageRankParams {
    pub edge_predicates: Option<Vec<String>>,
    pub damping: f64,
    pub max_iterations: i32,
    pub convergence_delta: f64,
    pub graph_uri: Option<String>,
    pub direction: String,
    pub edge_weight_predicate: Option<String>,
    pub topic: Option<String>,
    pub decay_rate: f64,
    pub temporal_predicate: Option<String>,
    pub seed_iris: Option<Vec<String>>,
    pub bias: f64,
    pub predicate_filter: Option<Vec<String>>,
}

impl Default for PageRankParams {
    fn default() -> Self {
        Self {
            edge_predicates: None,
            damping: 0.85,
            max_iterations: 100,
            convergence_delta: 0.0001,
            graph_uri: None,
            direction: "forward".to_owned(),
            edge_weight_predicate: None,
            topic: None,
            decay_rate: 0.0,
            temporal_predicate: None,
            seed_iris: None,
            bias: 0.15,
            predicate_filter: None,
        }
    }
}
