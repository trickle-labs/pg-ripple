//! JSON → N-Triples + JSON-LD multi-subject ingest helpers
//! (v0.52.0 / JSONLD-INGEST-02, split from bulk_load.rs v0.122.0 H17-02).

use crate::dictionary;

// ─── Private helpers ─────────────────────────────────────────────────────────

/// Escape a string for safe use inside an N-Triples double-quoted literal.
fn escape_nt_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
}

/// Convert a `serde_json::Value` to an N-Triples object term string.
fn json_value_to_nt_term(
    val: &serde_json::Value,
    context: &std::collections::HashMap<String, String>,
    bn_counter: &mut u64,
    extra: &mut String,
) -> Option<String> {
    match val {
        serde_json::Value::String(s) => Some(format!("\"{}\"", escape_nt_literal(s))),
        serde_json::Value::Number(n) => {
            if n.is_f64() {
                n.as_f64()
                    .map(|f| format!("\"{}\"^^<http://www.w3.org/2001/XMLSchema#decimal>", f))
            } else if let Some(i) = n.as_i64() {
                Some(format!(
                    "\"{}\"^^<http://www.w3.org/2001/XMLSchema#integer>",
                    i
                ))
            } else if let Some(u) = n.as_u64() {
                Some(format!(
                    "\"{}\"^^<http://www.w3.org/2001/XMLSchema#integer>",
                    u
                ))
            } else {
                let s = n.to_string();
                if s.contains('.') || s.contains('e') || s.contains('E') {
                    Some(format!(
                        "\"{}\"^^<http://www.w3.org/2001/XMLSchema#decimal>",
                        s
                    ))
                } else {
                    Some(format!(
                        "\"{}\"^^<http://www.w3.org/2001/XMLSchema#integer>",
                        s
                    ))
                }
            }
        }
        serde_json::Value::Bool(b) => Some(format!(
            "\"{}\"^^<http://www.w3.org/2001/XMLSchema#boolean>",
            b
        )),
        serde_json::Value::Null => None,
        serde_json::Value::Object(map) => {
            *bn_counter += 1;
            let bn = format!("_:b{}", bn_counter);
            for (k, v) in map {
                let pred_iri = resolve_key_to_iri(k, context);
                json_object_to_ntriples_inner(&bn, &pred_iri, v, context, bn_counter, extra);
            }
            Some(bn)
        }
        serde_json::Value::Array(_) => None,
    }
}

fn resolve_key_to_iri(key: &str, context: &std::collections::HashMap<String, String>) -> String {
    context.get(key).cloned().unwrap_or_else(|| key.to_owned())
}

fn json_object_to_ntriples_inner(
    subject: &str,
    pred_iri: &str,
    val: &serde_json::Value,
    context: &std::collections::HashMap<String, String>,
    bn_counter: &mut u64,
    out: &mut String,
) {
    let pred_term = format!("<{}>", pred_iri);
    match val {
        serde_json::Value::Array(items) => {
            for item in items {
                json_object_to_ntriples_inner(subject, pred_iri, item, context, bn_counter, out);
            }
        }
        _ => {
            if let Some(obj_term) = json_value_to_nt_term(val, context, bn_counter, out) {
                let s_term = if subject.starts_with("_:") {
                    subject.to_owned()
                } else {
                    format!("<{}>", subject)
                };
                out.push_str(&s_term);
                out.push(' ');
                out.push_str(&pred_term);
                out.push(' ');
                out.push_str(&obj_term);
                out.push_str(" .\n");
            }
        }
    }
}

/// RT-FIX-07: Validate that a JSON key is safe to expand under @vocab.
fn validate_iri_key_or_error(key: &str) {
    let forbidden =
        |c: char| c <= '\x20' || matches!(c, '"' | '<' | '>' | '{' | '}' | '|' | '\\' | '^' | '`');
    if let Some(bad) = key.chars().find(|&c| forbidden(c)) {
        pgrx::error!(
            "cannot derive predicate IRI from JSON key {:?}: \
             character {:?} is not allowed in IRI references — \
             add an explicit context entry, e.g. {:?}: \"ex:{}\"",
            key,
            bad,
            key,
            key.replace(' ', "_")
        );
    }
}

// ─── Public API ──────────────────────────────────────────────────────────────

