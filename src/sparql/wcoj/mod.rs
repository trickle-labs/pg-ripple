//! Worst-Case Optimal Join (WCOJ) optimiser for cyclic SPARQL patterns (v0.36.0).
//!
//! # Module layout (v0.114.0)
//!
//! | Sub-module | Contents |
//! |---|---|
//! | `executor`  | `WcojAnalysis`, planner hints, triangle benchmark |
//! | `trie`      | `SortedIterator`, `leapfrog_intersect`, `EdgeData` |
//! | `leapfrog`  | `CyclicBgpPattern`, `LftiBinding`, `execute_leapfrog_triejoin`, `try_leapfrog_select` |
//!
//! # GUC controls
//! - `pg_ripple.wcoj_enabled` (bool, default `true`) — master switch.
//! - `pg_ripple.wcoj_min_tables` (integer, default `3`) — minimum VP table joins.

use std::collections::{HashMap, HashSet};

pub mod executor;
pub mod leapfrog;
pub mod trie;

// Re-export so callers use `crate::sparql::wcoj::*` unchanged.
pub use executor::{analyse_bgp, apply_wcoj_hints, run_triangle_query, wcoj_session_preamble};
pub use leapfrog::{LftiBinding, try_leapfrog_select};

// ─── BGP variable extraction ──────────────────────────────────────────────────

/// Walk a `spargebra` `GraphPattern` and collect variable names per triple pattern.
pub fn extract_bgp_pattern_vars(pattern: &spargebra::algebra::GraphPattern) -> Vec<Vec<String>> {
    use spargebra::algebra::GraphPattern;
    use spargebra::term::{NamedNodePattern, TermPattern};

    let mut out: Vec<Vec<String>> = Vec::new();

    fn walk(p: &GraphPattern, out: &mut Vec<Vec<String>>) {
        match p {
            GraphPattern::Bgp { patterns } => {
                for tp in patterns {
                    let mut vars = Vec::new();
                    if let TermPattern::Variable(v) = &tp.subject {
                        vars.push(v.as_str().to_owned());
                    }
                    if let NamedNodePattern::Variable(v) = &tp.predicate {
                        vars.push(v.as_str().to_owned());
                    }
                    if let TermPattern::Variable(v) = &tp.object {
                        vars.push(v.as_str().to_owned());
                    }
                    if !vars.is_empty() {
                        out.push(vars);
                    }
                }
            }
            GraphPattern::Join { left, right }
            | GraphPattern::Union { left, right }
            | GraphPattern::LeftJoin { left, right, .. } => {
                walk(left, out);
                walk(right, out);
            }
            GraphPattern::Filter { inner, .. }
            | GraphPattern::Graph { inner, .. }
            | GraphPattern::Extend { inner, .. }
            | GraphPattern::OrderBy { inner, .. }
            | GraphPattern::Project { inner, .. }
            | GraphPattern::Distinct { inner }
            | GraphPattern::Reduced { inner }
            | GraphPattern::Slice { inner, .. }
            | GraphPattern::Group { inner, .. } => {
                walk(inner, out);
            }
            _ => {}
        }
    }

    walk(pattern, &mut out);
    out
}

// ─── Cycle detection ──────────────────────────────────────────────────────────

/// Detect whether a BGP variable adjacency graph contains a cycle.
pub fn detect_cyclic_bgp(pattern_vars: &[Vec<String>]) -> bool {
    if pattern_vars.len() < 3 {
        return false;
    }
    let mut adj: HashMap<String, HashSet<String>> = HashMap::new();
    for vars in pattern_vars {
        for i in 0..vars.len() {
            for j in (i + 1)..vars.len() {
                let a = &vars[i];
                let b = &vars[j];
                if a != b {
                    adj.entry(a.clone()).or_default().insert(b.clone());
                    adj.entry(b.clone()).or_default().insert(a.clone());
                }
            }
        }
    }
    let nodes: Vec<String> = adj.keys().cloned().collect();
    let mut visited: HashSet<String> = HashSet::new();
    let mut rec_stack: HashSet<String> = HashSet::new();
    for node in &nodes {
        if !visited.contains(node) && has_cycle_dfs(node, None, &adj, &mut visited, &mut rec_stack)
        {
            return true;
        }
    }
    false
}

fn has_cycle_dfs(
    node: &str,
    parent: Option<&str>,
    adj: &HashMap<String, HashSet<String>>,
    visited: &mut HashSet<String>,
    rec_stack: &mut HashSet<String>,
) -> bool {
    visited.insert(node.to_owned());
    rec_stack.insert(node.to_owned());
    if let Some(neighbors) = adj.get(node) {
        for neighbor in neighbors {
            if parent.is_some_and(|p| p == neighbor) {
                continue;
            }
            if !visited.contains(neighbor.as_str()) {
                if has_cycle_dfs(neighbor, Some(node), adj, visited, rec_stack) {
                    return true;
                }
            } else if rec_stack.contains(neighbor.as_str()) {
                return true;
            }
        }
    }
    rec_stack.remove(node);
    false
}

#[cfg(any(test, feature = "pg_test"))]
#[pgrx::pg_schema]
mod tests {
    use super::*;

    #[test]
    fn test_triangle_is_cyclic() {
        let p = vec![
            vec!["a".to_owned(), "b".to_owned()],
            vec!["b".to_owned(), "c".to_owned()],
            vec!["c".to_owned(), "a".to_owned()],
        ];
        assert!(detect_cyclic_bgp(&p));
    }

    #[test]
    fn test_star_is_acyclic() {
        let p = vec![
            vec!["root".to_owned(), "a".to_owned()],
            vec!["root".to_owned(), "b".to_owned()],
            vec!["root".to_owned(), "c".to_owned()],
        ];
        assert!(!detect_cyclic_bgp(&p));
    }

    #[test]
    fn test_chain_is_acyclic() {
        let p = vec![
            vec!["a".to_owned(), "b".to_owned()],
            vec!["b".to_owned(), "c".to_owned()],
        ];
        assert!(!detect_cyclic_bgp(&p));
    }

    #[test]
    fn test_square_is_cyclic() {
        let p = vec![
            vec!["a".to_owned(), "b".to_owned()],
            vec!["b".to_owned(), "c".to_owned()],
            vec!["c".to_owned(), "d".to_owned()],
            vec!["d".to_owned(), "a".to_owned()],
        ];
        assert!(detect_cyclic_bgp(&p));
    }

    #[test]
    fn test_single_not_cyclic() {
        assert!(!detect_cyclic_bgp(&[vec!["a".to_owned(), "b".to_owned()]]));
    }
}
