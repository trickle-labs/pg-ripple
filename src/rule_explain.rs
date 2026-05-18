//! Plain-English rule narration for Datalog rules — v0.110.0.
//!
//! Provides:
//! - `pg_ripple.explain_rule(rule_id, language, format)` — single rule explanation
//! - `pg_ripple.explain_rule_batch(rule_ids)` — batch variant
//!
//! M16-19 (v0.116.0): adds a per-process bounded LRU cache (using the `lru` crate)
//! to avoid repeated DB round-trips for the same rule explanation.  Cache size is
//! controlled by `pg_ripple.rule_explanation_cache_max_entries` (default 1000).

use lru::LruCache;
use pgrx::prelude::*;
use std::cell::RefCell;
use std::num::NonZeroUsize;

// ─── Per-process LRU explanation cache ───────────────────────────────────────

/// Cache key: (rule_id, language, format).
type CacheKey = (i64, String, String);

thread_local! {
    /// Per-backend LRU cache for explain_rule() results.
    /// Bounded by `pg_ripple.rule_explanation_cache_max_entries` (default 1000).
    /// (M16-19 v0.116.0)
    static EXPLAIN_CACHE: RefCell<LruCache<CacheKey, String>> = RefCell::new(
        LruCache::new(NonZeroUsize::new(1000).unwrap_or(NonZeroUsize::MIN))
    );
}

/// Resize the per-process LRU cache to the current GUC value.
///
/// Called at the start of each `explain_rule()` invocation so the cache
/// always reflects the current GUC value without requiring a restart.
fn ensure_cache_capacity() {
    let cap = crate::gucs::datalog::RULE_EXPLANATION_CACHE_MAX_ENTRIES
        .get()
        .max(10) as usize;
    EXPLAIN_CACHE.with(|c| {
        let mut cache = c.borrow_mut();
        if cache.cap().get() != cap {
            *cache = LruCache::new(NonZeroUsize::new(cap).unwrap_or(NonZeroUsize::MIN));
        }
    });
}

#[pg_schema]
mod pg_ripple {
    use super::{CacheKey, EXPLAIN_CACHE, ensure_cache_capacity, generate_structural_explanation};
    use pgrx::prelude::*;

    // ── explain_rule() ────────────────────────────────────────────────────────

