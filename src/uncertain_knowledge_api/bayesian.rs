//! Bayesian confidence update engine (v0.108.0 BAYES-01).
//!
//! Implements Bayes' theorem in odds form for dynamic belief revision:
//!   posterior = (λ · prior) / (λ · prior + (1 − prior))
//! where λ = likelihood_ratio = P(evidence | fact true) / P(evidence | fact false).
//!
//! ## API
//! - `update_confidence(subject, predicate, object, evidence JSONB) → FLOAT8`
//! - `bulk_update_confidence(data TEXT, format TEXT) → BIGINT`
//! - `vacuum_evidence_log() → BIGINT`
//!
//! ## Error codes
//! - PT0440: likelihood_ratio ≤ 0.0
//! - PT0441: confidence_update_strategy = 'manual'
//!
//! ## Sub-modules
//! This module is called from the `#[pg_extern]` wrappers in the parent `mod.rs`.

use pgrx::datum::DatumWithOid;
use pgrx::prelude::*;

// ─── Table bootstrap ──────────────────────────────────────────────────────────

/// Create `_pg_ripple.evidence_log` and `_pg_ripple.confidence_stale` if absent.
///
/// Idempotent — safe to call on every `update_confidence` invocation.
pub(crate) fn ensure_bayesian_catalog() {
    Spi::run(
        "CREATE TABLE IF NOT EXISTS _pg_ripple.evidence_log ( \
            id                   BIGSERIAL   PRIMARY KEY, \
            sid                  BIGINT      NOT NULL, \
            event_at             TIMESTAMPTZ NOT NULL DEFAULT now(), \
            source_iri           BIGINT, \
            likelihood_ratio     FLOAT8      NOT NULL, \
            prior_confidence     FLOAT8      NOT NULL, \
            posterior_confidence FLOAT8      NOT NULL \
        )",
    )
    .unwrap_or_else(|e| pgrx::warning!("evidence_log table creation: {e}"));

    Spi::run(
        "CREATE INDEX IF NOT EXISTS idx_evidence_log_sid \
         ON _pg_ripple.evidence_log (sid)",
    )
    .unwrap_or_else(|e| pgrx::warning!("evidence_log sid index: {e}"));

    Spi::run(
        "CREATE INDEX IF NOT EXISTS idx_evidence_log_event_at \
         ON _pg_ripple.evidence_log (event_at)",
    )
    .unwrap_or_else(|e| pgrx::warning!("evidence_log event_at index: {e}"));

    Spi::run(
        "CREATE TABLE IF NOT EXISTS _pg_ripple.confidence_stale ( \
            sid       BIGINT      NOT NULL PRIMARY KEY, \
            marked_at TIMESTAMPTZ NOT NULL DEFAULT now() \
        )",
    )
    .unwrap_or_else(|e| pgrx::warning!("confidence_stale table creation: {e}"));
}

// ─── Pure Bayesian formula ────────────────────────────────────────────────────

/// Bayesian update in odds form.
///
/// posterior = (λ · p₀) / (λ · p₀ + (1 − p₀))
///
/// Result is clamped to [0.001, 0.999] (never absolute certainty).
///
/// # Panics
/// Callers must validate that `likelihood_ratio > 0.0` before calling this.
pub(crate) fn bayesian_update(prior: f64, likelihood_ratio: f64) -> f64 {
    let numerator = likelihood_ratio * prior;
    let denominator = numerator + (1.0 - prior);
    let posterior = if denominator == 0.0 {
        prior
    } else {
        numerator / denominator
    };
    posterior.clamp(0.001, 0.999)
}

// ─── Noisy-OR update helper ───────────────────────────────────────────────────

/// Noisy-OR confidence combiner — delegate to the v0.87 formula.
///
/// Treats the evidence weight as a new independent source.
/// noisy_or(prior, weight) = 1 − (1 − prior) × (1 − weight)
/// `weight` is derived from the likelihood ratio: `1 − 1 / (1 + λ)`.
fn noisy_or_update(prior: f64, likelihood_ratio: f64) -> f64 {
    // Map likelihood ratio to a "weight" in [0, 1].
    let weight = 1.0 - 1.0 / (1.0 + likelihood_ratio);
    let posterior = 1.0 - (1.0 - prior) * (1.0 - weight);
    posterior.clamp(0.001, 0.999)
}

// ─── Core implementation ─────────────────────────────────────────────────────

