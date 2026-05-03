//! PageRank export — Turtle, JSON-LD, CSV, N-Triples serialisation.
//!
//! Integrates the IRI safety encoding from v0.89.0 (SEC-04).

use pgrx::prelude::*;

use super::PT0417;

/// Export PageRank scores in the requested format.
///
/// `format` must be one of `'turtle'`, `'jsonld'`, `'csv'`, `'ntriples'`.
/// Any other value raises PT0417.
pub fn export_pagerank(format: &str, top_k: Option<i32>, topic: Option<&str>) -> String {
    let supported = ["turtle", "jsonld", "csv", "ntriples"];
    if !supported.contains(&format) {
        pgrx::error!(
            "{PT0417}: unsupported export format '{}'; supported: {:?}",
            format,
            supported
        );
    }

    let limit_clause = top_k.map(|k| format!("LIMIT {k}")).unwrap_or_default();
    let topic_arg = topic.unwrap_or("");

    let rows: Vec<(String, f64, bool)> = Spi::connect(|c| {
        c.select(
            &format!(
                "SELECT d.value, ps.score, ps.stale \
                 FROM _pg_ripple.pagerank_scores ps \
                 JOIN _pg_ripple.dictionary d ON d.id = ps.node \
                 WHERE ps.topic = $1 \
                 ORDER BY ps.score DESC {limit_clause}"
            ),
            None,
            &[pgrx::datum::DatumWithOid::from(topic_arg)],
        )
        .unwrap_or_else(|e| pgrx::error!("export_pagerank: {e}"))
        .map(|row| {
            let iri = row.get::<String>(1).ok().flatten().unwrap_or_default();
            let score = row.get::<f64>(2).ok().flatten().unwrap_or(0.0);
            let stale = row.get::<bool>(3).ok().flatten().unwrap_or(false);
            (iri, score, stale)
        })
        .collect()
    });

    match format {
        "turtle" => {
            let mut out = String::from(
                "@prefix pg: <http://pg-ripple.io/ns#> .\n\
                 @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .\n\n",
            );
            for (iri, score, _) in &rows {
                let safe = safe_iri_output(iri);
                out.push_str(&format!(
                    "{safe} pg:hasPageRank \"{score:.8}\"^^xsd:double .\n"
                ));
            }
            out
        }
        "ntriples" => {
            let mut out = String::new();
            let pr_pred = "<http://pg-ripple.io/ns#hasPageRank>";
            let xsd_double = "^^<http://www.w3.org/2001/XMLSchema#double>";
            for (iri, score, _) in &rows {
                let safe = safe_iri_output(iri);
                out.push_str(&format!("{safe} {pr_pred} \"{score:.8}\"{xsd_double} .\n"));
            }
            out
        }
        "csv" => {
            let mut out = String::from("node_iri,score,stale\n");
            for (iri, score, stale) in &rows {
                let clean = iri.trim_matches(|c| c == '<' || c == '>');
                let safe_iri = percent_encode_iri(clean);
                out.push_str(&format!("{safe_iri},{score:.8},{stale}\n"));
            }
            out
        }
        "jsonld" => {
            let mut items = Vec::new();
            for (iri, score, _) in &rows {
                let clean = iri.trim_matches(|c| c == '<' || c == '>');
                let safe_iri = percent_encode_iri(clean);
                items.push(format!(
                    "  {{\"@id\":\"{safe_iri}\",\
                       \"http://pg-ripple.io/ns#hasPageRank\":\
                       {{\"@value\":{score:.8},\"@type\":\"xsd:double\"}}}}"
                ));
            }
            format!("[\n{}\n]", items.join(",\n"))
        }
        _ => unreachable!(),
    }
}

/// Wrap a raw IRI value (possibly with `<>` brackets) in safe N-Triples/Turtle form.
fn safe_iri_output(iri: &str) -> String {
    let bare = iri.trim_matches(|c| c == '<' || c == '>');
    format!("<{}>", percent_encode_iri(bare))
}

/// Percent-encode characters unsafe in IRI output (`>`, `<`, `"`, `\`, space,
/// and bytes < 0x20 or == 0x7F). Other characters are left as-is. (SEC-04, v0.89.0)
pub(crate) fn percent_encode_iri(iri: &str) -> String {
    let mut out = String::with_capacity(iri.len());
    for ch in iri.chars() {
        match ch {
            '<' => out.push_str("%3C"),
            '>' => out.push_str("%3E"),
            '"' => out.push_str("%22"),
            '\\' => out.push_str("%5C"),
            ' ' => out.push_str("%20"),
            c if (c as u32) < 0x20 || c == '\x7F' => {
                out.push_str(&format!("%{:02X}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}
