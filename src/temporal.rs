//! Temporal RDF query support.
//!
//! # v0.58.0 — Point-in-time queries
//!
//! `point_in_time(ts)` sets a session-local threshold so that all subsequent
//! SPARQL queries only see triples whose statement ID (`i`) was assigned before
//! `ts`.  The threshold is stored in the GUC-backed session variable
//! `_pg_ripple.pit_threshold`.
//!
//! `_pg_ripple.statement_id_timeline (sid BIGINT, inserted_at TIMESTAMPTZ)` maps
//! statement IDs to wall-clock insertion timestamps.  An AFTER INSERT trigger on
//! every VP delta table populates this table.
//!
//! # v0.106.0 — Temporal Fact Store & Basic Operators
//!
//! Dedicated `_pg_ripple.temporal_facts` table stores facts with validity
//! intervals `(valid_from, valid_to)`.  Predicates must be explicitly registered
//! as temporal via `pg_ripple.mark_temporal()` before temporal facts can be
//! inserted.
//!
//! Temporal operators for Datalog rules: `AFTER`, `BEFORE`, `DURING`.
//! SPARQL function: `pg:temporal_window(?subject, ?predicate, ?start, ?end)`.
//! SHACL constraint: `sh:validFor "P1Y"^^xsd:duration`.
//!
//! Error catalog:
//! - PT0430: predicate already registered with a different data model
//! - PT0431: cannot unmark a predicate that still has temporal facts
//! - PT0432: predicate is not registered as temporal

use pgrx::prelude::*;

// ─── Session-local PIT threshold ─────────────────────────────────────────────

/// Set the session-local point-in-time threshold.
///
/// All subsequent SPARQL queries will only see triples whose statement ID was
/// assigned before or at `ts`.  Pass `NULL` to clear the threshold.
///
/// # Errors
/// Raises an error if `_pg_ripple.statement_id_timeline` is not accessible
/// (extension not installed or schema search path issue).
#[pg_extern(schema = "pg_ripple")]
pub fn point_in_time(ts: pgrx::datum::TimestampWithTimeZone) {
    // Find the maximum SID inserted before `ts`.
    let threshold: i64 = Spi::get_one_with_args::<i64>(
        "SELECT COALESCE(MAX(sid), 0) FROM _pg_ripple.statement_id_timeline WHERE inserted_at <= $1",
        &[pgrx::datum::DatumWithOid::from(ts)],
    )
    .unwrap_or_else(|e| pgrx::error!("point_in_time: timeline query error: {e}"))
    .unwrap_or(0);

    // Store as a session-local GUC that the SQL generator reads.
    Spi::run_with_args(
        &format!("SET LOCAL \"_pg_ripple.pit_threshold\" = '{threshold}'"),
        &[],
    )
    .unwrap_or_else(|e| pgrx::error!("point_in_time: SET LOCAL error: {e}"));
}

/// Clear the session-local point-in-time threshold.
///
/// After calling this function, subsequent SPARQL queries see all triples.
#[pg_extern(schema = "pg_ripple")]
pub fn clear_point_in_time() {
    Spi::run_with_args("SET LOCAL \"_pg_ripple.pit_threshold\" = '0'", &[])
        .unwrap_or_else(|e| pgrx::error!("clear_point_in_time: SET LOCAL error: {e}"));
}

/// Return information about the current point-in-time threshold.
///
/// Returns a single row with `(threshold_sid BIGINT, approximate_ts TIMESTAMPTZ)`.
/// Returns `(0, NULL)` when no threshold is set.
#[pg_extern(schema = "pg_ripple")]
pub fn point_in_time_info() -> TableIterator<
    'static,
    (
        name!(threshold_sid, i64),
        name!(approximate_ts, Option<pgrx::datum::TimestampWithTimeZone>),
    ),
> {
    // Read the current threshold from the session GUC.
    let threshold: i64 = Spi::get_one::<i64>(
        "SELECT COALESCE(NULLIF(current_setting('_pg_ripple.pit_threshold', true), '')::bigint, 0)",
    )
    .unwrap_or(None)
    .unwrap_or(0);

    // Reverse-lookup the approximate insertion timestamp for this SID.
    let approx_ts: Option<pgrx::datum::TimestampWithTimeZone> = if threshold > 0 {
        Spi::get_one_with_args::<pgrx::datum::TimestampWithTimeZone>(
            "SELECT inserted_at FROM _pg_ripple.statement_id_timeline WHERE sid = $1",
            &[pgrx::datum::DatumWithOid::from(threshold)],
        )
        .unwrap_or(None)
    } else {
        None
    };

    TableIterator::once((threshold, approx_ts))
}

