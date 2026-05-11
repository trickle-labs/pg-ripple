//! Domain Rule Library Infrastructure (v0.104.0)
//!
//! A rule library is a Turtle file containing:
//! - Datalog rules as `xsd:string` literals on a `pg:rules` property
//! - Zero or more SHACL shapes
//! - Required metadata: `dcterms:title`, `dcterms:description`,
//!   `dcterms:license`, `owl:versionInfo`
//! - Optional: `pg:dependsOn` URLs for dependency libraries
//!
//! Catalog: `_pg_ripple.rule_libraries`
//!
//! ## Error codes
//! - PT0452: URL blocked by SSRF allowlist
//! - PT0453: dependency cycle detected
//! - PT0454: dependency fetch failed
//! - PT0455: non-permissive license requires explicit acceptance
//! - PT0456: library is required by another installed library
//! - PT0459: name conflicts with a built-in bundle
//!
//! Foundation: plans/expert-system.md §5 (v0.104.0)

use rio_api::model::{Literal, Subject, Term};
use rio_api::parser::TriplesParser;
use rio_turtle::TurtleError;
use rio_turtle::TurtleParser;

use pgrx::datum::DatumWithOid;
use pgrx::prelude::*;

// ─── Namespace constants ──────────────────────────────────────────────────────

/// pg: namespace for rule library vocabulary (informational).
#[allow(dead_code)]
const PG_LIB_NS: &str = "http://pg-ripple.org/lib/";

/// `pg:RuleLibrary` type IRI.
const PG_RULE_LIBRARY: &str = "http://pg-ripple.org/lib/RuleLibrary";

/// `pg:rules` property IRI — value is `xsd:string` Datalog rules text.
const PG_RULES: &str = "http://pg-ripple.org/lib/rules";

/// `pg:dependsOn` property IRI — value is an IRI of a dependency library URL.
const PG_DEPENDS_ON: &str = "http://pg-ripple.org/lib/dependsOn";

/// `dcterms:title` — human-readable name.
const DCTERMS_TITLE: &str = "http://purl.org/dc/terms/title";

/// `dcterms:description` — library description.
const DCTERMS_DESCRIPTION: &str = "http://purl.org/dc/terms/description";

/// `dcterms:license` — SPDX license IRI.
const DCTERMS_LICENSE: &str = "http://purl.org/dc/terms/license";

/// `owl:versionInfo` — version string.
const OWL_VERSION_INFO: &str = "http://www.w3.org/2002/07/owl#versionInfo";

/// `rdf:type` IRI.
const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// Licenses that are permissive enough to auto-accept (no `accept_license` flag needed).
const PERMISSIVE_LICENSES: &[&str] = &[
    "https://spdx.org/licenses/MIT.html",
    "https://spdx.org/licenses/Apache-2.0.html",
    "https://spdx.org/licenses/PostgreSQL.html",
];

// ─── Built-in bundle names (PT0459 collision check) ──────────────────────────

/// Names of built-in bundles shipped with pg_ripple (RB-01, v0.98.0–v0.99.0).
const BUILTIN_BUNDLE_NAMES: &[&str] = &[
    "skos",
    "skos-transitive",
    "skos-integrity",
    "rdfs",
    "owl-rl",
    "dcterms",
    "dcterms-integrity",
    "schema",
    "schema-integrity",
    "foaf",
    "foaf-integrity",
];

// ─── Metadata struct ──────────────────────────────────────────────────────────

/// Parsed metadata extracted from a rule library Turtle file.
#[derive(Debug, Default)]
struct LibraryMeta {
    /// Subject IRI of the `pg:RuleLibrary` resource (used to derive the name).
    subject_iri: String,
    /// `dcterms:title` value — used as the library name in the catalog.
    title: String,
    /// `dcterms:description` value.
    description: String,
    /// `dcterms:license` IRI.
    license_iri: String,
    /// `owl:versionInfo` value.
    version: String,
    /// Dependency library URLs from `pg:dependsOn`.
    dependencies: Vec<String>,
    /// Datalog rule text from `pg:rules` literals.
    rules_text: String,
}

// ─── Catalog helpers ──────────────────────────────────────────────────────────

