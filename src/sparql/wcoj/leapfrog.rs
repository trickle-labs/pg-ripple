//! Leapfrog Triejoin executor for cyclic SPARQL BGP patterns (v0.79.0).
//!
//! Contains `CyclicBgpPattern`, `LftiBinding`, `execute_leapfrog_triejoin`,
//! `try_leapfrog_select`, and related helpers.

use super::trie::{EdgeData, SortedIterator, leapfrog_intersect};
use super::detect_cyclic_bgp;

// ─── Types ────────────────────────────────────────────────────────────────────

/// One triple pattern in a cyclic BGP, with both variables identified by name
/// and the predicate encoded as an i64 dictionary ID.
#[derive(Debug, Clone)]
pub struct CyclicBgpPattern {
    /// Variable name in subject position (or `"_"` for a constant).
    pub subject_var: String,
    /// Encoded predicate ID (i64 dictionary key).
    pub pred_id: i64,
    /// Variable name in object position (or `"_"` for a constant).
    pub object_var: String,
}

/// Result of a Leapfrog Triejoin execution: one binding per output row,
/// mapping variable name → encoded i64 dictionary ID.
pub type LftiBinding = std::collections::HashMap<String, i64>;

// ─── Executor ────────────────────────────────────────────────────────────────

/// Execute a Leapfrog Triejoin for a cyclic BGP.
///
/// `patterns` — the triple patterns in the BGP (all predicates must be bound).
/// `variable_order` — the join order produced by the WCOJ planner (from
///   `analyse_bgp`).
///
/// Returns a vector of bindings.  Returns `None` when the executor cannot be
/// applied (e.g. an unbound predicate or a VP table that does not exist), so
/// the caller can fall back to the SQL hash-join path.
///
/// Each binding maps variable name → encoded i64 ID.
pub fn execute_leapfrog_triejoin(
    patterns: &[CyclicBgpPattern],
    variable_order: &[String],
) -> Option<Vec<LftiBinding>> {
    use std::collections::HashMap;

    if patterns.is_empty() || variable_order.is_empty() {
        return None;
    }

    // Load edge data for each unique predicate.
    let mut edge_cache: HashMap<i64, EdgeData> = HashMap::new();
    for pat in patterns {
        if let std::collections::hash_map::Entry::Vacant(e) = edge_cache.entry(pat.pred_id) {
            let ed = EdgeData::load_from_vp(pat.pred_id)?;
            e.insert(ed);
        }
    }

    // Build the binding by iterating variables in order.
    let mut result_bindings: Vec<LftiBinding> = Vec::new();
    let mut current_binding: HashMap<String, i64> = HashMap::new();

    lfti_recurse(
        patterns,
        variable_order,
        0,
        &edge_cache,
        &mut current_binding,
        &mut result_bindings,
    );

    Some(result_bindings)
}

/// Recursive depth-first search over the variable order.
///
/// At each depth, intersects the sorted value sets from all patterns
/// that reference the current variable, given the already-bound variables.
fn lfti_recurse(
    patterns: &[CyclicBgpPattern],
    variable_order: &[String],
    depth: usize,
    edge_cache: &std::collections::HashMap<i64, EdgeData>,
    current_binding: &mut std::collections::HashMap<String, i64>,
    result: &mut Vec<LftiBinding>,
) {
    if depth == variable_order.len() {
        // All variables bound — verify remaining unprocessed constraints and emit.
        result.push(current_binding.clone());
        return;
    }

    let var = &variable_order[depth];

    // Collect sorted candidate values for this variable from each pattern
    // that references it, respecting already-bound values.
    let mut iters: Vec<SortedIterator> = Vec::new();

    for pat in patterns {
        let ed = match edge_cache.get(&pat.pred_id) {
            Some(e) => e,
            None => continue,
        };

        let var_in_s = pat.subject_var == *var;
        let var_in_o = pat.object_var == *var;

        if !var_in_s && !var_in_o {
            continue;
        }

        // The other variable in this pattern.
        let other_s = pat.subject_var.as_str();
        let other_o = pat.object_var.as_str();

        if var_in_s {
            // Collect s values constrained by the (possibly bound) o.
            let candidates = if let Some(&bound_o) = current_binding.get(other_o) {
                // o is already bound — get all s where edge(s, bound_o) exists.
                ed.s_for_o(bound_o)
            } else {
                // o is not yet bound — all s values are candidates.
                ed.all_s()
            };
            iters.push(SortedIterator::new(candidates));
        } else {
            // var_in_o: Collect o values constrained by the (possibly bound) s.
            let candidates = if let Some(&bound_s) = current_binding.get(other_s) {
                // s is already bound — get all o where edge(bound_s, o) exists.
                ed.o_for_s(bound_s)
            } else {
                // s is not yet bound — all o values are candidates.
                ed.all_o()
            };
            iters.push(SortedIterator::new(candidates));
        }
    }

    if iters.is_empty() {
        // No constraints on this variable — enumerate all values from any edge.
        // Fall through with an unconstrained pass (shouldn't happen in a well-formed cyclic BGP).
        lfti_recurse(
            patterns,
            variable_order,
            depth + 1,
            edge_cache,
            current_binding,
            result,
        );
        return;
    }

    // Intersect the candidate sets using the Leapfrog algorithm.
    let common_values = leapfrog_intersect(&mut iters);

    for val in common_values {
        current_binding.insert(var.clone(), val);
        lfti_recurse(
            patterns,
            variable_order,
            depth + 1,
            edge_cache,
            current_binding,
            result,
        );
        current_binding.remove(var.as_str());
    }
}

// ─── Entry point ─────────────────────────────────────────────────────────────