/// Look up the statement ID for a (subject, predicate, object) triple.
///
/// Returns `None` when the triple is not found in any VP table.
fn lookup_sid(subject: &str, predicate: &str, object: &str) -> Option<i64> {
    let s_id = crate::dictionary::lookup(subject, 0)?;
    let p_id = crate::dictionary::lookup(predicate, 0)?;
    let o_id = crate::dictionary::lookup(object, 0)?;

    // Look in vp_rare first (catches predicates below the promotion threshold).
    let sid: Option<i64> = Spi::get_one_with_args(
        "SELECT i FROM _pg_ripple.vp_rare WHERE s = $1 AND p = $2 AND o = $3 LIMIT 1",
        &[
            DatumWithOid::from(s_id),
            DatumWithOid::from(p_id),
            DatumWithOid::from(o_id),
        ],
    )
    .unwrap_or(None);

    if sid.is_some() {
        return sid;
    }

    // Locate the dedicated VP table for this predicate, if any.
    let table_name: Option<String> = Spi::get_one_with_args(
        "SELECT '_pg_ripple.vp_' || id::text \
         FROM _pg_ripple.predicates WHERE id = $1 AND table_oid IS NOT NULL LIMIT 1",
        &[DatumWithOid::from(p_id)],
    )
    .unwrap_or(None);

    let tbl = table_name?;
    Spi::get_one_with_args(
        &format!("SELECT i FROM {tbl} WHERE s = $1 AND p = $2 AND o = $3 LIMIT 1"),
        &[
            DatumWithOid::from(s_id),
            DatumWithOid::from(p_id),
            DatumWithOid::from(o_id),
        ],
    )
    .unwrap_or(None)
}

/// Retrieve the current confidence for a statement ID from `_pg_ripple.confidence`.
///
/// Returns `1.0` when no confidence row exists (implicit certainty).
fn get_confidence(sid: i64) -> f64 {
    Spi::get_one_with_args(
        "SELECT confidence FROM _pg_ripple.confidence \
         WHERE statement_id = $1 ORDER BY asserted_at DESC LIMIT 1",
        &[DatumWithOid::from(sid)],
    )
    .unwrap_or(None)
    .unwrap_or(1.0)
}

/// Upsert the confidence for a statement ID in `_pg_ripple.confidence`.
fn set_confidence(sid: i64, new_confidence: f64) {
    crate::bulk_load::ensure_confidence_catalog();
    Spi::run_with_args(
        "INSERT INTO _pg_ripple.confidence (statement_id, confidence, model, asserted_at) \
         VALUES ($1, $2, 'bayesian', now()) \
         ON CONFLICT (statement_id, model) \
         DO UPDATE SET confidence = EXCLUDED.confidence, \
                       asserted_at = EXCLUDED.asserted_at",
        &[DatumWithOid::from(sid), DatumWithOid::from(new_confidence)],
    )
    .unwrap_or_else(|e| pgrx::error!("update_confidence: confidence upsert failed: {e}"));
}

/// Append a row to `_pg_ripple.evidence_log`.
fn log_evidence(
    sid: i64,
    source_iri_encoded: Option<i64>,
    likelihood_ratio: f64,
    prior: f64,
    posterior: f64,
) {
    Spi::run_with_args(
        "INSERT INTO _pg_ripple.evidence_log \
         (sid, source_iri, likelihood_ratio, prior_confidence, posterior_confidence) \
         VALUES ($1, $2, $3, $4, $5)",
        &[
            DatumWithOid::from(sid),
            DatumWithOid::from(source_iri_encoded),
            DatumWithOid::from(likelihood_ratio),
            DatumWithOid::from(prior),
            DatumWithOid::from(posterior),
        ],
    )
    .unwrap_or_else(|e| pgrx::warning!("evidence_log insert: {e}"));
}

