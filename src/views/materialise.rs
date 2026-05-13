//! Datalog-backed live views: create/drop/list_datalog_view.
//! (extracted from views/mod.rs in v0.114.0)

use pgrx::datum::DatumWithOid;
use pgrx::prelude::*;


use super::{
    PGTRICKLE_HINT,
    compile_sparql_for_view, validate_datalog_view_goal, validate_name,
};

/// Create a Datalog view from inline rules and a SPARQL SELECT goal.
///
/// The rules are parsed and stored (as if by `load_rules`), then the goal SPARQL
/// query is compiled against the derived VP tables and registered as a pg_trickle
/// stream table.
///
/// `rule_set_name` is the logical name used to store the rules.  If a rule set
/// with the same name already exists its rules are replaced.
pub(crate) fn create_datalog_view_from_rules(
    name: &str,
    rules: &str,
    rule_set_name: &str,
    goal: &str,
    schedule: &str,
    decode: bool,
    immediate: bool,
) -> i64 {
    if !crate::has_pg_trickle() {
        pgrx::error!(
            "pg_trickle is not installed — Datalog views require pg_trickle; hint: {}",
            PGTRICKLE_HINT
        );
    }
    if let Err(e) = validate_name(name) {
        pgrx::error!("invalid view name: {e}");
    }

    // Load the rules (this handles parse, stratify, store).
    crate::datalog::load_and_store_rules(rules, rule_set_name);

    // issue #89 (v0.112.0): validate goal predicates against rule heads + base predicates.
    validate_datalog_view_goal(Some(rule_set_name), goal);

    // Compile the goal SPARQL to SQL.
    // The stream table always stores BIGINT dictionary IDs (fix for issue #81).
    let (goal_sql, variables) = compile_sparql_for_view(goal)
        .unwrap_or_else(|e| pgrx::error!("Datalog view goal compilation failed: {e}"));

    let var_count = variables.len() as i64;
    let variables_json = serde_json::to_string(&variables).unwrap_or_else(|_| "[]".to_owned());

    let stream_table = format!("pg_ripple.{name}");

    // SQL-INJ-01 (v0.80.0): parameterised INSERT to prevent SQL injection.
    Spi::run_with_args(
        "INSERT INTO _pg_ripple.datalog_views \
         (name, rules, rule_set, goal, generated_sql, schedule, decode, stream_table, variables) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9::jsonb) \
         ON CONFLICT (name) DO UPDATE \
         SET rules = EXCLUDED.rules, \
             rule_set = EXCLUDED.rule_set, \
             goal = EXCLUDED.goal, \
             generated_sql = EXCLUDED.generated_sql, \
             schedule = EXCLUDED.schedule, \
             decode = EXCLUDED.decode, \
             stream_table = EXCLUDED.stream_table, \
             variables = EXCLUDED.variables",
        &[
            pgrx::datum::DatumWithOid::from(name),
            pgrx::datum::DatumWithOid::from(rules),
            pgrx::datum::DatumWithOid::from(rule_set_name),
            pgrx::datum::DatumWithOid::from(goal),
            pgrx::datum::DatumWithOid::from(goal_sql.as_str()),
            pgrx::datum::DatumWithOid::from(schedule),
            pgrx::datum::DatumWithOid::from(decode),
            pgrx::datum::DatumWithOid::from(stream_table.as_str()),
            pgrx::datum::DatumWithOid::from(variables_json.as_str()),
        ],
    )
    .unwrap_or_else(|e| pgrx::error!("failed to register Datalog view: {e}"));

    // Create the pg_trickle stream table.
    let escaped_stream_table = stream_table.replace('\'', "''");
    let escaped_schedule = schedule.replace('\'', "''");

    // IDEMPOTENT-02 (issue #83): drop any pre-existing stream table so that a
    // repeated call replaces the view cleanly instead of erroring.
    let _ = Spi::run(&format!(
        "SELECT pgtrickle.drop_stream_table(name => '{escaped_stream_table}')"
    ));

    let refresh_mode_clause = if immediate {
        ", refresh_mode => 'IMMEDIATE'"
    } else {
        ""
    };
    let pgt_sql = format!(
        "SELECT pgtrickle.create_stream_table(\
            name => '{escaped_stream_table}', \
            query => $__pgrdl_q${goal_sql}$__pgrdl_q$, \
            schedule => '{escaped_schedule}'\
            {refresh_mode_clause}\
        )"
    );
    Spi::run(&pgt_sql)
        .unwrap_or_else(|e| pgrx::error!("failed to create Datalog view stream table: {e}"));

    // If decode = true, create a thin companion VIEW for TEXT-decoded access.
    if decode {
        let decode_view = format!("pg_ripple.{name}_decoded");
        let inner_alias = "_dl_";
        let decode_cols: Vec<String> = variables
            .iter()
            .map(|v| {
                format!("(SELECT d.value FROM _pg_ripple.dictionary d WHERE d.id = {inner_alias}.{v}) AS {v}")
            })
            .collect();
        Spi::run(&format!(
            "CREATE OR REPLACE VIEW {decode_view} AS \
             SELECT {} FROM {stream_table} {inner_alias}",
            decode_cols.join(", ")
        ))
        .unwrap_or_else(|e| pgrx::error!("failed to create Datalog decode view: {e}"));
    }

    var_count
}

