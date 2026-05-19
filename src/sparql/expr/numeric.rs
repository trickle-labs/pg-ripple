//! Numeric SPARQL built-in function translation (H17-02, v0.122.0).
//!
//! Handles: `ABS`, `CEIL`, `FLOOR`, `ROUND`, `RAND`.

use std::collections::HashMap;

use spargebra::algebra::{Expression, Function};

use super::super::sqlgen::Ctx;
use super::{decode_lexical_sql, translate_arg_value};

/// Translate a numeric SPARQL built-in function in value context.
///
/// Returns `Some(sql)` for handled functions and `None` for all others.
/// Sets `is_numeric = true` for `ABS` and `RAND`.
pub(super) fn translate(
    func: &Function,
    args: &[Expression],
    bindings: &HashMap<String, String>,
    ctx: &mut Ctx,
    is_numeric: &mut bool,
) -> Option<String> {
    match func {
        Function::Abs => {
            *is_numeric = true;
            let col = translate_arg_value(args.first()?, bindings, ctx)?;
            let text = decode_lexical_sql(&col);
            Some(format!("abs(({text})::numeric)"))
        }
        Function::Ceil => {
            // Return typed literal preserving input type (xsd:decimal → xsd:decimal, etc.)
            let col = translate_arg_value(args.first()?, bindings, ctx)?;
            let text = decode_lexical_sql(&col);
            Some(format!(
                "pg_ripple.encode_typed_literal(\
                    ceil(({text})::numeric)::text, \
                    CASE WHEN {col} < 0 THEN 'http://www.w3.org/2001/XMLSchema#integer' \
                         ELSE COALESCE(\
                             (SELECT d.datatype FROM _pg_ripple.dictionary d WHERE d.id = {col} AND d.kind = 3), \
                             'http://www.w3.org/2001/XMLSchema#integer') \
                    END)"
            ))
        }
        Function::Floor => {
            let col = translate_arg_value(args.first()?, bindings, ctx)?;
            let text = decode_lexical_sql(&col);
            Some(format!(
                "pg_ripple.encode_typed_literal(\
                    floor(({text})::numeric)::text, \
                    CASE WHEN {col} < 0 THEN 'http://www.w3.org/2001/XMLSchema#integer' \
                         ELSE COALESCE(\
                             (SELECT d.datatype FROM _pg_ripple.dictionary d WHERE d.id = {col} AND d.kind = 3), \
                             'http://www.w3.org/2001/XMLSchema#integer') \
                    END)"
            ))
        }
        Function::Round => {
            let col = translate_arg_value(args.first()?, bindings, ctx)?;
            let text = decode_lexical_sql(&col);
            Some(format!(
                "pg_ripple.encode_typed_literal(\
                    round(({text})::numeric)::text, \
                    CASE WHEN {col} < 0 THEN 'http://www.w3.org/2001/XMLSchema#integer' \
                         ELSE COALESCE(\
                             (SELECT d.datatype FROM _pg_ripple.dictionary d WHERE d.id = {col} AND d.kind = 3), \
                             'http://www.w3.org/2001/XMLSchema#integer') \
                    END)"
            ))
        }
        Function::Rand => {
            // RAND() → raw double in [0, 1). Raw float, not dict-encoded.
            *is_numeric = true;
            Some("random()".to_owned())
        }
        _ => None,
    }
}
