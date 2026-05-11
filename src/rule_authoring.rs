//! Guided rule authoring & LLM rule extraction (v0.105.0).
//!
//! Provides three SQL functions:
//!
//! - `pg_ripple.validate_rule(rule TEXT) → JSONB` — static analysis of a
//!   Datalog rule without loading it; reports syntax errors, unbound head
//!   variables, unused body variables, and potential stratification issues.
//!
//! - `pg_ripple.draft_rule_from_nl(description TEXT, candidates INT DEFAULT 3)
//!   → TABLE(rank INT, rule TEXT, explanation TEXT)` — sends the natural-
//!   language description to the configured LLM endpoint together with the
//!   current predicate catalog as context and returns the top N candidate
//!   Datalog rules.
//!
//! - `pg_ripple.suggest_rules(graph_iri TEXT, examples JSONB DEFAULT NULL)
//!   → TABLE(rule TEXT, support BIGINT, explanation TEXT)` — scans VP tables
//!   for statistical co-occurrence patterns and proposes candidate rules.
//!   **Experimental**: API may change and results require domain expert review.
//!
//! # Error catalog
//!
//! - PT0457: `draft_rule_from_nl: candidates must be between 1 and 10, got %d`
//! - PT0458: `draft_rule_from_nl: pg_ripple.llm_endpoint is not configured`

use pgrx::prelude::*;
use serde_json::json;

use crate::datalog::{BodyLiteral, Term, parser::parse_rules, stratify::stratify};

// ─── validate_rule() ─────────────────────────────────────────────────────────