/// Ensure the `_pg_ripple.rule_libraries` catalog table exists.
///
/// This is a no-op when the table was already created by the migration script.
/// Called defensively at the start of every library function.
pub(crate) fn ensure_catalog() {
    Spi::run(
        "CREATE TABLE IF NOT EXISTS _pg_ripple.rule_libraries ( \
             name        TEXT PRIMARY KEY, \
             version     TEXT NOT NULL, \
             installed_at TIMESTAMPTZ NOT NULL DEFAULT now(), \
             description TEXT, \
             license_iri TEXT, \
             source_url  TEXT, \
             dependencies TEXT[], \
             shape_iris  TEXT[] \
         )",
    )
    .unwrap_or_else(|e| pgrx::warning!("rule_libraries: ensure_catalog failed: {e}"));
}

// ─── Turtle metadata parser ───────────────────────────────────────────────────

/// Parse a Turtle document and extract rule library metadata.
///
/// Walks all triples and populates a `LibraryMeta` struct.  Returns an error
/// message if parsing fails or required metadata is missing.
fn parse_library_metadata(turtle: &str) -> Result<LibraryMeta, String> {
    let mut meta = LibraryMeta::default();
    let mut subject_iri_candidate: Option<String> = None;

    // Collect all triples from the Turtle document.
    let mut triples: Vec<(String, String, String)> = Vec::new();
    {
        let mut parser = TurtleParser::new(turtle.as_bytes(), None);
        let result: Result<(), TurtleError> = parser.parse_all(&mut |triple| {
            let s = match triple.subject {
                Subject::NamedNode(nn) => nn.iri.to_owned(),
                Subject::BlankNode(bn) => format!("_:{}", bn.id),
                // Skip RDF-star quoted subjects.
                Subject::Triple(_) => return Ok(()),
            };
            let p = triple.predicate.iri.to_owned();
            let o = match triple.object {
                Term::NamedNode(nn) => nn.iri.to_owned(),
                Term::BlankNode(bn) => format!("_:{}", bn.id),
                Term::Literal(lit) => match lit {
                    Literal::Simple { value } => value.to_owned(),
                    Literal::LanguageTaggedString { value, .. } => value.to_owned(),
                    Literal::Typed { value, .. } => value.to_owned(),
                },
                // Skip RDF-star quoted objects.
                Term::Triple(_) => return Ok(()),
            };
            triples.push((s, p, o));
            Ok(())
        });
        if let Err(e) = result {
            return Err(format!("Turtle parse error: {e}"));
        }
    }

    // First pass: find the pg:RuleLibrary subject.
    for (s, p, o) in &triples {
        if p == RDF_TYPE && o == PG_RULE_LIBRARY {
            subject_iri_candidate = Some(s.clone());
            break;
        }
    }

    // If no typed resource, look for any subject with pg:rules or dcterms:title.
    if subject_iri_candidate.is_none() {
        for (s, p, _) in &triples {
            if p == PG_RULES || p == DCTERMS_TITLE || p == OWL_VERSION_INFO {
                subject_iri_candidate = Some(s.clone());
                break;
            }
        }
    }

    let Some(subject_iri) = subject_iri_candidate else {
        return Err(
            "rule library Turtle must declare a resource of type pg:RuleLibrary \
             with required metadata (dcterms:title, owl:versionInfo, dcterms:license)"
                .to_owned(),
        );
    };

    meta.subject_iri = subject_iri.clone();

    // Second pass: extract metadata for the identified subject.
    for (s, p, o) in &triples {
        if s != &subject_iri {
            continue;
        }
        match p.as_str() {
            DCTERMS_TITLE => meta.title = o.clone(),
            DCTERMS_DESCRIPTION => meta.description = o.clone(),
            DCTERMS_LICENSE => meta.license_iri = o.clone(),
            OWL_VERSION_INFO => meta.version = o.clone(),
            PG_DEPENDS_ON => meta.dependencies.push(o.clone()),
            PG_RULES => {
                if !meta.rules_text.is_empty() {
                    meta.rules_text.push('\n');
                }
                meta.rules_text.push_str(o);
            }
            _ => {}
        }
    }

    // Validate required fields.
    if meta.title.is_empty() {
        return Err("rule library must have a dcterms:title triple".to_owned());
    }
    if meta.version.is_empty() {
        return Err("rule library must have an owl:versionInfo triple".to_owned());
    }
    if meta.license_iri.is_empty() {
        return Err("rule library must have a dcterms:license triple".to_owned());
    }

    Ok(meta)
}

