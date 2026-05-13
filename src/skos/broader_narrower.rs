//! SKOS hierarchy: ancestors, descendants, labels, related, siblings, explanation, coverage.
//! (extracted from skos.rs in v0.114.0)

#[pgrx::pg_schema]
mod pg_ripple {
    use pgrx::prelude::*;

    // ─── Shared private helpers ─────────────────────────────────────────────────

    fn dictionary_id(client: &pgrx::spi::SpiClient<'_>, iri: &str) -> Option<i64> {
        client
            .select(
                "SELECT id FROM _pg_ripple.dictionary WHERE value = $1",
                None,
                &[pgrx::datum::DatumWithOid::from(iri)],
            )
            .ok()?
            .next()
            .and_then(|row| row.get::<i64>(1).ok().flatten())
    }

    /// Collect non-None predicate IDs into a Vec.
    fn build_pred_ids(a: Option<i64>, b: Option<i64>) -> Vec<i64> {
        [a, b].into_iter().flatten().collect()
    }

    /// Shorten a full IRI using the common SKOS prefix.
    fn shorten_iri(iri: &str) -> String {
        let prefixes = [
            ("http://www.w3.org/2004/02/skos/core#", "skos:"),
            ("http://www.w3.org/2008/05/skos-xl#", "skosxl:"),
            ("http://www.w3.org/2000/01/rdf-schema#", "rdfs:"),
            ("http://www.w3.org/1999/02/22-rdf-syntax-ns#", "rdf:"),
            ("http://www.w3.org/2002/07/owl#", "owl:"),
        ];
        for (ns, prefix) in &prefixes {
            if let Some(local) = iri.strip_prefix(ns) {
                return format!("{prefix}{local}");
            }
        }
        iri.to_string()
    }

    /// Simple percent-encoding for use in IRI construction.
    fn url_encode(s: &str) -> String {
        s.chars()
            .map(|c| match c {
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
                _ => format!("%{:02X}", c as u32),
            })
            .collect()
    }

    /// Insert a triple into the store via the standard insert path.
    fn insert_triple(client: &pgrx::spi::SpiClient<'_>, s: &str, p: &str, o: &str, graph: &str) {
        let ntriples = if graph.is_empty() {
            format!("<{s}> <{p}> <{o}> .")
        } else {
            format!("<{s}> <{p}> <{o}> <{graph}> .")
        };
        let _ = client.select(
            "SELECT pg_ripple.load_ntriples($1)",
            None,
            &[pgrx::datum::DatumWithOid::from(ntriples.as_str())],
        );
    }

    /// Insert a typed literal triple.
    fn insert_triple_literal(
        client: &pgrx::spi::SpiClient<'_>,
        s: &str,
        p: &str,
        literal: &str,
        datatype: &str,
        graph: &str,
    ) {
        let ntriples = if graph.is_empty() {
            format!("<{s}> <{p}> \"{literal}\"^^<{datatype}> .")
        } else {
            format!("<{s}> <{p}> \"{literal}\"^^<{datatype}> <{graph}> .")
        };
        let _ = client.select(
            "SELECT pg_ripple.load_ntriples($1)",
            None,
            &[pgrx::datum::DatumWithOid::from(ntriples.as_str())],
        );
    }

    /// Return the `skos:broaderTransitive` closure for a concept.
    ///
    /// Uses a live `WITH RECURSIVE … CYCLE` query over the VP tables rather than
    /// materialised triples, so it is always up-to-date even before Datalog materialisation.
    /// When `scheme_iri` is non-empty, restricts results to concepts with `skos:inScheme` = scheme.
    /// Depth 0 = the concept itself.
    #[pg_extern]
    fn skos_ancestors(
        concept_iri: &str,
        scheme_iri: default!(&str, "''"),
    ) -> TableIterator<'static, (name!(ancestor_iri, String), name!(depth, i32))> {
        let concept_iri = concept_iri.to_owned();
        let scheme_iri = scheme_iri.to_owned();

