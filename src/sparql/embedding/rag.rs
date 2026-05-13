//! RAG infrastructure: list_models, add_triples, contextualize, rag_retrieve.
//! (extracted from embedding.rs in v0.114.0)

use super::index::{pgvector_guard, similar_entities};

/// Enumerate all models stored in `_pg_ripple.embeddings`.
///
/// Returns a row per `(model, entity_count, dimensions)`.
/// When pgvector is absent, returns zero rows.
pub fn list_embedding_models() -> Vec<(String, i64, i32)> {
    if !pgvector_guard("list_embedding_models") {
        return Vec::new();
    }

    pgrx::Spi::connect(|c| {
        c.select(
            "SELECT model, COUNT(*) AS entity_count, \
                    MAX(vector_dims(embedding)) AS dimensions \
             FROM _pg_ripple.embeddings \
             GROUP BY model \
             ORDER BY entity_count DESC",
            None,
            &[],
        )
        .unwrap_or_else(|e| pgrx::error!("list_embedding_models: SPI error: {e}"))
        .map(|row| {
            let model: String = row.get::<String>(1).ok().flatten().unwrap_or_default();
            let entity_count: i64 = row.get::<i64>(2).ok().flatten().unwrap_or(0);
            let dimensions: i32 = row.get::<i32>(3).ok().flatten().unwrap_or(0);
            (model, entity_count, dimensions)
        })
        .collect()
    })
}

/// Materialise `:hasEmbedding` triples for entities present in `_pg_ripple.embeddings`.
///
/// Inserts triples `<entity_iri> <pg:hasEmbedding> "true"^^xsd:boolean` for every
/// entity that has at least one row in `_pg_ripple.embeddings`.  The SHACL shape
/// `examples/shacl_embedding_completeness.ttl` uses `sh:path :hasEmbedding ;
/// sh:minCount 1` to validate completeness.
///
/// Returns the count of newly inserted triples.
pub fn add_embedding_triples() -> i64 {
    // Collect entity IRIs from the embeddings table.
    let entity_ids: Vec<(i64, String)> = pgrx::Spi::connect(|c| {
        c.select(
            "SELECT DISTINCT e.entity_id, d.value \
             FROM _pg_ripple.embeddings e \
             JOIN _pg_ripple.dictionary d ON d.id = e.entity_id",
            None,
            &[],
        )
        .unwrap_or_else(|e| pgrx::error!("add_embedding_triples: SPI error: {e}"))
        .map(|row| {
            let id: i64 = row.get::<i64>(1).ok().flatten().unwrap_or(0);
            let value: String = row.get::<String>(2).ok().flatten().unwrap_or_default();
            (id, value)
        })
        .collect()
    });

    let mut inserted = 0i64;
    for (_entity_id, iri) in &entity_ids {
        let subject = format!("<{}>", iri.trim_start_matches('<').trim_end_matches('>'));
        crate::storage::insert_triple(
            &subject,
            "<https://pg-ripple.io/ns#hasEmbedding>",
            "\"true\"^^<http://www.w3.org/2001/XMLSchema#boolean>",
            0,
        );
        inserted += 1;
    }

    inserted
}

/// Produce a text representation of an entity's RDF neighborhood for embedding.
///
/// Runs a SPARQL-like query to collect:
///   - `rdfs:label` of the entity
///   - `rdf:type` IRIs
///   - labels of neighboring entities within `depth` hops (up to `max_neighbors`)
///
/// Returns a plain-text string suitable for passing to an embedding API.
///
/// When the entity is not found in the dictionary, returns the IRI local name.
/// Build a SQL fragment that retrieves decoded object strings for a given
/// subject (`s_id`) and predicate (`pred_id`).  Handles both predicates that
/// are still in `vp_rare` and predicates that have been promoted to a
/// dedicated HTAP VP table.
fn vp_objects_sql(s_id: i64, pred_id: i64, limit: i32) -> String {
    let has_dedicated = pgrx::Spi::get_one_with_args::<bool>(
        "SELECT table_oid IS NOT NULL FROM _pg_ripple.predicates WHERE id = $1",
        &[pgrx::datum::DatumWithOid::from(pred_id)],
    )
    .unwrap_or(None)
    .unwrap_or(false);

    if has_dedicated {
        format!(
            "SELECT d.value \
             FROM _pg_ripple.vp_{pred_id} vp \
             JOIN _pg_ripple.dictionary d ON d.id = vp.o \
             WHERE vp.s = {s_id} LIMIT {limit}"
        )
    } else {
        format!(
            "SELECT d.value \
             FROM _pg_ripple.vp_rare vr \
             JOIN _pg_ripple.dictionary d ON d.id = vr.o \
             WHERE vr.s = {s_id} AND vr.p = {pred_id} LIMIT {limit}"
        )
    }
}

