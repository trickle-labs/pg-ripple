//! What-if reasoning (hypothetical inference) — v0.102.0
//!
//! `hypothetical_inference_impl` runs the named rule set on a speculative copy
//! of the graph (with user-supplied asserts and retracts applied) and returns
//! the diff: which triples would be *newly derived* and which *no longer derived*.
//!
//! # Isolation guarantee
//!
//! All speculative changes are wrapped in a PostgreSQL internal sub-transaction
//! (`BeginInternalSubTransaction` / `RollbackAndReleaseCurrentSubTransaction`).
//! The sub-transaction is always rolled back, so the real VP tables are never
//! modified.
//!
//! # Algorithm
//!
//! 1. Collect all head predicate IDs for the named rule set.
//! 2. Snapshot **all** current triples for those predicates (regardless of
//!    `source`) — this is the "before" set.
//! 3. Open an internal sub-transaction.
//! 4. Apply hypothetical asserts via `insert_encoded_triple`.
//! 5. Apply hypothetical retracts via `delete_triple`.
//! 6. Delete **all** current triples for head predicates so that inference
//!    starts from a clean slate.
//! 7. Re-run inference (`run_inference`).
//! 8. Snapshot the new state — this is the "after" set.
//! 9. Roll back the sub-transaction (restores the real graph).
//! 10. Compute diff:
//!     - `derived`   = `after − before` minus the explicitly asserted triples
//!                     that happen to be head-predicate triples (those are
//!                     inputs, not conclusions).
//!     - `retracted` = `before − after`.
//! 11. Return `{"derived": [...], "retracted": [...]}`.

use std::collections::HashSet;

use serde_json::{Value, json};

use crate::gucs::datalog::HYPOTHETICAL_MAX_ASSERTIONS;
use crate::storage::dictionary_io::encode_rdf_term;
use crate::storage::ops::{delete_triple, insert_encoded_triple};

// ─── Encoded triple (s, p, o, g) ─────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct EncodedTriple {
    s: i64,
    p: i64,
    o: i64,
    g: i64,
}

// ─── JSON triple helper ───────────────────────────────────────────────────────

/// Parse a JSON object of shape `{"s": "…", "p": "…", "o": "…", "g": "…?"}`
/// into a raw-string tuple `(s_str, p_str, o_str, g_id)`.
fn parse_json_triple(obj: &Value) -> Option<(String, String, String, i64)> {
    let s = obj.get("s")?.as_str()?.to_owned();
    let p = obj.get("p")?.as_str()?.to_owned();
    let o = obj.get("o")?.as_str()?.to_owned();
    let g = obj.get("g").and_then(|v| v.as_i64()).unwrap_or(0);
    Some((s, p, o, g))
}

// ─── Snapshot helpers ─────────────────────────────────────────────────────────

/// Snapshot **all** triples (any source) for the given head predicates.
///
/// For predicates with a dedicated HTAP VP table we read from the
/// UNION-ALL view (`vp_{id}`); for rare predicates we read `vp_rare`.
/// Returns a set of encoded (s, p, o, g) quads.
fn snapshot_all_triples(head_preds: &[i64]) -> HashSet<EncodedTriple> {
    let mut result = HashSet::new();

    for &pred_id in head_preds {
        // Check for dedicated VP table.
        let has_dedicated = pgrx::Spi::get_one_with_args::<bool>(
            "SELECT EXISTS(\
                 SELECT 1 FROM _pg_ripple.predicates \
                 WHERE id = $1 AND table_oid IS NOT NULL\
             )",
            &[pgrx::datum::DatumWithOid::from(pred_id)],
        )
        .unwrap_or(None)
        .unwrap_or(false);

        if has_dedicated {
            // Read from dedicated VP view (unions main + delta).
            let rows: Vec<(i64, i64, i64)> = pgrx::Spi::connect(|c| {
                let sql = format!(
                    "SELECT s, o, g \
                     FROM _pg_ripple.vp_{pred_id}"
                );
                c.select(&sql, None, &[])
                    .unwrap_or_else(|e| pgrx::error!("snapshot_all_triples SPI error: {e}"))
                    .map(|row| {
                        let s = row.get::<i64>(1).ok().flatten().unwrap_or(0);
                        let o = row.get::<i64>(2).ok().flatten().unwrap_or(0);
                        let g = row.get::<i64>(3).ok().flatten().unwrap_or(0);
                        (s, o, g)
                    })
                    .collect()
            });
            for (s, o, g) in rows {
                result.insert(EncodedTriple {
                    s,
                    p: pred_id,
                    o,
                    g,
                });
            }

            // Also check vp_rare for any un-promoted rows.
            let rare_rows: Vec<(i64, i64, i64)> = pgrx::Spi::connect(|c| {
                c.select(
                    "SELECT s, o, g FROM _pg_ripple.vp_rare WHERE p = $1",
                    None,
                    &[pgrx::datum::DatumWithOid::from(pred_id)],
                )
                .unwrap_or_else(|e| pgrx::error!("snapshot vp_rare SPI error: {e}"))
                .map(|row| {
                    let s = row.get::<i64>(1).ok().flatten().unwrap_or(0);
                    let o = row.get::<i64>(2).ok().flatten().unwrap_or(0);
                    let g = row.get::<i64>(3).ok().flatten().unwrap_or(0);
                    (s, o, g)
                })
                .collect()
            });
            for (s, o, g) in rare_rows {
                result.insert(EncodedTriple {
                    s,
                    p: pred_id,
                    o,
                    g,
                });
            }
        } else {
            // Rare predicate — only in vp_rare.
            let rows: Vec<(i64, i64, i64)> = pgrx::Spi::connect(|c| {
                c.select(
                    "SELECT s, o, g FROM _pg_ripple.vp_rare WHERE p = $1",
                    None,
                    &[pgrx::datum::DatumWithOid::from(pred_id)],
                )
                .unwrap_or_else(|e| pgrx::error!("snapshot vp_rare SPI error: {e}"))
                .map(|row| {
                    let s = row.get::<i64>(1).ok().flatten().unwrap_or(0);
                    let o = row.get::<i64>(2).ok().flatten().unwrap_or(0);
                    let g = row.get::<i64>(3).ok().flatten().unwrap_or(0);
                    (s, o, g)
                })
                .collect()
            });
            for (s, o, g) in rows {
                result.insert(EncodedTriple {
                    s,
                    p: pred_id,
                    o,
                    g,
                });
            }
        }
    }

    result
}