// ─── Timeline table initialisation ───────────────────────────────────────────

/// Create `_pg_ripple.statement_id_timeline` and the associated trigger function.
///
/// Called from `initialize_schema()` (idempotent via `IF NOT EXISTS`).
pub fn initialize_timeline_schema() {
    // Timeline table.
    Spi::run_with_args(
        "CREATE TABLE IF NOT EXISTS _pg_ripple.statement_id_timeline ( \
             sid         BIGINT      NOT NULL PRIMARY KEY, \
             inserted_at TIMESTAMPTZ NOT NULL DEFAULT now() \
         )",
        &[],
    )
    .unwrap_or_else(|e| pgrx::warning!("statement_id_timeline creation: {e}"));

    // BRIN index on inserted_at for time-range scans.
    Spi::run_with_args(
        "CREATE INDEX IF NOT EXISTS idx_statement_id_timeline_ts \
         ON _pg_ripple.statement_id_timeline USING BRIN (inserted_at)",
        &[],
    )
    .unwrap_or_else(|e| pgrx::warning!("statement_id_timeline BRIN index: {e}"));

    // Trigger function that records each new SID with a timestamp.
    Spi::run_with_args(
        "CREATE OR REPLACE FUNCTION _pg_ripple.record_statement_timestamp() \
         RETURNS trigger LANGUAGE plpgsql AS $$ \
         BEGIN \
             INSERT INTO _pg_ripple.statement_id_timeline (sid, inserted_at) \
             VALUES (NEW.i, now()) \
             ON CONFLICT (sid) DO NOTHING; \
             RETURN NEW; \
         END; \
         $$",
        &[],
    )
    .unwrap_or_else(|e| pgrx::warning!("statement timeline trigger function: {e}"));

    // Also attach the trigger to vp_rare so that non-promoted predicates are
    // tracked too (the trigger is idempotent via CREATE ... IF NOT EXISTS).
    Spi::run_with_args(
        "DO $$ BEGIN \
           IF NOT EXISTS ( \
             SELECT 1 FROM pg_trigger t \
             JOIN pg_class c ON c.oid = t.tgrelid \
             JOIN pg_namespace n ON n.oid = c.relnamespace \
             WHERE n.nspname = '_pg_ripple' AND c.relname = 'vp_rare' \
               AND t.tgname = 'trg_timeline_vp_rare' \
           ) THEN \
             EXECUTE 'CREATE TRIGGER trg_timeline_vp_rare \
                      AFTER INSERT ON _pg_ripple.vp_rare \
                      FOR EACH ROW \
                      EXECUTE FUNCTION _pg_ripple.record_statement_timestamp()'; \
           END IF; \
         END $$",
        &[],
    )
    .unwrap_or_else(|e| pgrx::warning!("vp_rare timeline trigger: {e}"));
}

/// Attach the `record_statement_timestamp` trigger to a VP delta table.
///
/// Safe to call multiple times — uses `IF NOT EXISTS`.
pub fn attach_timeline_trigger(pred_id: i64) {
    let table = format!("_pg_ripple.vp_{pred_id}_delta");
    let trigger_name = format!("trg_timeline_vp_{pred_id}_delta");
    let sql = format!(
        "DO $$ BEGIN \
           IF NOT EXISTS ( \
             SELECT 1 FROM pg_trigger t \
             JOIN pg_class c ON c.oid = t.tgrelid \
             JOIN pg_namespace n ON n.oid = c.relnamespace \
             WHERE n.nspname = '_pg_ripple' AND c.relname = 'vp_{pred_id}_delta' \
               AND t.tgname = '{trigger_name}' \
           ) THEN \
             EXECUTE 'CREATE TRIGGER {trigger_name} \
                      AFTER INSERT ON {table} \
                      FOR EACH ROW \
                      EXECUTE FUNCTION _pg_ripple.record_statement_timestamp()'; \
           END IF; \
         END $$"
    );
    Spi::run_with_args(&sql, &[])
        .unwrap_or_else(|e| pgrx::warning!("attach_timeline_trigger vp_{pred_id}: {e}"));
}