pub fn contextualize_entity(entity_iri: &str, depth: i32, max_neighbors: i32) -> String {
    let iri_bare = entity_iri
        .trim_start_matches('<')
        .trim_end_matches('>')
        .to_owned();
    let entity_id = crate::dictionary::encode(&iri_bare, crate::dictionary::KIND_IRI);

    // Collect label.
    let rdfs_label_iri = "http://www.w3.org/2000/01/rdf-schema#label";
    let label_id = crate::dictionary::encode(rdfs_label_iri, crate::dictionary::KIND_IRI);
    let rdf_type_iri = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
    let type_id = crate::dictionary::encode(rdf_type_iri, crate::dictionary::KIND_IRI);

    // Build SQL for label lookup — works regardless of whether rdfs:label is promoted.
    let label_sql = vp_objects_sql(entity_id, label_id, 1);
    let label: String = pgrx::Spi::get_one::<String>(&label_sql)
        .unwrap_or(None)
        .unwrap_or_else(|| extract_local_name(&iri_bare));

    // Build SQL for type lookup — works regardless of whether rdf:type is promoted.
    let type_sql = vp_objects_sql(entity_id, type_id, 10);

    // Collect types.
    let types: Vec<String> = pgrx::Spi::connect(|c| {
        c.select(&type_sql, None, &[])
            .unwrap_or_else(|e| pgrx::error!("contextualize_entity: SPI error: {e}"))
            .map(|row: pgrx::spi::SpiHeapTupleData| {
                let v: String = row.get::<String>(1).ok().flatten().unwrap_or_default();
                extract_local_name(&v)
            })
            .collect()
    });

    // Collect neighbor labels (1-hop via vp_rare within depth=1 by default).
    let effective_depth = depth.clamp(1, 3);
    let limit = max_neighbors.clamp(1, 100);
    let neighbor_labels: Vec<String> = if effective_depth >= 1 {
        let neighbor_iris: Vec<String> = pgrx::Spi::connect(|c| {
            c.select(
                &format!(
                    "SELECT DISTINCT d2.value \
                     FROM _pg_ripple.vp_rare vr \
                     JOIN _pg_ripple.dictionary d2 ON d2.id = vr.o \
                     WHERE vr.s = {entity_id} AND d2.kind = 0 \
                     LIMIT {limit}"
                ),
                None,
                &[],
            )
            .unwrap_or_else(|e| pgrx::error!("contextualize_entity: SPI error: {e}"))
            .map(|row: pgrx::spi::SpiHeapTupleData| {
                row.get::<String>(1).ok().flatten().unwrap_or_default()
            })
            .collect()
        });
        neighbor_iris
            .into_iter()
            .map(|v| {
                // Look up label for this neighbor if available.
                let neighbor_id = crate::dictionary::encode(&v, crate::dictionary::KIND_IRI);
                let nb_label_sql = vp_objects_sql(neighbor_id, label_id, 1);
                pgrx::Spi::get_one::<String>(&nb_label_sql)
                    .unwrap_or(None)
                    .unwrap_or_else(|| extract_local_name(&v))
            })
            .collect()
    } else {
        Vec::new()
    };

    // Format the text representation.
    let mut parts = vec![label.clone()];
    if !types.is_empty() {
        parts.push(format!("Type: {}", types.join(", ")));
    }
    if !neighbor_labels.is_empty() {
        parts.push(format!("Related: {}", neighbor_labels.join(", ")));
    }
    parts.join(". ")
}