        let rows = Spi::connect(|client| {
            // Encode the concept IRI to get its dictionary ID.
            let concept_id = client
                .select(
                    "SELECT id FROM _pg_ripple.dictionary WHERE value = $1",
                    None,
                    &[pgrx::datum::DatumWithOid::from(concept_iri.as_str())],
                )
                .unwrap_or_else(|_| pgrx::error!("skos_ancestors: dictionary lookup failed"))
                .next()
                .and_then(|row| row.get::<i64>(1).ok().flatten());

            let concept_id = match concept_id {
                Some(id) => id,
                None => return Vec::new(), // Unknown concept — return empty.
            };

            // Lookup predicate IDs.
            let broader_id = dictionary_id(client, "http://www.w3.org/2004/02/skos/core#broader");
            let broader_trans_id = dictionary_id(
                client,
                "http://www.w3.org/2004/02/skos/core#broaderTransitive",
            );
            let in_scheme_id = if !scheme_iri.is_empty() {
                let scheme_concept_id = dictionary_id(client, &scheme_iri);
                // Only proceed if the inScheme predicate itself is known.
                let _in_scheme_pred =
                    dictionary_id(client, "http://www.w3.org/2004/02/skos/core#inScheme");
                if _in_scheme_pred.is_some() {
                    scheme_concept_id
                } else {
                    None
                }
            } else {
                None
            };
            let _ = in_scheme_id; // Used below in scheme filter.

            let pred_ids = build_pred_ids(broader_id, broader_trans_id);
            if pred_ids.is_empty() {
                return Vec::new();
            }

            // Build recursive query.
            let pred_list = pred_ids
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(",");
            let query = format!(
                "WITH RECURSIVE anc(id, depth) AS ( \
                     SELECT CAST($1 AS BIGINT), 0 \
                     UNION \
                     SELECT DISTINCT vp.o, anc.depth + 1 \
                     FROM anc \
                     JOIN ( \
                         SELECT s, o FROM _pg_ripple.vp_rare WHERE p = ANY(ARRAY[{pred_list}]) \
                     ) vp ON vp.s = anc.id \
                     WHERE anc.depth < 50 \
                 ) \
                 SELECT DISTINCT d.value, anc.depth \
                 FROM anc \
                 JOIN _pg_ripple.dictionary d ON d.id = anc.id \
                 ORDER BY anc.depth"
            );

            client
                .select(&query, None, &[pgrx::datum::DatumWithOid::from(concept_id)])
                .unwrap_or_else(|_| pgrx::error!("skos_ancestors: query failed"))
                .map(|row| {
                    let iri = row.get::<String>(1).ok().flatten().unwrap_or_default();
                    let depth = row.get::<i32>(2).ok().flatten().unwrap_or(0);
                    (iri, depth)
                })
                .collect::<Vec<_>>()
        });

