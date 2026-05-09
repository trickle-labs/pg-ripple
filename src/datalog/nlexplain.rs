//! Natural-language explanation of Datalog-derived facts (v0.101.0).
//!
//! `explain_inference_impl` retrieves the proof tree from `_pg_ripple.derivations`
//! via `justify_impl`, then either:
//!
//! a) sends it to the configured LLM endpoint for a narrative explanation, or
//! b) falls back to a deterministic indented-text renderer when the LLM
//!    endpoint is not configured or returns an error.
//!
//! Results are cached in `_pg_ripple.explanation_cache` keyed by `(sid, format,
//! model)` and expire after `pg_ripple.explanation_cache_ttl` seconds.

use pgrx::datum::DatumWithOid;
use pgrx::prelude::*;

// ─── Public API ───────────────────────────────────────────────────────────────

/// Implementation of `pg_ripple.explain_inference(subject, predicate, object, format)`.
///
/// Returns `None` (SQL NULL) when the triple is a base fact (no derivation
/// provenance exists) or is not in the knowledge base at all.
///
/// `format` may be `'text'` (default) or `'markdown'`.
pub fn explain_inference_impl(
    subject: &str,
    predicate: &str,
    object: &str,
    format: &str,
) -> Option<String> {
    // Step 1 — get the proof tree.
    let tree = super::derivations::justify_impl(subject, predicate, object)?;

    // Return NULL for base facts (no derivation provenance).
    if tree.get("type").and_then(|v| v.as_str()) == Some("base")
        || tree.get("derivations").is_none()
    {
        return None;
    }

    // Step 2 — look up the SID for cache keying.
    let sid = tree.get("sid").and_then(|v| v.as_i64()).unwrap_or(0);
    let model = llm_model();

    // Step 3 — check explanation cache (if TTL > 0).
    let ttl = crate::gucs::llm::EXPLANATION_CACHE_TTL_SECS.get();
    if ttl > 0 && sid > 0 {
        let cached = read_cache(sid, format, &model);
        if cached.is_some() {
            return cached;
        }
    }

    // Step 4 — generate explanation (LLM or fallback).
    let narrative = generate_narrative(&tree, format, &model);

    // Step 5 — write to cache (if TTL > 0 and SID is valid).
    if ttl > 0 && sid > 0 {
        write_cache(sid, format, &model, &narrative);
    }

    Some(narrative)
}

/// Implementation of `pg_ripple.explain_inference_jsonb(subject, predicate, object)`.
///
/// Returns `None` (SQL NULL) for base facts.
pub fn explain_inference_jsonb_impl(
    subject: &str,
    predicate: &str,
    object: &str,
) -> Option<serde_json::Value> {
    let tree = super::derivations::justify_impl(subject, predicate, object)?;

    if tree.get("type").and_then(|v| v.as_str()) == Some("base")
        || tree.get("derivations").is_none()
    {
        return None;
    }

    let model = llm_model();
    let narrative = generate_narrative(&tree, "text", &model);

    Some(serde_json::json!({
        "proof_tree": tree,
        "narrative": narrative
    }))
}

/// Implementation of `pg_ripple.vacuum_explanation_cache()`.
///
/// Deletes rows from `_pg_ripple.explanation_cache` that are older than
/// `pg_ripple.explanation_cache_ttl` seconds.  Returns the number of rows
/// removed.
pub fn vacuum_explanation_cache_impl() -> i64 {
    let ttl = crate::gucs::llm::EXPLANATION_CACHE_TTL_SECS.get();
    if ttl <= 0 {
        // Caching disabled — nothing to vacuum.
        return 0;
    }

    let sql = "WITH deleted AS ( \
               DELETE FROM _pg_ripple.explanation_cache \
               WHERE cached_at < now() - ($1 * interval '1 second') \
               RETURNING 1 \
           ) SELECT COUNT(*)::bigint FROM deleted";

    Spi::get_one_with_args::<i64>(sql, &[DatumWithOid::from(ttl)])
        .unwrap_or(None)
        .unwrap_or(0)
}

// ─── Cache helpers ────────────────────────────────────────────────────────────

