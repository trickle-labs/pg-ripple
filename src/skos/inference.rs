//! SKOS integrity rules constant and validate_skos function.
//! (extracted from skos.rs in v0.114.0)

#[pgrx::pg_schema]
mod pg_ripple {
    use pgrx::prelude::*;

    // ── SKOS-IC: validate_skos ────────────────────────────────────────────────

    /// Run SKOS integrity checks and return violations.
    ///
    /// Requires the `'skos-integrity'` shape bundle to be loaded.
    /// Returns one row per violation with violation ID, subject IRI, and message.
    ///
    /// Also checks SKOS-IC-05 (S14: at most one prefLabel per language) via SQL
    /// since it requires aggregation not expressible in basic Datalog.
    #[pg_extern]
    fn validate_skos() -> TableIterator<
        'static,
        (
            name!(violation_id, String),
            name!(subject, String),
            name!(message, String),
        ),
    > {
        let mut rows: Vec<(String, String, String)> = Vec::new();

        // IC-01 through IC-04, IC-06 through IC-10: query _pg_ripple.vp_rare
        // for skos:ic_violation triples produced by the constraint rules.
        let ic_via_rules = Spi::connect(|client| {
            // Query the dictionary for the skos:ic_violation predicate.
            let pred_id = client
                .select(
                    "SELECT id FROM _pg_ripple.dictionary \
                     WHERE value = 'http://www.w3.org/2004/02/skos/core#ic_violation'",
                    None,
                    &[],
                )
                .unwrap_or_else(|_| pgrx::error!("validate_skos: dictionary query failed"))
                .next()
                .and_then(|row| row.get::<i64>(1).ok().flatten());

            let pred_id = match pred_id {
                Some(id) => id,
                None => return Vec::new(), // No violations predicate means no constraint rules were fired.
            };

            client
                .select(
                    "SELECT d_s.value, d_o.value \
                     FROM _pg_ripple.vp_rare vp \
                     JOIN _pg_ripple.dictionary d_s ON d_s.id = vp.s \
                     JOIN _pg_ripple.dictionary d_o ON d_o.id = vp.o \
                     WHERE vp.p = $1 AND vp.source = 1",
                    None,
                    &[pgrx::datum::DatumWithOid::from(pred_id)],
                )
                .unwrap_or_else(|_| pgrx::error!("validate_skos: violation query failed"))
                .map(|row| {
                    let subject = row.get::<String>(1).ok().flatten().unwrap_or_default();
                    let message = row.get::<String>(2).ok().flatten().unwrap_or_default();
                    // Extract violation ID from message prefix (e.g. "SKOS-IC-01: ...").
                    let vid = message
                        .split(':')
                        .next()
                        .unwrap_or("SKOS-IC-??")
                        .trim()
                        .to_string();
                    (vid, subject, message)
                })
                .collect::<Vec<_>>()
        });
        rows.extend(ic_via_rules);

        // IC-05 (S14): at most one prefLabel per language per concept.
        // Detectable only via GROUP BY aggregation.
        let ic05 = Spi::connect(|client| {
            let pred_id = client
                .select(
                    "SELECT id FROM _pg_ripple.dictionary \
                     WHERE value = 'http://www.w3.org/2004/02/skos/core#prefLabel'",
                    None,
                    &[],
                )
                .unwrap_or_else(|_| pgrx::error!("validate_skos: ic05 dict query failed"))
                .next()
                .and_then(|row| row.get::<i64>(1).ok().flatten());

            let pred_id = match pred_id {
                Some(id) => id,
                None => return Vec::new(),
            };

            // Find subjects with more than one prefLabel literal sharing the same language tag.
            // We check this by looking at the literal values – simplified: flag any concept
            // with more than one prefLabel object with the same language datatype.
            client
                .select(
                    "SELECT d_s.value, count(DISTINCT d_o.id) AS cnt \
                     FROM _pg_ripple.vp_rare vp \
                     JOIN _pg_ripple.dictionary d_s ON d_s.id = vp.s \
                     JOIN _pg_ripple.dictionary d_o ON d_o.id = vp.o \
                     JOIN _pg_ripple.dictionary_literals dl ON dl.id = d_o.id \
                     WHERE vp.p = $1 \
                     GROUP BY d_s.value, dl.lang \
                     HAVING count(DISTINCT d_o.id) > 1",
                    None,
                    &[pgrx::datum::DatumWithOid::from(pred_id)],
                )
                .unwrap_or_else(|_| pgrx::error!("validate_skos: ic05 query failed"))
                .map(|row| {
                    let subject = row.get::<String>(1).ok().flatten().unwrap_or_default();
                    (
                        "SKOS-IC-05".to_string(),
                        subject.clone(),
                        format!(
                            "SKOS-IC-05: concept <{subject}> has multiple skos:prefLabel values with the same language tag (S14 violation)"
                        ),
                    )
                })
                .collect::<Vec<_>>()
        });
        rows.extend(ic05);

        TableIterator::new(rows)
    }

    // ── SKOS-05: SQL Helper Functions ─────────────────────────────────────────
}