/// End-to-end RAG retrieval: encode question, find k nearest entities, collect context.
///
/// Steps:
/// 1. Find `k` nearest entities to `question` via HNSW (falls back to full scan).
/// 2. Apply `sparql_filter` (optional SPARQL WHERE clause fragment) on the candidate set.
/// 3. For each surviving entity, call `contextualize_entity()` to build rich context.
/// 4. Return rows with `entity_iri`, `label`, `context_json`, `distance`.
///
/// `output_format`: `"jsonb"` (default) or `"jsonld"`.  When `"jsonld"`,
/// `context_json` is wrapped with `@type` and `@context` keys.
///
/// When pgvector is absent, returns zero rows with a WARNING.
pub fn rag_retrieve(
    question: &str,
    sparql_filter: Option<&str>,
    k: i32,
    model: Option<&str>,
    output_format: &str,
) -> Vec<(String, String, pgrx::JsonB, f64)> {
    if !pgvector_guard("rag_retrieve") {
        return Vec::new();
    }

    // Step 1: vector search.
    let candidates = similar_entities(question, k * 2, model);

    if candidates.is_empty() {
        return Vec::new();
    }

    // Step 2: optional SPARQL filter.
    let surviving_ids: Vec<i64> = if let Some(filter) = sparql_filter.filter(|s| !s.is_empty()) {
        // Build a SPARQL query that filters the candidate set.
        let candidate_iris: Vec<String> = candidates
            .iter()
            .map(|(_, iri, _)| format!("<{}>", iri.trim_start_matches('<').trim_end_matches('>')))
            .collect();
        let values_clause = candidate_iris.join(" ");
        let sparql =
            format!("SELECT ?entity WHERE {{ VALUES ?entity {{ {values_clause} }} {filter} }}");
        let rows = crate::sparql::sparql(&sparql);
        rows.iter()
            .filter_map(|row| {
                row.0.as_object().and_then(|obj| {
                    obj.values().next().and_then(|v| v.as_str()).map(|s| {
                        let iri = s.trim_start_matches('<').trim_end_matches('>');
                        crate::dictionary::encode(iri, crate::dictionary::KIND_IRI)
                    })
                })
            })
            .collect()
    } else {
        candidates.iter().map(|(id, _, _)| *id).collect()
    };

    // Step 3 & 4: contextualize and build output rows.
    let is_jsonld = output_format.eq_ignore_ascii_case("jsonld");

    candidates
        .iter()
        .filter(|(id, _, _)| surviving_ids.contains(id))
        .take(k as usize)
        .map(|(entity_id, entity_iri, distance)| {
            let iri_bare = entity_iri
                .trim_start_matches('<')
                .trim_end_matches('>')
                .to_owned();

            // Get label.
            let rdfs_label_id = crate::dictionary::encode(
                "http://www.w3.org/2000/01/rdf-schema#label",
                crate::dictionary::KIND_IRI,
            );
            let label: String = pgrx::Spi::get_one_with_args::<String>(
                "SELECT d.value FROM _pg_ripple.vp_rare vr \
                 JOIN _pg_ripple.dictionary d ON d.id = vr.o \
                 WHERE vr.s = $1 AND vr.p = $2 LIMIT 1",
                &[
                    pgrx::datum::DatumWithOid::from(*entity_id),
                    pgrx::datum::DatumWithOid::from(rdfs_label_id),
                ],
            )
            .unwrap_or(None)
            .unwrap_or_else(|| extract_local_name(&iri_bare));

            // Build context JSON.
            let context_text = contextualize_entity(&iri_bare, 1, 20);

            // Collect types.
            let rdf_type_id = crate::dictionary::encode(
                "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
                crate::dictionary::KIND_IRI,
            );
            let types: Vec<serde_json::Value> = pgrx::Spi::connect(|c| {
                c.select(
                    &format!(
                        "SELECT d.value FROM _pg_ripple.vp_rare vr \
                         JOIN _pg_ripple.dictionary d ON d.id = vr.o \
                         WHERE vr.s = {entity_id} AND vr.p = {rdf_type_id} LIMIT 10"
                    ),
                    None,
                    &[],
                )
                .unwrap_or_else(|e| pgrx::error!("rag_retrieve: SPI error: {e}"))
                .map(|row: pgrx::spi::SpiHeapTupleData| {
                    let v: String = row.get::<String>(1).ok().flatten().unwrap_or_default();
                    serde_json::Value::String(v)
                })
                .collect()
            });

            // Collect properties.
            let properties: Vec<serde_json::Value> = pgrx::Spi::connect(|c| {
                c.select(
                    &format!(
                        "SELECT pd.value AS p_iri, od.value AS o_val \
                         FROM _pg_ripple.vp_rare vr \
                         JOIN _pg_ripple.dictionary pd ON pd.id = vr.p \
                         JOIN _pg_ripple.dictionary od ON od.id = vr.o \
                         WHERE vr.s = {entity_id} \
                         LIMIT 20"
                    ),
                    None,
                    &[],
                )
                .unwrap_or_else(|e| pgrx::error!("rag_retrieve: SPI error: {e}"))
                .map(|row: pgrx::spi::SpiHeapTupleData| {
                    let p: String = row.get::<String>(1).ok().flatten().unwrap_or_default();
                    let o: String = row.get::<String>(2).ok().flatten().unwrap_or_default();
                    serde_json::json!({"predicate": p, "object": o})
                })
                .collect()
            });

            // Collect neighbor labels.
            let neighbors: Vec<serde_json::Value> = pgrx::Spi::connect(|c| {
                c.select(
                    &format!(
                        "SELECT DISTINCT od.value \
                         FROM _pg_ripple.vp_rare vr \
                         JOIN _pg_ripple.dictionary od ON od.id = vr.o \
                         WHERE vr.s = {entity_id} AND od.kind = 0 \
                         LIMIT 10"
                    ),
                    None,
                    &[],
                )
                .unwrap_or_else(|e| pgrx::error!("rag_retrieve: SPI error: {e}"))
                .map(|row: pgrx::spi::SpiHeapTupleData| {
                    let v: String = row.get::<String>(1).ok().flatten().unwrap_or_default();
                    serde_json::Value::String(v)
                })
                .collect()
            });

            let context_json: serde_json::Value = if is_jsonld {
                // JSON-LD framing output.
                let prefix_map = build_prefix_map();
                let context_obj: serde_json::Map<String, serde_json::Value> = prefix_map
                    .into_iter()
                    .map(|(k, v)| (k, serde_json::Value::String(v)))
                    .collect();
                serde_json::json!({
                    "@context": context_obj,
                    "@id": format!("<{iri_bare}>"),
                    "@type": types,
                    "rdfs:label": label,
                    "properties": properties,
                    "neighbors": neighbors,
                    "contextText": context_text
                })
            } else {
                serde_json::json!({
                    "label": label,
                    "types": types,
                    "properties": properties,
                    "neighbors": neighbors
                })
            };

            (iri_bare, label, pgrx::JsonB(context_json), *distance)
        })
        .collect()
}

/// Build a minimal prefix map from registered prefixes for JSON-LD @context.
fn build_prefix_map() -> Vec<(String, String)> {
    pgrx::Spi::connect(|c| {
        c.select(
            "SELECT prefix, expansion FROM _pg_ripple.prefixes LIMIT 50",
            None,
            &[],
        )
        .unwrap_or_else(|e| pgrx::error!("build_prefix_map: SPI error: {e}"))
        .map(|row| {
            let prefix: String = row.get::<String>(1).ok().flatten().unwrap_or_default();
            let expansion: String = row.get::<String>(2).ok().flatten().unwrap_or_default();
            (prefix, expansion)
        })
        .collect()
    })
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Extract the local name from an IRI string.
///
/// Returns the part after the last `#` or `/`.  Falls back to the full IRI.
pub(crate) fn extract_local_name(iri: &str) -> String {
    iri.rfind(['#', '/'])
        .map(|pos| &iri[pos + 1..])
        .filter(|s| !s.is_empty())
        .unwrap_or(iri)
        .to_owned()
}