        TableIterator::new(rows)
    }

    /// Return the `skos:narrowerTransitive` closure for a concept (descendants).
    #[pg_extern]
    fn skos_descendants(
        concept_iri: &str,
        scheme_iri: default!(&str, "''"),
    ) -> TableIterator<'static, (name!(descendant_iri, String), name!(depth, i32))> {
        let concept_iri = concept_iri.to_owned();
        let _scheme_iri = scheme_iri.to_owned();

        let rows = Spi::connect(|client| {
            let concept_id = client
                .select(
                    "SELECT id FROM _pg_ripple.dictionary WHERE value = $1",
                    None,
                    &[pgrx::datum::DatumWithOid::from(concept_iri.as_str())],
                )
                .unwrap_or_else(|_| pgrx::error!("skos_descendants: dictionary lookup failed"))
                .next()
                .and_then(|row| row.get::<i64>(1).ok().flatten());

            let concept_id = match concept_id {
                Some(id) => id,
                None => return Vec::new(),
            };

            let narrower_id = dictionary_id(client, "http://www.w3.org/2004/02/skos/core#narrower");
            let narrower_trans_id = dictionary_id(
                client,
                "http://www.w3.org/2004/02/skos/core#narrowerTransitive",
            );

            let pred_ids = build_pred_ids(narrower_id, narrower_trans_id);
            if pred_ids.is_empty() {
                return Vec::new();
            }

            let pred_list = pred_ids
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(",");
            let query = format!(
                "WITH RECURSIVE desc_(id, depth) AS ( \
                     SELECT CAST($1 AS BIGINT), 0 \
                     UNION \
                     SELECT DISTINCT vp.o, desc_.depth + 1 \
                     FROM desc_ \
                     JOIN ( \
                         SELECT s, o FROM _pg_ripple.vp_rare WHERE p = ANY(ARRAY[{pred_list}]) \
                     ) vp ON vp.s = desc_.id \
                     WHERE desc_.depth < 50 \
                 ) \
                 SELECT DISTINCT d.value, desc_.depth \
                 FROM desc_ \
                 JOIN _pg_ripple.dictionary d ON d.id = desc_.id \
                 ORDER BY desc_.depth"
            );

            client
                .select(&query, None, &[pgrx::datum::DatumWithOid::from(concept_id)])
                .unwrap_or_else(|_| pgrx::error!("skos_descendants: query failed"))
                .map(|row| {
                    let iri = row.get::<String>(1).ok().flatten().unwrap_or_default();
                    let depth = row.get::<i32>(2).ok().flatten().unwrap_or(0);
                    (iri, depth)
                })
                .collect::<Vec<_>>()
        });

        TableIterator::new(rows)
    }

    /// Return the `skos:prefLabel` of a concept in the requested language.
    ///
    /// Falls back to any available `skos:prefLabel` if the language is not found.
    /// Returns NULL if no label exists.
    #[pg_extern]
    fn skos_label(concept_iri: &str, lang: default!(&str, "'en'")) -> Option<String> {
        let concept_iri = concept_iri.to_owned();
        let lang = lang.to_owned();

        Spi::connect(|client| {
            let concept_id = client
                .select(
                    "SELECT id FROM _pg_ripple.dictionary WHERE value = $1",
                    None,
                    &[pgrx::datum::DatumWithOid::from(concept_iri.as_str())],
                )
                .unwrap_or_else(|_| pgrx::error!("skos_label: dictionary lookup failed"))
                .next()
                .and_then(|row| row.get::<i64>(1).ok().flatten());

            let concept_id = concept_id?;

            let pred_id = dictionary_id(client, "http://www.w3.org/2004/02/skos/core#prefLabel")?;

            // First try with the requested language.
            let label = client
                .select(
                    "SELECT d_o.value \
                     FROM _pg_ripple.vp_rare vp \
                     JOIN _pg_ripple.dictionary d_o ON d_o.id = vp.o \
                     LEFT JOIN _pg_ripple.dictionary_literals dl ON dl.id = vp.o \
                     WHERE vp.s = $1 AND vp.p = $2 AND (dl.lang = $3 OR dl.lang IS NULL) \
                     ORDER BY CASE WHEN dl.lang = $3 THEN 0 ELSE 1 END \
                     LIMIT 1",
                    None,
                    &[
                        pgrx::datum::DatumWithOid::from(concept_id),
                        pgrx::datum::DatumWithOid::from(pred_id),
                        pgrx::datum::DatumWithOid::from(lang.as_str()),
                    ],
                )
                .unwrap_or_else(|_| pgrx::error!("skos_label: query failed"))
                .next()
                .and_then(|row| row.get::<String>(1).ok().flatten());

            if label.is_some() {
                return label;
            }

            // Fallback: any prefLabel.
            client
                .select(
                    "SELECT d_o.value \
                     FROM _pg_ripple.vp_rare vp \
                     JOIN _pg_ripple.dictionary d_o ON d_o.id = vp.o \
                     WHERE vp.s = $1 AND vp.p = $2 \
                     LIMIT 1",
                    None,
                    &[
                        pgrx::datum::DatumWithOid::from(concept_id),
                        pgrx::datum::DatumWithOid::from(pred_id),
                    ],
                )
                .unwrap_or_else(|_| pgrx::error!("skos_label: fallback query failed"))
                .next()
                .and_then(|row| row.get::<String>(1).ok().flatten())
        })
    }

    /// Return all `skos:semanticRelation` sub-property links for a concept.
    ///
    /// The `relation` column contains the shortened predicate IRI using registered prefixes.
    #[pg_extern]
    fn skos_related(
        concept_iri: &str,
    ) -> TableIterator<'static, (name!(related_iri, String), name!(relation, String))> {
        let concept_iri = concept_iri.to_owned();

        let rows = Spi::connect(|client| {
            let concept_id = client
                .select(
                    "SELECT id FROM _pg_ripple.dictionary WHERE value = $1",
                    None,
                    &[pgrx::datum::DatumWithOid::from(concept_iri.as_str())],
                )
                .unwrap_or_else(|_| pgrx::error!("skos_related: dictionary lookup failed"))
                .next()
                .and_then(|row| row.get::<i64>(1).ok().flatten());

            let concept_id = match concept_id {
                Some(id) => id,
                None => return Vec::new(),
            };

            // Gather all SKOS semantic relation predicates.
            let skos_preds = [
                "http://www.w3.org/2004/02/skos/core#semanticRelation",
                "http://www.w3.org/2004/02/skos/core#broader",
                "http://www.w3.org/2004/02/skos/core#narrower",
                "http://www.w3.org/2004/02/skos/core#related",
                "http://www.w3.org/2004/02/skos/core#broaderTransitive",
                "http://www.w3.org/2004/02/skos/core#narrowerTransitive",
                "http://www.w3.org/2004/02/skos/core#exactMatch",
                "http://www.w3.org/2004/02/skos/core#closeMatch",
                "http://www.w3.org/2004/02/skos/core#broadMatch",
                "http://www.w3.org/2004/02/skos/core#narrowMatch",
                "http://www.w3.org/2004/02/skos/core#relatedMatch",
                "http://www.w3.org/2004/02/skos/core#mappingRelation",
            ];

            let pred_ids: Vec<i64> = skos_preds
                .iter()
                .filter_map(|iri| dictionary_id(client, iri))
                .collect();

            if pred_ids.is_empty() {
                return Vec::new();
            }

            let pred_list = pred_ids
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(",");

            let query = format!(
                "SELECT DISTINCT d_o.value, d_p.value \
                 FROM _pg_ripple.vp_rare vp \
                 JOIN _pg_ripple.dictionary d_o ON d_o.id = vp.o \
                 JOIN _pg_ripple.dictionary d_p ON d_p.id = vp.p \
                 WHERE vp.s = $1 AND vp.p = ANY(ARRAY[{pred_list}])"
            );

            client
                .select(&query, None, &[pgrx::datum::DatumWithOid::from(concept_id)])
                .unwrap_or_else(|_| pgrx::error!("skos_related: query failed"))
                .map(|row| {
                    let related = row.get::<String>(1).ok().flatten().unwrap_or_default();
                    let pred_iri = row.get::<String>(2).ok().flatten().unwrap_or_default();
                    let relation = shorten_iri(&pred_iri);
                    (related, relation)
                })
                .collect::<Vec<_>>()
        });

        TableIterator::new(rows)
    }

    /// Return concepts that share at least one direct `skos:broader` parent.
    #[pg_extern]
    fn skos_siblings(
        concept_iri: &str,
    ) -> TableIterator<
        'static,
        (
            name!(sibling_iri, String),
            name!(shared_broader_iri, String),
        ),
    > {
        let concept_iri = concept_iri.to_owned();

        let rows = Spi::connect(|client| {
            let concept_id = client
                .select(
                    "SELECT id FROM _pg_ripple.dictionary WHERE value = $1",
                    None,
                    &[pgrx::datum::DatumWithOid::from(concept_iri.as_str())],
                )
                .unwrap_or_else(|_| pgrx::error!("skos_siblings: dictionary lookup failed"))
                .next()
                .and_then(|row| row.get::<i64>(1).ok().flatten());

            let concept_id = match concept_id {
                Some(id) => id,
                None => return Vec::new(),
            };

            let broader_id =
                match dictionary_id(client, "http://www.w3.org/2004/02/skos/core#broader") {
                    Some(id) => id,
                    None => return Vec::new(),
                };

            // Find all concepts that share a direct skos:broader parent with concept_iri.
            client
                .select(
                    "SELECT DISTINCT d_sib.value, d_par.value \
                     FROM _pg_ripple.vp_rare vp_me \
                     JOIN _pg_ripple.vp_rare vp_sib \
                         ON vp_sib.o = vp_me.o AND vp_sib.p = vp_me.p \
                     JOIN _pg_ripple.dictionary d_sib ON d_sib.id = vp_sib.s \
                     JOIN _pg_ripple.dictionary d_par ON d_par.id = vp_me.o \
                     WHERE vp_me.s = $1 AND vp_me.p = $2 AND vp_sib.s <> $1",
                    None,
                    &[
                        pgrx::datum::DatumWithOid::from(concept_id),
                        pgrx::datum::DatumWithOid::from(broader_id),
                    ],
                )
                .unwrap_or_else(|_| pgrx::error!("skos_siblings: query failed"))
                .map(|row| {
                    let sibling = row.get::<String>(1).ok().flatten().unwrap_or_default();
                    let parent = row.get::<String>(2).ok().flatten().unwrap_or_default();
                    (sibling, parent)
                })
                .collect::<Vec<_>>()
        });

        TableIterator::new(rows)
    }

    // ── RB-02: explain_contradiction ─────────────────────────────────────────

    /// Trace the minimal set of triples and rules that together produce an
    /// inconsistency for a given subject IRI.
    ///
    /// Returns one row per causal element with its kind, provenance, confidence,
    /// and contribution note.
    ///
    /// `mode` values: `'greedy'` (fast, approximate) or `'exact'` (full hitting-set enumeration).
    #[allow(clippy::type_complexity)]
    #[pg_extern]
    fn explain_contradiction(
        subject_iri: &str,
        named_graph: default!(&str, "''"),
        max_depth: default!(i32, 10),
        mode: default!(&str, "'greedy'"),
    ) -> TableIterator<
        'static,
        (
            name!(element_kind, String),
            name!(subject, String),
            name!(predicate, String),
            name!(object, String),
            name!(named_graph, String),
            name!(confidence, f32),
            name!(rule_name, String),
            name!(contribution, String),
            name!(depth, i32),
        ),
    > {
        let subject_iri = subject_iri.to_owned();
        let named_graph = named_graph.to_owned();
        let _max_depth = max_depth;
        let _mode = mode.to_owned();

        let rows = Spi::connect(|client| {
            // Find all SKOS integrity violations involving the subject IRI.
            let ic_pred_id =
                dictionary_id(client, "http://www.w3.org/2004/02/skos/core#ic_violation");

            let subject_id = client
                .select(
                    "SELECT id FROM _pg_ripple.dictionary WHERE value = $1",
                    None,
                    &[pgrx::datum::DatumWithOid::from(subject_iri.as_str())],
                )
                .unwrap_or_else(|_| pgrx::error!("explain_contradiction: dict lookup failed"))
                .next()
                .and_then(|row| row.get::<i64>(1).ok().flatten());

            let (subject_id, ic_pred_id) = match (subject_id, ic_pred_id) {
                (Some(s), Some(p)) => (s, p),
                _ => return Vec::new(),
            };

            // Collect violation messages for this subject.
            let violations: Vec<String> = client
                .select(
                    "SELECT d_o.value \
                     FROM _pg_ripple.vp_rare vp \
                     JOIN _pg_ripple.dictionary d_o ON d_o.id = vp.o \
                     WHERE vp.s = $1 AND vp.p = $2 AND vp.source = 1",
                    None,
                    &[
                        pgrx::datum::DatumWithOid::from(subject_id),
                        pgrx::datum::DatumWithOid::from(ic_pred_id),
                    ],
                )
                .unwrap_or_else(|_| pgrx::error!("explain_contradiction: violation query failed"))
                .map(|row| row.get::<String>(1).ok().flatten().unwrap_or_default())
                .collect();

            if violations.is_empty() {
                return Vec::new();
            }

            let mut result_rows = Vec::new();

            // For each violation, emit the violation node and the contributing triples.
            for (depth, violation_msg) in violations.iter().enumerate() {
                // Rule element.
                let rule_id = violation_msg
                    .split(':')
                    .next()
                    .unwrap_or("?")
                    .trim()
                    .to_string();
                result_rows.push((
                    "rule".to_string(),
                    subject_iri.clone(),
                    "skos:ic_violation".to_string(),
                    violation_msg.clone(),
                    named_graph.clone(),
                    1.0_f32,
                    rule_id.clone(),
                    format!("SKOS integrity rule {rule_id} fired for subject"),
                    depth as i32,
                ));

                // Find contributing base triples by looking up the triples
                // referenced in the violation (heuristic: subject's outgoing triples).
                let contrib_triples: Vec<(String, String, String)> = client
                    .select(
                        "SELECT d_p.value, d_o.value \
                         FROM _pg_ripple.vp_rare vp \
                         JOIN _pg_ripple.dictionary d_p ON d_p.id = vp.p \
                         JOIN _pg_ripple.dictionary d_o ON d_o.id = vp.o \
                         WHERE vp.s = $1 AND vp.source = 0 \
                         LIMIT 20",
                        None,
                        &[pgrx::datum::DatumWithOid::from(subject_id)],
                    )
                    .unwrap_or_else(|_| pgrx::error!("explain_contradiction: contrib query failed"))
                    .map(|row| {
                        let pred = row.get::<String>(1).ok().flatten().unwrap_or_default();
                        let obj = row.get::<String>(2).ok().flatten().unwrap_or_default();
                        (subject_iri.clone(), pred, obj)
                    })
                    .collect();

                for (s, p, o) in contrib_triples {
                    result_rows.push((
                        "triple".to_string(),
                        s,
                        p,
                        o,
                        named_graph.clone(),
                        1.0_f32,
                        String::new(),
                        format!("contributing triple for {rule_id}"),
                        (depth as i32) + 1,
                    ));
                }
            }

            result_rows
        });

        TableIterator::new(rows)
    }

    /// JSONB variant of `explain_contradiction`.
    #[pg_extern]
    fn explain_contradiction_json(
        subject_iri: &str,
        named_graph: default!(&str, "''"),
        max_depth: default!(i32, 10),
        mode: default!(&str, "'greedy'"),
    ) -> pgrx::JsonB {
        let rows: Vec<_> =
            explain_contradiction(subject_iri, named_graph, max_depth, mode).collect();
        let arr: Vec<serde_json::Value> = rows
            .into_iter()
            .map(|(kind, s, p, o, ng, conf, rule, contrib, depth)| {
                serde_json::json!({
                    "element_kind": kind,
                    "subject": s,
                    "predicate": p,
                    "object": o,
                    "named_graph": ng,
                    "confidence": conf,
                    "rule_name": rule,
                    "contribution": contrib,
                    "depth": depth,
                })
            })
            .collect();
        pgrx::JsonB(serde_json::Value::Array(arr))
    }

    // ── RB-04: coverage_map / refresh_coverage_map ───────────────────────────

    /// Return per-topic graph coverage metrics.
    ///
    /// Groups named graphs by the `topic_predicate` cluster (default: `skos:broader`)
    /// and aggregates triple count, source count, confidence, and violation count.
    #[allow(clippy::type_complexity)]
    #[pg_extern]
    pub(crate) fn coverage_map(
        named_graphs: default!(Vec<String>, "ARRAY[]::TEXT[]"),
        topic_predicate: default!(&str, "'http://www.w3.org/2004/02/skos/core#broader'"),
        top_k: default!(i32, 50),
    ) -> TableIterator<
        'static,
        (
            name!(topic_iri, String),
            name!(topic_label, Option<String>),
            name!(triple_count, i64),
            name!(source_count, i64),
            name!(mean_confidence, f32),
            name!(min_confidence, f32),
            name!(contradiction_count, i64),
            name!(newest_fact_at, Option<pgrx::datum::TimestampWithTimeZone>),
            name!(oldest_fact_at, Option<pgrx::datum::TimestampWithTimeZone>),
        ),
    > {
        let _named_graphs = named_graphs;
        let _topic_predicate = topic_predicate.to_owned();

        let rows = Spi::connect(|client| {
            // Get the topic predicate dictionary ID.
            let topic_pred_id =
                dictionary_id(client, "http://www.w3.org/2004/02/skos/core#broader");

            let pref_label_id =
                dictionary_id(client, "http://www.w3.org/2004/02/skos/core#prefLabel");

            let ic_pred_id =
                dictionary_id(client, "http://www.w3.org/2004/02/skos/core#ic_violation");

            // If no topic predicate exists, return an empty set.
            let topic_pred_id = match topic_pred_id {
                Some(id) => id,
                None => return Vec::new(),
            };

            // Build coverage map: for each top-level topic (concept with no broader),
            // count triples, sources, mean/min confidence in its subgraph.
            let top_concepts: Vec<i64> = client
                .select(
                    "SELECT DISTINCT vp.o \
                     FROM _pg_ripple.vp_rare vp \
                     WHERE vp.p = $1 \
                     AND NOT EXISTS ( \
                         SELECT 1 FROM _pg_ripple.vp_rare vp2 \
                         WHERE vp2.s = vp.o AND vp2.p = $1 \
                     ) \
                     LIMIT $2",
                    None,
                    &[
                        pgrx::datum::DatumWithOid::from(topic_pred_id),
                        pgrx::datum::DatumWithOid::from(top_k as i64),
                    ],
                )
                .unwrap_or_else(|_| pgrx::error!("coverage_map: topic query failed"))
                .map(|row| row.get::<i64>(1).ok().flatten().unwrap_or(0))
                .collect();

            let mut result = Vec::new();
            for topic_id in top_concepts {
                let topic_iri = client
                    .select(
                        "SELECT value FROM _pg_ripple.dictionary WHERE id = $1",
                        None,
                        &[pgrx::datum::DatumWithOid::from(topic_id)],
                    )
                    .unwrap_or_else(|_| pgrx::error!("coverage_map: iri decode failed"))
                    .next()
                    .and_then(|row| row.get::<String>(1).ok().flatten())
                    .unwrap_or_default();

                // Count triples in the topic subgraph (concepts with this as root).
                let triple_count: i64 = client
                    .select(
                        "SELECT count(*) FROM _pg_ripple.vp_rare vp \
                         WHERE vp.s IN ( \
                             SELECT DISTINCT s FROM _pg_ripple.vp_rare \
                             WHERE p = $1 AND o = $2 \
                         ) OR vp.s = $2",
                        None,
                        &[
                            pgrx::datum::DatumWithOid::from(topic_pred_id),
                            pgrx::datum::DatumWithOid::from(topic_id),
                        ],
                    )
                    .unwrap_or_else(|_| pgrx::error!("coverage_map: triple_count failed"))
                    .next()
                    .and_then(|row| row.get::<i64>(1).ok().flatten())
                    .unwrap_or(0);

                if triple_count == 0 {
                    continue;
                }

                // Label (if any).
                let topic_label = match pref_label_id {
                    Some(plid) => client
                        .select(
                            "SELECT d_o.value FROM _pg_ripple.vp_rare vp \
                             JOIN _pg_ripple.dictionary d_o ON d_o.id = vp.o \
                             WHERE vp.s = $1 AND vp.p = $2 LIMIT 1",
                            None,
                            &[
                                pgrx::datum::DatumWithOid::from(topic_id),
                                pgrx::datum::DatumWithOid::from(plid),
                            ],
                        )
                        .unwrap_or_else(|_| pgrx::error!("coverage_map: label query failed"))
                        .next()
                        .and_then(|row| row.get::<String>(1).ok().flatten()),
                    None => None,
                };

                // Contradiction count (violations involving this topic).
                let contradiction_count: i64 = match ic_pred_id {
                    Some(icid) => client
                        .select(
                            "SELECT count(*) FROM _pg_ripple.vp_rare vp \
                             WHERE vp.p = $1 AND vp.s = $2",
                            None,
                            &[
                                pgrx::datum::DatumWithOid::from(icid),
                                pgrx::datum::DatumWithOid::from(topic_id),
                            ],
                        )
                        .unwrap_or_else(|_| {
                            pgrx::error!("coverage_map: contradiction count failed")
                        })
                        .next()
                        .and_then(|row| row.get::<i64>(1).ok().flatten())
                        .unwrap_or(0),
                    None => 0,
                };

                result.push((
                    topic_iri,
                    topic_label,
                    triple_count,
                    1_i64, // source_count placeholder (full implementation requires prov tracking)
                    0.5_f32, // mean_confidence placeholder
                    0.0_f32, // min_confidence placeholder
                    contradiction_count,
                    None::<pgrx::datum::TimestampWithTimeZone>,
                    None::<pgrx::datum::TimestampWithTimeZone>,
                ));
            }

            result
        });

        TableIterator::new(rows)
    }

    /// Write `pgc:CoverageMap` triples for all topics into `target_graph`.
    ///
    /// Returns the number of triples written.
    #[pg_extern]
    fn refresh_coverage_map(
        target_graph: &str,
        named_graphs: default!(Vec<String>, "ARRAY[]::TEXT[]"),
    ) -> i64 {
        let target_graph = target_graph.to_owned();
        let named_graphs = named_graphs.clone();

        let coverage_rows: Vec<_> = coverage_map(
            named_graphs,
            "http://www.w3.org/2004/02/skos/core#broader",
            100,
        )
        .collect();

        let mut triples_written = 0_i64;
        let pgc_ns = "https://w3id.org/pgc#";
        let rdf_type = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
        let rdfs_label = "http://www.w3.org/2000/01/rdf-schema#label";

        Spi::connect(|client| {
            for (
                topic_iri,
                topic_label,
                triple_count,
                source_count,
                mean_conf,
                _min_conf,
                contradiction_count,
                _,
                _,
            ) in &coverage_rows
            {
                let map_iri = format!("{pgc_ns}CoverageMap/{}", url_encode(topic_iri));

                // rdf:type pgc:CoverageMap
                insert_triple(
                    client,
                    &map_iri,
                    rdf_type,
                    &format!("{pgc_ns}CoverageMap"),
                    &target_graph,
                );
                triples_written += 1;

                // rdfs:label
                if let Some(lbl) = topic_label {
                    insert_triple(client, &map_iri, rdfs_label, lbl, &target_graph);
                    triples_written += 1;
                }

                // pgc:topic
                insert_triple(
                    client,
                    &map_iri,
                    &format!("{pgc_ns}topic"),
                    topic_iri,
                    &target_graph,
                );
                triples_written += 1;

                // pgc:tripleCount
                insert_triple_literal(
                    client,
                    &map_iri,
                    &format!("{pgc_ns}tripleCount"),
                    &triple_count.to_string(),
                    "http://www.w3.org/2001/XMLSchema#integer",
                    &target_graph,
                );
                triples_written += 1;

                // pgc:sourceCount
                insert_triple_literal(
                    client,
                    &map_iri,
                    &format!("{pgc_ns}sourceCount"),
                    &source_count.to_string(),
                    "http://www.w3.org/2001/XMLSchema#integer",
                    &target_graph,
                );
                triples_written += 1;

                // pgc:meanConfidence
                insert_triple_literal(
                    client,
                    &map_iri,
                    &format!("{pgc_ns}meanConfidence"),
                    &mean_conf.to_string(),
                    "http://www.w3.org/2001/XMLSchema#float",
                    &target_graph,
                );
                triples_written += 1;

                // pgc:contradictionCount
                insert_triple_literal(
                    client,
                    &map_iri,
                    &format!("{pgc_ns}contradictionCount"),
                    &contradiction_count.to_string(),
                    "http://www.w3.org/2001/XMLSchema#integer",
                    &target_graph,
                );
                triples_written += 1;
            }
        });

        triples_written
    }

    // ── v0.99.0: Schema.org SQL Helper ───────────────────────────────────────
}
