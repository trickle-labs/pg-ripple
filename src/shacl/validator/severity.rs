//! SHACL validation engine — focus-node collection, constraint dispatch,
//! synchronous validation, and the async validation batch processor.

use serde::Serialize;


// ─── Violation record ─────────────────────────────────────────────────────────

/// A violation entry in a SHACL validation report.
#[derive(Debug, Serialize)]
pub struct Violation {
    pub focus_node: String,
    pub shape_iri: String,
    pub path: Option<String>,
    pub constraint: String,
    pub message: String,
    pub severity: String,
    /// The offending value node, decoded (v0.48.0, W3C `sh:value`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sh_value: Option<String>,
    /// W3C constraint component IRI (v0.48.0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sh_source_constraint_component: Option<String>,
}

// ─── Recursive shape conformance ─────────────────────────────────────────────