// ─── Clear helpers ────────────────────────────────────────────────────────────

/// Delete **all** triples for the given predicates (regardless of source).
///
/// This is called inside the speculative sub-transaction before re-running
/// inference from a clean slate.  The sub-transaction rollback ensures the real
/// graph is not affected.
fn clear_all_for_predicates(head_preds: &[i64]) {
    for &pred_id in head_preds {
        let has_dedicated = pgrx::Spi::get_one_with_args::<bool>(
            "SELECT EXISTS(\
                 SELECT 1 FROM _pg_ripple.predicates \
                 WHERE id = $1 AND table_oid IS NOT NULL\
             )",
            &[pgrx::datum::DatumWithOid::from(pred_id)],
        )
        .unwrap_or(None)
        .unwrap_or(false);

        if has_dedicated {
            // Clear delta and main tables.
            let delta = format!("_pg_ripple.vp_{pred_id}_delta");
            let main = format!("_pg_ripple.vp_{pred_id}_main");
            pgrx::Spi::run(&format!("DELETE FROM {delta}"))
                .unwrap_or_else(|e| pgrx::warning!("clear delta SPI error: {e}"));
            pgrx::Spi::run(&format!("DELETE FROM {main}"))
                .unwrap_or_else(|e| pgrx::warning!("clear main SPI error: {e}"));
        }

        // Also clear vp_rare for this predicate (covers rare and un-promoted rows).
        pgrx::Spi::run_with_args(
            "DELETE FROM _pg_ripple.vp_rare WHERE p = $1",
            &[pgrx::datum::DatumWithOid::from(pred_id)],
        )
        .unwrap_or_else(|e| pgrx::warning!("clear vp_rare SPI error: {e}"));
    }
}

// ─── Decode helper ────────────────────────────────────────────────────────────

/// Decode an `EncodedTriple` back to a JSON object `{"s":…,"p":…,"o":…,"g":…}`.
fn decode_triple(t: &EncodedTriple) -> Value {
    let s = crate::dictionary::decode(t.s).unwrap_or_else(|| format!("_:{}", t.s));
    let p = crate::dictionary::decode(t.p).unwrap_or_else(|| format!("_:{}", t.p));
    let o = crate::dictionary::decode(t.o).unwrap_or_else(|| format!("_:{}", t.o));
    json!({"s": s, "p": p, "o": o, "g": t.g})
}

// ─── Main entry point ─────────────────────────────────────────────────────────