/// Derive the catalog name from a library subject IRI or title.
///
/// Uses the last fragment (`#local`) or path segment of the IRI.
/// Falls back to a slug of the title.
fn derive_name(meta: &LibraryMeta) -> String {
    let iri = &meta.subject_iri;
    // Try fragment first.
    if let Some(pos) = iri.rfind('#') {
        let frag = &iri[pos + 1..];
        if !frag.is_empty() {
            return frag.to_owned();
        }
    }
    // Try last path segment.
    if let Some(pos) = iri.rfind('/') {
        let seg = &iri[pos + 1..];
        if !seg.is_empty() && seg != "." {
            // Strip known file extensions.
            let seg = seg.strip_suffix(".ttl").unwrap_or(seg);
            return seg.to_owned();
        }
    }
    // Try URN last segment.
    if let Some(pos) = iri.rfind(':') {
        let seg = &iri[pos + 1..];
        if !seg.is_empty() {
            return seg.to_owned();
        }
    }
    // Fall back to a slug of the title.
    meta.title
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_owned()
}

// ─── SSRF check for URL sources ───────────────────────────────────────────────

/// Check whether a URL is permitted by the federation SSRF allowlist.
///
/// Returns `Err` with a PT0452 message when blocked.
fn check_ssrf(url: &str) -> Result<(), String> {
    crate::sparql::federation::policy::check_endpoint_policy(url).map_err(|e| {
        format!("install_rule_library: URL '{url}' is blocked by the SSRF allowlist (PT0452): {e}")
    })
}

// ─── Source fetching ──────────────────────────────────────────────────────────

/// Fetch the Turtle content from a source.
///
/// `source` is either:
/// - An HTTP/HTTPS URL (subject to SSRF allowlist check).
/// - An absolute local file path (read directly from the filesystem).
fn fetch_source(source: &str) -> Result<String, String> {
    if source.starts_with("http://") || source.starts_with("https://") {
        // SSRF check MUST happen before any network request.
        check_ssrf(source)?;
        ureq::get(source)
            .call()
            .map_err(|e| {
                format!(
                    "install_rule_library: dependency '{source}' could not be fetched (PT0454): {e}"
                )
            })
            .and_then(|resp| {
                resp.into_string().map_err(|e| {
                    format!(
                        "install_rule_library: dependency '{source}' response decode error: {e}"
                    )
                })
            })
    } else {
        // Local file path.
        std::fs::read_to_string(source)
            .map_err(|e| format!("install_rule_library: could not read file '{source}': {e}"))
    }
}

// ─── Cycle detection ─────────────────────────────────────────────────────────

/// Check for dependency cycles using iterative DFS.
///
/// `visiting` is the set of library names currently on the resolution stack.
/// Returns `Err(PT0453 message)` when a cycle is detected.
fn check_no_cycle(name: &str, visiting: &mut [String]) -> Result<(), String> {
    if visiting.iter().any(|n| n == name) {
        return Err(format!(
            "install_rule_library: dependency cycle detected involving library '{name}' (PT0453)"
        ));
    }
    Ok(())
}

// ─── Core install logic ───────────────────────────────────────────────────────

