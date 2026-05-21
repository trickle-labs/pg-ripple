//! CDC → pg_tide Outbox Bridge (v0.52.0, migrated in v0.127.0).
//!
//! Provides:
//! - `enable_cdc_bridge_trigger` — install a per-predicate VP-delta trigger that
//!   publishes decoded JSON-LD events to a pg_tide outbox within the same transaction.
//! - `disable_cdc_bridge_trigger` — drop the trigger.
//! - `cdc_bridge_triggers` — catalog SRF listing all active triggers.
//! - Bridge schema initialisation (`_pg_ripple.cdc_bridge_triggers` catalog).
//!
//! The optional background worker (`_pg_ripple.cdc_bridge_worker`) is registered
//! from `worker.rs`; the worker body reads from the CDC NOTIFY channel, performs
//! a bulk dictionary-decode SPI call, and publishes JSON-LD events into the
//! configured pg_tide outbox.
//!
//! # Graceful degradation
//!
//! All bridge SQL functions gate on the legacy `crate::TRICKLE_INTEGRATION.get()`
//! switch and on `crate::has_pg_tide()`.  When pg_tide is absent (or relay
//! integration is disabled), the functions return the `PT800` error code:
//!
//! ```text
//! PT800: pg_tide extension is not installed; install pg_tide to use bridge features
//! ```

use pgrx::datum::DatumWithOid;
use pgrx::prelude::*;

// ─── Error code ───────────────────────────────────────────────────────────────

/// Raise a user-facing error when pg_tide is not available or integration
/// is disabled via GUC.
///
/// Uses SQLSTATE `0A000` (feature_not_supported) to allow callers to catch
/// this specific error condition.
pub(crate) fn require_tide(fn_name: &str) {
    if !crate::TRICKLE_INTEGRATION.get() {
        pgrx::error!(
            "{fn_name}(): pg_ripple.trickle_integration is off; \
             set it to on to use pg_tide bridge features"
        );
    }
    if !crate::has_pg_tide() {
        pgrx::error!(
            "{fn_name}(): pg_tide extension is not installed; \
             install pg_tide from https://github.com/trickle-labs/pg-tide \
             and run CREATE EXTENSION pg_tide to use bridge features"
        );
    }
}

// ─── Schema initialisation ────────────────────────────────────────────────────

/// Create the `_pg_ripple.cdc_bridge_triggers` catalog table.
///
/// Called once from `storage::initialize_schema`.
pub fn initialize_cdc_bridge_schema() {
    Spi::run_with_args(
        "CREATE TABLE IF NOT EXISTS _pg_ripple.cdc_bridge_triggers ( \
             name         TEXT NOT NULL PRIMARY KEY, \
             predicate_id BIGINT NOT NULL, \
             outbox_table TEXT NOT NULL, \
             outbox_name  TEXT, \
             created_at   TIMESTAMPTZ NOT NULL DEFAULT now() \
         )",
        &[],
    )
    .unwrap_or_else(|e| pgrx::error!("cdc_bridge_triggers table creation error: {e}"));

    Spi::run_with_args(
        "ALTER TABLE _pg_ripple.cdc_bridge_triggers \
         ADD COLUMN IF NOT EXISTS outbox_name TEXT",
        &[],
    )
    .unwrap_or_else(|e| pgrx::error!("cdc_bridge_triggers outbox_name migration error: {e}"));

    Spi::run_with_args(
        "UPDATE _pg_ripple.cdc_bridge_triggers \
         SET outbox_name = outbox_table WHERE outbox_name IS NULL",
        &[],
    )
    .unwrap_or_else(|e| pgrx::error!("cdc_bridge_triggers outbox_name backfill error: {e}"));

    Spi::run_with_args(
        "ALTER TABLE _pg_ripple.cdc_bridge_triggers \
         ALTER COLUMN outbox_name SET NOT NULL",
        &[],
    )
    .unwrap_or_else(|e| {
        pgrx::error!("cdc_bridge_triggers outbox_name not-null migration error: {e}")
    });

    // PL/pgSQL trigger function used by per-predicate CDC bridge triggers.
    // Encodes the new row as a JSON-LD object and publishes it to a pg_tide outbox.
    // TG_ARGV[0] = predicate_id (bigint text), TG_ARGV[1] = pg_tide outbox name.
    Spi::run_with_args(
        r#"
CREATE OR REPLACE FUNCTION _pg_ripple.cdc_bridge_trigger_fn()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE
    pred_id    BIGINT  := TG_ARGV[0]::bigint;
    outbox_name TEXT   := TG_ARGV[1];
    s_iri      TEXT;
    p_iri      TEXT;
    o_iri      TEXT;
    payload    JSONB;
    headers    JSONB;
    dedup_key  TEXT;
    sid        BIGINT;
BEGIN
    SELECT value INTO s_iri FROM _pg_ripple.dictionary WHERE id = NEW.s;
    SELECT value INTO p_iri FROM _pg_ripple.dictionary WHERE id = pred_id;
    SELECT value INTO o_iri FROM _pg_ripple.dictionary WHERE id = NEW.o;

    sid := NEW.i;
    dedup_key := 'ripple:' || sid::text;

    payload := jsonb_build_object(
        '@context',   'https://schema.org/',
        '@id',        COALESCE(s_iri, '_:' || NEW.s::text),
        p_iri,        COALESCE(o_iri, NEW.o::text)
    );

    headers := jsonb_build_object(
        'event_id',     dedup_key,
        'dedup_key',    dedup_key,
        'event_type',   'pg_ripple.triple.insert',
        'predicate_id', pred_id,
        'statement_id', sid,
        'graph_id',     NEW.g
    );

    PERFORM tide.outbox_publish(outbox_name, payload, headers);

    RETURN NEW;
END;
$$"#,
        &[],
    )
    .unwrap_or_else(|e| pgrx::error!("cdc_bridge_trigger_fn creation error: {e}"));
}

