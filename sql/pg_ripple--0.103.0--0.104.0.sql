-- Migration 0.103.0 → 0.104.0: Domain Rule Library Infrastructure
--
-- Schema changes:
--   CREATE TABLE _pg_ripple.rule_libraries — catalog for installed external
--   rule libraries (name, version, installed_at, description, license_iri,
--   source_url, dependencies, shape_iris).
--
-- New Rust-compiled SQL functions exposed in this release:
--
--   pg_ripple.install_rule_library(source TEXT, accept_license BOOLEAN DEFAULT FALSE) → TEXT
--     Install a rule library from a URL or local file path.
--     Validates the SSRF allowlist for URL sources, resolves dependencies,
--     checks licenses, loads Datalog rules and SHACL shapes, and records
--     the library in _pg_ripple.rule_libraries.
--     Re-installing the same version is idempotent.
--
--   pg_ripple.upgrade_rule_library(name TEXT) → TEXT
--     Re-fetch and reload a library from its original source URL.
--     Raises PT0456 if another installed library depends on it.
--
--   pg_ripple.uninstall_rule_library(name TEXT) → VOID
--     Remove a library's rules, shapes, and catalog entry.
--     Raises PT0456 if another installed library depends on it.
--
--   pg_ripple.list_rule_libraries() → TABLE(name, version, installed_at, description, license_iri)
--     List all installed rule libraries.
--
-- New error codes (PT0452–PT0459):
--   PT0452  install_rule_library: URL blocked by the SSRF allowlist
--   PT0453  install_rule_library: dependency cycle detected
--   PT0454  install_rule_library: dependency could not be fetched
--   PT0455  install_rule_library: non-permissive license requires explicit acceptance
--   PT0456  upgrade/uninstall_rule_library: library is required by another installed library
--   PT0459  install_rule_library: name conflicts with a built-in bundle
--
-- New REST endpoint (pg_ripple_http):
--   GET /rule-libraries — returns pg_ripple.list_rule_libraries() as a JSON array

-- Create the rule libraries catalog table.
CREATE TABLE IF NOT EXISTS _pg_ripple.rule_libraries (
    name         TEXT PRIMARY KEY,
    version      TEXT NOT NULL,
    installed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    description  TEXT,
    license_iri  TEXT,
    source_url   TEXT,
    dependencies TEXT[],
    shape_iris   TEXT[]
);

COMMENT ON TABLE _pg_ripple.rule_libraries IS
    'Catalog of installed external rule libraries (v0.104.0).';