/// Install a single library (internal, no cycle-check guard; guard is in the public fn).
fn install_library_inner(
    source: &str,
    accept_license: bool,
    visiting: &mut Vec<String>,
) -> Result<String, String> {
    // Fetch the Turtle content.
    let turtle = fetch_source(source)?;

    // Parse metadata.
    let meta = parse_library_metadata(&turtle)?;
    let name = derive_name(&meta);

    // PT0459: name must not conflict with a built-in bundle.
    if BUILTIN_BUNDLE_NAMES.contains(&name.as_str()) {
        return Err(format!(
            "install_rule_library: name '{name}' conflicts with a built-in bundle (PT0459)"
        ));
    }

    // PT0455: license check (unless accept_license = TRUE or license is permissive).
    if !accept_license && !PERMISSIVE_LICENSES.contains(&meta.license_iri.as_str()) {
        return Err(format!(
            "install_rule_library: library '{name}' uses license '{}' \
             — set accept_license => TRUE to confirm acceptance (PT0455)",
            meta.license_iri
        ));
    }

    // Idempotency check: if this exact name+version is already installed, return.
    let already_installed = Spi::get_one_with_args::<bool>(
        "SELECT EXISTS(SELECT 1 FROM _pg_ripple.rule_libraries WHERE name = $1 AND version = $2)",
        &[
            DatumWithOid::from(name.as_str()),
            DatumWithOid::from(meta.version.as_str()),
        ],
    )
    .unwrap_or(None)
    .unwrap_or(false);

    if already_installed {
        return Ok(name);
    }

    // Cycle guard before resolving dependencies.
    check_no_cycle(&name, visiting)?;
    visiting.push(name.clone());

    // Resolve dependencies first (topological order).
    let mut dep_names: Vec<String> = Vec::with_capacity(meta.dependencies.len());
    for dep_url in &meta.dependencies {
        let dep_name = install_library_inner(dep_url, accept_license, visiting)?;
        dep_names.push(dep_name);
    }

    visiting.pop();

    // Load Datalog rules.
    if !meta.rules_text.trim().is_empty() {
        crate::datalog::builtins::register_standard_prefixes();
        crate::datalog::cache::invalidate(&name);
        crate::datalog::tabling_invalidate_all();
        let rule_set_ir = crate::datalog::parse_rules(&meta.rules_text, &name)
            .map_err(|e| format!("install_rule_library: rule parse error in '{name}': {e}"))?;
        crate::datalog::store_rules(&name, &rule_set_ir.rules);
    }

    // Load SHACL shapes (the parser gracefully handles files with no shapes).
    let shape_iris = load_shapes_from_turtle(&turtle, &name);

    // Record in the catalog.
    let deps_array = if dep_names.is_empty() {
        "NULL".to_owned()
    } else {
        let quoted: Vec<String> = dep_names
            .iter()
            .map(|n| format!("'{}'", n.replace('\'', "''")))
            .collect();
        format!("ARRAY[{}]::TEXT[]", quoted.join(", "))
    };

    let shape_array = if shape_iris.is_empty() {
        "NULL".to_owned()
    } else {
        let quoted: Vec<String> = shape_iris
            .iter()
            .map(|s| format!("'{}'", s.replace('\'', "''")))
            .collect();
        format!("ARRAY[{}]::TEXT[]", quoted.join(", "))
    };

    // Use dynamic SQL for the arrays since DatumWithOid doesn't support TEXT[].
    let sql = format!(
        "INSERT INTO _pg_ripple.rule_libraries \
             (name, version, description, license_iri, source_url, dependencies, shape_iris) \
         VALUES ($1, $2, $3, $4, $5, {deps_array}, {shape_array}) \
         ON CONFLICT (name) DO UPDATE \
             SET version = EXCLUDED.version, \
                 description = EXCLUDED.description, \
                 license_iri = EXCLUDED.license_iri, \
                 source_url = EXCLUDED.source_url, \
                 dependencies = EXCLUDED.dependencies, \
                 shape_iris = EXCLUDED.shape_iris, \
                 installed_at = now()"
    );

    Spi::run_with_args(
        &sql,
        &[
            DatumWithOid::from(name.as_str()),
            DatumWithOid::from(meta.version.as_str()),
            DatumWithOid::from(meta.description.as_str()),
            DatumWithOid::from(meta.license_iri.as_str()),
            DatumWithOid::from(source),
        ],
    )
    .map_err(|e| format!("install_rule_library: catalog insert failed: {e}"))?;

    Ok(name)
}

/// Load SHACL shapes from a Turtle file and return their IRIs.
///
/// Uses `crate::shacl::parse_and_store_shapes()` — returns an empty Vec when
/// the file contains no shapes.
fn load_shapes_from_turtle(turtle: &str, _lib_name: &str) -> Vec<String> {
    // Snapshot existing shape IRIs before loading.
    let before: Vec<String> = Spi::connect(|c| {
        c.select(
            "SELECT shape_iri FROM _pg_ripple.shacl_shapes ORDER BY shape_iri",
            None,
            &[],
        )
        .unwrap_or_else(|e| pgrx::error!("load_shapes_from_turtle: SPI error: {e}"))
        .map(|row| row.get::<String>(1).ok().flatten().unwrap_or_default())
        .collect::<Vec<String>>()
    });

    // Ask the SHACL parser to load shapes from the Turtle content.
    let _n = crate::shacl::parse_and_store_shapes(turtle);

    // Collect the newly added shape IRIs.
    let after: Vec<String> = Spi::connect(|c| {
        c.select(
            "SELECT shape_iri FROM _pg_ripple.shacl_shapes ORDER BY shape_iri",
            None,
            &[],
        )
        .unwrap_or_else(|e| pgrx::error!("load_shapes_from_turtle: SPI error after load: {e}"))
        .map(|row| row.get::<String>(1).ok().flatten().unwrap_or_default())
        .collect::<Vec<String>>()
    });

    let before_set: std::collections::HashSet<&str> = before.iter().map(|s| s.as_str()).collect();

    after
        .into_iter()
        .filter(|iri| !before_set.contains(iri.as_str()))
        .collect()
}