// ─── enable_cdc_bridge_trigger ────────────────────────────────────────────────

/// Install a CDC bridge trigger on the VP delta table for `predicate`.
///
/// When a triple is inserted into the VP delta table for the given predicate,
/// the trigger decodes the (s, p, o) dictionary IDs and writes a JSON-LD event
/// to the pg_tide outbox named by `outbox` in the same transaction.
///
/// # Errors
/// Raises `PT800` when pg_tide is absent or `trickle_integration = off`.
/// Raises an ERROR when the predicate IRI is not in the dictionary.
pub fn enable_cdc_bridge_trigger(name: &str, predicate: &str, outbox: &str) {
    require_tide("enable_cdc_bridge_trigger");

    // Validate name
    if name.is_empty() || name.len() > 63 {
        pgrx::error!("enable_cdc_bridge_trigger: name must be 1–63 characters");
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        pgrx::error!(
            "enable_cdc_bridge_trigger: name must contain only ASCII letters, digits, and underscores"
        );
    }

    if outbox.is_empty() || outbox.len() > 63 {
        pgrx::error!("enable_cdc_bridge_trigger: outbox name must be 1–63 characters");
    }

    let _ = Spi::get_one_with_args::<pgrx::JsonB>(
        "SELECT tide.outbox_status($1)",
        &[DatumWithOid::from(outbox)],
    )
    .unwrap_or_else(|_| {
        pgrx::error!(
            "enable_cdc_bridge_trigger: pg_tide outbox '{}' does not exist; \
             create it first with SELECT tide.outbox_create(...) ",
            outbox
        )
    })
    .unwrap_or_else(|| {
        pgrx::error!(
            "enable_cdc_bridge_trigger: pg_tide outbox '{}' returned no status; \
             create it first with SELECT tide.outbox_create(...) ",
            outbox
        )
    });

    // Resolve predicate IRI → dictionary ID
    let pred_iri = if predicate.starts_with('<') && predicate.ends_with('>') {
        &predicate[1..predicate.len() - 1]
    } else {
        predicate
    };
    let pred_id = crate::dictionary::lookup_iri(pred_iri).unwrap_or_else(|| {
        pgrx::error!(
            "enable_cdc_bridge_trigger: predicate IRI not in dictionary: {}",
            pred_iri
        )
    });

    // Determine VP delta table name
    let delta_table = match Spi::get_one_with_args::<i64>(
        "SELECT table_oid::bigint FROM _pg_ripple.predicates WHERE id = $1",
        &[DatumWithOid::from(pred_id)],
    ) {
        Ok(Some(_)) => format!("_pg_ripple.vp_{pred_id}_delta"),
        Ok(None) => "_pg_ripple.vp_rare".to_string(),
        Err(e) => pgrx::error!("enable_cdc_bridge_trigger: predicate catalog error: {e}"),
    };

    // Install trigger
    let trigger_name = format!("cdc_bridge_{name}");
    let outbox_literal = outbox.replace('\'', "''");
    let sql = format!(
        "CREATE TRIGGER {trigger_name} \
         AFTER INSERT ON {delta_table} \
         FOR EACH ROW EXECUTE FUNCTION _pg_ripple.cdc_bridge_trigger_fn({pred_id}, '{outbox_literal}')"
    );
    Spi::run_with_args(&sql, &[])
        .unwrap_or_else(|e| pgrx::error!("enable_cdc_bridge_trigger: trigger install error: {e}"));

    // Record in catalog
    Spi::run_with_args(
        "INSERT INTO _pg_ripple.cdc_bridge_triggers (name, predicate_id, outbox_table, outbox_name) \
         VALUES ($1, $2, $3, $3) ON CONFLICT (name) DO UPDATE \
         SET predicate_id = EXCLUDED.predicate_id, \
             outbox_table = EXCLUDED.outbox_table, \
             outbox_name = EXCLUDED.outbox_name, \
             created_at = now()",
        &[
            DatumWithOid::from(name),
            DatumWithOid::from(pred_id),
            DatumWithOid::from(outbox),
        ],
    )
    .unwrap_or_else(|e| pgrx::error!("enable_cdc_bridge_trigger: catalog insert error: {e}"));
}

