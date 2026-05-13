//! SKOS export: schema_type_ancestors, foaf_persons.
//! (extracted from skos.rs in v0.114.0)

#[pgrx::pg_schema]
mod pg_ripple {
    use pgrx::prelude::*;

    #[pg_extern]
    fn schema_type_ancestors(iri: &str) -> TableIterator<'static, (name!(ancestor_type, String),)> {
        let iri = iri.to_owned();

        let rows = Spi::connect(|client| {
            // Look up the resource IRI in the dictionary.
            let resource_id = client
                .select(
                    "SELECT id FROM _pg_ripple.dictionary WHERE value = $1",
                    None,
                    &[pgrx::datum::DatumWithOid::from(iri.as_str())],
                )
                .unwrap_or_else(|_| pgrx::error!("schema_type_ancestors: dictionary lookup failed"))
                .next()
                .and_then(|row| row.get::<i64>(1).ok().flatten());

            let resource_id = match resource_id {
                Some(id) => id,
                None => return Vec::new(),
            };

            // Look up rdf:type predicate ID.
            let type_pred_id = match client
                .select(
                    "SELECT id FROM _pg_ripple.dictionary WHERE value = $1",
                    None,
                    &[pgrx::datum::DatumWithOid::from(
                        "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
                    )],
                )
                .unwrap_or_else(|_| pgrx::error!("schema_type_ancestors: type pred lookup failed"))
                .next()
                .and_then(|row| row.get::<i64>(1).ok().flatten())
            {
                Some(id) => id,
                None => return Vec::new(),
            };

            // Query all rdf:type objects for the resource (includes inferred types).
            client
                .select(
                    "SELECT d.value \
                     FROM _pg_ripple.vp_rare vp \
                     JOIN _pg_ripple.dictionary d ON d.id = vp.o \
                     WHERE vp.s = $1 AND vp.p = $2 \
                       AND d.value LIKE 'https://schema.org/%' \
                     ORDER BY d.value",
                    None,
                    &[
                        pgrx::datum::DatumWithOid::from(resource_id),
                        pgrx::datum::DatumWithOid::from(type_pred_id),
                    ],
                )
                .unwrap_or_else(|_| pgrx::error!("schema_type_ancestors: query failed"))
                .map(|row| {
                    let type_iri = row.get::<String>(1).ok().flatten().unwrap_or_default();
                    (type_iri,)
                })
                .collect::<Vec<_>>()
        });

        TableIterator::new(rows)
    }

    // ── v0.99.0: FOAF SQL Helper ─────────────────────────────────────────────

    /// Return all `foaf:Person` IRIs visible in the current graph with their `foaf:name` label.
    ///
    /// Returns (person_iri, name_label) pairs.  `name_label` is NULL when no `foaf:name` is present.
    #[pg_extern]
    fn foaf_persons()
    -> TableIterator<'static, (name!(person_iri, String), name!(name_label, Option<String>))> {
        let rows = Spi::connect(|client| {
            // Look up predicate IDs.
            let person_class_id = client
                .select(
                    "SELECT id FROM _pg_ripple.dictionary WHERE value = $1",
                    None,
                    &[pgrx::datum::DatumWithOid::from(
                        "http://xmlns.com/foaf/0.1/Person",
                    )],
                )
                .unwrap_or_else(|_| pgrx::error!("foaf_persons: dictionary lookup failed"))
                .next()
                .and_then(|row| row.get::<i64>(1).ok().flatten());

            let person_class_id = match person_class_id {
                Some(id) => id,
                None => return Vec::new(), // foaf:Person not in dictionary yet
            };

            let type_pred_id = client
                .select(
                    "SELECT id FROM _pg_ripple.dictionary WHERE value = $1",
                    None,
                    &[pgrx::datum::DatumWithOid::from(
                        "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
                    )],
                )
                .unwrap_or_else(|_| pgrx::error!("foaf_persons: type pred lookup failed"))
                .next()
                .and_then(|row| row.get::<i64>(1).ok().flatten());

            let type_pred_id = match type_pred_id {
                Some(id) => id,
                None => return Vec::new(),
            };

            // Get all foaf:Person IRIs.
            let person_ids: Vec<i64> = client
                .select(
                    "SELECT vp.s FROM _pg_ripple.vp_rare vp \
                     WHERE vp.p = $1 AND vp.o = $2 \
                     ORDER BY vp.s",
                    None,
                    &[
                        pgrx::datum::DatumWithOid::from(type_pred_id),
                        pgrx::datum::DatumWithOid::from(person_class_id),
                    ],
                )
                .unwrap_or_else(|_| pgrx::error!("foaf_persons: persons query failed"))
                .map(|row| row.get::<i64>(1).ok().flatten().unwrap_or(0))
                .collect();

            if person_ids.is_empty() {
                return Vec::new();
            }

            // Get foaf:name predicate ID.
            let name_pred_id = client
                .select(
                    "SELECT id FROM _pg_ripple.dictionary WHERE value = $1",
                    None,
                    &[pgrx::datum::DatumWithOid::from(
                        "http://xmlns.com/foaf/0.1/name",
                    )],
                )
                .unwrap_or_else(|_| pgrx::error!("foaf_persons: name pred lookup failed"))
                .next()
                .and_then(|row| row.get::<i64>(1).ok().flatten());

            let mut result = Vec::new();
            for person_id in person_ids {
                let person_iri = client
                    .select(
                        "SELECT value FROM _pg_ripple.dictionary WHERE id = $1",
                        None,
                        &[pgrx::datum::DatumWithOid::from(person_id)],
                    )
                    .unwrap_or_else(|_| pgrx::error!("foaf_persons: iri decode failed"))
                    .next()
                    .and_then(|row| row.get::<String>(1).ok().flatten())
                    .unwrap_or_default();

                let name_label = match name_pred_id {
                    Some(np) => client
                        .select(
                            "SELECT d_o.value FROM _pg_ripple.vp_rare vp \
                             JOIN _pg_ripple.dictionary d_o ON d_o.id = vp.o \
                             WHERE vp.s = $1 AND vp.p = $2 LIMIT 1",
                            None,
                            &[
                                pgrx::datum::DatumWithOid::from(person_id),
                                pgrx::datum::DatumWithOid::from(np),
                            ],
                        )
                        .unwrap_or_else(|_| pgrx::error!("foaf_persons: name query failed"))
                        .next()
                        .and_then(|row| row.get::<String>(1).ok().flatten()),
                    None => None,
                };

                result.push((person_iri, name_label));
            }

            result
        });

        TableIterator::new(rows)
    }

    // ── Internal helpers ──────────────────────────────────────────────────────
}