/// Create a Datalog view referencing an existing named rule set.
pub(crate) fn create_datalog_view_from_rule_set(
    name: &str,
    rule_set: &str,
    goal: &str,
    schedule: &str,
    decode: bool,
    immediate: bool,
) -> i64 {
    if !crate::has_pg_trickle() {
        pgrx::error!(
            "pg_trickle is not installed — Datalog views require pg_trickle; hint: {}",
            PGTRICKLE_HINT
        );
    }
    if let Err(e) = validate_name(name) {
        pgrx::error!("invalid view name: {e}");
    }

    // Verify the rule set exists.
    let exists = Spi::get_one_with_args::<bool>(
        "SELECT EXISTS(SELECT 1 FROM _pg_ripple.rule_sets WHERE name = $1 AND active = true)",
        &[DatumWithOid::from(rule_set)],
    )
    .unwrap_or_else(|e| pgrx::error!("rule set lookup error: {e}"))
    .unwrap_or(false);

    if !exists {
        pgrx::error!("rule set '{}' not found or is inactive", rule_set);
    }

    // issue #89 (v0.112.0): validate goal predicates against rule heads + base predicates.
    validate_datalog_view_goal(Some(rule_set), goal);

    // Compile the goal SPARQL to SQL.
    // The stream table always stores BIGINT dictionary IDs (fix for issue #81).
    let (goal_sql, variables) = compile_sparql_for_view(goal)
        .unwrap_or_else(|e| pgrx::error!("Datalog view goal compilation failed: {e}"));

    let var_count = variables.len() as i64;
    let variables_json = serde_json::to_string(&variables).unwrap_or_else(|_| "[]".to_owned());

    let stream_table = format!("pg_ripple.{name}");

    // SQL-INJ-01 (v0.80.0): parameterised INSERT; NULL rules (rule-set-based view).
    Spi::run_with_args(
        "INSERT INTO _pg_ripple.datalog_views \
         (name, rules, rule_set, goal, generated_sql, schedule, decode, stream_table, variables) \
         VALUES ($1, NULL, $2, $3, $4, $5, $6, $7, $8::jsonb) \
         ON CONFLICT (name) DO UPDATE \
         SET rules = EXCLUDED.rules, \
             rule_set = EXCLUDED.rule_set, \
             goal = EXCLUDED.goal, \
             generated_sql = EXCLUDED.generated_sql, \
             schedule = EXCLUDED.schedule, \
             decode = EXCLUDED.decode, \
             stream_table = EXCLUDED.stream_table, \
             variables = EXCLUDED.variables",
        &[
            pgrx::datum::DatumWithOid::from(name),
            pgrx::datum::DatumWithOid::from(rule_set),
            pgrx::datum::DatumWithOid::from(goal),
            pgrx::datum::DatumWithOid::from(goal_sql.as_str()),
            pgrx::datum::DatumWithOid::from(schedule),
            pgrx::datum::DatumWithOid::from(decode),
            pgrx::datum::DatumWithOid::from(stream_table.as_str()),
            pgrx::datum::DatumWithOid::from(variables_json.as_str()),
        ],
    )
    .unwrap_or_else(|e| pgrx::error!("failed to register Datalog view: {e}"));

    // Create the pg_trickle stream table.
    let escaped_stream_table = stream_table.replace('\'', "''");
    let escaped_schedule = schedule.replace('\'', "''");

    // IDEMPOTENT-02 (issue #83): drop any pre-existing stream table so that a
    // repeated call replaces the view cleanly instead of erroring.
    let _ = Spi::run(&format!(
        "SELECT pgtrickle.drop_stream_table(name => '{escaped_stream_table}')"
    ));

    let refresh_mode_clause = if immediate {
        ", refresh_mode => 'IMMEDIATE'"
    } else {
        ""
    };
    let pgt_sql = format!(
        "SELECT pgtrickle.create_stream_table(\
            name => '{escaped_stream_table}', \
            query => $__pgrdl_q${goal_sql}$__pgrdl_q$, \
            schedule => '{escaped_schedule}'\
            {refresh_mode_clause}\
        )"
    );
    Spi::run(&pgt_sql)
        .unwrap_or_else(|e| pgrx::error!("failed to create Datalog view stream table: {e}"));

    // If decode = true, create a thin companion VIEW for TEXT-decoded access.
    if decode {
        let decode_view = format!("pg_ripple.{name}_decoded");
        let inner_alias = "_dl_";
        let decode_cols: Vec<String> = variables
            .iter()
            .map(|v| {
                format!("(SELECT d.value FROM _pg_ripple.dictionary d WHERE d.id = {inner_alias}.{v}) AS {v}")
            })
            .collect();
        Spi::run(&format!(
            "CREATE OR REPLACE VIEW {decode_view} AS \
             SELECT {} FROM {stream_table} {inner_alias}",
            decode_cols.join(", ")
        ))
        .unwrap_or_else(|e| pgrx::error!("failed to create Datalog decode view: {e}"));
    }

    var_count
}