// ─── disable_cdc_bridge_trigger ───────────────────────────────────────────────

/// Drop a CDC bridge trigger previously installed by `enable_cdc_bridge_trigger`.
pub fn disable_cdc_bridge_trigger(name: &str) {
    // Look up predicate_id for the trigger — return silently when not found.
    let pred_ids: Vec<i64> = Spi::connect(|c| {
        c.select(
            "SELECT predicate_id FROM _pg_ripple.cdc_bridge_triggers WHERE name = $1",
            None,
            &[DatumWithOid::from(name)],
        )
        .unwrap_or_else(|e| pgrx::error!("disable_cdc_bridge_trigger: catalog error: {e}"))
        .filter_map(|row| row.get::<i64>(1).ok().flatten())
        .collect()
    });
    let pred_id = match pred_ids.first() {
        Some(&id) => id,
        None => return, // trigger not registered — no-op
    };

    let delta_table = match Spi::get_one_with_args::<i64>(
        "SELECT table_oid::bigint FROM _pg_ripple.predicates WHERE id = $1",
        &[DatumWithOid::from(pred_id)],
    ) {
        Ok(Some(_)) => format!("_pg_ripple.vp_{pred_id}_delta"),
        Ok(None) => "_pg_ripple.vp_rare".to_owned(),
        Err(_) => format!("_pg_ripple.vp_{pred_id}_delta"),
    };

    let trigger_name = format!("cdc_bridge_{name}");
    let sql = format!("DROP TRIGGER IF EXISTS {trigger_name} ON {delta_table}");
    Spi::run_with_args(&sql, &[])
        .unwrap_or_else(|e| pgrx::error!("disable_cdc_bridge_trigger: {e}"));

    Spi::run_with_args(
        "DELETE FROM _pg_ripple.cdc_bridge_triggers WHERE name = $1",
        &[DatumWithOid::from(name)],
    )
    .unwrap_or_else(|e| pgrx::error!("disable_cdc_bridge_trigger: catalog delete error: {e}"));
}

// ─── cdc_bridge_triggers SRF ─────────────────────────────────────────────────

/// Row type returned by the `cdc_bridge_triggers()` SRF.
pub struct CdcBridgeTriggerRow {
    /// User-supplied trigger name.
    pub name: String,
    /// Predicate IRI.
    pub predicate: String,
    /// Target pg_tide outbox name.
    pub outbox: String,
    /// Whether the underlying PG trigger exists.
    pub active: bool,
}

/// List all registered CDC bridge triggers.
pub fn list_cdc_bridge_triggers() -> Vec<CdcBridgeTriggerRow> {
    let mut rows = Vec::new();
    let result = Spi::connect(|client| {
        let tup_table = client.select(
            "SELECT t.name, d.value AS predicate, COALESCE(t.outbox_name, t.outbox_table) AS outbox, \
             EXISTS( \
               SELECT 1 FROM pg_trigger pg \
               JOIN pg_class c ON c.oid = pg.tgrelid \
               WHERE pg.tgname = 'cdc_bridge_' || t.name \
             ) AS active \
             FROM _pg_ripple.cdc_bridge_triggers t \
             JOIN _pg_ripple.dictionary d ON d.id = t.predicate_id \
             ORDER BY t.name",
            None,
            &[],
        );
        match tup_table {
            Ok(table) => {
                for row in table {
                    let name: String = row["name"].value().unwrap_or(None).unwrap_or_default();
                    let predicate: String =
                        row["predicate"].value().unwrap_or(None).unwrap_or_default();
                    let outbox: String = row["outbox"].value().unwrap_or(None).unwrap_or_default();
                    let active: bool = row["active"].value().unwrap_or(None).unwrap_or(false);
                    rows.push(CdcBridgeTriggerRow {
                        name,
                        predicate,
                        outbox,
                        active,
                    });
                }
            }
            Err(e) => {
                pgrx::warning!("cdc_bridge_triggers: catalog query error: {e}");
            }
        }
        Ok::<(), pgrx::spi::Error>(())
    });
    if let Err(e) = result {
        pgrx::warning!("cdc_bridge_triggers: SPI connect error: {e}");
    }
    rows
}