// ─── Public SQL-visible functions ─────────────────────────────────────────────

#[pgrx::pg_schema]
mod pg_ripple {
    use super::*;
    /// Install a rule library from a URL or local file path.
    ///
    /// `source` — URL (https?://) or absolute local file path.
    /// `accept_license` — set `TRUE` to accept non-permissive licenses.
    ///
    /// Returns the library name on success.  Re-installing the same version is
    /// idempotent.
    ///
    /// ## Error codes
    /// - PT0452: URL blocked by SSRF allowlist
    /// - PT0453: dependency cycle
    /// - PT0454: dependency fetch failed
    /// - PT0455: non-permissive license without explicit acceptance
    /// - PT0459: name conflicts with a built-in bundle
    #[pg_extern]
    pub fn install_rule_library(source: &str, accept_license: default!(bool, "false")) -> String {
        super::ensure_catalog();
        let mut visiting: Vec<String> = Vec::new();
        match super::install_library_inner(source, accept_license, &mut visiting) {
            Ok(name) => name,
            Err(e) => pgrx::error!("{e}"),
        }
    }

    /// Upgrade an installed rule library by re-fetching from its source URL.
    ///
    /// Replaces rules and shapes, updates the version.  Raises PT0456 if
    /// another installed library depends on this one.
    ///
    /// Returns the library name on success.
    #[pg_extern]
    pub fn upgrade_rule_library(name: &str) -> String {
        super::ensure_catalog();

        // Fetch the current source URL.
        let source_url = Spi::get_one_with_args::<String>(
            "SELECT source_url FROM _pg_ripple.rule_libraries WHERE name = $1",
            &[DatumWithOid::from(name)],
        )
        .unwrap_or(None);

        let Some(source_url) = source_url else {
            pgrx::error!(
                "upgrade_rule_library: library '{name}' is not installed; \
                 install it first with install_rule_library()"
            );
        };

        // Check that nothing depends on this library (PT0456).
        let dep_count = Spi::get_one_with_args::<i64>(
            "SELECT count(*) FROM _pg_ripple.rule_libraries \
             WHERE $1 = ANY(dependencies)",
            &[DatumWithOid::from(name)],
        )
        .unwrap_or(None)
        .unwrap_or(0);

        if dep_count > 0 {
            // Find the first dependent library name for the error message.
            let dependent = Spi::get_one_with_args::<String>(
                "SELECT name FROM _pg_ripple.rule_libraries \
                 WHERE $1 = ANY(dependencies) LIMIT 1",
                &[DatumWithOid::from(name)],
            )
            .unwrap_or(None)
            .unwrap_or_else(|| "unknown".to_owned());
            pgrx::error!(
                "upgrade_rule_library: library '{name}' is required by '{dependent}' \
                 — uninstall the dependent first (PT0456)"
            );
        }

        // Remove old rules and shapes before re-installing.
        uninstall_library_inner(name);

        // Re-install from the original source.
        let mut visiting: Vec<String> = Vec::new();
        match super::install_library_inner(&source_url, true, &mut visiting) {
            Ok(n) => n,
            Err(e) => pgrx::error!("{e}"),
        }
    }

