//! GraphRAG BYOG Parquet export (M15-13, v0.96.0).
//! Moved from export/mod.rs lines 557-1495.

use std::collections::BTreeMap;

use super::nt_term_to_jsonld_value;

// ─── GraphRAG BYOG Parquet export (v0.26.0) ──────────────────────────────────

/// Strip N-Triples formatting from a SPARQL result value.
///
/// - `<https://example.org/foo>` → `https://example.org/foo`
/// - `"Alice"` → `Alice`
/// - `"Alice"^^<xsd:string>` → `Alice`
/// - `"42"^^<xsd:integer>` → `42`
/// - `_:b0` → `_:b0` (blank nodes kept as-is)
fn strip_nt(s: &str) -> String {
    let s = s.trim();
    if s.starts_with('<') && s.ends_with('>') {
        s[1..s.len() - 1].to_owned()
    } else if let Some(inner) = s.strip_prefix('"') {
        // Find closing quote (simple scan, not full N-Triples parser)
        let inner_end = inner.find('"').unwrap_or(inner.len());
        inner[..inner_end].to_owned()
    } else {
        s.to_owned()
    }
}

/// Parse an optional integer from a SPARQL N-Triples literal like `"42"^^<xsd:integer>`.
fn parse_nt_integer(s: &str) -> Option<i64> {
    strip_nt(s).parse::<i64>().ok()
}

/// Parse an optional float from a SPARQL N-Triples literal like `"0.95"^^<xsd:float>`.
fn parse_nt_float(s: &str) -> Option<f64> {
    strip_nt(s).parse::<f64>().ok()
}

/// Extract a string value from a SPARQL JsonB result row.
fn get_str(obj: &serde_json::Map<String, serde_json::Value>, key: &str) -> Option<String> {
    obj.get(key)
        .and_then(|v| v.as_str())
        .map(strip_nt)
        .filter(|s| !s.is_empty())
}

/// Build the GRAPH clause for a SPARQL query.
/// Returns (open_clause, close_clause) — both empty when graph_iri is empty/NULL.
fn graph_clause(graph_iri: &str) -> (String, &'static str) {
    let clean = graph_iri.trim().trim_matches(|c| c == '<' || c == '>');
    if clean.is_empty() {
        (String::new(), "")
    } else {
        (format!("GRAPH <{clean}> {{ "), " }}")
    }
}

/// Security check: path must not contain `..` and the parent directory must exist.
fn validate_output_path(path: &str) -> Result<(), String> {
    if path.contains("..") {
        return Err("output path must not contain '..'".to_owned());
    }
    if path.is_empty() {
        return Err("output path must not be empty".to_owned());
    }
    Ok(())
}

/// Export all `gr:Entity` nodes from a named graph to a Parquet file.
///
/// Columns: `id`, `title`, `type`, `description`, `text_unit_ids`, `frequency`, `degree`.
/// The `text_unit_ids` column contains a JSON-encoded array of text unit IRI strings.
///
/// Requires superuser.  Returns the number of entity rows written.
pub fn export_graphrag_entities(graph_iri: &str, output_path: &str) -> i64 {
    // SAFETY: superuser() is a PostgreSQL function with no thread-safety concerns.
    if !unsafe { pgrx::pg_sys::superuser() } {
        pgrx::error!("export_graphrag_entities: requires superuser");
    }
    if let Err(e) = validate_output_path(output_path) {
        pgrx::error!("export_graphrag_entities: {e}");
    }

    let (graph_open, graph_close) = graph_clause(graph_iri);
    let sparql = format!(
        "SELECT ?entity ?title ?entityType ?description ?frequency ?degree \
         WHERE {{ \
           {graph_open} \
           ?entity <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://graphrag.org/ns/Entity> . \
           OPTIONAL {{ ?entity <https://graphrag.org/ns/title> ?title }} \
           OPTIONAL {{ ?entity <https://graphrag.org/ns/type> ?entityType }} \
           OPTIONAL {{ ?entity <https://graphrag.org/ns/description> ?description }} \
           OPTIONAL {{ ?entity <https://graphrag.org/ns/frequency> ?frequency }} \
           OPTIONAL {{ ?entity <https://graphrag.org/ns/degree> ?degree }} \
           {graph_close} \
         }}"
    );

    let results = crate::sparql::sparql(&sparql);

    // Collect column arrays
    let mut ids: Vec<String> = Vec::new();
    let mut titles: Vec<String> = Vec::new();
    let mut entity_types: Vec<String> = Vec::new();
    let mut descriptions: Vec<String> = Vec::new();
    let mut text_unit_ids: Vec<String> = Vec::new();
    let mut frequencies: Vec<i64> = Vec::new();
    let mut degrees: Vec<i64> = Vec::new();

    for row in &results {
        let obj = match row.0.as_object() {
            Some(o) => o,
            None => continue,
        };
        let id = get_str(obj, "entity").unwrap_or_default();
        if id.is_empty() {
            continue;
        }
        ids.push(id);
        titles.push(get_str(obj, "title").unwrap_or_default());
        entity_types.push(get_str(obj, "entityType").unwrap_or_default());
        descriptions.push(get_str(obj, "description").unwrap_or_default());
        text_unit_ids.push("[]".to_owned()); // populated by text-unit linkage query if needed
        frequencies.push(
            obj.get("frequency")
                .and_then(|v| v.as_str())
                .and_then(parse_nt_integer)
                .unwrap_or(0),
        );
        degrees.push(
            obj.get("degree")
                .and_then(|v| v.as_str())
                .and_then(parse_nt_integer)
                .unwrap_or(0),
        );
    }

    let row_count = ids.len() as i64;
    write_entities_parquet(
        output_path,
        ids,
        titles,
        entity_types,
        descriptions,
        text_unit_ids,
        frequencies,
        degrees,
    );
    row_count
}