/// Return the current session PIT threshold (0 = no filter).
///
/// Used by the SQL generator to append `AND i <= $threshold` to VP table scans.
// Q15-01: internal API field; kept for public API surface or future extension consumers.
#[allow(dead_code)]
pub fn current_pit_threshold() -> i64 {
    Spi::get_one::<i64>(
        "SELECT COALESCE(current_setting('_pg_ripple.pit_threshold', true)::bigint, 0)",
    )
    .unwrap_or(None)
    .unwrap_or(0)
}

// ─── v0.106.0 — Temporal Fact Store ──────────────────────────────────────────

/// Create `_pg_ripple.temporal_facts`, `_pg_ripple.temporal_predicates`, and
/// their indexes.
///
/// Called from `initialize_schema()` (idempotent via `IF NOT EXISTS`).
pub fn initialize_temporal_store_schema() {
    // temporal_predicates registry.
    Spi::run_with_args(
        "CREATE TABLE IF NOT EXISTS _pg_ripple.temporal_predicates ( \
             predicate_id BIGINT PRIMARY KEY, \
             data_model   TEXT NOT NULL \
                          CHECK (data_model IN ('snapshot', 'versioned')) \
         )",
        &[],
    )
    .unwrap_or_else(|e| pgrx::warning!("temporal_predicates creation: {e}"));

    // temporal_facts table — no changes to VP table schemas.
    Spi::run_with_args(
        "CREATE TABLE IF NOT EXISTS _pg_ripple.temporal_facts ( \
             s          BIGINT      NOT NULL, \
             p          BIGINT      NOT NULL, \
             o          BIGINT      NOT NULL, \
             g          BIGINT      NOT NULL DEFAULT 0, \
             valid_from TIMESTAMPTZ NOT NULL, \
             valid_to   TIMESTAMPTZ \
         )",
        &[],
    )
    .unwrap_or_else(|e| pgrx::warning!("temporal_facts creation: {e}"));

    // B-tree on (s, p, valid_from, valid_to) for subject-scoped temporal queries.
    Spi::run_with_args(
        "CREATE INDEX IF NOT EXISTS idx_temporal_facts_s_p_vf_vt \
         ON _pg_ripple.temporal_facts (s, p, valid_from, valid_to)",
        &[],
    )
    .unwrap_or_else(|e| pgrx::warning!("temporal_facts s,p index: {e}"));

    // B-tree on (p, valid_from, valid_to) for predicate-scoped temporal scans.
    Spi::run_with_args(
        "CREATE INDEX IF NOT EXISTS idx_temporal_facts_p_vf_vt \
         ON _pg_ripple.temporal_facts (p, valid_from, valid_to)",
        &[],
    )
    .unwrap_or_else(|e| pgrx::warning!("temporal_facts p index: {e}"));

    // Partial B-tree on (valid_from, valid_to) WHERE valid_to IS NULL for
    // currently-valid (open-ended interval) facts.
    Spi::run_with_args(
        "CREATE INDEX IF NOT EXISTS idx_temporal_facts_open \
         ON _pg_ripple.temporal_facts (valid_from, valid_to) \
         WHERE valid_to IS NULL",
        &[],
    )
    .unwrap_or_else(|e| pgrx::warning!("temporal_facts open-interval index: {e}"));
}