/// Static analysis of a Datalog rule without loading it.
///
/// Returns `{"valid": true}` on a clean rule, or
/// `{"valid": false, "errors": [...], "warnings": [...]}` with structured
/// diagnostic entries.  Each entry has `"code"` and `"message"` fields.
///
/// **Errors** (rule cannot be used as-is):
/// - Syntax errors reported by the Datalog parser.
/// - Head variables not bound in the rule body (unsafe rule).
/// - Unsafe negation (a negated body atom introduces variables not bound
///   by positive body atoms).
///
/// **Warnings** (rule may behave unexpectedly):
/// - Body variables that do not appear in the head (unused variables).
/// - Rules where stratification analysis detects potential issues.
/// - Rules whose head predicate does not exist in the current schema.
#[pg_extern(schema = "pg_ripple", name = "validate_rule")]
pub fn validate_rule(rule: &str) -> pgrx::JsonB {
    let mut errors: Vec<serde_json::Value> = Vec::new();
    let mut warnings: Vec<serde_json::Value> = Vec::new();

    // ── 1. Parse ─────────────────────────────────────────────────────────────
    let rule_set = match parse_rules(rule, "_validate") {
        Ok(rs) => rs,
        Err(e) => {
            return pgrx::JsonB(json!({
                "valid": false,
                "errors": [{"code": "SYNTAX_ERROR", "message": e}],
                "warnings": []
            }));
        }
    };

    if rule_set.rules.is_empty() {
        return pgrx::JsonB(json!({
            "valid": false,
            "errors": [{"code": "EMPTY_RULE", "message": "no rule found in input"}],
            "warnings": []
        }));
    }

    // ── 2. Per-rule checks ────────────────────────────────────────────────────
    for parsed_rule in &rule_set.rules {
        // Collect variables bound by positive body atoms.
        let mut positive_body_vars: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        // Collect all variables appearing anywhere in the body.
        let mut all_body_vars: std::collections::HashSet<String> = std::collections::HashSet::new();

        for lit in &parsed_rule.body {
            match lit {
                BodyLiteral::Positive(atom) => {
                    collect_vars_from_terms(
                        &[&atom.s, &atom.p, &atom.o, &atom.g],
                        &mut positive_body_vars,
                    );
                    collect_vars_from_terms(
                        &[&atom.s, &atom.p, &atom.o, &atom.g],
                        &mut all_body_vars,
                    );
                }
                BodyLiteral::Negated(atom) => {
                    // Variables introduced only in negated atoms are unsafe.
                    collect_vars_from_terms(
                        &[&atom.s, &atom.p, &atom.o, &atom.g],
                        &mut all_body_vars,
                    );
                }
                BodyLiteral::Compare(l, _, r) => {
                    collect_var_from_term(l, &mut all_body_vars);
                    collect_var_from_term(r, &mut all_body_vars);
                }
                BodyLiteral::Assign(result_var, lhs, _, rhs) => {
                    positive_body_vars.insert(result_var.clone());
                    all_body_vars.insert(result_var.clone());
                    collect_var_from_term(lhs, &mut all_body_vars);
                    collect_var_from_term(rhs, &mut all_body_vars);
                }
                BodyLiteral::StringBuiltin(sb) => {
                    use crate::datalog::StringBuiltin;
                    match sb {
                        StringBuiltin::Strlen(t, _, r) => {
                            collect_var_from_term(t, &mut all_body_vars);
                            collect_var_from_term(r, &mut all_body_vars);
                        }
                        StringBuiltin::Regex(t, _) => {
                            collect_var_from_term(t, &mut all_body_vars);
                        }
                    }
                }
                BodyLiteral::Aggregate(agg) => {
                    positive_body_vars.insert(agg.result_var.clone());
                    all_body_vars.insert(agg.result_var.clone());
                    all_body_vars.insert(agg.agg_var.clone());
                }
            }
        }

        // Check head variables are bound by positive body atoms.
        if let Some(head) = &parsed_rule.head {
            let head_vars: Vec<String> = [&head.s, &head.p, &head.o, &head.g]
                .iter()
                .filter_map(|t| {
                    if let Term::Var(v) = t {
                        Some(v.clone())
                    } else {
                        None
                    }
                })
                .collect();

            for hv in &head_vars {
                if !positive_body_vars.contains(hv.as_str()) {
                    errors.push(json!({
                        "code": "UNBOUND_HEAD_VARIABLE",
                        "message": format!(
                            "head variable ?{hv} is not bound by any positive body atom"
                        )
                    }));
                }
            }

            // Check for body variables not in the head.
            for bv in &positive_body_vars {
                if !head_vars.contains(bv) {
                    warnings.push(json!({
                        "code": "UNUSED_BODY_VARIABLE",
                        "message": format!("body variable ?{bv} does not appear in the head")
                    }));
                }
            }
        }

        // Check negated atoms for variables not bound by positive body atoms.
        for lit in &parsed_rule.body {
            if let BodyLiteral::Negated(atom) = lit {
                let neg_vars: Vec<String> = [&atom.s, &atom.p, &atom.o, &atom.g]
                    .iter()
                    .filter_map(|t| {
                        if let Term::Var(v) = t {
                            Some(v.clone())
                        } else {
                            None
                        }
                    })
                    .collect();
                for nv in &neg_vars {
                    if !positive_body_vars.contains(nv.as_str()) {
                        errors.push(json!({
                            "code": "UNSAFE_NEGATION",
                            "message": format!(
                                "variable ?{nv} in negated atom is not bound by any positive body atom"
                            )
                        }));
                    }
                }
            }
        }
    }

    // ── 3. Stratification check ───────────────────────────────────────────────
    if errors.is_empty() {
        match stratify(&rule_set.rules) {
            Ok(_) => {} // clean
            Err(e) => {
                warnings.push(json!({
                    "code": "STRATIFICATION_ISSUE",
                    "message": format!("potential stratification issue: {e}")
                }));
            }
        }
    }

    // ── 4. Schema check: head predicate exists? ───────────────────────────────
    for parsed_rule in &rule_set.rules {
        if let Some(head) = &parsed_rule.head
            && let Term::Const(pred_id) = head.p
        {
            let exists: bool = Spi::get_one_with_args::<bool>(
                "SELECT EXISTS(SELECT 1 FROM _pg_ripple.predicates WHERE id = $1)",
                &[pgrx::datum::DatumWithOid::from(pred_id)],
            )
            .unwrap_or(None)
            .unwrap_or(false);

            if !exists {
                // Decode the predicate IRI for a friendly message.
                let iri = crate::dictionary::decode(pred_id)
                    .unwrap_or_else(|| format!("predicate_id:{pred_id}"));
                warnings.push(json!({
                    "code": "UNKNOWN_HEAD_PREDICATE",
                    "message": format!(
                        "head predicate <{iri}> does not exist in the current schema; \
                         the rule may never derive any triples"
                    )
                }));
            }
        }
    }

    // ── 5. Assemble result ────────────────────────────────────────────────────
    if errors.is_empty() {
        pgrx::JsonB(json!({
            "valid": true,
            "warnings": warnings
        }))
    } else {
        pgrx::JsonB(json!({
            "valid": false,
            "errors": errors,
            "warnings": warnings
        }))
    }
}

// ─── Helper functions ─────────────────────────────────────────────────────────