/// Export all `gr:Relationship` nodes from a named graph to a Parquet file.
///
/// Columns: `id`, `source`, `target`, `description`, `weight`, `combined_degree`, `text_unit_ids`.
///
/// Requires superuser.  Returns the number of relationship rows written.
pub fn export_graphrag_relationships(graph_iri: &str, output_path: &str) -> i64 {
    // SAFETY: superuser() is a standard PostgreSQL function.
    if !unsafe { pgrx::pg_sys::superuser() } {
        pgrx::error!("export_graphrag_relationships: requires superuser");
    }
    if let Err(e) = validate_output_path(output_path) {
        pgrx::error!("export_graphrag_relationships: {e}");
    }

    let (graph_open, graph_close) = graph_clause(graph_iri);
    let sparql = format!(
        "SELECT ?rel ?source ?target ?description ?weight \
         WHERE {{ \
           {graph_open} \
           ?rel <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://graphrag.org/ns/Relationship> . \
           OPTIONAL {{ ?rel <https://graphrag.org/ns/source> ?source }} \
           OPTIONAL {{ ?rel <https://graphrag.org/ns/target> ?target }} \
           OPTIONAL {{ ?rel <https://graphrag.org/ns/description> ?description }} \
           OPTIONAL {{ ?rel <https://graphrag.org/ns/weight> ?weight }} \
           {graph_close} \
         }}"
    );

    let results = crate::sparql::sparql(&sparql);

    let mut ids: Vec<String> = Vec::new();
    let mut sources: Vec<String> = Vec::new();
    let mut targets: Vec<String> = Vec::new();
    let mut descriptions: Vec<String> = Vec::new();
    let mut weights: Vec<f64> = Vec::new();
    let mut combined_degrees: Vec<i64> = Vec::new();
    let mut text_unit_ids: Vec<String> = Vec::new();

    for row in &results {
        let obj = match row.0.as_object() {
            Some(o) => o,
            None => continue,
        };
        let id = get_str(obj, "rel").unwrap_or_default();
        if id.is_empty() {
            continue;
        }
        ids.push(id);
        sources.push(get_str(obj, "source").unwrap_or_default());
        targets.push(get_str(obj, "target").unwrap_or_default());
        descriptions.push(get_str(obj, "description").unwrap_or_default());
        weights.push(
            obj.get("weight")
                .and_then(|v| v.as_str())
                .and_then(parse_nt_float)
                .unwrap_or(0.0),
        );
        combined_degrees.push(0); // populated by a follow-up join query if needed
        text_unit_ids.push("[]".to_owned());
    }

    let row_count = ids.len() as i64;
    write_relationships_parquet(
        output_path,
        ids,
        sources,
        targets,
        descriptions,
        weights,
        combined_degrees,
        text_unit_ids,
    );
    row_count
}