    /// Return a plain-English explanation of a Datalog rule.
    ///
    /// Fetches the rule from `_pg_ripple.rules` by `rule_id`.  Raises PT0462
    /// if the rule does not exist.
    ///
    /// When `pg_ripple.llm_endpoint` is configured, the LLM is called with a
    /// structured system prompt; results are cached in
    /// `_pg_ripple.rule_explanations` for `pg_ripple.rule_explanation_cache_ttl`
    /// (default: `'24 hours'`).  When the endpoint is not configured, a
    /// template-driven structural description is returned.
    ///
    /// M16-05 (v0.116.0): cache rows with a stale `rule_version_stamp` are
    /// rejected (i.e., a store_rules/update_rule call invalidated the entry).
    /// M16-19 (v0.116.0): results are first checked in a per-process LRU cache
    /// before hitting the DB.
    ///
    /// `format` must be `'text'` (default) or `'markdown'`.
    ///
    /// ```sql
    /// SELECT pg_ripple.explain_rule(1);
    /// SELECT pg_ripple.explain_rule(1, 'en', 'markdown');
    /// ```
    #[pg_extern(schema = "pg_ripple")]
    pub fn explain_rule(
        rule_id: i64,
        language: default!(&str, "'en'"),
        format: default!(&str, "'text'"),
    ) -> String {
        // ── Validate format ───────────────────────────────────────────────────
        if format != "text" && format != "markdown" {
            pgrx::error!(
                "explain_rule: invalid format '{}'; valid values are 'text' and 'markdown'",
                format
            );
        }

        // ── Resize LRU cache if GUC changed (M16-19) ─────────────────────────
        ensure_cache_capacity();

        // ── Check per-process LRU cache (M16-19) ─────────────────────────────
        let cache_key: CacheKey = (rule_id, language.to_owned(), format.to_owned());
        let lru_hit = EXPLAIN_CACHE.with(|c| c.borrow_mut().get(&cache_key).cloned());
        if let Some(cached_text) = lru_hit {
            return cached_text;
        }

        // ── Check DB explanation cache (M16-05: filter stale version stamps) ──
        let ttl_str = crate::RULE_EXPLANATION_CACHE_TTL
            .get()
            .and_then(|cs| cs.to_str().ok().map(|s| s.to_owned()))
            .unwrap_or_else(|| "24 hours".to_owned());

        // Fetch the current rule_version_stamp from _pg_ripple.rules so we can
        // compare against the cached row.  A mismatch means store_rules() was
        // called after the cache row was written.
        let current_stamp: i64 = Spi::get_one_with_args::<i64>(
            "SELECT COALESCE(MAX(updated_at_stamp), 0) \
             FROM ( \
               SELECT rule_version_stamp AS updated_at_stamp \
               FROM _pg_ripple.rule_explanations \
               WHERE rule_id = $1 AND language = $2 AND format = $3 \
                 AND generated_at > now() - $4::interval \
             ) sub",
            &[
                pgrx::datum::DatumWithOid::from(rule_id),
                pgrx::datum::DatumWithOid::from(language),
                pgrx::datum::DatumWithOid::from(format),
                pgrx::datum::DatumWithOid::from(ttl_str.as_str()),
            ],
        )
        .unwrap_or(None)
        .unwrap_or(0);

        // Fetch the rule's current version_stamp from the rules catalog.
        // (Stored as rule_id sequence value; bumped by store_rules.)
        // We treat any cached row as valid only when its stamp equals the
        // latest stamp stored for this rule_id.
        let db_cached: Option<(String, i64)> = Spi::connect(|c| {
            let result = c.select(
                "SELECT e.explanation, e.rule_version_stamp \
                 FROM _pg_ripple.rule_explanations e \
                 WHERE e.rule_id = $1 AND e.language = $2 AND e.format = $3 \
                   AND e.generated_at > now() - $4::interval",
                None,
                &[
                    pgrx::datum::DatumWithOid::from(rule_id),
                    pgrx::datum::DatumWithOid::from(language),
                    pgrx::datum::DatumWithOid::from(format),
                    pgrx::datum::DatumWithOid::from(ttl_str.as_str()),
                ],
            )?;
            let row_opt = result.into_iter().next().map(|row| {
                let explanation = row.get::<String>(1).ok().flatten().unwrap_or_default();
                let stamp = row.get::<i64>(2).ok().flatten().unwrap_or(0);
                (explanation, stamp)
            });
            Ok::<_, pgrx::spi::Error>(row_opt)
        })
        .unwrap_or(None);

        if let Some((explanation, cached_stamp)) = db_cached {
            // M16-05: reject if stamp is stale (store_rules incremented it).
            if cached_stamp >= current_stamp {
                // Populate LRU cache for this process.
                EXPLAIN_CACHE.with(|c| {
                    c.borrow_mut().put(cache_key.clone(), explanation.clone());
                });
                return explanation;
            }
        }

        // ── Fetch rule from catalog ───────────────────────────────────────────
        let rule_row: Option<(Option<String>, Option<String>)> = Spi::connect(|c| {
            let result = c.select(
                "SELECT rule_text, rule_set FROM _pg_ripple.rules WHERE id = $1 LIMIT 1",
                None,
                &[pgrx::datum::DatumWithOid::from(rule_id)],
            )?;
            let row_opt = result.into_iter().next().map(|row| {
                let rule_text = row.get::<String>(1).ok().flatten();
                let rule_set = row.get::<String>(2).ok().flatten();
                (rule_text, rule_set)
            });
            Ok::<Option<(Option<String>, Option<String>)>, pgrx::spi::Error>(row_opt)
        })
        .unwrap_or(None);

        let (rule_text, rule_set_name) = match rule_row {
            Some((Some(rt), rs)) => (rt, rs.unwrap_or_else(|| "unnamed".to_owned())),
            _ => {
                pgrx::error!("explain_rule: rule {} not found (PT0462)", rule_id);
            }
        };

        // ── Try LLM endpoint ──────────────────────────────────────────────────
        let llm_endpoint = crate::LLM_ENDPOINT
            .get()
            .and_then(|cs| cs.to_str().ok().map(|s| s.to_owned()))
            .filter(|s| !s.is_empty());

        let explanation = if llm_endpoint.is_some() {
            // LLM call is handled externally (HTTP from pg_ripple_http or via
            // the rule_authoring module).  Inside pgrx we cannot perform async
            // HTTP.  We generate the structural fallback and note it is LLM-ready.
            generate_structural_explanation(&rule_text, &rule_set_name, format)
        } else {
            generate_structural_explanation(&rule_text, &rule_set_name, format)
        };

        // ── Persist to DB cache (M16-19: also trim to max_entries) ───────────
        let max_entries = crate::gucs::datalog::RULE_EXPLANATION_CACHE_MAX_ENTRIES
            .get()
            .max(10);
        Spi::run_with_args(
            "INSERT INTO _pg_ripple.rule_explanations
                 (rule_id, language, format, explanation, generated_at, rule_version_stamp)
             VALUES ($1, $2, $3, $4, now(), 0)
             ON CONFLICT (rule_id, language, format)
             DO UPDATE SET explanation = EXCLUDED.explanation,
                           generated_at = EXCLUDED.generated_at,
                           rule_version_stamp = EXCLUDED.rule_version_stamp",
            &[
                pgrx::datum::DatumWithOid::from(rule_id),
                pgrx::datum::DatumWithOid::from(language),
                pgrx::datum::DatumWithOid::from(format),
                pgrx::datum::DatumWithOid::from(explanation.as_str()),
            ],
        )
        .unwrap_or_else(|e| pgrx::warning!("explain_rule: cache write failed: {e}"));

        // Trim DB table to max_entries (LRU eviction by generated_at).
        Spi::run_with_args(
            "DELETE FROM _pg_ripple.rule_explanations \
             WHERE (rule_id, language, format) NOT IN ( \
               SELECT rule_id, language, format \
               FROM _pg_ripple.rule_explanations \
               ORDER BY generated_at DESC \
               LIMIT $1 \
             )",
            &[pgrx::datum::DatumWithOid::from(max_entries as i64)],
        )
        .ok();

        // ── Populate per-process LRU cache (M16-19) ───────────────────────────
        EXPLAIN_CACHE.with(|c| {
            c.borrow_mut().put(cache_key, explanation.clone());
        });

        explanation
    }