fn collect_vars_from_terms(terms: &[&Term], out: &mut std::collections::HashSet<String>) {
    for t in terms {
        collect_var_from_term(t, out);
    }
}

fn collect_var_from_term(t: &Term, out: &mut std::collections::HashSet<String>) {
    if let Term::Var(v) = t {
        out.insert(v.clone());
    }
}

// ─── draft_rule_from_nl() ─────────────────────────────────────────────────────

/// Translate a natural-language rule description to Datalog via the LLM
/// endpoint.
///
/// Returns the top `candidates` candidate rules (ranked by LLM confidence),
/// each with the Datalog rule text and a one-sentence explanation.
///
/// The returned rules are **not loaded** — the caller must explicitly call
/// `load_rules()` after review.
///
/// # Error codes
///
/// - PT0457: `candidates` is outside the range [1, 10].
/// - PT0458: `pg_ripple.llm_endpoint` is not configured.
#[pg_extern(schema = "pg_ripple", name = "draft_rule_from_nl")]
pub fn draft_rule_from_nl(
    description: &str,
    candidates: default!(i32, 3),
) -> TableIterator<
    'static,
    (
        name!(rank, i32),
        name!(rule, String),
        name!(explanation, String),
    ),
> {
    // PT0457: validate candidates range.
    if !(1..=10).contains(&candidates) {
        pgrx::error!(
            "PT0457: draft_rule_from_nl: candidates must be between 1 and 10, got {candidates}"
        );
    }

    // PT0458: check llm_endpoint is configured.
    let endpoint = crate::gucs::llm::LLM_ENDPOINT.get().and_then(|s| {
        let s = s.to_str().unwrap_or("").trim().to_owned();
        if s.is_empty() { None } else { Some(s) }
    });

    let endpoint = match endpoint {
        Some(e) => e,
        None => {
            pgrx::error!("PT0458: draft_rule_from_nl: pg_ripple.llm_endpoint is not configured");
        }
    };

    // If endpoint is 'mock', return canned responses for testing.
    if endpoint.eq_ignore_ascii_case("mock") {
        let rows: Vec<(i32, String, String)> = (1..=candidates)
            .map(|rank| {
                let rule = format!(
                    "?x <http://example.org/derivedFrom{rank}> ?y :- \
                     ?x <http://example.org/source> ?y ."
                );
                let explanation = format!(
                    "Candidate {rank}: derives a relationship based on the description: {description}"
                );
                (rank, rule, explanation)
            })
            .collect();
        return TableIterator::new(rows);
    }

    let model = crate::gucs::llm::LLM_MODEL
        .get()
        .and_then(|s| {
            let s = s.to_str().unwrap_or("").trim().to_owned();
            if s.is_empty() { None } else { Some(s) }
        })
        .unwrap_or_else(|| "gpt-4o".to_owned());

    let api_key_env = crate::gucs::llm::LLM_API_KEY_ENV
        .get()
        .and_then(|s| {
            let s = s.to_str().unwrap_or("").trim().to_owned();
            if s.is_empty() { None } else { Some(s) }
        })
        .unwrap_or_else(|| "PG_RIPPLE_LLM_API_KEY".to_owned());
    let api_key = std::env::var(&api_key_env).unwrap_or_default();

    // Build predicate catalog context.
    let catalog_context = build_predicate_catalog_context();

    // Build the prompt requesting N candidate rules.
    let prompt = format!(
        "You are a Datalog rule generator for pg_ripple, a PostgreSQL RDF triple store.\n\n\
         Datalog rule syntax:\n\
         ?head_s <predicate_IRI> ?head_o :- ?body_s1 <pred_IRI1> ?body_o1, ?body_s2 <pred_IRI2> ?body_o2 .\n\n\
         Rules use ?variables, full IRI literals <...>, and string literals \"...\". \
         Each rule ends with a period. Negation uses NOT(?s <p> ?o).\n\n\
         Current predicate catalog (most frequent predicates):\n{catalog_context}\n\n\
         Natural language description:\n{description}\n\n\
         Return exactly {candidates} candidate Datalog rules as a JSON array. \
         Each element must have:\n\
           - \"rule\": the Datalog rule text\n\
           - \"explanation\": one sentence describing what the rule does\n\n\
         Output ONLY the JSON array, no markdown, no extra text."
    );

    // Call LLM.
    let response = crate::llm::call_llm_endpoint_pub(&endpoint, &model, &api_key, &prompt);

    match response {
        Ok(body) => {
            let candidates_vec = parse_draft_rule_response(&body, candidates);
            TableIterator::new(candidates_vec)
        }
        Err(e) => {
            pgrx::warning!("draft_rule_from_nl: LLM call failed: {e}");
            TableIterator::new(vec![])
        }
    }
}