/// Export all `gr:TextUnit` nodes from a named graph to a Parquet file.
///
/// Columns: `id`, `text`, `n_tokens`, `document_id`, `entity_ids`, `relationship_ids`.
///
/// Requires superuser.  Returns the number of text unit rows written.
pub fn export_graphrag_text_units(graph_iri: &str, output_path: &str) -> i64 {
    // SAFETY: superuser() is a standard PostgreSQL function.
    if !unsafe { pgrx::pg_sys::superuser() } {
        pgrx::error!("export_graphrag_text_units: requires superuser");
    }
    if let Err(e) = validate_output_path(output_path) {
        pgrx::error!("export_graphrag_text_units: {e}");
    }

    let (graph_open, graph_close) = graph_clause(graph_iri);
    let sparql = format!(
        "SELECT ?tu ?text ?tokenCount ?documentId \
         WHERE {{ \
           {graph_open} \
           ?tu <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://graphrag.org/ns/TextUnit> . \
           OPTIONAL {{ ?tu <https://graphrag.org/ns/text> ?text }} \
           OPTIONAL {{ ?tu <https://graphrag.org/ns/tokenCount> ?tokenCount }} \
           OPTIONAL {{ ?tu <https://graphrag.org/ns/documentId> ?documentId }} \
           {graph_close} \
         }}"
    );

    let results = crate::sparql::sparql(&sparql);

    let mut ids: Vec<String> = Vec::new();
    let mut texts: Vec<String> = Vec::new();
    let mut n_tokens: Vec<i64> = Vec::new();
    let mut document_ids: Vec<String> = Vec::new();
    let mut entity_ids: Vec<String> = Vec::new();
    let mut relationship_ids: Vec<String> = Vec::new();

    for row in &results {
        let obj = match row.0.as_object() {
            Some(o) => o,
            None => continue,
        };
        let id = get_str(obj, "tu").unwrap_or_default();
        if id.is_empty() {
            continue;
        }
        ids.push(id);
        texts.push(get_str(obj, "text").unwrap_or_default());
        n_tokens.push(
            obj.get("tokenCount")
                .and_then(|v| v.as_str())
                .and_then(parse_nt_integer)
                .unwrap_or(0),
        );
        document_ids.push(get_str(obj, "documentId").unwrap_or_default());
        entity_ids.push("[]".to_owned());
        relationship_ids.push("[]".to_owned());
    }

    let row_count = ids.len() as i64;
    write_text_units_parquet(
        output_path,
        ids,
        texts,
        n_tokens,
        document_ids,
        entity_ids,
        relationship_ids,
    );
    row_count
}