/// Propagate updated base-fact confidence to downstream derived facts.
///
/// Walks `_pg_ripple.derivations` up to `max_depth` levels.  Facts beyond
/// `max_depth` are inserted into `_pg_ripple.confidence_stale` for background
/// reprocessing.
///
/// This is a best-effort operation — individual update failures are logged as
/// WARNINGs and do not abort the transaction.
fn propagate_downstream(sid: i64, max_depth: i32) {
    // Walk the derivation DAG breadth-first, bounded by max_depth.
    // Each level: find rules whose antecedent_sids include the current level's SIDs,
    // then compute the new confidence for the derived SID (noisy-OR over all antecedents).

    let mut current_sids: Vec<i64> = vec![sid];

    for depth in 0..max_depth {
        if current_sids.is_empty() {
            break;
        }

        // Build an IN-clause literal to avoid array parameter difficulties.
        // Values are i64 integers — safe from SQL injection.
        let sid_list = current_sids
            .iter()
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
            .join(", ");

        // Find all derived SIDs that depend on the current wave of updated SIDs.
        let derived: Vec<i64> = Spi::connect(|c| {
            let sql = format!(
                "SELECT DISTINCT derived_sid \
                 FROM _pg_ripple.derivations \
                 WHERE antecedent_sids && ARRAY[{sid_list}]::bigint[]"
            );
            match c.select(&sql, None, &[]) {
                Ok(tbl) => tbl
                    .map(|row| row.get::<i64>(1).ok().flatten().unwrap_or(0))
                    .filter(|&x| x != 0)
                    .collect::<Vec<_>>(),
                Err(e) => {
                    pgrx::warning!("propagate_downstream: derivations scan depth {depth}: {e}");
                    Vec::new()
                }
            }
        });

        if derived.is_empty() {
            break;
        }

        // At the last allowed depth, move any remaining derived SIDs to confidence_stale.
        if depth == max_depth - 1 {
            // Check if there are further descendants — if so, mark them stale.
            for &dsid in &derived {
                let has_further: Option<i64> = Spi::get_one_with_args(
                    "SELECT 1 FROM _pg_ripple.derivations WHERE $1 = ANY(antecedent_sids) LIMIT 1",
                    &[DatumWithOid::from(dsid)],
                )
                .unwrap_or(None);
                if has_further.is_some() {
                    Spi::run_with_args(
                        "INSERT INTO _pg_ripple.confidence_stale (sid) \
                         VALUES ($1) ON CONFLICT (sid) DO UPDATE SET marked_at = now()",
                        &[DatumWithOid::from(dsid)],
                    )
                    .unwrap_or_else(|e| pgrx::warning!("confidence_stale insert: {e}"));
                }
            }
        }

        // Update each derived SID's confidence.
        for &dsid in &derived {
            // Re-read all antecedent confidences and combine via noisy-OR.
            let antecedents: Vec<i64> = Spi::connect(|c| {
                match c.select(
                    "SELECT unnest(antecedent_sids) \
                     FROM _pg_ripple.derivations \
                     WHERE derived_sid = $1 LIMIT 1",
                    None,
                    &[DatumWithOid::from(dsid)],
                ) {
                    Ok(tbl) => tbl
                        .map(|row| row.get::<i64>(1).ok().flatten().unwrap_or(0))
                        .filter(|&x| x != 0)
                        .collect::<Vec<_>>(),
                    Err(_) => Vec::new(),
                }
            });

            if antecedents.is_empty() {
                continue;
            }

            // Combine antecedent confidences via noisy-OR (conservative estimate).
            let combined = antecedents.iter().fold(0.0f64, |acc, &asid| {
                let c = get_confidence(asid);
                // noisy-OR: P(A ∨ B) = 1 − (1−A)(1−B)
                1.0 - (1.0 - acc) * (1.0 - c)
            });
            let combined = combined.clamp(0.001, 0.999);

            let prior_derived = get_confidence(dsid);
            if (prior_derived - combined).abs() > 1e-9 {
                set_confidence(dsid, combined);
                log_evidence(dsid, None, 1.0, prior_derived, combined);
            }
        }

        current_sids = derived;
    }
}

// ─── Public entry points ──────────────────────────────────────────────────────