/// Parse the LLM response for draft_rule_from_nl.
///
/// Expects a JSON array of `{"rule": "...", "explanation": "..."}` objects.
fn parse_draft_rule_response(body: &str, expected: i32) -> Vec<(i32, String, String)> {
    // Try to parse as JSON.
    let parsed: serde_json::Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(_) => {
            // Try to extract JSON array from the response (LLM may wrap it).
            let stripped = strip_to_json_array(body);
            match serde_json::from_str(&stripped) {
                Ok(v) => v,
                Err(_) => {
                    pgrx::warning!("draft_rule_from_nl: could not parse LLM response as JSON");
                    return vec![];
                }
            }
        }
    };

    // Also try extracting from choices[0].message.content for OpenAI responses.
    let arr = if let Some(content) = parsed
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
    {
        let stripped = strip_to_json_array(content);
        match serde_json::from_str::<serde_json::Value>(&stripped) {
            Ok(v) => v,
            Err(_) => {
                pgrx::warning!("draft_rule_from_nl: could not parse LLM content as JSON");
                return vec![];
            }
        }
    } else {
        parsed
    };

    let arr = match arr.as_array() {
        Some(a) => a.clone(),
        None => {
            pgrx::warning!("draft_rule_from_nl: LLM response is not a JSON array");
            return vec![];
        }
    };

    arr.into_iter()
        .take(expected as usize)
        .enumerate()
        .map(|(i, item)| {
            let rank = (i + 1) as i32;
            let rule = item
                .get("rule")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
            let explanation = item
                .get("explanation")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
            (rank, rule, explanation)
        })
        .collect()
}

/// Strip surrounding text to extract a JSON array from an LLM response.
fn strip_to_json_array(s: &str) -> String {
    let s = s.trim();
    // Remove markdown code fences.
    let s = if let Some(inner) = s.strip_prefix("```json") {
        inner.trim_end_matches("```").trim()
    } else if let Some(inner) = s.strip_prefix("```") {
        inner.trim_end_matches("```").trim()
    } else {
        s
    };
    // Find the outermost JSON array.
    if let Some(start) = s.find('[')
        && let Some(end) = s.rfind(']')
        && end >= start
    {
        return s[start..=end].to_owned();
    }
    s.to_owned()
}

/// Build a brief predicate catalog description for the LLM prompt.
fn build_predicate_catalog_context() -> String {
    Spi::connect(|client| {
        let rows = client.select(
            "SELECT d.value, p.triple_count \
             FROM _pg_ripple.predicates p \
             JOIN _pg_ripple.dictionary d ON d.id = p.id \
             ORDER BY p.triple_count DESC \
             LIMIT 30",
            None,
            &[],
        )?;
        let mut lines = Vec::new();
        for row in rows {
            if let (Some(iri), Some(count)) = (row.get::<String>(1)?, row.get::<i64>(2)?) {
                lines.push(format!("<{iri}> ({count} triples)"));
            }
        }
        if lines.is_empty() {
            return Ok::<_, pgrx::spi::Error>("(no predicates in catalog)".to_owned());
        }
        Ok::<_, pgrx::spi::Error>(lines.join("\n"))
    })
    .unwrap_or_else(|_| "(catalog unavailable)".to_owned())
}

// ─── suggest_rules() ─────────────────────────────────────────────────────────

/// Scan `graph_iri` for statistical co-occurrence patterns in the VP tables
/// and propose candidate Datalog rules.
///
/// **Marked experimental** — API may change; results require domain expert
/// validation before committing.
///
/// Returns at most `pg_ripple.suggest_rules_max_candidates` candidates.
/// `examples` (optional) filters candidates to those able to derive the
/// given example triples.
#[pg_extern(schema = "pg_ripple", name = "suggest_rules")]
pub fn suggest_rules(
    graph_iri: &str,
    examples: default!(Option<pgrx::JsonB>, "NULL"),
) -> TableIterator<
    'static,
    (
        name!(rule, String),
        name!(support, i64),
        name!(explanation, String),
    ),