// ─── Parquet writers ──────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn write_entities_parquet(
    path: &str,
    ids: Vec<String>,
    titles: Vec<String>,
    entity_types: Vec<String>,
    descriptions: Vec<String>,
    text_unit_ids: Vec<String>,
    frequencies: Vec<i64>,
    degrees: Vec<i64>,
) {
    use parquet::column::writer::ColumnWriter;
    use parquet::data_type::ByteArray;
    use parquet::file::properties::WriterProperties;
    use parquet::file::writer::SerializedFileWriter;
    use parquet::schema::parser::parse_message_type;
    use std::fs::File;
    use std::sync::Arc;

    let schema_str = "message schema {
        REQUIRED BYTE_ARRAY id (UTF8);
        OPTIONAL BYTE_ARRAY title (UTF8);
        OPTIONAL BYTE_ARRAY type (UTF8);
        OPTIONAL BYTE_ARRAY description (UTF8);
        OPTIONAL BYTE_ARRAY text_unit_ids (UTF8);
        OPTIONAL INT64 frequency;
        OPTIONAL INT64 degree;
    }";

    let schema = Arc::new(
        parse_message_type(schema_str)
            .unwrap_or_else(|e| pgrx::error!("entities parquet schema error: {e}")),
    );
    let props = Arc::new(WriterProperties::builder().build());
    let file = File::create(path).unwrap_or_else(|e| {
        pgrx::error!("export_graphrag_entities: cannot create file '{path}': {e}")
    });
    let mut writer = SerializedFileWriter::new(file, schema, props)
        .unwrap_or_else(|e| pgrx::error!("export_graphrag_entities: writer init error: {e}"));

    if !ids.is_empty() {
        let mut rg = writer
            .next_row_group()
            .unwrap_or_else(|e| pgrx::error!("entities row group error: {e}"));

        // Helper: convert Vec<String> to Vec<ByteArray>
        let to_bytes = |v: &[String]| -> Vec<ByteArray> {
            v.iter().map(|s| s.as_bytes().to_vec().into()).collect()
        };

        // id column
        {
            let mut cw = rg
                .next_column()
                .unwrap_or_else(|e| pgrx::error!("entities id col error: {e}"))
                .unwrap_or_else(|| pgrx::error!("expected id column in parquet schema"));
            {
                if let ColumnWriter::ByteArrayColumnWriter(w) = cw.untyped() {
                    w.write_batch(&to_bytes(&ids), None, None)
                        .unwrap_or_else(|e| pgrx::error!("entities id write: {e}"));
                }
            }
            cw.close()
                .unwrap_or_else(|e| pgrx::error!("entities id close: {e}"));
        }
        // title column
        {
            let mut cw = rg
                .next_column()
                .unwrap_or_else(|e| pgrx::error!("entities title col error: {e}"))
                .unwrap_or_else(|| pgrx::error!("expected title column in parquet schema"));
            {
                if let ColumnWriter::ByteArrayColumnWriter(w) = cw.untyped() {
                    w.write_batch(&to_bytes(&titles), None, None)
                        .unwrap_or_else(|e| pgrx::error!("entities title write: {e}"));
                }
            }
            cw.close()
                .unwrap_or_else(|e| pgrx::error!("entities title close: {e}"));
        }
        // type column
        {
            let mut cw = rg
                .next_column()
                .unwrap_or_else(|e| pgrx::error!("entities type col error: {e}"))
                .unwrap_or_else(|| pgrx::error!("expected type column in parquet schema"));
            {
                if let ColumnWriter::ByteArrayColumnWriter(w) = cw.untyped() {
                    w.write_batch(&to_bytes(&entity_types), None, None)
                        .unwrap_or_else(|e| pgrx::error!("entities type write: {e}"));
                }
            }
            cw.close()
                .unwrap_or_else(|e| pgrx::error!("entities type close: {e}"));
        }
        // description column
        {
            let mut cw = rg
                .next_column()
                .unwrap_or_else(|e| pgrx::error!("entities description col error: {e}"))
                .unwrap_or_else(|| pgrx::error!("expected description column in parquet schema"));
            {
                if let ColumnWriter::ByteArrayColumnWriter(w) = cw.untyped() {
                    w.write_batch(&to_bytes(&descriptions), None, None)
                        .unwrap_or_else(|e| pgrx::error!("entities description write: {e}"));
                }
            }
            cw.close()
                .unwrap_or_else(|e| pgrx::error!("entities description close: {e}"));
        }
        // text_unit_ids column
        {
            let mut cw = rg
                .next_column()
                .unwrap_or_else(|e| pgrx::error!("entities text_unit_ids col error: {e}"))
                .unwrap_or_else(|| pgrx::error!("expected text_unit_ids column in parquet schema"));
            {
                if let ColumnWriter::ByteArrayColumnWriter(w) = cw.untyped() {
                    w.write_batch(&to_bytes(&text_unit_ids), None, None)
                        .unwrap_or_else(|e| pgrx::error!("entities text_unit_ids write: {e}"));
                }
            }
            cw.close()
                .unwrap_or_else(|e| pgrx::error!("entities text_unit_ids close: {e}"));
        }
        // frequency column
        {
            let mut cw = rg
                .next_column()
                .unwrap_or_else(|e| pgrx::error!("entities frequency col error: {e}"))
                .unwrap_or_else(|| pgrx::error!("expected frequency column in parquet schema"));
            {
                if let ColumnWriter::Int64ColumnWriter(w) = cw.untyped() {
                    w.write_batch(&frequencies, None, None)
                        .unwrap_or_else(|e| pgrx::error!("entities frequency write: {e}"));
                }
            }
            cw.close()
                .unwrap_or_else(|e| pgrx::error!("entities frequency close: {e}"));
        }
        // degree column
        {
            let mut cw = rg
                .next_column()
                .unwrap_or_else(|e| pgrx::error!("entities degree col error: {e}"))
                .unwrap_or_else(|| pgrx::error!("expected degree column in parquet schema"));
            {
                if let ColumnWriter::Int64ColumnWriter(w) = cw.untyped() {
                    w.write_batch(&degrees, None, None)
                        .unwrap_or_else(|e| pgrx::error!("entities degree write: {e}"));
                }
            }
            cw.close()
                .unwrap_or_else(|e| pgrx::error!("entities degree close: {e}"));
        }

        rg.close()
            .unwrap_or_else(|e| pgrx::error!("entities row group close: {e}"));
    }

    writer
        .close()
        .unwrap_or_else(|e| pgrx::error!("export_graphrag_entities: writer close: {e}"));
}