/// Convert a flat JSON object to N-Triples.
pub fn json_to_ntriples(
    payload: &serde_json::Value,
    subject_iri: &str,
    type_iri: Option<&str>,
    context_map: Option<&serde_json::Value>,
) -> String {
    let map = match payload {
        serde_json::Value::Object(m) => m,
        _ => {
            pgrx::warning!("json_to_ntriples: payload must be a JSON object");
            return String::new();
        }
    };

    let mut ctx: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    if let Some(serde_json::Value::Object(c)) = context_map {
        if let Some(serde_json::Value::String(vocab)) = c.get("@vocab") {
            for (k, _) in map {
                if !k.starts_with('@') && !c.contains_key(k.as_str()) {
                    validate_iri_key_or_error(k);
                    ctx.insert(k.clone(), format!("{}{}", vocab, k));
                }
            }
        }
        for (k, v) in c {
            if k == "@vocab" || k.starts_with('@') {
                continue;
            }
            match v {
                serde_json::Value::String(iri) => {
                    ctx.insert(k.clone(), iri.clone());
                }
                serde_json::Value::Object(meta) => {
                    if let Some(serde_json::Value::String(iri)) = meta.get("@id") {
                        ctx.insert(k.clone(), iri.clone());
                    } else if let Some(serde_json::Value::String(vocab)) = c.get("@vocab") {
                        ctx.insert(k.clone(), format!("{}{}", vocab, k));
                    }
                }
                _ => {}
            }
        }
    }

    let mut out = String::with_capacity(512);
    let mut bn_counter: u64 = 0;

    if let Some(t) = type_iri {
        out.push_str(&format!(
            "<{}> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <{}> .\n",
            subject_iri, t
        ));
    }

    for (key, val) in map {
        if key.starts_with('@') {
            continue;
        }
        let pred_iri = resolve_key_to_iri(key, &ctx);
        json_object_to_ntriples_inner(subject_iri, &pred_iri, val, &ctx, &mut bn_counter, &mut out);
    }

    out
}

/// Convert JSON to N-Triples and load immediately.
pub fn json_to_ntriples_and_load(
    payload: &serde_json::Value,
    subject_iri: &str,
    type_iri: Option<&str>,
    context_map: Option<&serde_json::Value>,
) -> i64 {
    let nt = json_to_ntriples(payload, subject_iri, type_iri, context_map);
    if nt.is_empty() {
        return 0;
    }
    super::load_ntriples(&nt, false)
}

/// Ingest a full JSON-LD document (JSONLD-INGEST-02).
pub fn json_ld_load(document: &serde_json::Value, default_graph: Option<&str>) -> i64 {
    let outer_graph: Option<String> = match document {
        serde_json::Value::Object(obj) => obj
            .get("@id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned()),
        _ => None,
    };

    let nodes: Vec<&serde_json::Value> = match document {
        serde_json::Value::Object(obj) => {
            if let Some(serde_json::Value::Array(graph)) = obj.get("@graph") {
                graph.iter().collect()
            } else {
                vec![document]
            }
        }
        serde_json::Value::Array(arr) => arr.iter().collect(),
        _ => {
            pgrx::error!("json_ld_load: document must be a JSON object or array");
        }
    };

    let mut total = 0i64;

    for node in nodes {
        let obj = match node {
            serde_json::Value::Object(o) => o,
            _ => {
                pgrx::warning!("json_ld_load: skipping non-object node in @graph");
                continue;
            }
        };

        let subject_iri = obj
            .get("@id")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| {
                pgrx::error!(
                    "json_ld_load: top-level node in @graph is missing @id; \
                     all JSON-LD nodes must have an @id when using json_ld_load(). \
                     Provide an @id field or use json_to_ntriples_and_load() with an explicit subject IRI."
                )
            });

        let ctx = node
            .get("@context")
            .or_else(|| document.as_object().and_then(|d| d.get("@context")));

        let graph_id: i64 = {
            let g_iri = outer_graph
                .as_deref()
                .or(default_graph)
                .filter(|s| !s.is_empty());
            match g_iri {
                None => 0,
                Some(g) => dictionary::encode(g, dictionary::KIND_IRI),
            }
        };

        let payload_obj: serde_json::Map<String, serde_json::Value> = obj
            .iter()
            .filter(|(k, _)| !k.starts_with('@'))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        if payload_obj.is_empty() {
            continue;
        }

        let payload = serde_json::Value::Object(payload_obj);
        let nt = json_to_ntriples(&payload, subject_iri, None, ctx);
        if nt.is_empty() {
            continue;
        }

        total += if graph_id == 0 {
            super::load_ntriples(&nt, false)
        } else {
            super::load_ntriples_into_graph(&nt, graph_id)
        };
    }

    total
}