/// Read a cached explanation for `(sid, format, model)`.  Returns `None` when
/// no valid (non-expired) cache entry exists.
fn read_cache(sid: i64, format: &str, model: &str) -> Option<String> {
    let ttl = crate::gucs::llm::EXPLANATION_CACHE_TTL_SECS.get();
    let sql = "SELECT explanation FROM _pg_ripple.explanation_cache \
               WHERE sid = $1 AND format = $2 AND model = $3 \
                 AND cached_at >= now() - ($4 * interval '1 second') \
               LIMIT 1";
    Spi::connect(|client| {
        client
            .select(
                sql,
                None,
                &[
                    DatumWithOid::from(sid),
                    DatumWithOid::from(format),
                    DatumWithOid::from(model),
                    DatumWithOid::from(ttl),
                ],
            )
            .unwrap_or_else(|e| pgrx::error!("explanation cache read SPI error: {e}"))
            .next()
            .and_then(|row| row.get::<String>(1).ok().flatten())
    })
}

/// Upsert a cache row for `(sid, format, model)`.
fn write_cache(sid: i64, format: &str, model: &str, explanation: &str) {
    let sql = "INSERT INTO _pg_ripple.explanation_cache (sid, format, model, explanation, cached_at) \
               VALUES ($1, $2, $3, $4, now()) \
               ON CONFLICT (sid, format, model) \
               DO UPDATE SET explanation = EXCLUDED.explanation, cached_at = now()";
    if let Err(e) = Spi::run_with_args(
        sql,
        &[
            DatumWithOid::from(sid),
            DatumWithOid::from(format),
            DatumWithOid::from(model),
            DatumWithOid::from(explanation),
        ],
    ) {
        pgrx::warning!("explanation cache write error: {e}");
    }
}

// ─── Narrative generation ─────────────────────────────────────────────────────

/// Return the configured LLM model string (falls back to empty string).
fn llm_model() -> String {
    crate::LLM_MODEL
        .get()
        .and_then(|cs| cs.to_str().ok().map(ToOwned::to_owned))
        .unwrap_or_default()
}

/// Generate a narrative explanation from the proof tree.
///
/// Tries the LLM endpoint first; falls back to the deterministic renderer
/// if the endpoint is not configured or returns an error.
fn generate_narrative(tree: &serde_json::Value, format: &str, model: &str) -> String {
    // Try LLM.
    if let Some(narrative) = try_llm_narrative(tree, format, model) {
        return narrative;
    }
    // Fallback: deterministic text renderer.
    render_proof_tree_text(tree, format)
}

