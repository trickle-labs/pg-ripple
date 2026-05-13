//! Hybrid search: Reciprocal Rank Fusion of SPARQL + vector results.
//! (extracted from embedding.rs in v0.114.0)

use super::index::{pgvector_guard, similar_entities};

/// Hybrid search using Reciprocal Rank Fusion of SPARQL and vector results.
///
/// Executes `sparql_query` to get a SPARQL-ranked candidate set, then executes
/// `similar_entities(query_text, k*10)` for the vector-ranked set.  Applies RRF
/// with $k_{rrf} = 60$; `alpha` controls SPARQL vs vector weight.
///
/// Returns top-`k` entities sorted by descending `rrf_score`.
///
/// When pgvector is absent, returns zero rows with a WARNING.
pub fn hybrid_search(
    sparql_query: &str,
    query_text: &str,
    k: i32,
    alpha: f64,
    model: Option<&str>,
) -> Vec<(i64, String, f64, i32, i32)> {
    if !pgvector_guard("hybrid_search") {
        return Vec::new();
    }

    let k_rrf: f64 = 60.0;
    let alpha = alpha.clamp(0.0, 1.0);

    // --- SPARQL-ranked candidates ---
    let sparql_rows = crate::sparql::sparql(sparql_query);
    let mut sparql_ids: Vec<i64> = Vec::new();
    for row in &sparql_rows {
        if let Some(obj) = row.0.as_object() {
            // Expect ?entity binding as an IRI string.
            for (_key, val) in obj.iter() {
                if let Some(s) = val.as_str() {
                    let iri = s.trim_start_matches('<').trim_end_matches('>');
                    let id = crate::dictionary::encode(iri, crate::dictionary::KIND_IRI);
                    if id != 0 {
                        sparql_ids.push(id);
                        break;
                    }
                }
            }
        }
    }

    // --- Vector-ranked candidates ---
    let vector_k = (k * 10).max(20);
    let vector_rows = similar_entities(query_text, vector_k, model);

    // --- RRF fusion ---
    use std::collections::HashMap;

    // entity_id → (entity_iri, sparql_rank, vector_rank)
    let mut scores: HashMap<i64, (String, i32, i32)> = HashMap::new();

    for (rank, &entity_id) in sparql_ids.iter().enumerate() {
        let iri = crate::dictionary::decode(entity_id).unwrap_or_default();
        let entry = scores.entry(entity_id).or_insert((iri, 0, 0));
        entry.1 = rank as i32 + 1; // 1-based rank
    }

    for (rank, (entity_id, entity_iri, _distance)) in vector_rows.iter().enumerate() {
        let entry = scores
            .entry(*entity_id)
            .or_insert((entity_iri.clone(), 0, 0));
        entry.2 = rank as i32 + 1; // 1-based rank
        if entry.0.is_empty() {
            entry.0 = entity_iri.clone();
        }
    }

    // Compute RRF scores.
    let mut results: Vec<(i64, String, f64, i32, i32)> = scores
        .into_iter()
        .map(|(entity_id, (entity_iri, sparql_rank, vector_rank))| {
            let sparql_rrf = if sparql_rank > 0 {
                1.0 / (k_rrf + sparql_rank as f64)
            } else {
                0.0
            };
            let vector_rrf = if vector_rank > 0 {
                1.0 / (k_rrf + vector_rank as f64)
            } else {
                0.0
            };
            let rrf_score = alpha * sparql_rrf + (1.0 - alpha) * vector_rrf;
            (entity_id, entity_iri, rrf_score, sparql_rank, vector_rank)
        })
        .collect();

    // Sort by descending RRF score.
    results.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(k as usize);
    results
}