#[allow(clippy::too_many_arguments)]
fn write_relationships_parquet(
    path: &str,
    ids: Vec<String>,
    sources: Vec<String>,
    targets: Vec<String>,
    descriptions: Vec<String>,
    weights: Vec<f64>,
    combined_degrees: Vec<i64>,
    text_unit_ids: Vec<String>,
) {
    use parquet::column::writer::ColumnWriter;
    use parquet::data_type::ByteArray;
    use parquet::file::properties::WriterProperties;
    use parquet::file::writer::SerializedFileWriter;
    use parquet::schema::parser::parse_message_type;
    use std::fs::File;
    use std::sync::Arc;

    let schema_str = "message schema {
        REQUIRED BYTE_ARRAY id (UTF8);
        OPTIONAL BYTE_ARRAY source (UTF8);
        OPTIONAL BYTE_ARRAY target (UTF8);
        OPTIONAL BYTE_ARRAY description (UTF8);
        OPTIONAL DOUBLE weight;
        OPTIONAL INT64 combined_degree;
        OPTIONAL BYTE_ARRAY text_unit_ids (UTF8);
    }";

    let schema = Arc::new(
        parse_message_type(schema_str)
            .unwrap_or_else(|e| pgrx::error!("relationships parquet schema error: {e}")),
    );
    let props = Arc::new(WriterProperties::builder().build());
    let file = File::create(path).unwrap_or_else(|e| {
        pgrx::error!("export_graphrag_relationships: cannot create file '{path}': {e}")
    });
    let mut writer = SerializedFileWriter::new(file, schema, props)
        .unwrap_or_else(|e| pgrx::error!("export_graphrag_relationships: writer init error: {e}"));

    if !ids.is_empty() {
        let mut rg = writer
            .next_row_group()
            .unwrap_or_else(|e| pgrx::error!("relationships row group error: {e}"));

        let to_bytes = |v: &[String]| -> Vec<ByteArray> {
            v.iter().map(|s| s.as_bytes().to_vec().into()).collect()
        };

        // id column
        {
            let mut cw = rg
                .next_column()
                .unwrap_or_else(|e| pgrx::error!("relationships id col: {e}"))
                .unwrap_or_else(|| pgrx::error!("expected id column in parquet schema"));
            {
                if let ColumnWriter::ByteArrayColumnWriter(w) = cw.untyped() {
                    w.write_batch(&to_bytes(&ids), None, None)
                        .unwrap_or_else(|e| pgrx::error!("rel id write: {e}"));
                }
            }
            cw.close()
                .unwrap_or_else(|e| pgrx::error!("rel id close: {e}"));
        }
        // source column
        {
            let mut cw = rg
                .next_column()
                .unwrap_or_else(|e| pgrx::error!("relationships source col: {e}"))
                .unwrap_or_else(|| pgrx::error!("expected source column in parquet schema"));
            {
                if let ColumnWriter::ByteArrayColumnWriter(w) = cw.untyped() {
                    w.write_batch(&to_bytes(&sources), None, None)
                        .unwrap_or_else(|e| pgrx::error!("rel source write: {e}"));
                }
            }
            cw.close()
                .unwrap_or_else(|e| pgrx::error!("rel source close: {e}"));
        }
        // target column
        {
            let mut cw = rg
                .next_column()
                .unwrap_or_else(|e| pgrx::error!("relationships target col: {e}"))
                .unwrap_or_else(|| pgrx::error!("expected target column in parquet schema"));
            {
                if let ColumnWriter::ByteArrayColumnWriter(w) = cw.untyped() {
                    w.write_batch(&to_bytes(&targets), None, None)
                        .unwrap_or_else(|e| pgrx::error!("rel target write: {e}"));
                }
            }
            cw.close()
                .unwrap_or_else(|e| pgrx::error!("rel target close: {e}"));
        }
        // description column
        {
            let mut cw = rg
                .next_column()
                .unwrap_or_else(|e| pgrx::error!("relationships description col: {e}"))
                .unwrap_or_else(|| pgrx::error!("expected description column in parquet schema"));
            {
                if let ColumnWriter::ByteArrayColumnWriter(w) = cw.untyped() {
                    w.write_batch(&to_bytes(&descriptions), None, None)
                        .unwrap_or_else(|e| pgrx::error!("rel description write: {e}"));
                }
            }
            cw.close()
                .unwrap_or_else(|e| pgrx::error!("rel description close: {e}"));
        }
        // weight column (DOUBLE)
        {
            let mut cw = rg
                .next_column()
                .unwrap_or_else(|e| pgrx::error!("relationships weight col: {e}"))
                .unwrap_or_else(|| pgrx::error!("expected weight column in parquet schema"));
            {
                if let ColumnWriter::DoubleColumnWriter(w) = cw.untyped() {
                    w.write_batch(&weights, None, None)
                        .unwrap_or_else(|e| pgrx::error!("rel weight write: {e}"));
                }
            }
            cw.close()
                .unwrap_or_else(|e| pgrx::error!("rel weight close: {e}"));
        }
        // combined_degree column
        {
            let mut cw = rg
                .next_column()
                .unwrap_or_else(|e| pgrx::error!("relationships combined_degree col: {e}"))
                .unwrap_or_else(|| {
                    pgrx::error!("expected combined_degree column in parquet schema")
                });
            {
                if let ColumnWriter::Int64ColumnWriter(w) = cw.untyped() {
                    w.write_batch(&combined_degrees, None, None)
                        .unwrap_or_else(|e| pgrx::error!("rel combined_degree write: {e}"));
                }
            }
            cw.close()
                .unwrap_or_else(|e| pgrx::error!("rel combined_degree close: {e}"));
        }
        // text_unit_ids column
        {
            let mut cw = rg
                .next_column()
                .unwrap_or_else(|e| pgrx::error!("relationships text_unit_ids col: {e}"))
                .unwrap_or_else(|| pgrx::error!("expected text_unit_ids column in parquet schema"));
            {
                if let ColumnWriter::ByteArrayColumnWriter(w) = cw.untyped() {
                    w.write_batch(&to_bytes(&text_unit_ids), None, None)
                        .unwrap_or_else(|e| pgrx::error!("rel text_unit_ids write: {e}"));
                }
            }
            cw.close()
                .unwrap_or_else(|e| pgrx::error!("rel text_unit_ids close: {e}"));
        }

        rg.close()
            .unwrap_or_else(|e| pgrx::error!("relationships row group close: {e}"));
    }

    writer
        .close()
        .unwrap_or_else(|e| pgrx::error!("export_graphrag_relationships: writer close: {e}"));
}

