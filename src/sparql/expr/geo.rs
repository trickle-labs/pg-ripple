//! GeoSPARQL custom function translation (H17-02, v0.122.0).
//!
//! Handles `geof:distance`, `geof:area`, `geof:boundary`, `geof:buffer`,
//! `geof:convexHull`, `geof:envelope`, `geo:asWKT`, `geo:hasSpatialAccuracy`.

use std::collections::HashMap;

use spargebra::algebra::Expression;

use super::super::sqlgen::Ctx;
use super::{decode_lexical_sql, postgis_available, translate_arg_value};

fn encode_literal(sql: String) -> String {
    format!("pg_ripple.encode_term({sql}, 2::int2)")
}

/// Translate a GeoSPARQL custom function IRI in value context.
///
/// Returns `Some(sql)` for recognised GeoSPARQL IRIs and `None` for all others.
/// Sets `is_numeric = true` for functions that return raw numeric values.
pub(super) fn translate_custom(
    iri: &str,
    args: &[Expression],
    bindings: &HashMap<String, String>,
    ctx: &mut Ctx,
    is_numeric: &mut bool,
) -> Option<String> {
    match iri {
        "http://www.opengis.net/def/function/geosparql/distance" => {
            // geof:distance(?a, ?b, unit) → numeric distance (metres for unit-of-measure)
            *is_numeric = true;
            if !postgis_available() {
                return Some("NULL".to_string());
            }
            let a_col = translate_arg_value(args.first()?, bindings, ctx)?;
            let b_col = translate_arg_value(args.get(1)?, bindings, ctx)?;
            let a_wkt = decode_lexical_sql(&a_col);
            let b_wkt = decode_lexical_sql(&b_col);
            Some(format!(
                "ST_Distance(\
                    ST_GeomFromText({a_wkt})::geography, \
                    ST_GeomFromText({b_wkt})::geography\
                  )"
            ))
        }
        "http://www.opengis.net/def/function/geosparql/area" => {
            *is_numeric = true;
            if !postgis_available() {
                return Some("NULL".to_string());
            }
            let a_col = translate_arg_value(args.first()?, bindings, ctx)?;
            let a_wkt = decode_lexical_sql(&a_col);
            Some(format!("ST_Area(ST_GeomFromText({a_wkt})::geography)"))
        }
        "http://www.opengis.net/def/function/geosparql/boundary" => {
            // Returns a WKT literal of the boundary geometry.
            if !postgis_available() {
                return Some(encode_literal("NULL".to_string()));
            }
            let a_col = translate_arg_value(args.first()?, bindings, ctx)?;
            let a_wkt = decode_lexical_sql(&a_col);
            Some(encode_literal(format!(
                "ST_AsText(ST_Boundary(ST_GeomFromText({a_wkt})))"
            )))
        }
        // v0.56.0 L-1.1: geof:buffer, geof:convexHull, geof:envelope
        "http://www.opengis.net/def/function/geosparql/buffer" => {
            // geof:buffer(?geom, radius, units) → WKT of buffered geometry.
            if !postgis_available() {
                return Some(encode_literal("NULL".to_string()));
            }
            let a_col = translate_arg_value(args.first()?, bindings, ctx)?;
            let a_wkt = decode_lexical_sql(&a_col);
            // Radius arg: literal numeric or variable. Default 0.
            let radius_sql = args.get(1).map_or("0".to_string(), |e| {
                if let Expression::Literal(lit) = e {
                    lit.value().to_owned()
                } else {
                    translate_arg_value(e, bindings, ctx)
                        .map(|c| decode_lexical_sql(&c))
                        .unwrap_or_else(|| "0".to_string())
                }
            });
            Some(encode_literal(format!(
                "ST_AsText(ST_Buffer(ST_GeomFromText({a_wkt}), {radius_sql}))"
            )))
        }
        "http://www.opengis.net/def/function/geosparql/convexHull" => {
            // geof:convexHull(?geom) → WKT of convex hull.
            if !postgis_available() {
                return Some(encode_literal("NULL".to_string()));
            }
            let a_col = translate_arg_value(args.first()?, bindings, ctx)?;
            let a_wkt = decode_lexical_sql(&a_col);
            Some(encode_literal(format!(
                "ST_AsText(ST_ConvexHull(ST_GeomFromText({a_wkt})))"
            )))
        }
        "http://www.opengis.net/def/function/geosparql/envelope" => {
            // geof:envelope(?geom) → WKT of bounding box.
            if !postgis_available() {
                return Some(encode_literal("NULL".to_string()));
            }
            let a_col = translate_arg_value(args.first()?, bindings, ctx)?;
            let a_wkt = decode_lexical_sql(&a_col);
            Some(encode_literal(format!(
                "ST_AsText(ST_Envelope(ST_GeomFromText({a_wkt})))"
            )))
        }
        // v0.56.0 L-1.1: geo:asWKT and geo:hasSpatialAccuracy
        "http://www.opengis.net/ontology/spatialrelations/asWKT"
        | "http://www.opengis.net/ont/geosparql#asWKT" => {
            // Decode the IRI column to its lexical string value.
            let col = translate_arg_value(args.first()?, bindings, ctx)?;
            Some(decode_lexical_sql(&col))
        }
        // geo:hasSpatialAccuracy(iri) → literal value of spatial accuracy.
        "http://www.opengis.net/ont/geosparql#hasSpatialAccuracy" => {
            let col = translate_arg_value(args.first()?, bindings, ctx)?;
            Some(decode_lexical_sql(&col))
        }
        _ => None,
    }
}
