//! XSD type-cast helper functions for SPARQL expr translation (M15-13, v0.96.0).
//!
//! Moved from `expr/mod.rs` (lines 1535-1625) to keep mod.rs under 800 lines.

/// Return the full XSD datatype IRI if `iri` is a SPARQL 1.1 constructor
/// (e.g. `xsd:integer`) that we support, otherwise `None`.
pub(super) fn xsd_cast_datatype(iri: &str) -> Option<&'static str> {
    match iri {
        "http://www.w3.org/2001/XMLSchema#integer" => {
            Some("http://www.w3.org/2001/XMLSchema#integer")
        }
        "http://www.w3.org/2001/XMLSchema#decimal" => {
            Some("http://www.w3.org/2001/XMLSchema#decimal")
        }
        "http://www.w3.org/2001/XMLSchema#double" => {
            Some("http://www.w3.org/2001/XMLSchema#double")
        }
        "http://www.w3.org/2001/XMLSchema#float" => Some("http://www.w3.org/2001/XMLSchema#float"),
        "http://www.w3.org/2001/XMLSchema#string" => {
            Some("http://www.w3.org/2001/XMLSchema#string")
        }
        "http://www.w3.org/2001/XMLSchema#boolean" => {
            Some("http://www.w3.org/2001/XMLSchema#boolean")
        }
        "http://www.w3.org/2001/XMLSchema#dateTime" => {
            Some("http://www.w3.org/2001/XMLSchema#dateTime")
        }
        _ => None,
    }
}

/// Build SQL that casts the encoded bigint `col` to `dt` and re-encodes.
/// Returns NULL on cast failure.
pub(super) fn xsd_cast_sql(col: &str, dt: &str) -> String {
    // Decode column to its lexical string form.
    let lex = format!(
        "CASE WHEN ({col}) IS NULL THEN NULL \
         WHEN ({col}) < 0 THEN \
           ((({col}) & 72057594037927935::bigint) - 36028797018963968::bigint)::text \
         ELSE (SELECT d.value FROM _pg_ripple.dictionary d WHERE d.id = ({col}) LIMIT 1) \
         END"
    );
    // Numeric pattern: optional sign, digits with optional decimal point, optional exponent.
    let num_re = r"^[+\-]?(\d+\.?\d*|\d*\.\d+)([eE][+\-]?\d+)?$";
    match dt {
        "http://www.w3.org/2001/XMLSchema#integer" => {
            // Truncate to integer (floor toward zero). Use regex to guard against bad input.
            format!(
                "CASE WHEN ({lex}) IS NULL OR ({lex}) !~ '{num_re}' THEN NULL \
                 ELSE pg_ripple.encode_typed_literal(\
                   trunc(({lex})::numeric)::bigint::text, \
                   'http://www.w3.org/2001/XMLSchema#integer') END"
            )
        }
        "http://www.w3.org/2001/XMLSchema#decimal" => {
            format!(
                "CASE WHEN ({lex}) IS NULL OR ({lex}) !~ '{num_re}' THEN NULL \
                 ELSE pg_ripple.encode_typed_literal(\
                   trim_scale(({lex})::numeric)::text, \
                   'http://www.w3.org/2001/XMLSchema#decimal') END"
            )
        }
        "http://www.w3.org/2001/XMLSchema#double" | "http://www.w3.org/2001/XMLSchema#float" => {
            format!(
                "CASE WHEN ({lex}) IS NULL OR ({lex}) !~ '{num_re}' THEN NULL \
                 ELSE pg_ripple.encode_typed_literal(\
                   pg_ripple.xsd_double_fmt(({lex})::float8::text), \
                   'http://www.w3.org/2001/XMLSchema#double') END"
            )
        }
        "http://www.w3.org/2001/XMLSchema#string" => {
            // Re-encode the lexical value as xsd:string (= plain literal in RDF 1.1).
            format!(
                "pg_ripple.encode_typed_literal(\
                   {lex}, \
                   'http://www.w3.org/2001/XMLSchema#string')"
            )
        }
        "http://www.w3.org/2001/XMLSchema#boolean" => {
            format!(
                "pg_ripple.encode_typed_literal(\
                   CASE WHEN lower({lex}) IN ('true','1') THEN 'true' \
                        WHEN lower({lex}) IN ('false','0') THEN 'false' \
                        ELSE NULL END, \
                   'http://www.w3.org/2001/XMLSchema#boolean')"
            )
        }
        "http://www.w3.org/2001/XMLSchema#dateTime" => {
            format!(
                "pg_ripple.encode_typed_literal(\
                   ({lex})::timestamptz::text, \
                   'http://www.w3.org/2001/XMLSchema#dateTime')"
            )
        }
        _ => "NULL".to_string(),
    }
}