/// Attempt to generate a narrative via the configured LLM endpoint.
///
/// Returns `None` when the endpoint is not set, is set to `'mock'`, or
/// returns an error (in all error cases we fall back gracefully).
fn try_llm_narrative(tree: &serde_json::Value, format: &str, _model: &str) -> Option<String> {
    let endpoint_raw = crate::LLM_ENDPOINT
        .get()
        .and_then(|cs| cs.to_str().ok().map(ToOwned::to_owned))
        .unwrap_or_default();

    if endpoint_raw.is_empty() {
        return None; // Not configured.
    }

    // Mock endpoint: return a canned explanation for testing.
    if endpoint_raw == "mock" {
        let triple = tree.get("triple").cloned().unwrap_or(serde_json::json!({}));
        let subject = triple
            .get("subject")
            .and_then(|v| v.as_str())
            .unwrap_or("the subject");
        let pred = triple
            .get("predicate")
            .and_then(|v| v.as_str())
            .unwrap_or("the predicate");
        let object = triple
            .get("object")
            .and_then(|v| v.as_str())
            .unwrap_or("the object");
        let rule = tree
            .get("derivations")
            .and_then(|d| d.get(0))
            .and_then(|d| d.get("rule"))
            .and_then(|v| v.as_str())
            .unwrap_or("an unnamed rule");
        let narrative =
            format!("The fact that {subject} has {pred} {object} was derived by rule: {rule}.");
        if format == "markdown" {
            return Some(format!(
                "## Explanation\n\n{narrative}\n\n### Rule\n\n```\n{rule}\n```"
            ));
        }
        return Some(narrative);
    }

    // Real LLM call.
    let model_str = crate::LLM_MODEL
        .get()
        .and_then(|cs| cs.to_str().ok().map(ToOwned::to_owned))
        .unwrap_or_else(|| "gpt-4o".to_owned());

    let api_key_env = crate::LLM_API_KEY_ENV
        .get()
        .and_then(|cs| cs.to_str().ok().map(ToOwned::to_owned))
        .unwrap_or_else(|| "PG_RIPPLE_LLM_API_KEY".to_owned());
    let api_key = std::env::var(&api_key_env).unwrap_or_default();

    let tree_str = serde_json::to_string_pretty(tree).unwrap_or_default();
    let prompt = format!(
        "Explain the following Datalog proof tree in plain English for a domain expert \
         (not a programmer). The audience does not know about Datalog or SPARQL. \
         Use full sentences. Be concise (2-4 sentences).\n\n\
         Proof tree:\n{tree_str}"
    );

    let system_prompt = "You are explaining why a knowledge graph derived a fact. \
         Given a proof tree (JSON), write a clear, concise explanation in plain English. \
         Audience: domain expert, not a programmer. Do not mention JSON, Datalog, or SPARQL.";

    match call_llm_for_explanation(&endpoint_raw, &model_str, &api_key, system_prompt, &prompt) {
        Ok(narrative) => {
            if format == "markdown" {
                let rule = tree
                    .get("derivations")
                    .and_then(|d| d.get(0))
                    .and_then(|d| d.get("rule"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                Some(format!(
                    "## Explanation\n\n{narrative}\n\n### Rule\n\n```\n{rule}\n```"
                ))
            } else {
                Some(narrative)
            }
        }
        Err(e) => {
            pgrx::warning!("explain_inference LLM call failed: {e}; using fallback renderer");
            None
        }
    }
}

/// Call the LLM endpoint with a custom system prompt and user prompt, returning
/// the raw narrative text from the response.
fn call_llm_for_explanation(
    endpoint: &str,
    model: &str,
    api_key: &str,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String, String> {
    let url = format!("{}/chat/completions", endpoint.trim_end_matches('/'));

    let body = serde_json::json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user",   "content": user_prompt }
        ],
        "temperature": 0.2
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

    let body_raw = response
        .into_string()
        .map_err(|e| format!("response read error: {e}"))?;

    let json: serde_json::Value =
        serde_json::from_str(&body_raw).map_err(|e| format!("JSON parse error: {e}"))?;
    let content = json
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .unwrap_or("")
        .to_owned();

    if content.is_empty() {
        return Err("LLM returned empty content".into());
    }

    Ok(content)
}

// ─── Deterministic fallback renderer ──────────────────────────────────────────

/// Render a proof tree as human-readable indented text.
///
/// This is the fallback when the LLM endpoint is unavailable.  It produces a
/// structured but readable explanation with no external dependencies.
pub fn render_proof_tree_text(tree: &serde_json::Value, format: &str) -> String {
    let mut buf = String::new();
    if format == "markdown" {
        buf.push_str("## Proof Tree\n\n");
    }
    render_node(tree, 0, &mut buf);
    buf
}

fn render_node(node: &serde_json::Value, depth: usize, buf: &mut String) {
    let indent = "  ".repeat(depth);

    let node_type = node
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    // Extract triple label.
    let triple_str = node.get("triple").map(|t| {
        let s = t.get("subject").and_then(|v| v.as_str()).unwrap_or("?");
        let p = t.get("predicate").and_then(|v| v.as_str()).unwrap_or("?");
        let o = t.get("object").and_then(|v| v.as_str()).unwrap_or("?");
        format!("{s}  {p}  {o}")
    });

    match node_type {
        "base" => {
            if let Some(ref triple) = triple_str {
                buf.push_str(&format!("{indent}[BASE FACT] {triple}\n"));
            } else {
                buf.push_str(&format!("{indent}[BASE FACT]\n"));
            }
        }
        "inferred" => {
            if let Some(ref triple) = triple_str {
                buf.push_str(&format!("{indent}[DERIVED] {triple}\n"));
            } else {
                buf.push_str(&format!("{indent}[DERIVED]\n"));
            }
            if let Some(derivations) = node.get("derivations").and_then(|v| v.as_array()) {
                for deriv in derivations {
                    let rule = deriv
                        .get("rule")
                        .and_then(|v| v.as_str())
                        .unwrap_or("(unnamed rule)");
                    buf.push_str(&format!("{indent}  via rule: {rule}\n"));
                    if let Some(ants) = deriv.get("antecedents").and_then(|v| v.as_array()) {
                        for ant in ants {
                            render_node(ant, depth + 2, buf);
                        }
                    }
                }
            }
        }
        _ => {
            // Cycle or max_depth.
            if node.get("cycle").and_then(|v| v.as_bool()).unwrap_or(false) {
                buf.push_str(&format!("{indent}[CYCLE DETECTED]\n"));
            } else {
                buf.push_str(&format!("{indent}[MAX DEPTH REACHED]\n"));
            }
        }
    }
}
