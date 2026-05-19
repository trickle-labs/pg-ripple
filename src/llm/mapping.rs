//! Automated Ontology Mapping (v0.57.0) — extracted from mod.rs v0.122.0 H17-02.

use pgrx::prelude::*;

// ─── Automated Ontology Mapping (v0.57.0) ─────────────────────────────────────

/// Suggest cross-ontology class alignments using label similarity.
///
/// `method = 'lexical'` uses Jaccard similarity over tokenized `rdfs:label` values.
/// `method = 'embedding'` uses KGE embedding similarity (requires `kge_enabled = on`).
///
/// Returns a table of (source_class, target_class, confidence) pairs.
#[pg_extern(schema = "pg_ripple", name = "suggest_mappings")]
pub fn suggest_mappings(
    source_ontology_graph: &str,
    target_ontology_graph: &str,
    method: default!(&str, "'lexical'"),
) -> TableIterator<
    'static,
    (
        name!(source_class, String),
        name!(target_class, String),
        name!(confidence, f64),
    ),
> {
    use pgrx::datum::DatumWithOid;

    let rdfs_label = crate::dictionary::encode(
        "http://www.w3.org/2000/01/rdf-schema#label",
        crate::dictionary::KIND_IRI,
    );
    let src_graph_id = if source_ontology_graph.is_empty() {
        0i64
    } else {
        crate::dictionary::encode(source_ontology_graph, crate::dictionary::KIND_IRI)
    };
    let tgt_graph_id = if target_ontology_graph.is_empty() {
        0i64
    } else {
        crate::dictionary::encode(target_ontology_graph, crate::dictionary::KIND_IRI)
    };

    // Collect (entity_id, label) pairs from each graph.
    let collect_labels = |graph_id: i64| -> Vec<(i64, String)> {
        Spi::connect(|client| {
            let rows = client.select(
                "SELECT s, o FROM _pg_ripple.vp_rare WHERE p = $1 AND g = $2 LIMIT 500",
                None,
                &[DatumWithOid::from(rdfs_label), DatumWithOid::from(graph_id)],
            )?;
            let mut pairs = Vec::new();
            for row in rows {
                let s = row.get::<i64>(1)?.unwrap_or(0);
                let o = row.get::<i64>(2)?.unwrap_or(0);
                if s != 0
                    && o != 0
                    && let Some(label) = crate::dictionary::decode(o)
                {
                    pairs.push((s, label));
                }
            }
            Ok::<_, pgrx::spi::Error>(pairs)
        })
        .unwrap_or_default()
    };

    let src_labels = collect_labels(src_graph_id);
    let tgt_labels = collect_labels(tgt_graph_id);

    let use_embedding = method.eq_ignore_ascii_case("embedding");

    let mut results: Vec<(String, String, f64)> = Vec::new();

    for (src_id, src_label) in &src_labels {
        let mut best_score = 0.0f64;
        let mut best_tgt_id = 0i64;

        for (tgt_id, tgt_label) in &tgt_labels {
            let score = if use_embedding && crate::KGE_ENABLED.get() {
                // Use KGE embedding similarity.
                kge_entity_similarity(*src_id, *tgt_id)
            } else {
                // Lexical Jaccard similarity over tokenized labels.
                jaccard_similarity(src_label, tgt_label)
            };

            if score > best_score {
                best_score = score;
                best_tgt_id = *tgt_id;
            }
        }

        if best_score > 0.3 && best_tgt_id != 0 {
            let src_iri = crate::dictionary::decode(*src_id).unwrap_or_default();
            let tgt_iri = crate::dictionary::decode(best_tgt_id).unwrap_or_default();
            if !src_iri.is_empty() && !tgt_iri.is_empty() {
                results.push((src_iri, tgt_iri, best_score));
            }
        }
    }

    // Sort by confidence descending.
    results.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    TableIterator::new(results)
}

/// Compute cosine similarity between two entity KGE embeddings.
fn kge_entity_similarity(entity_a: i64, entity_b: i64) -> f64 {
    use pgrx::datum::DatumWithOid;

    let fetch_emb = |eid: i64| -> Option<Vec<f64>> {
        Spi::connect(|client| {
            let rows = client.select(
                "SELECT embedding::text FROM _pg_ripple.kge_embeddings WHERE entity_id = $1",
                None,
                &[DatumWithOid::from(eid)],
            )?;
            let mut result = None;
            for row in rows {
                if let Some(s) = row.get::<String>(1)? {
                    result = parse_embedding_str(&s);
                    break;
                }
            }
            Ok::<_, pgrx::spi::Error>(result)
        })
        .ok()
        .flatten()
    };
    let emb_a = fetch_emb(entity_a);
    let emb_b = fetch_emb(entity_b);

    match (emb_a, emb_b) {
        (Some(a), Some(b)) => {
            let len = a.len().min(b.len());
            let dot: f64 = a[..len]
                .iter()
                .zip(b[..len].iter())
                .map(|(x, y)| x * y)
                .sum();
            let na: f64 = a[..len].iter().map(|x| x * x).sum::<f64>().sqrt();
            let nb: f64 = b[..len].iter().map(|x| x * x).sum::<f64>().sqrt();
            if na < 1e-10 || nb < 1e-10 {
                0.0
            } else {
                dot / (na * nb)
            }
        }
        _ => 0.0,
    }
}

/// Parse a pgvector embedding string into a Vec<f64>.
fn parse_embedding_str(s: &str) -> Option<Vec<f64>> {
    let s = s.trim().trim_start_matches('[').trim_end_matches(']');
    if s.is_empty() {
        return None;
    }
    let values: Vec<f64> = s
        .split(',')
        .filter_map(|x| x.trim().parse::<f64>().ok())
        .collect();
    if values.is_empty() {
        None
    } else {
        Some(values)
    }
}

/// Compute Jaccard similarity between two label strings (tokenized on whitespace).
fn jaccard_similarity(a: &str, b: &str) -> f64 {
    let tokens_a: std::collections::HashSet<&str> = a.split_whitespace().collect();
    let tokens_b: std::collections::HashSet<&str> = b.split_whitespace().collect();

    if tokens_a.is_empty() && tokens_b.is_empty() {
        return 1.0;
    }
    if tokens_a.is_empty() || tokens_b.is_empty() {
        return 0.0;
    }

    let intersection = tokens_a.intersection(&tokens_b).count();
    let union = tokens_a.union(&tokens_b).count();

    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}
