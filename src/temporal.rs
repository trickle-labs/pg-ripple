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
/// When `time_zone` is provided (e.g. `'America/New_York'`), the lookup query
/// uses `$1 AT TIME ZONE $2` to reinterpret the timestamp before comparison,
/// allowing callers to supply a wall-clock time in a named time zone.
///
/// # Errors
/// Raises an error if `_pg_ripple.statement_id_timeline` is not accessible
/// (extension not installed or schema search path issue).
#[pg_extern(schema = "pg_ripple")]
pub fn point_in_time(
    ts: pgrx::datum::TimestampWithTimeZone,
    time_zone: default!(Option<String>, "NULL"),
) {
    // Build the comparison expression: apply AT TIME ZONE if requested.
    let threshold: i64 = if let Some(ref tz) = time_zone {
        let tz_safe = tz.replace('\'', "''");
        Spi::get_one_with_args::<i64>(
            &format!(
                "SELECT COALESCE(MAX(sid), 0) FROM _pg_ripple.statement_id_timeline \
                 WHERE inserted_at <= ($1 AT TIME ZONE '{tz_safe}')"
            ),
            &[pgrx::datum::DatumWithOid::from(ts)],
        )
        .unwrap_or_else(|e| pgrx::error!("point_in_time: timeline query error: {e}"))
        .unwrap_or(0)
    } else {
        Spi::get_one_with_args::<i64>(
            "SELECT COALESCE(MAX(sid), 0) FROM _pg_ripple.statement_id_timeline WHERE inserted_at <= $1",
            &[pgrx::datum::DatumWithOid::from(ts)],
        )
        .unwrap_or_else(|e| pgrx::error!("point_in_time: timeline query error: {e}"))
        .unwrap_or(0)
    };

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
/// # Parameters
/// - `predicate_iri`: the predicate IRI to register.
/// - `data_model`: `'snapshot'` (default) or `'versioned'`.
/// - `time_zone`: optional default time zone (e.g. `'UTC'`, `'America/New_York'`)
///   stored in `_pg_ripple.temporal_predicates.default_tz`.  Used by temporal
///   query helpers to interpret validity timestamps.
///
/// # Errors
/// - PT0430: predicate is already registered with a **different** data model.
///   Re-registering with the same model is a no-op.
#[pg_extern(schema = "pg_ripple")]
pub fn mark_temporal(
    predicate_iri: &str,
    data_model: default!(String, "'snapshot'"),
    time_zone: default!(Option<String>, "NULL"),
) {
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
        // Same model — update time_zone if provided, then return.
        if let Some(ref tz) = time_zone {
            Spi::run_with_args(
                "UPDATE _pg_ripple.temporal_predicates SET default_tz = $2 \
                 WHERE predicate_id = $1",
                &[
                    pgrx::datum::DatumWithOid::from(pred_id),
                    pgrx::datum::DatumWithOid::from(tz.as_str()),
                ],
            )
            .unwrap_or_else(|e| pgrx::warning!("mark_temporal: update default_tz: {e}"));
        }
        return;
    }

    Spi::run_with_args(
        "INSERT INTO _pg_ripple.temporal_predicates (predicate_id, data_model, default_tz) \
         VALUES ($1, $2, $3) ON CONFLICT (predicate_id) DO NOTHING",
        &[
            pgrx::datum::DatumWithOid::from(pred_id),
            pgrx::datum::DatumWithOid::from(data_model),
            pgrx::datum::DatumWithOid::from(time_zone.as_deref()),
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

// ─── v0.107.0 — Sequential Temporal Operators ─────────────────────────────────

/// `pg_ripple.temporal_within(subject, predicate, duration)` — WITHIN operator.
///
/// Returns `true` if `(subject, predicate, *)` holds at least once within the most
/// recent `duration` interval relative to the current transaction time.
///
/// `duration` should be an ISO 8601 duration string such as `'P3D'` (3 days) or
/// `'PT1H'` (1 hour).
///
/// Example:
/// ```sql
/// SELECT pg_ripple.temporal_within(
///     'http://example.org/Alice',
///     'http://example.org/feverReading',
///     'P3D'
/// );
/// ```
#[pg_extern(schema = "pg_ripple")]
pub fn temporal_within(subject_iri: &str, predicate_iri: &str, duration_iso: &str) -> bool {
    let s_id = crate::dictionary::encode(subject_iri, 0);
    let p_id = crate::dictionary::encode(predicate_iri, 0);

    Spi::get_one_with_args::<bool>(
        "SELECT EXISTS( \
           SELECT 1 FROM _pg_ripple.temporal_facts \
           WHERE s = $1 AND p = $2 \
             AND valid_from >= (transaction_timestamp() - $3::interval) \
         )",
        &[
            pgrx::datum::DatumWithOid::from(s_id),
            pgrx::datum::DatumWithOid::from(p_id),
            pgrx::datum::DatumWithOid::from(duration_iso),
        ],
    )
    .unwrap_or(None)
    .unwrap_or(false)
}

/// `pg_ripple.temporal_sequence(subj1, pred1, obj1, subj2, pred2, obj2, window)`
/// — SEQUENCE operator.
///
/// Returns `true` if event1 (`subj1 pred1 obj1`) occurs strictly before event2
/// (`subj2 pred2 obj2`) and both fall within `window` of each other.
///
/// Any argument that is an empty string (`''`) is treated as a wildcard (matches
/// any value in that position).
///
/// Example:
/// ```sql
/// SELECT pg_ripple.temporal_sequence(
///     'http://example.org/Alice', 'http://example.org/login', '',
///     'http://example.org/Alice', 'http://example.org/locked', '',
///     'PT1H'
/// );
/// ```
#[pg_extern(schema = "pg_ripple")]
pub fn temporal_sequence(
    subj1: &str,
    pred1: &str,
    obj1: &str,
    subj2: &str,
    pred2: &str,
    obj2: &str,
    window_iso: &str,
) -> bool {
    let p1_id = crate::dictionary::encode(pred1, 0);
    let p2_id = crate::dictionary::encode(pred2, 0);

    // Encode non-wildcard subjects and objects.
    let s1_cond = if subj1.is_empty() {
        String::new()
    } else {
        let id = crate::dictionary::encode(subj1, 0);
        format!("AND e1.s = {id}")
    };
    let o1_cond = if obj1.is_empty() {
        String::new()
    } else {
        let id = crate::dictionary::encode(obj1, 0);
        format!("AND e1.o = {id}")
    };
    let s2_cond = if subj2.is_empty() {
        String::new()
    } else {
        let id = crate::dictionary::encode(subj2, 0);
        format!("AND e2.s = {id}")
    };
    let o2_cond = if obj2.is_empty() {
        String::new()
    } else {
        let id = crate::dictionary::encode(obj2, 0);
        format!("AND e2.o = {id}")
    };

    let sql = format!(
        "SELECT EXISTS( \
           SELECT 1 \
           FROM _pg_ripple.temporal_facts e1 \
           JOIN _pg_ripple.temporal_facts e2 ON TRUE \
           WHERE e1.p = {p1_id} AND e2.p = {p2_id} \
             {s1_cond} {o1_cond} {s2_cond} {o2_cond} \
             AND e1.valid_from < e2.valid_from \
             AND e2.valid_from - e1.valid_from <= $1::interval \
         )"
    );

    Spi::get_one_with_args::<bool>(&sql, &[pgrx::datum::DatumWithOid::from(window_iso)])
        .unwrap_or(None)
        .unwrap_or(false)
}

/// `pg_ripple.temporal_consecutive(n, predicate, window)` — CONSECUTIVE operator.
///
/// Returns `true` if there exist at least `n` rows for the given `predicate` in
/// `_pg_ripple.temporal_facts` where each successive `valid_from` is strictly
/// greater than the previous and all `n` fall within `window` duration of the first.
///
/// Example:
/// ```sql
/// SELECT pg_ripple.temporal_consecutive(3, 'http://example.org/feverReading', 'P3D');
/// ```
#[pg_extern(schema = "pg_ripple")]
pub fn temporal_consecutive(n: i64, predicate_iri: &str, window_iso: &str) -> bool {
    let p_id = crate::dictionary::encode(predicate_iri, 0);

    // Use a window function to find groups of n readings for the same subject
    // that all fall within the given duration window.
    Spi::get_one_with_args::<bool>(
        "SELECT EXISTS ( \
           SELECT 1 \
           FROM ( \
             SELECT s, valid_from, \
                    ROW_NUMBER() OVER (PARTITION BY s ORDER BY valid_from) AS rn, \
                    MIN(valid_from) OVER (PARTITION BY s) AS first_vf \
             FROM _pg_ripple.temporal_facts \
             WHERE p = $1 \
           ) ranked \
           WHERE rn = $2 \
             AND valid_from - first_vf <= $3::interval \
         )",
        &[
            pgrx::datum::DatumWithOid::from(p_id),
            pgrx::datum::DatumWithOid::from(n),
            pgrx::datum::DatumWithOid::from(window_iso),
        ],
    )
    .unwrap_or(None)
    .unwrap_or(false)
}

/// `pg_ripple.retract_triple_temporal(subject, predicate, graph)` — retract a
/// temporal fact.
///
/// Semantics depend on the predicate's data model:
/// - `snapshot`: UPDATE the current open-ended row's `valid_to = transaction_timestamp()`.
/// - `versioned`: close the latest open row (INSERT a new row is not required by
///    retraction — the existing row is just closed).
///
/// Returns the number of rows affected (0 if no open row existed).
///
/// # Errors
/// - PT0432: predicate is not registered as temporal.
#[pg_extern(schema = "pg_ripple")]
pub fn retract_triple_temporal(
    subject_iri: &str,
    predicate_iri: &str,
    graph: default!(Option<String>, "NULL"),
) -> i64 {
    let s_id = crate::dictionary::encode(subject_iri, 0);
    let p_id = crate::dictionary::encode(predicate_iri, 0);
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

    if data_model.is_none() {
        pgrx::error!(
            "PT0432: retract_triple_temporal: predicate '{}' is not registered as temporal",
            predicate_iri
        );
    }

    // Close the latest open-ended row for (s, p, g).
    // Both snapshot and versioned models close the latest open row on retraction.
    let rows_affected: i64 = Spi::get_one_with_args::<i64>(
        "WITH closed AS ( \
           UPDATE _pg_ripple.temporal_facts \
           SET valid_to = transaction_timestamp() \
           WHERE s = $1 AND p = $2 AND g = $3 AND valid_to IS NULL \
           RETURNING 1 \
         ) SELECT COUNT(*)::bigint FROM closed",
        &[
            pgrx::datum::DatumWithOid::from(s_id),
            pgrx::datum::DatumWithOid::from(p_id),
            pgrx::datum::DatumWithOid::from(g_id),
        ],
    )
    .unwrap_or(None)
    .unwrap_or(0);

    rows_affected
}

// ── Allen's Interval Relation SQL Functions (v0.118.0 Feature 4) ─────────────
//
// These SQL wrappers expose the 7 Allen temporal interval relations as callable
// pg_ripple schema functions, in addition to their SPARQL FILTER function form
// (http://pg-ripple.org/functions/*) and Datalog built-in form (ALLEN_*).
//
// Naming follows Allen (1983): before, meets, overlaps, during, finishes,
// starts, equals. Each relation takes four TIMESTAMPTZ arguments:
//   (a_start, a_end, b_start, b_end)
// and returns a BOOLEAN.

/// `pg_ripple.allen_before(a_start, a_end, b_start, b_end)` — interval A ends
/// before interval B begins: `a_end <= b_start`.
#[pg_extern(schema = "pg_ripple")]
pub fn allen_before(
    a_start: pgrx::datum::TimestampWithTimeZone,
    a_end: pgrx::datum::TimestampWithTimeZone,
    b_start: pgrx::datum::TimestampWithTimeZone,
    _b_end: pgrx::datum::TimestampWithTimeZone,
) -> bool {
    let _ = a_start; // a_start not needed for this relation
    a_end <= b_start
}

/// `pg_ripple.allen_meets(a_start, a_end, b_start, b_end)` — interval A ends
/// exactly when B begins: `a_end = b_start`.
#[pg_extern(schema = "pg_ripple")]
pub fn allen_meets(
    _a_start: pgrx::datum::TimestampWithTimeZone,
    a_end: pgrx::datum::TimestampWithTimeZone,
    b_start: pgrx::datum::TimestampWithTimeZone,
    _b_end: pgrx::datum::TimestampWithTimeZone,
) -> bool {
    a_end == b_start
}

/// `pg_ripple.allen_overlaps(a_start, a_end, b_start, b_end)` — interval A
/// starts before B, they overlap, and A ends before B:
/// `a_start < b_start AND a_end > b_start AND a_end < b_end`.
#[pg_extern(schema = "pg_ripple")]
pub fn allen_overlaps(
    a_start: pgrx::datum::TimestampWithTimeZone,
    a_end: pgrx::datum::TimestampWithTimeZone,
    b_start: pgrx::datum::TimestampWithTimeZone,
    b_end: pgrx::datum::TimestampWithTimeZone,
) -> bool {
    a_start < b_start && a_end > b_start && a_end < b_end
}

/// `pg_ripple.allen_during(a_start, a_end, b_start, b_end)` — interval A is
/// entirely contained within B: `a_start > b_start AND a_end < b_end`.
#[pg_extern(schema = "pg_ripple")]
pub fn allen_during(
    a_start: pgrx::datum::TimestampWithTimeZone,
    a_end: pgrx::datum::TimestampWithTimeZone,
    b_start: pgrx::datum::TimestampWithTimeZone,
    b_end: pgrx::datum::TimestampWithTimeZone,
) -> bool {
    a_start > b_start && a_end < b_end
}

/// `pg_ripple.allen_finishes(a_start, a_end, b_start, b_end)` — interval A
/// ends at the same time as B and starts after B:
/// `a_end = b_end AND a_start > b_start`.
#[pg_extern(schema = "pg_ripple")]
pub fn allen_finishes(
    a_start: pgrx::datum::TimestampWithTimeZone,
    a_end: pgrx::datum::TimestampWithTimeZone,
    b_start: pgrx::datum::TimestampWithTimeZone,
    b_end: pgrx::datum::TimestampWithTimeZone,
) -> bool {
    a_end == b_end && a_start > b_start
}

/// `pg_ripple.allen_starts(a_start, a_end, b_start, b_end)` — interval A
/// starts at the same time as B and ends before B:
/// `a_start = b_start AND a_end < b_end`.
#[pg_extern(schema = "pg_ripple")]
pub fn allen_starts(
    a_start: pgrx::datum::TimestampWithTimeZone,
    a_end: pgrx::datum::TimestampWithTimeZone,
    b_start: pgrx::datum::TimestampWithTimeZone,
    b_end: pgrx::datum::TimestampWithTimeZone,
) -> bool {
    a_start == b_start && a_end < b_end
}

/// `pg_ripple.allen_equals(a_start, a_end, b_start, b_end)` — intervals A and
/// B are identical: `a_start = b_start AND a_end = b_end`.
#[pg_extern(schema = "pg_ripple")]
pub fn allen_equals(
    a_start: pgrx::datum::TimestampWithTimeZone,
    a_end: pgrx::datum::TimestampWithTimeZone,
    b_start: pgrx::datum::TimestampWithTimeZone,
    b_end: pgrx::datum::TimestampWithTimeZone,
) -> bool {
    a_start == b_start && a_end == b_end
}

// ─── v0.125.0 — Temporal Graph Snapshots (FEAT-02) ───────────────────────────

/// Create `_pg_ripple.graph_snapshots` catalog and `snapshot_id_seq`.
///
/// Called from `initialize_schema()` (idempotent via `IF NOT EXISTS`).
pub fn initialize_graph_snapshots_schema() {
    Spi::run_with_args(
        "CREATE SEQUENCE IF NOT EXISTS _pg_ripple.snapshot_id_seq",
        &[],
    )
    .unwrap_or_else(|e| pgrx::warning!("snapshot_id_seq creation: {e}"));

    Spi::run_with_args(
        "CREATE TABLE IF NOT EXISTS _pg_ripple.graph_snapshots ( \
             snapshot_id  BIGINT      NOT NULL DEFAULT nextval('_pg_ripple.snapshot_id_seq') \
                          PRIMARY KEY, \
             graph_iri    TEXT        NOT NULL, \
             snapshot_iri TEXT        NOT NULL UNIQUE, \
             captured_at  TIMESTAMPTZ NOT NULL, \
             triple_count BIGINT, \
             expires_at   TIMESTAMPTZ \
         )",
        &[],
    )
    .unwrap_or_else(|e| pgrx::warning!("graph_snapshots creation: {e}"));

    Spi::run_with_args(
        "CREATE INDEX IF NOT EXISTS idx_graph_snapshots_graph_iri \
         ON _pg_ripple.graph_snapshots (graph_iri, captured_at DESC)",
        &[],
    )
    .unwrap_or_else(|e| pgrx::warning!("graph_snapshots graph_iri index: {e}"));

    Spi::run_with_args(
        "CREATE INDEX IF NOT EXISTS idx_graph_snapshots_expires_at \
         ON _pg_ripple.graph_snapshots (expires_at) \
         WHERE expires_at IS NOT NULL",
        &[],
    )
    .unwrap_or_else(|e| pgrx::warning!("graph_snapshots expires_at index: {e}"));
}

/// `pg_ripple.graph_at(graph_iri, snapshot_time)` — materialise a named-graph
/// snapshot from `_pg_ripple.temporal_facts` at the given timestamp and register
/// it in `_pg_ripple.graph_snapshots`.
///
/// Returns the snapshot IRI (a `urn:snapshot:…` string) that can be used
/// directly in `GRAPH <snapshot_iri> { … }` SPARQL queries.
///
/// The snapshot captures all temporal facts for `graph_iri` whose validity
/// interval contains `snapshot_time` (i.e. `valid_from <= snapshot_time AND
/// (valid_to IS NULL OR valid_to > snapshot_time)`).
///
/// The `expires_at` timestamp is set to `snapshot_time +
/// pg_ripple.snapshot_retention_days` days; set `snapshot_retention_days = 0`
/// to keep snapshots indefinitely.
#[pg_extern(schema = "pg_ripple")]
pub fn graph_at(graph_iri: &str, snapshot_time: pgrx::datum::TimestampWithTimeZone) -> String {
    let g_id = crate::dictionary::encode(graph_iri, 0);

    // Build a deterministic snapshot IRI from the graph IRI + timestamp.
    let ts_str: String = Spi::get_one_with_args::<String>(
        "SELECT to_char($1 AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')",
        &[pgrx::datum::DatumWithOid::from(snapshot_time)],
    )
    .unwrap_or(None)
    .unwrap_or_else(|| "unknown".to_owned());

    // Sanitise graph_iri for inclusion in the URN.
    let iri_slug: String = graph_iri
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let snapshot_iri = format!("urn:snapshot:{iri_slug}:{ts_str}");

    // Count the temporal facts valid at snapshot_time for this graph.
    let triple_count: i64 = Spi::get_one_with_args::<i64>(
        "SELECT COUNT(*)::bigint FROM _pg_ripple.temporal_facts \
         WHERE g = $1 AND valid_from <= $2 \
           AND (valid_to IS NULL OR valid_to > $2)",
        &[
            pgrx::datum::DatumWithOid::from(g_id),
            pgrx::datum::DatumWithOid::from(snapshot_time),
        ],
    )
    .unwrap_or(None)
    .unwrap_or(0);

    // Compute expires_at based on the retention GUC.
    // $3 is snapshot_time (captured_at); expires_at = snapshot_time + N days.
    let retention_days = crate::gucs::storage::SNAPSHOT_RETENTION_DAYS.get();
    let expires_at_expr = if retention_days > 0 {
        format!("$3 + interval '{retention_days} days'")
    } else {
        "NULL::TIMESTAMPTZ".to_owned()
    };

    let insert_sql = format!(
        "INSERT INTO _pg_ripple.graph_snapshots \
           (graph_iri, snapshot_iri, captured_at, triple_count, expires_at) \
         VALUES ($1, $2, $3, $4, {expires_at_expr}) \
         ON CONFLICT (snapshot_iri) DO UPDATE \
           SET triple_count = EXCLUDED.triple_count, \
               captured_at  = EXCLUDED.captured_at, \
               expires_at   = EXCLUDED.expires_at \
         RETURNING snapshot_iri"
    );

    let returned: Option<String> = Spi::get_one_with_args::<String>(
        &insert_sql,
        &[
            pgrx::datum::DatumWithOid::from(graph_iri),
            pgrx::datum::DatumWithOid::from(snapshot_iri.as_str()),
            pgrx::datum::DatumWithOid::from(snapshot_time),
            pgrx::datum::DatumWithOid::from(triple_count),
        ],
    )
    .unwrap_or(None);

    returned.unwrap_or(snapshot_iri)
}

/// `pg_ripple.graph_diff(graph_iri, from_ts, to_ts)` — return the delta between
/// two temporal snapshots of a named graph as a set of `(s, p, o, change)` rows.
///
/// `change` is `'added'` for facts that are present at `to_ts` but not at
/// `from_ts`, and `'removed'` for facts present at `from_ts` but not at
/// `to_ts`.  Facts present at both timestamps are not returned.
///
/// All IDs are dictionary-encoded `BIGINT` values.  Callers can decode them
/// with `pg_ripple.decode(id)`.
#[pg_extern(schema = "pg_ripple")]
pub fn graph_diff(
    graph_iri: &str,
    from_ts: pgrx::datum::TimestampWithTimeZone,
    to_ts: pgrx::datum::TimestampWithTimeZone,
) -> TableIterator<
    'static,
    (
        name!(s, i64),
        name!(p, i64),
        name!(o, i64),
        name!(change, String),
    ),
> {
    let g_id = crate::dictionary::encode(graph_iri, 0);

    let rows: Vec<(i64, i64, i64, String)> = Spi::connect(|client| {
        let result = client.select(
            "SELECT s, p, o, change FROM ( \
               SELECT s, p, o, 'added'::text AS change \
               FROM _pg_ripple.temporal_facts \
               WHERE g = $1 AND valid_from <= $3 \
                 AND (valid_to IS NULL OR valid_to > $3) \
               EXCEPT \
               SELECT s, p, o, 'added'::text AS change \
               FROM _pg_ripple.temporal_facts \
               WHERE g = $1 AND valid_from <= $2 \
                 AND (valid_to IS NULL OR valid_to > $2) \
               UNION ALL \
               SELECT s, p, o, 'removed'::text AS change \
               FROM _pg_ripple.temporal_facts \
               WHERE g = $1 AND valid_from <= $2 \
                 AND (valid_to IS NULL OR valid_to > $2) \
               EXCEPT \
               SELECT s, p, o, 'removed'::text AS change \
               FROM _pg_ripple.temporal_facts \
               WHERE g = $1 AND valid_from <= $3 \
                 AND (valid_to IS NULL OR valid_to > $3) \
             ) delta \
             ORDER BY change, s, p, o",
            None,
            &[
                pgrx::datum::DatumWithOid::from(g_id),
                pgrx::datum::DatumWithOid::from(from_ts),
                pgrx::datum::DatumWithOid::from(to_ts),
            ],
        );
        match result {
            Ok(tup_table) => tup_table
                .into_iter()
                .filter_map(|row| {
                    let s: i64 = row.get_by_name::<i64, _>("s").ok()??;
                    let p: i64 = row.get_by_name::<i64, _>("p").ok()??;
                    let o: i64 = row.get_by_name::<i64, _>("o").ok()??;
                    let change: String = row.get_by_name::<String, _>("change").ok()??;
                    Some((s, p, o, change))
                })
                .collect(),
            Err(e) => {
                pgrx::warning!("graph_diff query error: {e}");
                Vec::new()
            }
        }
    });

    TableIterator::new(rows)
}

/// Prune expired snapshots from `_pg_ripple.graph_snapshots`.
///
/// Called from the merge background worker on each tick (worker_idx == 0 only).
/// Deletes rows where `expires_at <= now()` when
/// `pg_ripple.snapshot_retention_days > 0`.
pub fn prune_expired_snapshots() {
    if crate::gucs::storage::SNAPSHOT_RETENTION_DAYS.get() == 0 {
        return;
    }
    Spi::run_with_args(
        "DELETE FROM _pg_ripple.graph_snapshots \
         WHERE expires_at IS NOT NULL AND expires_at <= now()",
        &[],
    )
    .unwrap_or_else(|e| pgrx::warning!("prune_expired_snapshots: {e}"));
}

/// Return the current live snapshot count from `_pg_ripple.graph_snapshots`.
///
/// Used by the HTTP companion service to update the
/// `pg_ripple_graph_snapshots_total` Prometheus gauge.
#[pg_extern(schema = "pg_ripple")]
pub fn graph_snapshots_count() -> i64 {
    Spi::get_one::<i64>("SELECT COUNT(*)::bigint FROM _pg_ripple.graph_snapshots")
        .unwrap_or(None)
        .unwrap_or(0)
}