pub fn hypothetical_inference_impl(
    hypotheses: serde_json::Value,
    rules: &str,
) -> serde_json::Value {
    crate::datalog::ensure_catalog();

    // ── 1. Enforce max-assertions limit ────────────────────────────────────────
    let max_assertions = HYPOTHETICAL_MAX_ASSERTIONS.get() as usize;
    let assert_list: Vec<&Value> = hypotheses
        .get("assert")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().collect())
        .unwrap_or_default();
    let retract_list: Vec<&Value> = hypotheses
        .get("retract")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().collect())
        .unwrap_or_default();

    if assert_list.len() > max_assertions {
        pgrx::error!(
            "PT0450: hypothetical_inference called with {} assertions, \
             which exceeds pg_ripple.hypothetical_max_assertions = {}",
            assert_list.len(),
            max_assertions
        );
    }

    // ── 2. Collect head predicate IDs ──────────────────────────────────────────
    let head_preds: Vec<i64> = pgrx::Spi::connect(|c| {
        c.select(
            "SELECT DISTINCT head_pred \
             FROM _pg_ripple.rules \
             WHERE rule_set = $1 AND active = true AND head_pred IS NOT NULL",
            None,
            &[pgrx::datum::DatumWithOid::from(rules)],
        )
        .unwrap_or_else(|e| pgrx::error!("hypothetical_inference: rule query SPI error: {e}"))
        .map(|row| row.get::<i64>(1).ok().flatten().unwrap_or(0))
        .filter(|&id| id != 0)
        .collect()
    });

    if head_preds.is_empty() {
        // No active rules — nothing to derive.
        return json!({"derived": [], "retracted": []});
    }

    // ── 3. Snapshot "before" ────────────────────────────────────────────────────
    let before: HashSet<EncodedTriple> = snapshot_all_triples(&head_preds);

    // Encode asserted triples now so we can track which ones are head-predicate
    // assertions (to exclude them from the "derived" output).
    let mut asserted_head_triples: HashSet<EncodedTriple> = HashSet::new();
    let mut parsed_asserts: Vec<(i64, i64, i64, i64)> = Vec::new();
    for val in &assert_list {
        if let Some((s_str, p_str, o_str, g)) = parse_json_triple(val) {
            let s_id = encode_rdf_term(&s_str);
            let p_id = encode_rdf_term(&p_str);
            let o_id = encode_rdf_term(&o_str);
            parsed_asserts.push((s_id, p_id, o_id, g));
            if head_preds.contains(&p_id) {
                asserted_head_triples.insert(EncodedTriple {
                    s: s_id,
                    p: p_id,
                    o: o_id,
                    g,
                });
            }
        }
    }

    let mut parsed_retracts: Vec<(String, String, String, i64)> = Vec::new();
    for val in &retract_list {
        if let Some((s_str, p_str, o_str, g)) = parse_json_triple(val) {
            parsed_retracts.push((s_str, p_str, o_str, g));
        }
    }

    // ── 4. Open internal sub-transaction ───────────────────────────────────────
    // SAFETY: BeginInternalSubTransaction is a PostgreSQL internal API that
    // creates a subtransaction savepoint.  We always pair it with
    // RollbackAndReleaseCurrentSubTransaction so the speculative changes never
    // escape to the outer transaction.
    unsafe {
        pgrx::pg_sys::BeginInternalSubTransaction(std::ptr::null());
    }

    // ── 5. Apply hypothetical asserts ──────────────────────────────────────────
    for (s_id, p_id, o_id, g) in &parsed_asserts {
        insert_encoded_triple(*s_id, *p_id, *o_id, *g);
    }

    // ── 6. Apply hypothetical retracts ─────────────────────────────────────────
    for (s_str, p_str, o_str, g) in &parsed_retracts {
        delete_triple(s_str, p_str, o_str, *g);
    }

    // ── 7. Clear all head-predicate triples (clean slate for re-inference) ─────
    clear_all_for_predicates(&head_preds);

    // ── 8. Re-run inference ────────────────────────────────────────────────────
    crate::datalog::run_inference(rules);

    // ── 9. Snapshot "after" ─────────────────────────────────────────────────────
    let after: HashSet<EncodedTriple> = snapshot_all_triples(&head_preds);

    // ── 10. Roll back — restore the real graph ─────────────────────────────────
    // SAFETY: Must be called after BeginInternalSubTransaction to discard all
    // speculative writes.
    unsafe {
        pgrx::pg_sys::RollbackAndReleaseCurrentSubTransaction();
    }

    // ── 11. Compute diff ───────────────────────────────────────────────────────
    // "derived" = triples that appeared after inference but did not exist
    // before, *excluding* triples that were explicitly asserted (they are
    // inputs, not conclusions).
    let mut derived: Vec<Value> = after
        .difference(&before)
        .filter(|t| !asserted_head_triples.contains(t))
        .map(decode_triple)
        .collect();
    derived.sort_by_key(|v| v.to_string());

    // "retracted" = triples that existed before but no longer exist after the
    // hypothetical changes and re-inference.
    let mut retracted: Vec<Value> = before.difference(&after).map(decode_triple).collect();
    retracted.sort_by_key(|v| v.to_string());

    json!({"derived": derived, "retracted": retracted})
}
