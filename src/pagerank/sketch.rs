//! Count-Min Sketch top-K for approximate PageRank score tracking.
//!
//! The sketch is parameterized by:
//! - `pg_ripple.pagerank_sketch_width` (INT, default 2000): counters per row.
//!   Memory per sketch: width × depth × 8 bytes.
//!   Error bound: ε = e / width (expected additive error per query).
//! - `pg_ripple.pagerank_sketch_depth` (INT, default 5): number of hash rows.
//!   Error probability: δ = e^{-depth} (at depth=5, δ ≈ 0.0067).
//!
//! Reference: Cormode & Muthukrishnan (2005), "An improved data stream summary:
//! the count-min sketch and its applications."
//!
//! This module is a stub — the sketch is read via GUC parameters. Full
//! integration with `pg:topN_approx()` is delivered as part of the SPARQL
//! function binding in `src/sparql/embedding.rs`.

/// Returns the configured sketch width (counters per row).
pub fn sketch_width() -> i32 {
    crate::PAGERANK_SKETCH_WIDTH.get()
}

/// Returns the configured sketch depth (number of hash rows / hash functions).
pub fn sketch_depth() -> i32 {
    crate::PAGERANK_SKETCH_DEPTH.get()
}

/// Compute the memory footprint of one sketch instance in bytes.
pub fn sketch_memory_bytes() -> usize {
    sketch_width() as usize * sketch_depth() as usize * 8
}