    /// Uninstall a rule library.
    ///
    /// Removes all rules and shapes installed by this library.
    /// Raises PT0456 if another installed library depends on `name`.
    #[pg_extern]
    pub fn uninstall_rule_library(name: &str) {
        super::ensure_catalog();

        // Verify it exists.
        let exists = Spi::get_one_with_args::<bool>(
            "SELECT EXISTS(SELECT 1 FROM _pg_ripple.rule_libraries WHERE name = $1)",
            &[DatumWithOid::from(name)],
        )
        .unwrap_or(None)
        .unwrap_or(false);

        if !exists {
            pgrx::error!("uninstall_rule_library: library '{name}' is not installed");
        }

        // Check that nothing depends on this library (PT0456).
        let dep_count = Spi::get_one_with_args::<i64>(
            "SELECT count(*) FROM _pg_ripple.rule_libraries \
             WHERE $1 = ANY(dependencies)",
            &[DatumWithOid::from(name)],
        )
        .unwrap_or(None)
        .unwrap_or(0);

        if dep_count > 0 {
            let dependent = Spi::get_one_with_args::<String>(
                "SELECT name FROM _pg_ripple.rule_libraries \
                 WHERE $1 = ANY(dependencies) LIMIT 1",
                &[DatumWithOid::from(name)],
            )
            .unwrap_or(None)
            .unwrap_or_else(|| "unknown".to_owned());
            pgrx::error!(
                "uninstall_rule_library: library '{name}' is required by '{dependent}' \
                 — uninstall the dependent first (PT0456)"
            );
        }

        super::uninstall_library_inner(name);
    }

    /// List all installed rule libraries.
    ///
    /// Returns one row per library with columns:
    /// `name`, `version`, `installed_at`, `description`, `license_iri`.
    #[pg_extern]
    pub fn list_rule_libraries() -> TableIterator<
        'static,
        (
            name!(name, String),
            name!(version, String),
            name!(installed_at, String),
            name!(description, String),
            name!(license_iri, String),
        ),
    > {
        super::ensure_catalog();
        let rows: Vec<(String, String, String, String, String)> = Spi::connect(|c| {
            c.select(
                "SELECT name, version, \
                     to_char(installed_at, 'YYYY-MM-DD HH24:MI:SS') AS installed_at, \
                     coalesce(description, '') AS description, \
                     coalesce(license_iri, '') AS license_iri \
                 FROM _pg_ripple.rule_libraries \
                 ORDER BY name",
                None,
                &[],
            )
            .unwrap_or_else(|e| pgrx::error!("list_rule_libraries SPI error: {e}"))
            .map(|row| {
                let n: String = row.get::<String>(1).ok().flatten().unwrap_or_default();
                let v: String = row.get::<String>(2).ok().flatten().unwrap_or_default();
                let ts: String = row.get::<String>(3).ok().flatten().unwrap_or_default();
                let d: String = row.get::<String>(4).ok().flatten().unwrap_or_default();
                let l: String = row.get::<String>(5).ok().flatten().unwrap_or_default();
                (n, v, ts, d, l)
            })
            .collect()
        });
        TableIterator::new(rows)
    }
}

/// Internal uninstall logic (shared by `uninstall_rule_library` and `upgrade_rule_library`).
fn uninstall_library_inner(name: &str) {
    // Remove Datalog rules.
    crate::datalog::cache::invalidate(name);
    crate::datalog::tabling_invalidate_all();
    let _ = Spi::run_with_args(
        "DELETE FROM _pg_ripple.rules WHERE rule_set = $1",
        &[DatumWithOid::from(name)],
    );
    let _ = Spi::run_with_args(
        "DELETE FROM _pg_ripple.rule_sets WHERE name = $1",
        &[DatumWithOid::from(name)],
    );

    // Remove SHACL shapes tracked for this library.
    let shape_iris: Vec<String> = Spi::connect(|c| {
        c.select(
            "SELECT unnest(shape_iris) FROM _pg_ripple.rule_libraries WHERE name = $1",
            None,
            &[DatumWithOid::from(name)],
        )
        .unwrap_or_else(|e| pgrx::error!("uninstall_rule_library: SPI error: {e}"))
        .map(|row| row.get::<String>(1).ok().flatten().unwrap_or_default())
        .collect::<Vec<String>>()
    });

    for shape_iri in &shape_iris {
        let _ = Spi::run_with_args(
            "DELETE FROM _pg_ripple.shacl_shapes WHERE shape_iri = $1",
            &[DatumWithOid::from(shape_iri.as_str())],
        );
    }

    // Remove catalog row.
    let _ = Spi::run_with_args(
        "DELETE FROM _pg_ripple.rule_libraries WHERE name = $1",
        &[DatumWithOid::from(name)],
    );
}