/// Main `update_confidence` implementation.
///
/// Called from the `#[pg_extern]` wrapper in `mod.rs`.
///
/// Returns `(prior, posterior)`.
pub(crate) fn update_confidence_impl(
    subject: &str,
    predicate: &str,
    object: &str,
    evidence_json: &str,
) -> (f64, f64) {
    ensure_bayesian_catalog();
    crate::bulk_load::ensure_confidence_catalog();

    // ── Parse evidence JSON ───────────────────────────────────────────────────
    let ev: serde_json::Value = serde_json::from_str(evidence_json)
        .unwrap_or_else(|e| pgrx::error!("update_confidence: invalid evidence JSON: {e}"));

    let likelihood_ratio: f64 = ev
        .get("likelihood_ratio")
        .and_then(|v| v.as_f64())
        .unwrap_or_else(|| {
            pgrx::error!(
                "update_confidence: evidence JSON must contain 'likelihood_ratio' (PT0440)"
            )
        });

    if likelihood_ratio <= 0.0 {
        pgrx::error!(
            "update_confidence: likelihood_ratio must be positive, got {} (PT0440)",
            likelihood_ratio
        );
    }

    let source_iri: Option<&str> = ev.get("source").and_then(|v| v.as_str());

    // ── Check strategy GUC ────────────────────────────────────────────────────
    let strategy = crate::gucs::datalog::CONFIDENCE_UPDATE_STRATEGY
        .get()
        .and_then(|s| s.to_str().ok().map(|s| s.to_owned()))
        .unwrap_or_else(|| "bayesian".to_owned());

    if strategy == "manual" {
        pgrx::error!(
            "update_confidence: confidence_update_strategy is 'manual' — \
             set confidence directly via insert_triple() (PT0441)"
        );
    }

    // ── Look up the triple's SID ──────────────────────────────────────────────
    let sid = lookup_sid(subject, predicate, object).unwrap_or_else(|| {
        pgrx::error!("update_confidence: triple not found: <{subject}> <{predicate}> <{object}>")
    });

    // ── Retrieve prior confidence ─────────────────────────────────────────────
    let prior = get_confidence(sid);

    // ── Apply update formula ──────────────────────────────────────────────────
    let posterior = if strategy == "noisy-or" {
        noisy_or_update(prior, likelihood_ratio)
    } else {
        bayesian_update(prior, likelihood_ratio)
    };

    // ── Encode source IRI ─────────────────────────────────────────────────────
    let source_encoded: Option<i64> = source_iri.map(|iri| crate::dictionary::encode(iri, 0));

    // ── Persist ───────────────────────────────────────────────────────────────
    set_confidence(sid, posterior);
    log_evidence(sid, source_encoded, likelihood_ratio, prior, posterior);

    // ── Propagate downstream ──────────────────────────────────────────────────
    let max_depth = crate::gucs::datalog::CONFIDENCE_PROPAGATION_MAX_DEPTH.get();
    // Only propagate when derivations table is populated (record_derivations = on).
    let has_derivations: Option<i64> = Spi::get_one_with_args(
        "SELECT 1 FROM _pg_ripple.derivations WHERE $1 = ANY(antecedent_sids) LIMIT 1",
        &[DatumWithOid::from(sid)],
    )
    .unwrap_or(None);

    if has_derivations.is_some() {
        propagate_downstream(sid, max_depth);
    }

    (prior, posterior)
}

/// `bulk_update_confidence` implementation.
///
/// Accepts CSV or JSON-L data. Each row: `subject, predicate, object, source, likelihood_ratio`.
/// Returns count of facts updated.
pub(crate) fn bulk_update_confidence_impl(data: &str, format: &str) -> i64 {
    ensure_bayesian_catalog();
    crate::bulk_load::ensure_confidence_catalog();

    let batch_size = crate::gucs::datalog::CONFIDENCE_BATCH_SIZE.get().max(1) as usize;

    let mut updated: i64 = 0;

    match format {
        "json" | "jsonl" | "json-l" => {
            let mut batch_count = 0usize;
            for line in data.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let row: serde_json::Value = match serde_json::from_str(line) {
                    Ok(v) => v,
                    Err(e) => {
                        pgrx::warning!("bulk_update_confidence: skipping invalid JSON line: {e}");
                        continue;
                    }
                };
                let subject = row
                    .get("subject")
                    .and_then(|v| v.as_str())
                    .unwrap_or_else(|| {
                        pgrx::warning!("bulk_update_confidence: missing 'subject' field");
                        ""
                    });
                if subject.is_empty() {
                    continue;
                }
                let predicate = row
                    .get("predicate")
                    .and_then(|v| v.as_str())
                    .unwrap_or_else(|| {
                        pgrx::warning!("bulk_update_confidence: missing 'predicate' field");
                        ""
                    });
                if predicate.is_empty() {
                    continue;
                }
                let object = row
                    .get("object")
                    .and_then(|v| v.as_str())
                    .unwrap_or_else(|| {
                        pgrx::warning!("bulk_update_confidence: missing 'object' field");
                        ""
                    });
                if object.is_empty() {
                    continue;
                }
                let source = row
                    .get("source")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned();
                let lr = match row.get("likelihood_ratio").and_then(|v| v.as_f64()) {
                    Some(v) => v,
                    None => {
                        pgrx::warning!(
                            "bulk_update_confidence: missing or non-numeric 'likelihood_ratio'"
                        );
                        continue;
                    }
                };
                if lr <= 0.0 {
                    pgrx::warning!(
                        "bulk_update_confidence: skipping row with likelihood_ratio <= 0"
                    );
                    continue;
                }

                // Collapse duplicate SIDs within a batch (product of likelihood ratios).
                let ev = serde_json::json!({"source": source, "likelihood_ratio": lr});
                let ev_str = ev.to_string();
                match std::panic::catch_unwind(|| {
                    update_confidence_impl(subject, predicate, object, &ev_str)
                }) {
                    Ok(_) => {
                        updated += 1;
                        batch_count += 1;
                    }
                    Err(_) => {
                        pgrx::warning!(
                            "bulk_update_confidence: failed to update triple ({subject}, {predicate}, {object})"
                        );
                    }
                }

                if batch_count >= batch_size {
                    batch_count = 0;
                }
            }
        }
        _ => {
            // Default: CSV — subject,predicate,object,source,likelihood_ratio
            for line in data.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let parts: Vec<&str> = line.splitn(5, ',').collect();
                if parts.len() < 5 {
                    pgrx::warning!(
                        "bulk_update_confidence: CSV line has fewer than 5 fields: {line}"
                    );
                    continue;
                }
                let subject = parts[0].trim();
                let predicate = parts[1].trim();
                let object = parts[2].trim();
                let source = parts[3].trim().to_owned();
                let lr: f64 = match parts[4].trim().parse() {
                    Ok(v) => v,
                    Err(_) => {
                        pgrx::warning!(
                            "bulk_update_confidence: non-numeric likelihood_ratio in line: {line}"
                        );
                        continue;
                    }
                };
                if lr <= 0.0 {
                    pgrx::warning!(
                        "bulk_update_confidence: skipping row with likelihood_ratio <= 0"
                    );
                    continue;
                }

                let ev = serde_json::json!({"source": source, "likelihood_ratio": lr});
                let ev_str = ev.to_string();
                match std::panic::catch_unwind(|| {
                    update_confidence_impl(subject, predicate, object, &ev_str)
                }) {
                    Ok(_) => {
                        updated += 1;
                    }
                    Err(_) => {
                        pgrx::warning!(
                            "bulk_update_confidence: failed to update triple ({subject}, {predicate}, {object})"
                        );
                    }
                }
            }
        }
    }

    updated
}