> {
    let max_candidates = crate::gucs::llm::SUGGEST_RULES_MAX_CANDIDATES.get() as usize;

    let graph_id: i64 = if graph_iri.is_empty() {
        0
    } else {
        crate::dictionary::encode(graph_iri, crate::dictionary::KIND_IRI)
    };

    // Parse example triples if provided.
    let example_patterns: Vec<(String, String, String)> = parse_example_triples(examples.as_ref());

    // Find co-occurrence patterns: pairs of predicates that share subjects.
    let candidates = find_cooccurrence_patterns(graph_id, max_candidates);

    // Filter by examples if provided.
    let candidates = if example_patterns.is_empty() {
        candidates
    } else {
        filter_by_examples(candidates, &example_patterns)
    };

    TableIterator::new(candidates)
}

/// Parse example triples from the optional JSONB argument.
fn parse_example_triples(examples: Option<&pgrx::JsonB>) -> Vec<(String, String, String)> {
    let Some(pgrx::JsonB(json)) = examples else {
        return vec![];
    };
    let Some(arr) = json.as_array() else {
        return vec![];
    };
    arr.iter()
        .filter_map(|item| {
            let s = item.get("s")?.as_str()?.to_owned();
            let p = item.get("p")?.as_str()?.to_owned();
            let o = item.get("o")?.as_str()?.to_owned();
            Some((s, p, o))
        })
        .collect()
}

/// Find predicate co-occurrence patterns via SQL scan.
///
/// Returns `(rule_text, support, explanation)` tuples for pairs of predicates
/// that frequently co-occur on the same subject.
fn find_cooccurrence_patterns(graph_id: i64, limit: usize) -> Vec<(String, i64, String)> {
    use pgrx::datum::DatumWithOid;

    // Query for predicate pairs that share subjects, counting support.
    // We use vp_rare as a representative sample (cross-predicate stats).
    let sql = "
        SELECT
            p1.iri  AS pred1_iri,
            p2.iri  AS pred2_iri,
            count(*) AS support
        FROM (
            SELECT DISTINCT s, p FROM _pg_ripple.vp_rare
            WHERE g = $1 OR $1 = 0
            LIMIT 10000
        ) a
        JOIN (
            SELECT DISTINCT s, p FROM _pg_ripple.vp_rare
            WHERE g = $1 OR $1 = 0
            LIMIT 10000
        ) b ON a.s = b.s AND a.p < b.p
        JOIN LATERAL (
            SELECT value AS iri FROM _pg_ripple.dictionary WHERE id = a.p
        ) p1 ON true
        JOIN LATERAL (
            SELECT value AS iri FROM _pg_ripple.dictionary WHERE id = b.p
        ) p2 ON true
        GROUP BY p1.iri, p2.iri
        HAVING count(*) > 1
        ORDER BY support DESC
        LIMIT $2
    ";

    let results = Spi::connect(|client| {
        let rows = client.select(
            sql,
            None,
            &[
                DatumWithOid::from(graph_id),
                DatumWithOid::from(limit as i64),
            ],
        )?;
        let mut out = Vec::new();
        for row in rows {
            let pred1 = row.get::<String>(1)?.unwrap_or_default();
            let pred2 = row.get::<String>(2)?.unwrap_or_default();
            let support = row.get::<i64>(3)?.unwrap_or(0);
            if !pred1.is_empty() && !pred2.is_empty() {
                out.push((pred1, pred2, support));
            }
        }
        Ok::<_, pgrx::spi::Error>(out)
    })
    .unwrap_or_default();

    results
        .into_iter()
        .map(|(pred1, pred2, support)| {
            let rule = format!(
                "?x <{pred1}> ?o1, ?x <{pred2}> ?o2 :- \
                 ?x <{pred1}> ?o1 . \
                 # Note: review and define a head predicate before loading"
            );
            let explanation = format!(
                "Subjects that have predicate <{pred1}> also frequently have \
                 predicate <{pred2}> ({support} co-occurrences)"
            );
            (rule, support, explanation)
        })
        .collect()
}

/// Filter candidates by checking which ones could derive the example triples.
///
/// Simple heuristic: keep candidates whose rule mentions a predicate
/// appearing in the example triples.
fn filter_by_examples(
    candidates: Vec<(String, i64, String)>,
    examples: &[(String, String, String)],
) -> Vec<(String, i64, String)> {
    let example_predicates: std::collections::HashSet<&str> =
        examples.iter().map(|(_, p, _)| p.as_str()).collect();

    candidates
        .into_iter()
        .filter(|(rule, _, _)| example_predicates.iter().any(|p| rule.contains(*p)))
        .collect()
}