/// Register a predicate as temporal.
///
/// # Errors
/// - PT0430: predicate is already registered with a **different** data model.
///   Re-registering with the same model is a no-op.
#[pg_extern(schema = "pg_ripple")]
pub fn mark_temporal(predicate_iri: &str, data_model: default!(String, "'snapshot'")) {
    let data_model = data_model.as_str();
    if !matches!(data_model, "snapshot" | "versioned") {
        pgrx::error!(
            "mark_temporal: invalid data_model '{}'; expected 'snapshot' or 'versioned'",
            data_model
        );
    }

    // Encode the predicate IRI to a dictionary ID.
    let pred_id = crate::dictionary::encode(predicate_iri, 0 /* IRI kind */);

    // Check for an existing registration.
    let existing: Option<String> = Spi::get_one_with_args::<String>(
        "SELECT data_model FROM _pg_ripple.temporal_predicates WHERE predicate_id = $1",
        &[pgrx::datum::DatumWithOid::from(pred_id)],
    )
    .unwrap_or(None);

    if let Some(existing_model) = existing {
        if existing_model != data_model {
            // PT0430: already registered with a different model.
            pgrx::error!(
                "PT0430: mark_temporal: predicate '{}' is already registered with data model '{}'",
                predicate_iri,
                existing_model
            );
        }
        // Same model — idempotent, nothing to do.
        return;
    }

    Spi::run_with_args(
        "INSERT INTO _pg_ripple.temporal_predicates (predicate_id, data_model) \
         VALUES ($1, $2) ON CONFLICT (predicate_id) DO NOTHING",
        &[
            pgrx::datum::DatumWithOid::from(pred_id),
            pgrx::datum::DatumWithOid::from(data_model),
        ],
    )
    .unwrap_or_else(|e| pgrx::error!("mark_temporal: insert error: {e}"));
}

/// Unregister a predicate as temporal.
///
/// # Errors
/// - PT0431: predicate still has existing temporal facts — delete them first.
#[pg_extern(schema = "pg_ripple")]
pub fn unmark_temporal(predicate_iri: &str) {
    let pred_id = crate::dictionary::encode(predicate_iri, 0 /* IRI kind */);

    // Count existing temporal facts for this predicate.
    let count: i64 = Spi::get_one_with_args::<i64>(
        "SELECT COUNT(*)::bigint FROM _pg_ripple.temporal_facts WHERE p = $1",
        &[pgrx::datum::DatumWithOid::from(pred_id)],
    )
    .unwrap_or(None)
    .unwrap_or(0);

    if count > 0 {
        // PT0431: cannot unregister when temporal facts still exist.
        pgrx::error!(
            "PT0431: unmark_temporal: predicate '{}' has {} existing temporal facts — delete them first",
            predicate_iri,
            count
        );
    }

    Spi::run_with_args(
        "DELETE FROM _pg_ripple.temporal_predicates WHERE predicate_id = $1",
        &[pgrx::datum::DatumWithOid::from(pred_id)],
    )
    .unwrap_or_else(|e| pgrx::error!("unmark_temporal: delete error: {e}"));
}

/// Insert a temporal fact.
///
/// For `snapshot` model: if an open-ended row already exists for the same
/// `(s, p, o, g)`, closes it by setting `valid_to = valid_from` of the new row.
///
/// For `versioned` model: always inserts a new row regardless of existing rows.
///
/// Returns the dictionary-encoded statement reference (s XOR p XOR o) as BIGINT
/// for convenience.
///
/// # Errors
/// - PT0432: predicate is not registered as temporal.
#[pg_extern(schema = "pg_ripple")]
pub fn insert_triple_temporal(
    subject: &str,
    predicate: &str,
    object: &str,
    valid_from: pgrx::datum::TimestampWithTimeZone,
    valid_to: default!(Option<pgrx::datum::TimestampWithTimeZone>, "NULL"),
    graph: default!(Option<String>, "NULL"),
) -> i64 {
    let s_id = crate::dictionary::encode(subject, 0);
    let p_id = crate::dictionary::encode(predicate, 0);
    let o_id = crate::dictionary::encode(object, 0);
    let g_id: i64 = graph
        .as_deref()
        .map(|g| crate::dictionary::encode(g, 0))
        .unwrap_or(0);

    // Check the predicate is registered as temporal.
    let data_model: Option<String> = Spi::get_one_with_args::<String>(
        "SELECT data_model FROM _pg_ripple.temporal_predicates WHERE predicate_id = $1",
        &[pgrx::datum::DatumWithOid::from(p_id)],
    )
    .unwrap_or(None);

    let data_model = match data_model {
        Some(m) => m,
        None => {
            pgrx::error!(
                "PT0432: insert_triple_temporal: predicate '{}' is not registered as temporal — call mark_temporal() first",
                predicate
            );
        }
    };

    // For 'snapshot' model: close any existing open-ended row for (s, p, g),
    // regardless of the object value (snapshot = at most one current value).
    if data_model == "snapshot" {
        Spi::run_with_args(
            "UPDATE _pg_ripple.temporal_facts \
             SET valid_to = $1 \
             WHERE s = $2 AND p = $3 AND g = $4 AND valid_to IS NULL",
            &[
                pgrx::datum::DatumWithOid::from(valid_from),
                pgrx::datum::DatumWithOid::from(s_id),
                pgrx::datum::DatumWithOid::from(p_id),
                pgrx::datum::DatumWithOid::from(g_id),
            ],
        )
        .unwrap_or_else(|e| pgrx::error!("insert_triple_temporal: close snapshot error: {e}"));
    }

    // Insert the new temporal fact.
    Spi::run_with_args(
        "INSERT INTO _pg_ripple.temporal_facts (s, p, o, g, valid_from, valid_to) \
         VALUES ($1, $2, $3, $4, $5, $6)",
        &[
            pgrx::datum::DatumWithOid::from(s_id),
            pgrx::datum::DatumWithOid::from(p_id),
            pgrx::datum::DatumWithOid::from(o_id),
            pgrx::datum::DatumWithOid::from(g_id),
            pgrx::datum::DatumWithOid::from(valid_from),
            pgrx::datum::DatumWithOid::from(valid_to),
        ],
    )
    .unwrap_or_else(|e| pgrx::error!("insert_triple_temporal: insert error: {e}"));

    // Return a reference value combining s, p, o for caller convenience.
    s_id ^ p_id ^ o_id
}