#[allow(clippy::too_many_arguments)]
fn write_text_units_parquet(
    path: &str,
    ids: Vec<String>,
    texts: Vec<String>,
    n_tokens: Vec<i64>,
    document_ids: Vec<String>,
    entity_ids: Vec<String>,
    relationship_ids: Vec<String>,
) {
    use parquet::column::writer::ColumnWriter;
    use parquet::data_type::ByteArray;
    use parquet::file::properties::WriterProperties;
    use parquet::file::writer::SerializedFileWriter;
    use parquet::schema::parser::parse_message_type;
    use std::fs::File;
    use std::sync::Arc;

    let schema_str = "message schema {
        REQUIRED BYTE_ARRAY id (UTF8);
        OPTIONAL BYTE_ARRAY text (UTF8);
        OPTIONAL INT64 n_tokens;
        OPTIONAL BYTE_ARRAY document_id (UTF8);
        OPTIONAL BYTE_ARRAY entity_ids (UTF8);
        OPTIONAL BYTE_ARRAY relationship_ids (UTF8);
    }";

    let schema = Arc::new(
        parse_message_type(schema_str)
            .unwrap_or_else(|e| pgrx::error!("text_units parquet schema error: {e}")),
    );
    let props = Arc::new(WriterProperties::builder().build());
    let file = File::create(path).unwrap_or_else(|e| {
        pgrx::error!("export_graphrag_text_units: cannot create file '{path}': {e}")
    });
    let mut writer = SerializedFileWriter::new(file, schema, props)
        .unwrap_or_else(|e| pgrx::error!("export_graphrag_text_units: writer init error: {e}"));

    if !ids.is_empty() {
        let mut rg = writer
            .next_row_group()
            .unwrap_or_else(|e| pgrx::error!("text_units row group error: {e}"));

        let to_bytes = |v: &[String]| -> Vec<ByteArray> {
            v.iter().map(|s| s.as_bytes().to_vec().into()).collect()
        };

        // id column
        {
            let mut cw = rg
                .next_column()
                .unwrap_or_else(|e| pgrx::error!("text_units id col: {e}"))
                .unwrap_or_else(|| pgrx::error!("expected id column in parquet schema"));
            {
                if let ColumnWriter::ByteArrayColumnWriter(w) = cw.untyped() {
                    w.write_batch(&to_bytes(&ids), None, None)
                        .unwrap_or_else(|e| pgrx::error!("tu id write: {e}"));
                }
            }
            cw.close()
                .unwrap_or_else(|e| pgrx::error!("tu id close: {e}"));
        }
        // text column
        {
            let mut cw = rg
                .next_column()
                .unwrap_or_else(|e| pgrx::error!("text_units text col: {e}"))
                .unwrap_or_else(|| pgrx::error!("expected text column in parquet schema"));
            {
                if let ColumnWriter::ByteArrayColumnWriter(w) = cw.untyped() {
                    w.write_batch(&to_bytes(&texts), None, None)
                        .unwrap_or_else(|e| pgrx::error!("tu text write: {e}"));
                }
            }
            cw.close()
                .unwrap_or_else(|e| pgrx::error!("tu text close: {e}"));
        }
        // n_tokens column
        {
            let mut cw = rg
                .next_column()
                .unwrap_or_else(|e| pgrx::error!("text_units n_tokens col: {e}"))
                .unwrap_or_else(|| pgrx::error!("expected n_tokens column in parquet schema"));
            {
                if let ColumnWriter::Int64ColumnWriter(w) = cw.untyped() {
                    w.write_batch(&n_tokens, None, None)
                        .unwrap_or_else(|e| pgrx::error!("tu n_tokens write: {e}"));
                }
            }
            cw.close()
                .unwrap_or_else(|e| pgrx::error!("tu n_tokens close: {e}"));
        }
        // document_id column
        {
            let mut cw = rg
                .next_column()
                .unwrap_or_else(|e| pgrx::error!("text_units document_id col: {e}"))
                .unwrap_or_else(|| pgrx::error!("expected document_id column in parquet schema"));
            {
                if let ColumnWriter::ByteArrayColumnWriter(w) = cw.untyped() {
                    w.write_batch(&to_bytes(&document_ids), None, None)
                        .unwrap_or_else(|e| pgrx::error!("tu document_id write: {e}"));
                }
            }
            cw.close()
                .unwrap_or_else(|e| pgrx::error!("tu document_id close: {e}"));
        }
        // entity_ids column
        {
            let mut cw = rg
                .next_column()
                .unwrap_or_else(|e| pgrx::error!("text_units entity_ids col: {e}"))
                .unwrap_or_else(|| pgrx::error!("expected entity_ids column in parquet schema"));
            {
                if let ColumnWriter::ByteArrayColumnWriter(w) = cw.untyped() {
                    w.write_batch(&to_bytes(&entity_ids), None, None)
                        .unwrap_or_else(|e| pgrx::error!("tu entity_ids write: {e}"));
                }
            }
            cw.close()
                .unwrap_or_else(|e| pgrx::error!("tu entity_ids close: {e}"));
        }
        // relationship_ids column
        {
            let mut cw = rg
                .next_column()
                .unwrap_or_else(|e| pgrx::error!("text_units relationship_ids col: {e}"))
                .unwrap_or_else(|| {
                    pgrx::error!("expected relationship_ids column in parquet schema")
                });
            {
                if let ColumnWriter::ByteArrayColumnWriter(w) = cw.untyped() {
                    w.write_batch(&to_bytes(&relationship_ids), None, None)
                        .unwrap_or_else(|e| pgrx::error!("tu relationship_ids write: {e}"));
                }
            }
            cw.close()
                .unwrap_or_else(|e| pgrx::error!("tu relationship_ids close: {e}"));
        }

        rg.close()
            .unwrap_or_else(|e| pgrx::error!("text_units row group close: {e}"));
    }

    writer
        .close()
        .unwrap_or_else(|e| pgrx::error!("export_graphrag_text_units: writer close: {e}"));
}

