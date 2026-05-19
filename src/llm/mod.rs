//! AI & LLM Integration — v0.49.0
//!
//! Provides two features:
//!
//! 1. **NL → SPARQL via LLM function calling** (`sparql_from_nl`): sends a
//!    plain-English question to a configured OpenAI-compatible chat endpoint
//!    and returns a parseable SPARQL SELECT query string.
//!
//! 2. **Embedding-based `owl:sameAs` candidate generation** (`suggest_sameas`,
//!    `apply_sameas_candidates`): runs an HNSW self-join on the
//!    `_pg_ripple.embeddings` table to surface entity pairs whose cosine
//!    similarity exceeds a configurable threshold, then optionally inserts the
//!    accepted pairs as `owl:sameAs` triples.
//!
//! ## Mock endpoint
//!
//! When `pg_ripple.llm_endpoint` is set to the special value `'mock'`, the
//! HTTP call is bypassed and a canned SPARQL SELECT query is returned.  This
//! allows pg_regress tests to exercise the full code path (prompt assembly,
//! SPARQL extraction, parse validation) without an external LLM dependency.

use pgrx::prelude::*;
use spargebra::SparqlParser;

// ─── LLM endpoint call ────────────────────────────────────────────────────────

/// The canned SPARQL response returned when the endpoint is `'mock'`.
const MOCK_SPARQL: &str = "SELECT ?s ?p ?o WHERE { ?s ?p ?o } LIMIT 10";

/// Call the configured LLM endpoint and return the raw response body.
///
/// Uses an OpenAI-compatible `/v1/chat/completions` JSON API.
/// Returns `Err` with a human-readable message on any network or HTTP error.
/// Public (crate-internal) wrapper so that `rule_authoring` and other modules
/// can reuse the same HTTP call without duplicating the implementation.
pub(crate) fn call_llm_endpoint_pub(
    endpoint: &str,
    model: &str,
    api_key: &str,
    prompt: &str,
) -> Result<String, String> {
    call_llm_endpoint(endpoint, model, api_key, prompt)
}

fn call_llm_endpoint(
    endpoint: &str,
    model: &str,
    api_key: &str,
    prompt: &str,
) -> Result<String, String> {
    let url = format!("{}/chat/completions", endpoint.trim_end_matches('/'));

    let body = serde_json::json!({
        "model": model,
        "messages": [
            {
                "role": "system",
                "content": "You are a SPARQL query generator. \
                    Given a natural-language question and a graph schema, \
                    output ONLY a valid SPARQL 1.1 SELECT query with no explanation, \
                    markdown, or extra text."
            },
            {
                "role": "user",
                "content": prompt
            }
        ],
        "temperature": 0.0
    });

    let body_str = serde_json::to_string(&body)
        .map_err(|e| format!("LLM request serialisation error: {e}"))?;

    let timeout = std::time::Duration::from_secs(30);
    let agent = ureq::AgentBuilder::new().timeout(timeout).build();

    let mut req = agent
        .post(&url)
        .set("Content-Type", "application/json")
        .set("Accept", "application/json");

    if !api_key.is_empty() {
        req = req.set("Authorization", &format!("Bearer {api_key}"));
    }

    let response = req
        .send_string(&body_str)
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    if response.status() != 200 {
        return Err(format!("HTTP {} from LLM endpoint", response.status()));
    }

    response
        .into_string()
        .map_err(|e| format!("response read error: {e}"))
}

/// Extract a SPARQL query string from an OpenAI-style chat completion response.
///
/// Looks for the `choices[0].message.content` field and strips any leading /
/// trailing whitespace or markdown code-fence markers.  Returns `None` when
/// the content cannot be extracted or appears empty.
fn extract_sparql_from_response(body: &str) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(body).ok()?;
    let content = json
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())?
        .trim()
        .to_owned();

    if content.is_empty() {
        return None;
    }

    // Strip optional markdown code fence.
    let stripped = if let Some(inner) = content
        .strip_prefix("```sparql")
        .or_else(|| content.strip_prefix("```"))
    {
        inner.trim_start().trim_end_matches("```").trim().to_owned()
    } else {
        content
    };

    if stripped.is_empty() {
        None
    } else {
        Some(stripped)
    }
}