/// Return `true` if the given predicate is registered as temporal.
///
/// Used by the query routing layer to dispatch to `temporal_facts` vs VP tables.
pub fn is_temporal_predicate(pred_id: i64) -> bool {
    Spi::get_one_with_args::<bool>(
        "SELECT EXISTS(SELECT 1 FROM _pg_ripple.temporal_predicates WHERE predicate_id = $1)",
        &[pgrx::datum::DatumWithOid::from(pred_id)],
    )
    .unwrap_or(None)
    .unwrap_or(false)
}

/// Return a SQL table expression for reading temporal facts of `pred_id`.
///
/// Used by the Datalog compiler when routing temporal predicate atoms.
/// The returned expression exposes columns `(s, o, g, valid_from, valid_to)`.
// Q15-01: internal API; kept for future compiler paths and external consumers.
#[allow(dead_code)]
pub fn temporal_read_expr(pred_id: i64) -> String {
    format!(
        "(SELECT s, o, g, valid_from, valid_to \
          FROM _pg_ripple.temporal_facts WHERE p = {pred_id})"
    )
}

/// Return a SQL table expression for reading temporal facts with a time filter.
///
/// `filter_sql` is a raw WHERE fragment such as `valid_from > $ts`.
pub fn temporal_read_expr_filtered(pred_id: i64, filter_sql: &str) -> String {
    format!(
        "(SELECT s, o, g, valid_from, valid_to \
          FROM _pg_ripple.temporal_facts WHERE p = {pred_id} AND ({filter_sql}))"
    )
}

/// `pg_ripple.temporal_window(subject, predicate, start_ts, end_ts)` — SPARQL
/// filter function.
///
/// Returns `true` if a temporal fact for `(subject, predicate, *)` exists with
/// a validity interval overlapping `[start_ts, end_ts]`.
///
/// Used internally by the SPARQL expression translator; not intended for direct
/// SQL use.
#[pg_extern(schema = "pg_ripple")]
pub fn temporal_window(
    subject_iri: &str,
    predicate_iri: &str,
    start_ts: pgrx::datum::TimestampWithTimeZone,
    end_ts: pgrx::datum::TimestampWithTimeZone,
) -> bool {
    let s_id = crate::dictionary::encode(subject_iri, 0);
    let p_id = crate::dictionary::encode(predicate_iri, 0);

    Spi::get_one_with_args::<bool>(
        "SELECT EXISTS( \
           SELECT 1 FROM _pg_ripple.temporal_facts \
           WHERE s = $1 AND p = $2 \
             AND tstzrange(valid_from, valid_to, '[)') && tstzrange($3, $4, '[)') \
         )",
        &[
            pgrx::datum::DatumWithOid::from(s_id),
            pgrx::datum::DatumWithOid::from(p_id),
            pgrx::datum::DatumWithOid::from(start_ts),
            pgrx::datum::DatumWithOid::from(end_ts),
        ],
    )
    .unwrap_or(None)
    .unwrap_or(false)
}