// ─── Single-triple and star-pattern JSON-LD serializers (v0.52.0) ─────────────

/// Decode a dictionary ID to a human-readable string, falling back to `_:{id}`
/// for unknown entries (e.g. blank nodes that were never stored in the dictionary).
fn decode_id_to_str(id: i64) -> String {
    crate::dictionary::decode(id).unwrap_or_else(|| format!("_:{}", id))
}

/// Convert a single triple `(s, p, o)` from dictionary IDs to a JSON-LD object.
///
/// Returns a JSON-LD node with an inline `@context` block.  The predicate IRI
/// is used as the property key; the object is represented as a JSON-LD value
/// object (`{"@id": "..."}` for IRIs, `{"@value": "..."}` for literals).
///
/// The function uses the backend-local LRU dictionary cache, so repeated calls
/// for common IRIs (class names, property names) incur no SPI round-trips.
pub fn triple_to_jsonld(s: i64, p: i64, o: i64) -> serde_json::Value {
    let s_str = decode_id_to_str(s);
    let p_str = decode_id_to_str(p);
    let o_str = decode_id_to_str(o);

    let s_id_val = if s_str.starts_with('<') && s_str.ends_with('>') {
        serde_json::json!(s_str[1..s_str.len() - 1])
    } else {
        serde_json::json!(s_str)
    };

    let p_key = if p_str.starts_with('<') && p_str.ends_with('>') {
        p_str[1..p_str.len() - 1].to_owned()
    } else {
        p_str.clone()
    };

    let o_val = nt_term_to_jsonld_value(&o_str);

    serde_json::json!({
        "@id": s_id_val,
        p_key: [o_val]
    })
}