/// Prune expired `_pg_ripple.evidence_log` rows.
///
/// Uses the `pg_ripple.evidence_log_retention` GUC (default: `'1 year'`).
/// Returns the count of rows deleted.
pub(crate) fn vacuum_evidence_log_impl() -> i64 {
    ensure_bayesian_catalog();

    let retention_raw = crate::gucs::datalog::EVIDENCE_LOG_RETENTION
        .get()
        .and_then(|s| s.to_str().ok().map(|s| s.to_owned()))
        .unwrap_or_else(|| "1 year".to_owned());

    // Sanitize: keep only alphanumeric, space, dash, and dot — valid for interval literals.
    let retention: String = retention_raw
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '.')
        .collect();

    Spi::get_one::<i64>(&format!(
        "WITH deleted AS ( \
           DELETE FROM _pg_ripple.evidence_log \
           WHERE event_at < now() - INTERVAL '{retention}' \
           RETURNING 1 \
         ) SELECT COUNT(*)::bigint FROM deleted"
    ))
    .unwrap_or(None)
    .unwrap_or(0)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bayesian_update_increases_confidence() {
        let prior = 0.5;
        let posterior = bayesian_update(prior, 3.0);
        assert!(posterior > prior, "LR > 1 should increase confidence");
    }

    #[test]
    fn bayesian_update_decreases_confidence() {
        let prior = 0.8;
        let posterior = bayesian_update(prior, 0.2);
        assert!(posterior < prior, "LR < 1 should decrease confidence");
    }

    #[test]
    fn bayesian_update_clamps_to_range() {
        // Extremely high likelihood ratio — should not reach 1.0.
        let high = bayesian_update(0.9999, 1_000_000.0);
        assert!(high <= 0.999);
        assert!(high >= 0.001);

        // Near-zero likelihood ratio — should not reach 0.0.
        let low = bayesian_update(0.001, 0.000_001);
        assert!(low >= 0.001);
        assert!(low <= 0.999);
    }

    #[test]
    fn bayesian_update_neutral_likelihood() {
        // LR = 1.0 should leave confidence unchanged.
        let prior = 0.6;
        let posterior = bayesian_update(prior, 1.0);
        assert!((posterior - prior).abs() < 1e-12);
    }

    #[test]
    fn noisy_or_update_increases_with_positive_lr() {
        let prior = 0.5;
        let posterior = noisy_or_update(prior, 3.0);
        assert!(posterior > prior);
    }
}
