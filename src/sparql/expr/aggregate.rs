//! Aggregate SPARQL function translation stub (H17-02, v0.122.0).
//!
//! SPARQL 1.1 aggregate functions (`COUNT`, `SUM`, `MIN`, `MAX`, `AVG`,
//! `GROUP_CONCAT`, `SAMPLE`) are handled at the algebra level via
//! `spargebra::algebra::AggregateExpression`, not as `Function::Custom` calls.
//! This module is reserved for any future aggregate-function value-context
//! overloads (e.g. windowed aggregates in SPARQL 1.2).

use std::collections::HashMap;

use spargebra::algebra::{Expression, Function};

use super::super::sqlgen::Ctx;

/// Translate an aggregate-related SPARQL function in value context (stub).
///
/// Currently always returns `None` — aggregates are handled by the algebra
/// layer and do not appear in the value-context translation path.
#[allow(unused_variables)]
pub(super) fn translate(
    func: &Function,
    args: &[Expression],
    bindings: &HashMap<String, String>,
    ctx: &mut Ctx,
    is_numeric: &mut bool,
) -> Option<String> {
    None
}