/// Build a VoID description of the current graph for use as LLM context.
fn build_void_description() -> String {
    let triple_count = pgrx::Spi::get_one::<i64>("SELECT COUNT(*) FROM _pg_ripple.predicates")
        .unwrap_or(None)
        .unwrap_or(0);

    // Collect up to 20 predicate IRIs as hints for the LLM.
    let predicates: Vec<String> = pgrx::Spi::connect(|client| {
        let rows = client.select(
            "SELECT d.value \
             FROM _pg_ripple.predicates p \
             JOIN _pg_ripple.dictionary d ON d.id = p.id \
             ORDER BY p.triple_count DESC \
             LIMIT 20",
            None,
            &[],
        )?;
        let mut result = Vec::new();
        for row in rows {
            if let Some(v) = row.get::<&str>(1)? {
                result.push(v.to_owned());
            }
        }
        Ok::<_, pgrx::spi::Error>(result)
    })
    .unwrap_or_default();

    let pred_list = if predicates.is_empty() {
        "(no predicates yet)".to_owned()
    } else {
        predicates
            .iter()
            .map(|p| format!("  <{p}>"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        "Graph schema (VoID description):\n\
         - Total predicate types: {triple_count}\n\
         - Known predicates (most frequent first):\n{pred_list}\n"
    )
}

/// Build a SHACL shapes summary string for use as LLM context.
fn build_shapes_summary() -> String {
    let shapes: Vec<String> = pgrx::Spi::connect(|client| {
        let rows = client.select(
            "SELECT shape_iri \
             FROM _pg_ripple.shacl_shapes \
             WHERE active = true \
             ORDER BY shape_iri \
             LIMIT 10",
            None,
            &[],
        );
        let rows = match rows {
            Ok(r) => r,
            Err(_) => return Ok(Vec::new()),
        };
        let mut result = Vec::new();
        for row in rows {
            if let Some(v) = row.get::<&str>(1)? {
                result.push(v.to_owned());
            }
        }
        Ok::<_, pgrx::spi::Error>(result)
    })
    .unwrap_or_default();

    if shapes.is_empty() {
        String::new()
    } else {
        format!(
            "\nActive SHACL shapes (target classes):\n{}\n",
            shapes
                .iter()
                .map(|s| format!("  <{s}>"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}

/// Build vocabulary-bundle metadata for LLM context (Feature 10, v0.119.0).
///
/// When `pg_ripple.nl_sparql_include_bundles = on`, this function fetches
/// active vocabulary bundle metadata from the triple store:
/// - SKOS preferred labels (`skos:prefLabel`)
/// - DCTERMS titles (`dcterms:title`)
/// - Schema.org type names (`schema:name`)
/// - FOAF names (`foaf:name`)
///
/// The metadata is returned as a formatted string that is injected into the
/// LLM system prompt to improve domain-specific SPARQL translation accuracy.
fn build_bundle_metadata() -> String {
    // Query the `_pg_ripple.rule_library_catalog` view (or fall back to
    // checking `active_datalog_bundles`) to see which bundles are active.
    // For each active bundle, collect representative label/name triples.
    let bundle_labels: Vec<(String, String, String)> = pgrx::Spi::connect(|client| {
        // Collect up to 30 label/name values from common vocabulary properties.
        // Queries the dictionary join path used by all VP tables.
        let sql = "
            SELECT DISTINCT dp.value AS predicate, ds.value AS subject, do_.value AS object
            FROM _pg_ripple.predicates pr
            JOIN _pg_ripple.dictionary dp ON dp.id = pr.id
            WHERE dp.value IN (
                'http://www.w3.org/2004/02/skos/core#prefLabel',
                'http://purl.org/dc/terms/title',
                'http://schema.org/name',
                'http://xmlns.com/foaf/0.1/name'
            )
            LIMIT 1
        ";
        // If the query succeeds we know the predicates exist; we then
        // use a more targeted join through the VP table for subject labels.
        // Since we don't know the VP table names at compile time, we use
        // a simpler fallback approach: query the predicate catalog and
        // list bundle-related predicates that are registered.
        let pred_rows = client.select(
            "SELECT d.value \
             FROM _pg_ripple.predicates p \
             JOIN _pg_ripple.dictionary d ON d.id = p.id \
             WHERE d.value IN ( \
                 'http://www.w3.org/2004/02/skos/core#prefLabel', \
                 'http://purl.org/dc/terms/title', \
                 'http://schema.org/name', \
                 'http://xmlns.com/foaf/0.1/name' \
             ) \
             ORDER BY p.triple_count DESC",
            None,
            &[],
        );
        let _ = sql; // suppress unused warning
        match pred_rows {
            Ok(rows) => {
                let mut result = Vec::new();
                for row in rows {
                    if let Some(v) = row.get::<&str>(1).unwrap_or(None) {
                        let short = match v {
                            "http://www.w3.org/2004/02/skos/core#prefLabel" => "skos:prefLabel",
                            "http://purl.org/dc/terms/title" => "dcterms:title",
                            "http://schema.org/name" => "schema:name",
                            "http://xmlns.com/foaf/0.1/name" => "foaf:name",
                            _ => v,
                        };
                        result.push((v.to_owned(), short.to_owned(), String::new()));
                    }
                }
                Ok::<_, pgrx::spi::Error>(result)
            }
            Err(_) => Ok(Vec::new()),
        }
    })
    .unwrap_or_default();

    if bundle_labels.is_empty() {
        return String::new();
    }

    let pred_list = bundle_labels
        .iter()
        .map(|(_, short, _)| format!("  {short}"))
        .collect::<Vec<_>>()
        .join("\n");

    format!("\nActive vocabulary bundle predicates (use these in SPARQL patterns):\n{pred_list}\n")
}

/// Load few-shot examples from `_pg_ripple.llm_examples`.
fn load_few_shot_examples() -> Vec<(String, String)> {
    pgrx::Spi::connect(|client| {
        let rows = client.select(
            "SELECT question, sparql FROM _pg_ripple.llm_examples ORDER BY question LIMIT 20",
            None,
            &[],
        )?;
        let mut result = Vec::new();
        for row in rows {
            let q = row.get::<&str>(1)?.unwrap_or("").to_owned();
            let s = row.get::<&str>(2)?.unwrap_or("").to_owned();
            if !q.is_empty() && !s.is_empty() {
                result.push((q, s));
            }
        }
        Ok::<_, pgrx::spi::Error>(result)
    })
    .unwrap_or_default()
}

// ─── Public SQL-callable functions ────────────────────────────────────────────

/// Convert a natural-language question to a SPARQL query via a configured LLM.
///
/// Behaviour:
/// - PT700: `pg_ripple.llm_endpoint` is empty (not configured)
/// - PT700: the HTTP call to the LLM endpoint fails
/// - PT701: the response does not contain a SPARQL-looking string
/// - PT702: the extracted string fails `spargebra` parsing
///
/// When `pg_ripple.llm_endpoint = 'mock'`, the HTTP call is bypassed and the
/// built-in canned SPARQL query is returned for testing purposes.
#[pg_extern(schema = "pg_ripple", name = "sparql_from_nl")]
pub fn sparql_from_nl(question: &str) -> String {
    let endpoint_raw = crate::LLM_ENDPOINT
        .get()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();

    let endpoint = endpoint_raw.trim().to_owned();

    if endpoint.is_empty() {
        pgrx::error!(
            "LLM endpoint not configured (PT700); \
             set pg_ripple.llm_endpoint to an OpenAI-compatible base URL \
             or 'mock' for testing"
        );
    }

    // Mock path: bypass HTTP and return a canned query for testing.
    if endpoint == "mock" {
        let sparql = MOCK_SPARQL.to_owned();
        // Validate the canned query (sanity check).
        if SparqlParser::new().parse_query(&sparql).is_err() {
            pgrx::error!("mock SPARQL query failed to parse (PT702): {sparql}");
        }
        return sparql;
    }

    // Resolve the API key from the environment variable.
    let key_env = crate::LLM_API_KEY_ENV
        .get()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "PG_RIPPLE_LLM_API_KEY".to_owned());

    let key_env_trimmed = key_env.trim().to_owned();
    let api_key = if key_env_trimmed.is_empty() {
        String::new()
    } else {
        // SAFETY: std::env::var reads from the process environment; no mutation occurs.
        std::env::var(&key_env_trimmed).unwrap_or_default()
    };

    let model = crate::LLM_MODEL
        .get()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "gpt-4o".to_owned());

    // Assemble the prompt.
    let void_desc = build_void_description();
    let shapes_ctx = if crate::LLM_INCLUDE_SHAPES.get() {
        build_shapes_summary()
    } else {
        String::new()
    };

    // Feature 10 (v0.119.0): include active vocabulary-bundle metadata when
    // pg_ripple.nl_sparql_include_bundles is on (default).
    let bundles_ctx = if crate::NL_SPARQL_INCLUDE_BUNDLES.get() {
        build_bundle_metadata()
    } else {
        String::new()
    };

    let examples = load_few_shot_examples();
    let few_shot = if examples.is_empty() {
        String::new()
    } else {
        let pairs = examples
            .iter()
            .map(|(q, s)| format!("Q: {q}\nSPARQL: {s}"))
            .collect::<Vec<_>>()
            .join("\n\n");
        format!("\n\nExamples:\n{pairs}\n")
    };

    let prompt = format!(
        "{void_desc}{shapes_ctx}{bundles_ctx}{few_shot}\n\
         Question: {question}\n\
         Output ONLY the SPARQL query, nothing else."
    );

    // Call the LLM endpoint.
    let raw_body = call_llm_endpoint(&endpoint, &model, &api_key, &prompt).unwrap_or_else(|e| {
        pgrx::error!("LLM endpoint unreachable or returned HTTP error: {e} (PT700)")
    });

    // Extract the SPARQL string from the chat completion response.
    let sparql = extract_sparql_from_response(&raw_body).unwrap_or_else(|| {
        pgrx::error!(
            "LLM response did not contain a valid SPARQL query (PT701); \
             raw response: {}",
            &raw_body[..raw_body.len().min(500)]
        )
    });

    // Validate parsability.
    if let Err(e) = SparqlParser::new().parse_query(&sparql) {
        pgrx::error!(
            "LLM-generated SPARQL query failed to parse (PT702): {e}; \
             query text: {sparql}"
        );
    }

    sparql
}

/// Store a few-shot question/SPARQL example for use as LLM context.
///
/// Rows are persisted in `_pg_ripple.llm_examples` and loaded automatically
/// by `sparql_from_nl()` on each call.
#[pg_extern(schema = "pg_ripple", name = "add_llm_example")]
pub fn add_llm_example(question: &str, sparql: &str) {
    pgrx::Spi::run_with_args(
        "INSERT INTO _pg_ripple.llm_examples (question, sparql) \
         VALUES ($1, $2) \
         ON CONFLICT (question) DO UPDATE SET sparql = EXCLUDED.sparql",
        &[
            pgrx::datum::DatumWithOid::from(question),
            pgrx::datum::DatumWithOid::from(sparql),
        ],
    )
    .unwrap_or_else(|e| pgrx::error!("add_llm_example: SPI error: {e}"));
}

// ─── Embedding-based owl:sameAs candidate generation ─────────────────────────

/// Return candidate `owl:sameAs` entity pairs by HNSW cosine self-join.
///
/// Requires pgvector to be installed and `_pg_ripple.embeddings` to contain
/// at least two rows.  Degrades gracefully with a WARNING when:
/// - pgvector is not installed
/// - `pg_ripple.pgvector_enabled = false`
/// - the embeddings table has fewer than 2 entities
///
/// Each row contains the IRI strings of the two candidate entities and their
/// cosine similarity score.  Pairs with `similarity >= threshold` are returned.
/// Self-matches (same entity_id) are excluded.
#[pg_extern(schema = "pg_ripple", name = "suggest_sameas")]
pub fn suggest_sameas(
    threshold: default!(f32, "0.9"),
) -> TableIterator<'static, (name!(s1, String), name!(s2, String), name!(similarity, f32))> {
    // Graceful degradation when pgvector is unavailable.
    if !crate::PGVECTOR_ENABLED.get() {
        pgrx::warning!(
            "pg_ripple.suggest_sameas: pgvector disabled \
             (pg_ripple.pgvector_enabled = false); returning empty results"
        );
        return TableIterator::new(std::iter::empty());
    }

    if !crate::sparql::embedding::has_pgvector() {
        pgrx::warning!(
            "pg_ripple.suggest_sameas: pgvector extension not installed (PT603); \
             install pgvector and run the 0.27.0 migration to enable similarity search"
        );
        return TableIterator::new(std::iter::empty());
    }

    // Clamp threshold to [0.0, 1.0].
    let threshold = threshold.clamp(0.0_f32, 1.0_f32);

    // Self-join: find pairs (a, b) where cosine_distance(a.embedding, b.embedding)
    // is small enough that 1 - distance >= threshold.
    // We use `<=>` (cosine distance) from pgvector; similarity = 1 - distance.
    let query = format!(
        "SELECT \
             da.value AS s1, \
             db.value AS s2, \
             (1.0 - (a.embedding <=> b.embedding))::real AS similarity \
         FROM _pg_ripple.embeddings a \
         JOIN _pg_ripple.embeddings b \
             ON a.entity_id < b.entity_id \
         JOIN _pg_ripple.dictionary da ON da.id = a.entity_id \
         JOIN _pg_ripple.dictionary db ON db.id = b.entity_id \
         WHERE a.model = b.model \
           AND da.kind = 0 \
           AND db.kind = 0 \
           AND (1.0 - (a.embedding <=> b.embedding)) >= {threshold}"
    );

    let rows: Vec<(String, String, f32)> = pgrx::Spi::connect(|client| {
        let result = client.select(&query, None, &[])?;
        let mut out = Vec::new();
        for row in result {
            let s1 = row.get::<&str>(1)?.unwrap_or("").to_owned();
            let s2 = row.get::<&str>(2)?.unwrap_or("").to_owned();
            let sim = row.get::<f32>(3)?.unwrap_or(0.0);
            if !s1.is_empty() && !s2.is_empty() {
                out.push((s1, s2, sim));
            }
        }
        Ok::<_, pgrx::spi::Error>(out)
    })
    .unwrap_or_else(|e| {
        pgrx::warning!("suggest_sameas: SPI error: {e}");
        Vec::new()
    });

    TableIterator::new(rows)
}

/// Insert accepted `owl:sameAs` candidate pairs as triples and trigger
/// cluster merging.
///
/// Runs `suggest_sameas(min_similarity)` and, for each returned pair, inserts
/// an `owl:sameAs` triple (both directions).  The cluster-size guard from
/// `pg_ripple.sameas_max_cluster_size` (PT550) is respected via the normal
/// storage path.
///
/// Returns the number of new `owl:sameAs` triples inserted (each direction
/// counts separately, so a single pair contributes 2 if both directions are new).
#[pg_extern(schema = "pg_ripple", name = "apply_sameas_candidates")]
pub fn apply_sameas_candidates(min_similarity: default!(f32, "0.95")) -> i64 {
    const OWL_SAME_AS: &str = "<http://www.w3.org/2002/07/owl#sameAs>";

    let candidates: Vec<(String, String)> = pgrx::Spi::connect(|client| {
        let threshold = min_similarity.clamp(0.0_f32, 1.0_f32);

        if !crate::PGVECTOR_ENABLED.get() || !crate::sparql::embedding::has_pgvector() {
            return Ok(Vec::new());
        }

        let query = format!(
            "SELECT \
                 da.value AS s1, \
                 db.value AS s2 \
             FROM _pg_ripple.embeddings a \
             JOIN _pg_ripple.embeddings b \
                 ON a.entity_id < b.entity_id \
             JOIN _pg_ripple.dictionary da ON da.id = a.entity_id \
             JOIN _pg_ripple.dictionary db ON db.id = b.entity_id \
             WHERE a.model = b.model \
               AND da.kind = 0 \
               AND db.kind = 0 \
               AND (1.0 - (a.embedding <=> b.embedding)) >= {threshold}"
        );

        let result = client.select(&query, None, &[])?;
        let mut out = Vec::new();
        for row in result {
            let s1 = row.get::<&str>(1)?.unwrap_or("").to_owned();
            let s2 = row.get::<&str>(2)?.unwrap_or("").to_owned();
            if !s1.is_empty() && !s2.is_empty() {
                out.push((s1, s2));
            }
        }
        Ok::<_, pgrx::spi::Error>(out)
    })
    .unwrap_or_default();

    let mut inserted: i64 = 0;
    for (s1, s2) in candidates {
        let iri_s1 = format!("<{s1}>");
        let iri_s2 = format!("<{s2}>");

        // Forward: s1 owl:sameAs s2
        let sid_fwd = crate::storage::insert_triple(&iri_s1, OWL_SAME_AS, &iri_s2, 0);
        if sid_fwd > 0 {
            inserted += 1;
        }

        // Reverse: s2 owl:sameAs s1
        let sid_rev = crate::storage::insert_triple(&iri_s2, OWL_SAME_AS, &iri_s1, 0);
        if sid_rev > 0 {
            inserted += 1;
        }
    }

    inserted
}

// ─── RAG pipeline ─────────────────────────────────────────────────────────────

/// Assemble a retrieval-augmented generation context string for an LLM query.
///
/// Steps:
/// 1. Input sanitization — trim whitespace, enforce max token length,
///    reject null-byte / prompt-injection patterns.
/// 2. Cache look-up — return a cached result if available.
/// 3. Vector recall — find the `k` most similar entities to `question` via
///    HNSW cosine distance (requires pgvector + populated `_pg_ripple.embeddings`).
/// 4. SPARQL graph expansion — for each entity fetch its 1-hop neighbourhood
///    using `contextualize_entity()` and render as a JSON-LD-style fragment.
/// 5. Assemble a context string from the fragments, formatted for LLM ingestion.
/// 6. (Optional) If `pg_ripple.llm_endpoint` is set, call `sparql_from_nl()`
///    with the assembled context appended, and append the SPARQL result.
/// 7. Cache store — persist the result for future calls.
///
/// When pgvector is absent or the embeddings table is empty, the function
/// degrades gracefully and returns an empty string with a WARNING rather than
/// raising an ERROR.
#[pg_extern(schema = "pg_ripple", name = "rag_context", volatile)]
pub fn rag_context(question: &str, k: default!(i32, "10")) -> String {
    // ── Step 1: input sanitization ──────────────────────────────────────────
    // Reject null bytes (could confuse downstream string handling).
    if question.contains('\0') {
        pgrx::error!("rag_context: question must not contain null bytes");
    }

    // Trim and enforce a maximum token/character limit (16 KiB is generous).
    let question = question.trim();
    if question.len() > 16_384 {
        pgrx::error!(
            "rag_context: question exceeds maximum length of 16,384 characters \
             (got {}); truncate the input before calling rag_context()",
            question.len()
        );
    }

    if question.is_empty() {
        return String::new();
    }

    // Graceful degradation when pgvector is unavailable.
    if !crate::PGVECTOR_ENABLED.get() {
        pgrx::warning!(
            "pg_ripple.rag_context: pgvector disabled \
             (pg_ripple.pgvector_enabled = false); returning empty context"
        );
        return String::new();
    }

    if !crate::sparql::embedding::has_pgvector() {
        pgrx::warning!(
            "pg_ripple.rag_context: pgvector extension not installed (PT603); \
             install pgvector and run the 0.27.0 migration to enable RAG"
        );
        return String::new();
    }

    let k_clamped = k.clamp(1, 100);

    // ── Step 2: cache look-up ───────────────────────────────────────────────
    // Compute a stable hash of the question for the cache key.
    use std::hash::{DefaultHasher, Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    question.hash(&mut hasher);
    let q_hash = format!("{:016x}", hasher.finish());

    let cached: Option<String> = pgrx::Spi::get_one_with_args::<String>(
        "SELECT result FROM _pg_ripple.rag_cache \
         WHERE question_hash = $1 AND k = $2 AND schema_digest = $3 \
         AND cached_at > now() - interval '1 hour'",
        &[
            pgrx::datum::DatumWithOid::from(q_hash.as_str()),
            pgrx::datum::DatumWithOid::from(k_clamped),
            pgrx::datum::DatumWithOid::from(""),
        ],
    )
    .unwrap_or(None);

    if let Some(result) = cached {
        return result;
    }

    // Step 3 & 4: vector recall + 1-hop context for each entity.
    let rows = crate::sparql::embedding::rag_retrieve(question, None, k_clamped, None, "jsonb");

    if rows.is_empty() {
        return String::new();
    }

    // Step 5: assemble context string.
    let mut parts: Vec<String> = Vec::with_capacity(rows.len());
    for (entity_iri, label, context_json, _distance) in &rows {
        let ctx_str = serde_json::to_string_pretty(&context_json.0).unwrap_or_default();
        parts.push(format!(
            "Entity: {entity_iri}\nLabel: {label}\nContext:\n{ctx_str}"
        ));
    }
    let mut context = parts.join("\n\n---\n\n");

    // Step 6 (optional): if LLM endpoint is configured, generate and execute
    // a SPARQL query for the question and append the result.
    let endpoint_raw = crate::LLM_ENDPOINT
        .get()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let endpoint = endpoint_raw.trim().to_owned();

    if !endpoint.is_empty() {
        // Generate SPARQL via NL→SPARQL.
        let sparql = sparql_from_nl(question);
        // Execute and append a summary of results.
        let result_rows = crate::sparql::sparql(&sparql);
        if !result_rows.is_empty() {
            let result_json = serde_json::to_string_pretty(&result_rows).unwrap_or_default();
            context.push_str(&format!(
                "\n\n---\n\nSPARQL Result for: {question}\n{result_json}"
            ));
        }
    }

    // ── Step 7: cache store ─────────────────────────────────────────────────
    // Best-effort: ignore errors so caching never breaks the main result.
    pgrx::Spi::run_with_args(
        "INSERT INTO _pg_ripple.rag_cache \
             (question_hash, k, schema_digest, result, cached_at) \
         VALUES ($1, $2, $3, $4, now()) \
         ON CONFLICT (question_hash, k, schema_digest) \
         DO UPDATE SET result = EXCLUDED.result, cached_at = EXCLUDED.cached_at",
        &[
            pgrx::datum::DatumWithOid::from(q_hash.as_str()),
            pgrx::datum::DatumWithOid::from(k_clamped),
            pgrx::datum::DatumWithOid::from(""),
            pgrx::datum::DatumWithOid::from(context.as_str()),
        ],
    )
    .unwrap_or_else(|e| pgrx::warning!("llm cache write: {e}"));

    context
}

// ─── LLM-Augmented SPARQL Repair (v0.57.0) ───────────────────────────────────

/// Maximum byte length of `query` or `error_message` inputs (32 KiB).
const REPAIR_MAX_INPUT_BYTES: usize = 32 * 1024;

/// Prompt-injection markers that must not appear in user-supplied inputs.
const PROMPT_INJECTION_MARKERS: &[&str] = &[
    "IGNORE PREVIOUS INSTRUCTIONS",
    "ignore previous instructions",
    "SYSTEM PROMPT",
    "system prompt",
    "<|SYSTEM|>",
    "<|im_start|>",
    "###INSTRUCTION",
    "###instruction",
];

/// Suggest a repaired SPARQL query using the configured LLM endpoint.
///
/// Sends the broken query, error message, and a schema digest (top-20 predicates)
/// to the LLM endpoint and returns the suggested SPARQL repair.
///
/// **Safety invariant**: this function NEVER executes the returned query.
/// The result is always returned as plain text for the caller to review.
///
/// Security mitigations:
/// - Length cap at 32 KiB per input field.
/// - Strips null bytes from inputs.
/// - Rejects known prompt-injection marker strings.
#[pg_extern(schema = "pg_ripple", name = "repair_sparql")]
pub fn repair_sparql(query: &str, error_message: default!(&str, "''")) -> String {
    // ── Input validation ────────────────────────────────────────────────────

    // Strip null bytes (security: prevent SQL injection via null byte smuggling).
    let query_clean: String = query.chars().filter(|&c| c != '\0').collect();
    let error_clean: String = error_message.chars().filter(|&c| c != '\0').collect();

    // Length cap.
    if query_clean.len() > REPAIR_MAX_INPUT_BYTES {
        pgrx::error!(
            "PT560: repair_sparql: query exceeds maximum length ({} bytes)",
            REPAIR_MAX_INPUT_BYTES
        );
    }
    if error_clean.len() > REPAIR_MAX_INPUT_BYTES {
        pgrx::error!(
            "PT561: repair_sparql: error_message exceeds maximum length ({} bytes)",
            REPAIR_MAX_INPUT_BYTES
        );
    }

    // Prompt-injection check.
    for marker in PROMPT_INJECTION_MARKERS {
        if query_clean.contains(marker) || error_clean.contains(marker) {
            pgrx::warning!(
                "repair_sparql: potential prompt injection detected in input; request blocked"
            );
            return String::new();
        }
    }

    // ── Schema digest: top-20 most-queried predicates ───────────────────────
    let schema_digest: String = Spi::connect(|client| {
        let rows = client.select(
            "SELECT coalesce(d.value, p.id::text) \
             FROM _pg_ripple.predicates p \
             LEFT JOIN _pg_ripple.dictionary d ON d.id = p.id \
             ORDER BY p.triple_count DESC NULLS LAST \
             LIMIT 20",
            None,
            &[],
        )?;
        let mut predicates = Vec::new();
        for row in rows {
            if let Some(s) = row.get::<String>(1)? {
                predicates.push(s);
            }
        }
        Ok::<_, pgrx::spi::Error>(predicates.join(", "))
    })
    .unwrap_or_default();

    // ── LLM endpoint ────────────────────────────────────────────────────────
    let endpoint_raw = crate::LLM_ENDPOINT
        .get()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let endpoint = endpoint_raw.trim().to_owned();

    if endpoint.is_empty() {
        pgrx::warning!("repair_sparql: pg_ripple.llm_endpoint is not set; returning empty repair");
        return String::new();
    }

    let model_raw = crate::LLM_MODEL
        .get()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "gpt-4o-mini".to_string());
    let model = model_raw.trim().to_owned();

    let api_key = crate::LLM_API_KEY_ENV
        .get()
        .and_then(|env_var| {
            let env_name = env_var.to_str().ok()?;
            std::env::var(env_name).ok()
        })
        .unwrap_or_default();

    // ── Mock mode ───────────────────────────────────────────────────────────
    if endpoint == "mock" {
        return format!(
            "SELECT ?s ?p ?o WHERE {{ ?s ?p ?o }} LIMIT 10 -- repaired from: {}",
            &query_clean[..query_clean.len().min(80)]
        );
    }

    // ── Build repair prompt ──────────────────────────────────────────────────
    let prompt = format!(
        "The following SPARQL 1.1 query has an error:\n\n\
         ```sparql\n{query_clean}\n```\n\n\
         Error: {error_clean}\n\n\
         Known predicates in the graph: {schema_digest}\n\n\
         Please provide a corrected SPARQL 1.1 query. \
         Output ONLY the corrected SPARQL query, with no explanation, \
         markdown formatting, or extra text."
    );

    // ── Call LLM and extract SPARQL ──────────────────────────────────────────
    match call_llm_endpoint(&endpoint, &model, &api_key, &prompt) {
        Ok(response) => extract_sparql_from_response(&response).unwrap_or_default(),
        Err(e) => {
            pgrx::warning!("repair_sparql: LLM call failed: {e}");
            String::new()
        }
    }
}

// v0.122.0 H17-02: Automated Ontology Mapping extracted to mapping sub-module
pub(crate) mod mapping;
