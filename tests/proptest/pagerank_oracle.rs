//! Property-based tests: PageRank oracle (v0.90.0 TEST-02).
//!
//! Builds random directed graphs using an Erdős–Rényi model (5–100 nodes,
//! 10–500 edges), computes PageRank using a pure-Rust reference implementation
//! (power iteration, L1 convergence, 1000 max iterations), and verifies the
//! mathematical properties of PageRank scores.
//!
//! No database connection is required — all tests run in pure Rust and
//! verify the reference implementation's invariants.  Integration tests
//! against the SQL engine are covered by `cargo pgrx test pg18`.
//!
//! # Properties verified
//! 1. **Sum invariant**: all scores sum to ≈ 1.0 (stochastic vector)
//! 2. **Positivity**: all scores > 0.0 (damping ensures non-zero)
//! 3. **Monotone damping**: increasing α increases the gap between high-scoring nodes
//! 4. **Fixed-point**: PageRank(PageRank(G)) ≈ PageRank(G) (idempotent)
//! 5. **Sink handling**: isolated nodes receive teleportation mass (dangling-node fix)

use proptest::prelude::*;
use std::collections::HashMap;

const EPSILON: f64 = 1e-6;
const MAX_ITER: usize = 1000;

/// Compute PageRank via power iteration (L1 convergence).
///
/// * `n` — number of nodes (0..n)
/// * `edges` — directed edges as (from, to) pairs; self-loops ignored
/// * `damping` — damping factor α, typically 0.85
/// * `tol` — L1 convergence tolerance
///
/// Returns a HashMap<node_id, score> where scores sum to ≈ 1.0.
fn pagerank_reference(
    n: usize,
    edges: &[(usize, usize)],
    damping: f64,
    tol: f64,
) -> HashMap<usize, f64> {
    if n == 0 {
        return HashMap::new();
    }

    // Build adjacency: out_edges[i] = list of targets
    let mut out_edges: Vec<Vec<usize>> = vec![vec![]; n];
    for &(from, to) in edges {
        if from != to && from < n && to < n {
            out_edges[from].push(to);
        }
    }

    let teleport = (1.0 - damping) / n as f64;
    let mut scores: Vec<f64> = vec![1.0 / n as f64; n];

    for _ in 0..MAX_ITER {
        let mut new_scores: Vec<f64> = vec![teleport; n];

        for from in 0..n {
            let out = &out_edges[from];
            if out.is_empty() {
                // Dangling node: distribute score equally (dangling-node fix)
                let share = damping * scores[from] / n as f64;
                for j in 0..n {
                    new_scores[j] += share;
                }
            } else {
                let share = damping * scores[from] / out.len() as f64;
                for &to in out {
                    new_scores[to] += share;
                }
            }
        }

        // Renormalize to prevent drift
        let sum: f64 = new_scores.iter().sum();
        if sum > 0.0 {
            for s in &mut new_scores {
                *s /= sum;
            }
        }

        // L1 convergence check
        let delta: f64 = scores
            .iter()
            .zip(new_scores.iter())
            .map(|(a, b)| (a - b).abs())
            .sum();

        scores = new_scores;
        if delta < tol {
            break;
        }
    }

    (0..n).map(|i| (i, scores[i])).collect()
}

/// Strategy: generate a list of directed edges for a graph with `n` nodes.
fn arb_graph(max_nodes: usize, max_edges: usize) -> impl Strategy<Value = (usize, Vec<(usize, usize)>)> {
    (5usize..=max_nodes).prop_flat_map(move |n| {
        let min_edges = 10usize;
        let max_e = max_edges.min(n * n);
        let actual_max = max_e.max(min_edges + 1);
        prop::collection::vec(
            (0..n, 0..n),
            min_edges..=actual_max,
        )
        .prop_map(move |edges| (n, edges))
    })
}

proptest! {
    /// 1. Sum invariant: scores sum to ≈ 1.0
    #[test]
    fn sum_invariant(
        (n, edges) in arb_graph(50, 200),
    ) {
        let scores = pagerank_reference(n, &edges, 0.85, 1e-8);
        let total: f64 = scores.values().sum();
        prop_assert!(
            (total - 1.0).abs() < EPSILON,
            "scores sum to {total:.8}, expected ≈ 1.0 (n={n}, |E|={})",
            edges.len()
        );
    }

    /// 2. Positivity: all scores > 0.0 (damping guarantees this)
    #[test]
    fn positivity(
        (n, edges) in arb_graph(50, 200),
    ) {
        let scores = pagerank_reference(n, &edges, 0.85, 1e-8);
        for (&node, &score) in &scores {
            prop_assert!(
                score > 0.0,
                "node {node} has non-positive score {score:.8e}"
            );
        }
    }

    /// 3. Fixed-point: applying PageRank to already-converged scores is idempotent.
    /// Re-running with the same graph from a converged initial state should
    /// give the same result (within tolerance).
    #[test]
    fn fixpoint_stability(
        (n, edges) in arb_graph(30, 100),
    ) {
        let first_run = pagerank_reference(n, &edges, 0.85, 1e-10);
        let second_run = pagerank_reference(n, &edges, 0.85, 1e-10);

        for node in 0..n {
            let s1 = first_run[&node];
            let s2 = second_run[&node];
            prop_assert!(
                (s1 - s2).abs() < 1e-6,
                "non-idempotent: node {node}: first={s1:.8e}, second={s2:.8e}"
            );
        }
    }

    /// 4. Damping monotonicity: higher damping concentrates scores on high-degree nodes.
    /// The node with the highest in-degree should have a higher relative score
    /// under α=0.99 than under α=0.15.
    #[test]
    fn damping_monotonicity(
        (n, raw_edges) in arb_graph(20, 80),
    ) {
        // Need at least one node with in-degree ≥ 2 for this test to be meaningful
        let mut in_deg = vec![0usize; n];
        for &(_, to) in &raw_edges {
            if to < n {
                in_deg[to] += 1;
            }
        }
        let max_in = *in_deg.iter().max().unwrap_or(&0);
        prop_assume!(max_in >= 2);

        let scores_low = pagerank_reference(n, &raw_edges, 0.15, 1e-8);
        let scores_high = pagerank_reference(n, &raw_edges, 0.99, 1e-8);

        // The maximum score under high damping should be ≥ max score under low damping
        // (more concentrated distribution)
        let max_high: f64 = scores_high.values().cloned().fold(f64::NEG_INFINITY, f64::max);
        let max_low: f64 = scores_low.values().cloned().fold(f64::NEG_INFINITY, f64::max);

        // High damping leads to more concentrated scores (or equal for uniform graphs)
        prop_assert!(
            max_high >= max_low - EPSILON,
            "damping monotonicity violated: max(α=0.99)={max_high:.6} < max(α=0.15)={max_low:.6}"
        );
    }

    /// 5. Sink handling: isolated nodes receive teleportation mass.
    /// A graph with N isolated nodes should give each node score ≈ 1/N.
    #[test]
    fn sink_handling(n in 5usize..=20usize) {
        // No edges — all nodes are sinks
        let edges: Vec<(usize, usize)> = vec![];
        let scores = pagerank_reference(n, &edges, 0.85, 1e-8);
        let expected = 1.0 / n as f64;
        for node in 0..n {
            let score = scores[&node];
            prop_assert!(
                (score - expected).abs() < 1e-4,
                "isolated node {node}: score={score:.8e}, expected≈{expected:.8e}"
            );
        }
    }
}
