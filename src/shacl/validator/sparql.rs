#![allow(unused_imports, dead_code)]
use pgrx::datum::DatumWithOid;
use pgrx::prelude::*;
use serde::Serialize;
use crate::shacl::constraints;
use crate::shacl::{PropertyShape, Shape, ShapeConstraint, ShapeTarget};
use super::severity::Violation;
use super::node::{validate_sync, node_conforms_to_shape, count_qualifying_values};
use super::property::{
    validate_property_shape, get_all_predicate_iris_for_node, collect_focus_nodes, get_value_ids,
    get_vp_table_name, encode_shacl_in_value, value_has_datatype,
    value_has_rdf_type, value_has_node_kind, get_language_tag,
    compare_dictionary_values, count_values_in_graph,
};

/// Like `validate_sync` but accepts a pre-loaded shapes slice.
pub(crate) fn validate_sync_with_shapes(
    s_id: i64,
    p_id: i64,
    o_id: i64,
    g_id: i64,
    shapes: &[Shape],
) -> Result<(), String> {
    for shape in shapes {
        if shape.deactivated {
            continue;
        }

        let is_focus = match &shape.target {
            ShapeTarget::None => false,
            ShapeTarget::Node(iris) => iris
                .iter()
                .any(|iri| crate::dictionary::lookup_iri(iri) == Some(s_id)),
            ShapeTarget::Class(class_iri) => {
                let class_id = match crate::dictionary::lookup_iri(class_iri) {
                    Some(id) => id,
                    None => continue,
                };
                let rdf_type_id = match crate::dictionary::lookup_iri(
                    "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
                ) {
                    Some(id) => id,
                    None => continue,
                };
                value_has_rdf_type(s_id, rdf_type_id, class_id)
            }
            ShapeTarget::SubjectsOf(pred_iri) => {
                crate::dictionary::lookup_iri(pred_iri) == Some(p_id)
            }
            ShapeTarget::ObjectsOf(pred_iri) => {
                crate::dictionary::lookup_iri(pred_iri) == Some(p_id)
            }
        };

        if !is_focus {
            continue;
        }

        for ps in &shape.properties {
            let ps_path_id = match crate::dictionary::lookup_iri(&ps.path_iri) {
                Some(id) => id,
                None => continue,
            };
            if ps_path_id != p_id {
                continue;
            }

            for c in &ps.constraints {
                match c {
                    ShapeConstraint::MaxCount(n) => {
                        let current = count_values_in_graph(s_id, p_id, g_id);
                        if current + 1 > *n {
                            let focus_iri = crate::dictionary::decode(s_id)
                                .unwrap_or_else(|| format!("_id_{s_id}"));
                            return Err(format!(
                                "SHACL violation: <{}> sh:maxCount {n} for <{}>: \
                                 found {} existing value(s), limit is {n}",
                                focus_iri, ps.path_iri, current
                            ));
                        }
                    }
                    ShapeConstraint::Datatype(dt_iri) => {
                        if !value_has_datatype(o_id, dt_iri) {
                            let focus_iri = crate::dictionary::decode(s_id)
                                .unwrap_or_else(|| format!("_id_{s_id}"));
                            return Err(format!(
                                "SHACL violation: <{}> sh:datatype <{dt_iri}> for <{}>: \
                                 object id {o_id} does not have the required datatype",
                                focus_iri, ps.path_iri
                            ));
                        }
                    }
                    ShapeConstraint::In(allowed_iris) => {
                        let allowed_ids: Vec<i64> = allowed_iris
                            .iter()
                            .filter_map(|iri| crate::dictionary::lookup_iri(iri))
                            .collect();
                        if !allowed_ids.contains(&o_id) {
                            let focus_iri = crate::dictionary::decode(s_id)
                                .unwrap_or_else(|| format!("_id_{s_id}"));
                            return Err(format!(
                                "SHACL violation: <{}> sh:in for <{}>: \
                                 object id {o_id} is not in the allowed value set",
                                focus_iri, ps.path_iri
                            ));
                        }
                    }
                    ShapeConstraint::Pattern(regex, _) => {
                        let lexical = crate::dictionary::decode(o_id).unwrap_or_default();
                        let lexical_clean = if lexical.starts_with('"') {
                            lexical
                                .trim_start_matches('"')
                                .split('"')
                                .next()
                                .unwrap_or(&lexical)
                                .to_owned()
                        } else {
                            lexical.clone()
                        };
                        let matches: Option<bool> = Spi::get_one_with_args::<bool>(
                            "SELECT $1 ~ $2",
                            &[
                                DatumWithOid::from(lexical_clean.as_str()),
                                DatumWithOid::from(regex.as_str()),
                            ],
                        )
                        .unwrap_or(None);
                        if !matches.unwrap_or(false) {
                            let focus_iri = crate::dictionary::decode(s_id)
                                .unwrap_or_else(|| format!("_id_{s_id}"));
                            return Err(format!(
                                "SHACL violation: <{}> sh:pattern '{regex}' for <{}>: \
                                 value '{lexical_clean}' does not match",
                                focus_iri, ps.path_iri
                            ));
                        }
                    }
                    ShapeConstraint::MinCount(_) => {}
                    ShapeConstraint::Class(class_iri) => {
                        let class_id_opt = crate::dictionary::lookup_iri(class_iri);
                        let rdf_type_id_opt = crate::dictionary::lookup_iri(
                            "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
                        );
                        let has_class = match (class_id_opt, rdf_type_id_opt) {
                            (Some(cid), Some(tid)) => value_has_rdf_type(o_id, tid, cid),
                            _ => false,
                        };
                        if !has_class {
                            let focus_iri = crate::dictionary::decode(s_id)
                                .unwrap_or_else(|| format!("_id_{s_id}"));
                            return Err(format!(
                                "SHACL violation: <{}> sh:class <{class_iri}> for <{}>: \
                                 object id {o_id} is not an instance of the required class",
                                focus_iri, ps.path_iri
                            ));
                        }
                    }
                    ShapeConstraint::Node(ref_shape_iri) => {
                        if !node_conforms_to_shape(o_id, ref_shape_iri, g_id, shapes) {
                            let focus_iri = crate::dictionary::decode(s_id)
                                .unwrap_or_else(|| format!("_id_{s_id}"));
                            return Err(format!(
                                "SHACL violation: <{}> sh:node <{ref_shape_iri}> for <{}>: \
                                 object id {o_id} does not conform to the referenced shape",
                                focus_iri, ps.path_iri
                            ));
                        }
                    }
                    ShapeConstraint::Or(shape_iris) => {
                        let conforms = shape_iris
                            .iter()
                            .any(|s| node_conforms_to_shape(o_id, s, g_id, shapes));
                        if !conforms {
                            let focus_iri = crate::dictionary::decode(s_id)
                                .unwrap_or_else(|| format!("_id_{s_id}"));
                            return Err(format!(
                                "SHACL violation: <{}> sh:or for <{}>: \
                                 object id {o_id} does not conform to any of the sh:or shapes",
                                focus_iri, ps.path_iri
                            ));
                        }
                    }
                    ShapeConstraint::And(shape_iris) => {
                        for s in shape_iris {
                            if !node_conforms_to_shape(o_id, s, g_id, shapes) {
                                let focus_iri = crate::dictionary::decode(s_id)
                                    .unwrap_or_else(|| format!("_id_{s_id}"));
                                return Err(format!(
                                    "SHACL violation: <{}> sh:and <{s}> for <{}>: \
                                     object id {o_id} does not conform to the required shape",
                                    focus_iri, ps.path_iri
                                ));
                            }
                        }
                    }
                    ShapeConstraint::Not(ref_shape_iri) => {
                        if node_conforms_to_shape(o_id, ref_shape_iri, g_id, shapes) {
                            let focus_iri = crate::dictionary::decode(s_id)
                                .unwrap_or_else(|| format!("_id_{s_id}"));
                            return Err(format!(
                                "SHACL violation: <{}> sh:not <{ref_shape_iri}> for <{}>: \
                                 object id {o_id} must not conform to the referenced shape",
                                focus_iri, ps.path_iri
                            ));
                        }
                    }
                    ShapeConstraint::QualifiedValueShape {
                        shape_iri: qvs_iri,
                        min_count: _,
                        max_count,
                    } => {
                        if let Some(max) = max_count {
                            let existing_qualifying =
                                count_qualifying_values(s_id, p_id, g_id, qvs_iri, shapes);
                            if existing_qualifying + 1 > *max {
                                let focus_iri = crate::dictionary::decode(s_id)
                                    .unwrap_or_else(|| format!("_id_{s_id}"));
                                return Err(format!(
                                    "SHACL violation: <{}> sh:qualifiedMaxCount {max} for <{}>: \
                                     found {} qualifying value(s), limit is {max}",
                                    focus_iri, ps.path_iri, existing_qualifying
                                ));
                            }
                        }
                    }
                    ShapeConstraint::HasValue(expected_val) => {
                        let _ = expected_val;
                    }
                    ShapeConstraint::NodeKind(kind_iri) => {
                        if !value_has_node_kind(o_id, kind_iri) {
                            let focus_iri = crate::dictionary::decode(s_id)
                                .unwrap_or_else(|| format!("_id_{s_id}"));
                            return Err(format!(
                                "SHACL violation: <{}> sh:nodeKind <{kind_iri}> for <{}>: \
                                 value id {o_id} does not match required node kind",
                                focus_iri, ps.path_iri
                            ));
                        }
                    }
                    ShapeConstraint::LanguageIn(allowed_tags) => {
                        let lang_opt = get_language_tag(o_id);
                        let ok = match &lang_opt {
                            Some(lang) => {
                                let lang_lower = lang.to_lowercase();
                                allowed_tags.iter().any(|t| {
                                    let bare = t.trim_matches('"');
                                    bare.to_lowercase() == lang_lower
                                })
                            }
                            None => false,
                        };
                        if !ok {
                            let focus_iri = crate::dictionary::decode(s_id)
                                .unwrap_or_else(|| format!("_id_{s_id}"));
                            return Err(format!(
                                "SHACL violation: <{}> sh:languageIn for <{}>: \
                                 value id {o_id} language {:?} not in allowed list {:?}",
                                focus_iri,
                                ps.path_iri,
                                lang_opt.as_deref().unwrap_or("none"),
                                allowed_tags
                            ));
                        }
                    }
                    ShapeConstraint::UniqueLang
                    | ShapeConstraint::LessThan(_)
                    | ShapeConstraint::LessThanOrEquals(_)
                    | ShapeConstraint::GreaterThan(_)
                    | ShapeConstraint::Closed { .. }
                    | ShapeConstraint::Equals(_)
                    | ShapeConstraint::Disjoint(_)
                    | ShapeConstraint::MinLength(_)
                    | ShapeConstraint::MaxLength(_)
                    | ShapeConstraint::Xone(_)
                    | ShapeConstraint::MinExclusive(_)
                    | ShapeConstraint::MaxExclusive(_)
                    | ShapeConstraint::MinInclusive(_)
                    | ShapeConstraint::MaxInclusive(_)
                    | ShapeConstraint::SparqlConstraint { .. }
                    // v0.106.0: sh:validFor — not applicable to insert-time guard; skip.
                    | ShapeConstraint::ValidFor(_) => {}
                }
            }
        }
    }

    Ok(())
}