/// Collect all triples for a given subject into a single JSON-LD document.
///
/// Uses a star-pattern query over all VP tables to retrieve every triple where
/// `s = subject`.  Predicates are grouped into a single JSON-LD node, making
/// this more efficient than calling `triple_to_jsonld` once per predicate for
/// an entity burst.
pub fn triples_to_jsonld_by_subject(subject: i64) -> serde_json::Value {
    // Collect all (p, o) pairs for the subject across all VP tables.
    let rows = crate::storage::triples_for_subject(subject);

    if rows.is_empty() {
        return serde_json::json!({"@id": decode_id_to_str(subject)});
    }

    let s_str = decode_id_to_str(subject);
    let s_id_val = if s_str.starts_with('<') && s_str.ends_with('>') {
        serde_json::json!(s_str[1..s_str.len() - 1])
    } else {
        serde_json::json!(s_str)
    };

    let mut node = serde_json::Map::new();
    node.insert("@id".to_owned(), s_id_val);

    // Group by predicate
    let mut by_pred: BTreeMap<String, Vec<serde_json::Value>> = BTreeMap::new();
    for (p, o) in rows {
        let p_str = decode_id_to_str(p);
        let p_key = if p_str.starts_with('<') && p_str.ends_with('>') {
            p_str[1..p_str.len() - 1].to_owned()
        } else {
            p_str
        };
        let o_str = decode_id_to_str(o);
        let o_val = nt_term_to_jsonld_value(&o_str);
        by_pred.entry(p_key).or_default().push(o_val);
    }

    for (k, vals) in by_pred {
        node.insert(k, serde_json::Value::Array(vals));
    }

    serde_json::Value::Object(node)
}

// ─── v0.72.0: export_jsonld_node() implementation (JSONLD-NODE-01) ────────────

/// Recursively strip listed keys from every JSON object in `val`.
fn strip_keys_recursive(val: &mut serde_json::Value, strip: &[String]) {
    match val {
        serde_json::Value::Object(map) => {
            for key in strip {
                map.remove(key.as_str());
            }
            for v in map.values_mut() {
                strip_keys_recursive(v, strip);
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr.iter_mut() {
                strip_keys_recursive(item, strip);
            }
        }
        _ => {}
    }
}

/// Core implementation for `pg_ripple.export_jsonld_node()`.
///
/// Returns:
/// - `Ok(Some(Value))` — node found and serialised.
/// - `Ok(None)`        — no triples match the subject; SQL NULL.
/// - `Err(String)`     — invalid arguments or internal error.
pub fn export_jsonld_node_impl(
    mut frame: serde_json::Value,
    subject_id: i64,
    strip: Vec<String>,
) -> Result<Option<serde_json::Value>, String> {
    // Guard: frame must not already contain @id; we inject it.
    if let serde_json::Value::Object(ref obj) = frame
        && obj.contains_key("@id")
    {
        return Err("export_jsonld_node: frame must not contain '@id'; \
             subject_id provides the subject IRI"
            .to_owned());
    }

    // Look up the IRI for subject_id.
    let iri = crate::dictionary::decode(subject_id).ok_or_else(|| {
        format!("export_jsonld_node: subject_id {subject_id} not found in dictionary")
    })?;

    // Inject @id into the frame.
    if let serde_json::Value::Object(ref mut obj) = frame {
        obj.insert("@id".to_owned(), serde_json::Value::String(iri));
    }

    // Execute framing — graph = None (all graphs).
    let result = crate::framing::frame_and_execute(&frame, None, "@once", false, false)
        .map_err(|e| format!("export_jsonld_node framing error: {e}"))?;

    // Extract @graph[0].
    let node_opt = match &result {
        serde_json::Value::Object(obj) => obj.get("@graph").and_then(|g| {
            if let serde_json::Value::Array(arr) = g {
                arr.first().cloned()
            } else {
                None
            }
        }),
        _ => None,
    };

    let mut node = match node_opt {
        None => return Ok(None),
        Some(n) => n,
    };

    // Recursively strip requested keys from the node tree.
    if !strip.is_empty() {
        strip_keys_recursive(&mut node, &strip);
    }

    Ok(Some(node))
}
