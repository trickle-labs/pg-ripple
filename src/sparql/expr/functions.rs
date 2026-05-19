//! SPARQL 1.1 built-in function translation — value context dispatch table
//! (M15-13, v0.96.0; H17-02 refactor v0.122.0).
//!
//! This file is intentionally a **thin dispatch table** only.  All actual SQL
//! generation logic lives in the sibling sub-modules declared in `mod.rs`:
//!
//! | Sub-module         | Functions handled                                  |
//! |--------------------|---------------------------------------------------|
//! | `string`           | STR, STRLEN, SUBSTR, UCASE, LCASE, CONCAT, …      |
//! | `datetime`         | NOW, YEAR, MONTH, DAY, HOURS, MINUTES, …          |
//! | `numeric`          | ABS, CEIL, FLOOR, ROUND, RAND                     |
//! | `iri`              | IRI/URI, BNODE, LANG, DATATYPE, XSD casts          |
//! | `geo`              | geof:distance, geof:area, geof:buffer, …           |
//! | `temporal`         | pg:temporal_window, Allen's relations, similarity  |
//! | `aggregate`        | (stub; aggregates handled at algebra level)        |

use std::collections::HashMap;

use spargebra::algebra::{Expression, Function};

use super::super::sqlgen::Ctx;
use super::{aggregate, datetime, geo, iri, numeric, string, temporal};

// ─── Value context ────────────────────────────────────────────────────────────

/// Translate a `FunctionCall` in a value context (BIND / SELECT expression).
///
/// Returns a SQL expression that evaluates to a `BIGINT` (dictionary ID) for
/// string/IRI/blank-node results, or a raw SQL numeric value for integer/float
/// results.  The caller must set `*is_numeric = true` for the latter so the
/// output pipeline skips dictionary decode.
pub(crate) fn translate_function_value(
    func: &Function,
    args: &[Expression],
    bindings: &HashMap<String, String>,
    ctx: &mut Ctx,
    is_numeric: &mut bool,
) -> Option<String> {
    // Delegate to category sub-modules in order; each returns None for unrecognised functions.
    if let Some(sql) = string::translate(func, args, bindings, ctx, is_numeric) {
        return Some(sql);
    }
    if let Some(sql) = datetime::translate(func, args, bindings, ctx, is_numeric) {
        return Some(sql);
    }
    if let Some(sql) = numeric::translate(func, args, bindings, ctx, is_numeric) {
        return Some(sql);
    }
    if let Some(sql) = iri::translate(func, args, bindings, ctx, is_numeric) {
        return Some(sql);
    }
    if let Some(sql) = aggregate::translate(func, args, bindings, ctx, is_numeric) {
        return Some(sql);
    }

    // Custom function IRIs — try XSD casts first, then geo, then temporal.
    if let Function::Custom(name) = func {
        let custom_iri = name.as_str();
        if let Some(sql) = iri::translate_xsd_cast(custom_iri, args, bindings, ctx) {
            return Some(sql);
        }
        if let Some(sql) = geo::translate_custom(custom_iri, args, bindings, ctx, is_numeric) {
            return Some(sql);
        }
        if let Some(sql) = temporal::translate_custom(custom_iri, args, bindings, ctx, is_numeric) {
            return Some(sql);
        }
    }

    None
}

// ─── Helpers used by the module ───────────────────────────────────────────────

/// Check whether a function returns a numeric (raw integer/float) in value context.
pub(crate) fn is_numeric_function(func: &Function) -> bool {
    matches!(
        func,
        Function::StrLen
            | Function::Abs
            | Function::Rand
            | Function::Year
            | Function::Month
            | Function::Day
            | Function::Hours
            | Function::Minutes // CEIL, FLOOR, ROUND, SECONDS return typed literal dict IDs, not raw numerics.
    )
}