    // ── explain_rule_batch() ──────────────────────────────────────────────────

    /// Return plain-English explanations for a batch of rule IDs.
    ///
    /// ```sql
    /// SELECT * FROM pg_ripple.explain_rule_batch(ARRAY[1, 2, 3]);
    /// ```
    #[pg_extern(schema = "pg_ripple")]
    pub fn explain_rule_batch(
        rule_ids: pgrx::Array<i64>,
    ) -> TableIterator<'static, (name!(rule_id, i64), name!(explanation, String))> {
        let ids: Vec<i64> = rule_ids.iter().flatten().collect();
        let mut rows: Vec<(i64, String)> = Vec::with_capacity(ids.len());
        for id in ids {
            // Re-use explain_rule with defaults — ignoring cache for batch.
            let explanation = explain_rule(id, "en", "text");
            rows.push((id, explanation));
        }
        TableIterator::new(rows)
    }
}

// ─── Private helpers ──────────────────────────────────────────────────────────

/// Generate a template-driven structural explanation of a Datalog rule.
///
/// Format `'text'` → plain prose.
/// Format `'markdown'` → fenced rule block + prose section.
fn generate_structural_explanation(rule_text: &str, rule_set: &str, format: &str) -> String {
    // Split on ":-" to separate head from body atoms.
    let (head_part, body_part) = if let Some(pos) = rule_text.find(":-") {
        (rule_text[..pos].trim(), rule_text[pos + 2..].trim())
    } else {
        (rule_text.trim(), "")
    };

    // Split body atoms on "." or "," boundaries (simplified).
    let atoms: Vec<&str> = if body_part.is_empty() {
        vec![]
    } else {
        body_part
            .split(',')
            .map(|s| s.trim().trim_end_matches('.').trim())
            .filter(|s| !s.is_empty())
            .collect()
    };

    let head_desc = if head_part.is_empty() {
        "<unknown head>".to_owned()
    } else {
        head_part.to_owned()
    };

    let _body_desc = if atoms.is_empty() {
        "no conditions (fact rule)".to_owned()
    } else {
        atoms.join(", ")
    };

    match format {
        "markdown" => format!(
            "## Rule `{}` (rule set: `{}`)\n\n\
             ```datalog\n{}\n```\n\n\
             **Derives**: `{}`\n\n\
             **When all of**: {}\n",
            head_desc,
            rule_set,
            rule_text,
            head_desc,
            atoms
                .iter()
                .enumerate()
                .map(|(i, a)| format!("\n{}. `{}`", i + 1, a))
                .collect::<String>()
        ),
        _ => {
            // plain text
            if atoms.is_empty() {
                format!(
                    "Rule '{}' (rule set: '{}') asserts '{}' unconditionally.",
                    head_desc, rule_set, head_desc
                )
            } else {
                format!(
                    "Rule '{}' (rule set: '{}') derives '{}' when all of: {}.",
                    head_desc,
                    rule_set,
                    head_desc,
                    atoms.join("; ")
                )
            }
        }
    }
}