/// Drop a Datalog view and its underlying stream table.
pub(crate) fn drop_datalog_view(name: &str) -> bool {
    if !crate::has_pg_trickle() {
        pgrx::error!(
            "pg_trickle is not installed — Datalog views require pg_trickle; hint: {}",
            PGTRICKLE_HINT
        );
    }

    let stream_table = format!("pg_ripple.{name}");
    let escaped_stream_table = stream_table.replace('\'', "''");
    let decode_view = format!("pg_ripple.{name}_decoded");

    // Drop the companion decode view if it was created (ignore error if absent).
    let _ = Spi::run(&format!("DROP VIEW IF EXISTS {decode_view}"));

    // Drop the stream table (ignore error if already gone).
    let _ = Spi::run(&format!(
        "SELECT pgtrickle.drop_stream_table(name => '{escaped_stream_table}')"
    ));

    // Remove from catalog.
    Spi::run(&format!(
        "DELETE FROM _pg_ripple.datalog_views WHERE name = '{}'",
        name.replace('\'', "''")
    ))
    .unwrap_or_else(|e| pgrx::error!("failed to remove Datalog view from catalog: {e}"));

    true
}

/// List all registered Datalog views.
///
/// Returns a JSONB array of objects.
pub(crate) fn list_datalog_views() -> pgrx::JsonB {
    Spi::get_one::<pgrx::JsonB>(
        "SELECT COALESCE(json_agg(row_to_json(v))::jsonb, '[]'::jsonb) \
         FROM (SELECT name, rule_set, goal, schedule, decode, stream_table, variables, created_at \
               FROM _pg_ripple.datalog_views ORDER BY created_at) v",
    )
    .unwrap_or_else(|e| pgrx::error!("list_datalog_views SPI error: {e}"))
    .unwrap_or_else(|| pgrx::JsonB(serde_json::Value::Array(vec![])))
}

// ─── ExtVP Semi-join Tables ───────────────────────────────────────────────────