/// Try to execute a SELECT query using the Leapfrog Triejoin executor.
///
/// Returns `None` if the query cannot be handled by LFTI (non-cyclic pattern,
/// unbound predicates, cardinality below threshold, or WCOJ disabled).
/// Returns `Some(bindings)` when LFTI executed successfully.
///
/// Each binding maps variable name → encoded i64 ID.
pub fn try_leapfrog_select(query: &spargebra::Query) -> Option<Vec<LftiBinding>> {
    if !crate::WCOJ_ENABLED.get() {
        return None;
    }

    let pattern = match query {
        spargebra::Query::Select { pattern, .. } => pattern,
        _ => return None,
    };

    // Extract flat BGP patterns.
    let bgp_triples = extract_flat_bgp(pattern)?;
    if bgp_triples.is_empty() {
        return None;
    }

    // Check minimum table count.
    let min_tables = crate::WCOJ_MIN_TABLES.get() as usize;
    if bgp_triples.len() < min_tables {
        return None;
    }

    // Build pattern_vars for cycle detection.
    let pattern_vars: Vec<Vec<String>> = bgp_triples
        .iter()
        .map(|(s_var, _, o_var)| {
            let mut v = Vec::new();
            if let Some(s) = s_var {
                v.push(s.clone());
            }
            if let Some(o) = o_var {
                v.push(o.clone());
            }
            v
        })
        .collect();

    if !detect_cyclic_bgp(&pattern_vars) {
        return None;
    }

    // Encode predicates; bail out if any predicate is unbound.
    let mut cyclic_patterns: Vec<CyclicBgpPattern> = Vec::new();
    for (s_var, pred_iri, o_var) in &bgp_triples {
        let pred_iri = pred_iri.as_ref()?;
        let pred_id = crate::dictionary::lookup_iri(pred_iri)?;
        cyclic_patterns.push(CyclicBgpPattern {
            subject_var: s_var.clone().unwrap_or_else(|| "_".to_string()),
            pred_id,
            object_var: o_var.clone().unwrap_or_else(|| "_".to_string()),
        });
    }

    // Check minimum cardinality threshold.
    let min_card = crate::gucs::sparql::WCOJ_MIN_CARDINALITY.get() as usize;
    if min_card > 0 {
        for pat in &cyclic_patterns {
            // Rough cardinality check: edge count for this predicate.
            use pgrx::datum::DatumWithOid;
            use pgrx::prelude::*;
            let card: i64 = Spi::get_one_with_args::<i64>(
                "SELECT COALESCE(triple_count, 0) FROM _pg_ripple.predicates WHERE id = $1",
                &[DatumWithOid::from(pat.pred_id)],
            )
            .ok()
            .flatten()
            .unwrap_or(0);
            if (card as usize) < min_card {
                return None; // Too small; fall back to hash join.
            }
        }
    }

    // Build variable order from pattern variables (all distinct variables in order of appearance).
    let mut seen = std::collections::HashSet::new();
    let mut variable_order: Vec<String> = Vec::new();
    for vars in &pattern_vars {
        for v in vars {
            if seen.insert(v.clone()) {
                variable_order.push(v.clone());
            }
        }
    }

    // Execute the Leapfrog Triejoin.
    execute_leapfrog_triejoin(&cyclic_patterns, &variable_order)
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Extract a flat list of (subject_var, predicate_iri, object_var) triples from
/// a purely flat BGP pattern.  Returns `None` if the pattern is not a flat BGP.
type FlatBgpTriple = (Option<String>, Option<String>, Option<String>);

#[allow(clippy::type_complexity)]
fn extract_flat_bgp(pattern: &spargebra::algebra::GraphPattern) -> Option<Vec<FlatBgpTriple>> {
    use spargebra::algebra::GraphPattern;
    use spargebra::term::{NamedNodePattern, TermPattern};

    match pattern {
        GraphPattern::Bgp { patterns } => {
            let mut result = Vec::new();
            for tp in patterns {
                let s_var = match &tp.subject {
                    TermPattern::Variable(v) => Some(v.as_str().to_owned()),
                    _ => None,
                };
                let p_iri = match &tp.predicate {
                    NamedNodePattern::NamedNode(nn) => Some(nn.as_str().to_owned()),
                    _ => None, // Variable predicate — cannot use LFTI
                };
                let o_var = match &tp.object {
                    TermPattern::Variable(v) => Some(v.as_str().to_owned()),
                    _ => None,
                };
                result.push((s_var, p_iri, o_var));
            }
            Some(result)
        }
        GraphPattern::Project { inner, .. }
        | GraphPattern::Distinct { inner }
        | GraphPattern::Reduced { inner }
        | GraphPattern::OrderBy { inner, .. }
        | GraphPattern::Slice { inner, .. } => extract_flat_bgp(inner),
        _ => None,
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(any(test, feature = "pg_test"))]
#[pgrx::pg_schema]
mod lfti_tests {
    use super::*;

    #[test]
    fn test_triangle_is_cyclic() {
        // Triangle: ?a-?b, ?b-?c, ?c-?a
        let patterns = vec![
            vec!["a".to_owned(), "b".to_owned()],
            vec!["b".to_owned(), "c".to_owned()],
            vec!["c".to_owned(), "a".to_owned()],
        ];
        assert!(detect_cyclic_bgp(&patterns));
    }

    #[test]
    fn test_star_is_acyclic() {
        // Star pattern: ?root with 3 arms — no cycle
        let patterns = vec![
            vec!["root".to_owned(), "a".to_owned()],
            vec!["root".to_owned(), "b".to_owned()],
            vec!["root".to_owned(), "c".to_owned()],
        ];
        assert!(!detect_cyclic_bgp(&patterns));
    }
}
