# Rule Libraries

> **Version**: v0.104.0  
> **Foundation**: [plans/expert-system.md §5](../../plans/expert-system.md)

A **rule library** is a versioned, distributable package of Datalog inference rules
and SHACL validation shapes. Libraries allow teams to share domain-specific
reasoning logic — patient-matching rules, anti-money-laundering patterns,
supply-chain constraints — without each team re-implementing the same logic from
scratch.

Rule libraries are analogous to npm packages or pip packages, but for
knowledge-graph reasoning.

---

## Library Format

A rule library is a single Turtle (`.ttl`) file with the following structure:

```turtle
@prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .
@prefix dcterms: <http://purl.org/dc/terms/> .
@prefix owl: <http://www.w3.org/2002/07/owl#> .
@prefix pg: <http://pg-ripple.org/lib/> .
@prefix sh: <http://www.w3.org/ns/shacl#> .

# ── Required: library declaration ────────────────────────────────────────────

<urn:my-org:my-library> a pg:RuleLibrary ;
    dcterms:title       "my-library" ;
    dcterms:description "Human-readable description of what the library does." ;
    dcterms:license     <https://spdx.org/licenses/MIT.html> ;
    owl:versionInfo     "1.2.0" ;

    # ── Optional: dependencies ───────────────────────────────────────────────

    # Declare another library as a dependency (its URL or local path).
    pg:dependsOn <https://example.org/shared-library.ttl> ;

    # ── Optional: Datalog rules ──────────────────────────────────────────────

    pg:rules """
        ?x <https://schema.org/ancestor> ?z :-
            ?x <https://schema.org/parent> ?z .

        ?x <https://schema.org/ancestor> ?z :-
            ?x <https://schema.org/parent> ?y ,
            ?y <https://schema.org/ancestor> ?z .
    """ .

# ── Optional: SHACL shapes ────────────────────────────────────────────────────

<https://my-org.example/shapes/PersonShape>
    a sh:NodeShape ;
    sh:targetClass <https://schema.org/Person> ;
    sh:property [
        sh:path <https://schema.org/name> ;
        sh:minCount 1 ;
        sh:datatype <http://www.w3.org/2001/XMLSchema#string> ;
    ] .
```

### Required metadata triples

All four of these triples **must** appear on the `pg:RuleLibrary` subject:

| Property | Value |
|---|---|
| `dcterms:title` | Short identifier used as the library name in the catalog (plain string) |
| `dcterms:description` | Human-readable explanation of what the library does |
| `dcterms:license` | SPDX license IRI (see [License requirements](#license-requirements)) |
| `owl:versionInfo` | Version string (e.g. `"1.2.0"`) |

### Optional triples

| Property | Value |
|---|---|
| `pg:rules` | `xsd:string` literal containing one or more Datalog rule bodies |
| `pg:dependsOn` | IRI of a dependency library (URL or local path); can appear multiple times |
| SHACL shapes | Any `sh:NodeShape` / `sh:PropertyShape` resources in the same file |

---

## Installing a Library

```sql
-- Install from a local file (absolute path):
SELECT pg_ripple.install_rule_library('/opt/pg_ripple/libraries/my-library.ttl');

-- Install from an HTTPS URL:
SELECT pg_ripple.install_rule_library(
    'https://libraries.example.org/my-library-1.2.0.ttl'
);
```

`install_rule_library` returns the library name on success.

**Idempotency**: re-installing the same name + version is a no-op. The function
returns the name without error or duplication.

### Accepting non-permissive licenses

If a library carries a non-permissive license (anything other than MIT,
Apache-2.0, or the PostgreSQL License), you must explicitly acknowledge this:

```sql
SELECT pg_ripple.install_rule_library(
    '/opt/libraries/gpl-library.ttl',
    accept_license => true
);
```

Without `accept_license => true`, `install_rule_library` raises **PT0455** and
exits without loading the library.

---

## Listing Installed Libraries

```sql
SELECT * FROM pg_ripple.list_rule_libraries();
```

| Column | Type | Description |
|---|---|---|
| `name` | `TEXT` | Library identifier (from `dcterms:title`) |
| `version` | `TEXT` | Version from `owl:versionInfo` |
| `installed_at` | `TEXT` | Timestamp of installation |
| `description` | `TEXT` | Library description |
| `license_iri` | `TEXT` | SPDX license IRI |

---

## Upgrading a Library

```sql
SELECT pg_ripple.upgrade_rule_library('my-library');
```

`upgrade_rule_library` re-fetches the library from its original `source_url`,
replaces all installed rules and shapes, and updates the version in the catalog.

**PT0456**: If another installed library depends on `my-library`, the upgrade
is rejected — uninstall the dependent library first.

---

## Uninstalling a Library

```sql
SELECT pg_ripple.uninstall_rule_library('my-library');
```

This removes:
- All Datalog rules for the library's rule set name.
- All SHACL shapes loaded from the library's Turtle file.
- The catalog row in `_pg_ripple.rule_libraries`.

**PT0456**: Rejected when another installed library declares `my-library` as
a dependency. Uninstall the dependent library first.

---

## REST API

The `pg_ripple_http` companion service exposes a read-only endpoint:

```http
GET /rule-libraries
Authorization: Bearer <token>
```

Returns a JSON array of installed library objects:

```json
[
  {
    "name": "my-library",
    "version": "1.2.0",
    "installed_at": "2026-05-10 12:00:00",
    "description": "Human-readable description.",
    "license_iri": "https://spdx.org/licenses/MIT.html"
  }
]
```

---

## Dependency Resolution

Dependencies declared with `pg:dependsOn` are resolved **recursively in
topological order**: dependency libraries are installed before the library that
requires them.

**Constraints**:
- Dependency graphs must be **acyclic**. Cycles raise **PT0453**.
- Only **single-version** dependencies are supported — no semver ranges.
- If a declared dependency URL cannot be fetched, **PT0454** is raised.

---

## License Requirements

pg_ripple distinguishes **permissive** from **non-permissive** licenses:

| License | SPDX IRI | Auto-accepted |
|---|---|---|
| MIT | `https://spdx.org/licenses/MIT.html` | ✅ Yes |
| Apache-2.0 | `https://spdx.org/licenses/Apache-2.0.html` | ✅ Yes |
| PostgreSQL License | `https://spdx.org/licenses/PostgreSQL.html` | ✅ Yes |
| Any other | — | ❌ Requires `accept_license => true` |

### Operator responsibilities

Before installing a library with `accept_license => true`:

1. **Read the license text** at the SPDX IRI.
2. **Consult your legal team** if the library will be used in a commercial
   product or distributed as part of a service.
3. **Document** the accepted license decision in your change-management log.
4. **Review the library content** — pg_ripple does not audit library rules for
   correctness, safety, or regulatory compliance.

pg_ripple does not provide legal advice. The `accept_license` flag is a
technical confirmation mechanism, not a legal guarantee.

---

## SSRF Protection

URL-based library sources are validated against the federation SSRF allowlist
before any network request is made. The check follows the same policy as
`SERVICE` endpoints in SPARQL queries:

- **`default-deny`** (default): blocks RFC-1918, loopback, link-local addresses.
- **`allowlist`**: only URLs listed in `pg_ripple.federation_allowed_endpoints`.
- **`open`**: no restrictions (development/testing only).

A blocked URL raises **PT0452**.

---

## Authoring a Library

### 1. Write the Turtle file

Follow the [Library Format](#library-format) above.  Rules must be valid
pg_ripple Datalog syntax — test them with `pg_ripple.load_rules()` first.

### 2. Validate locally

```bash
# Test rule syntax:
psql -c "SELECT pg_ripple.load_rules(\$\$<rules here>\$\$, 'test')"

# Test install from local path:
psql -c "SELECT pg_ripple.install_rule_library('/path/to/my-library.ttl')"

# Verify it appears in the catalog:
psql -c "SELECT * FROM pg_ripple.list_rule_libraries()"

# Uninstall after testing:
psql -c "SELECT pg_ripple.uninstall_rule_library('my-library')"
```

### 3. Publish

Rule libraries can be published:
- As files on an HTTPS server.
- As GitHub releases with raw URLs.
- As OCI artifacts or Helm config maps (for Kubernetes deployments).

There is no central registry — operators install from any URL or local path
they trust.

### 4. Versioning

Use semantic versioning (`MAJOR.MINOR.PATCH`) in `owl:versionInfo`.  When you
release a new version, update the Turtle file and re-publish.  Users upgrade
with `upgrade_rule_library()`.

---

## Error Reference

| Code | When | Message |
|---|---|---|
| PT0452 | URL blocked by SSRF allowlist | `install_rule_library: URL '...' is blocked by the SSRF allowlist` |
| PT0453 | Circular dependency detected | `install_rule_library: dependency cycle detected involving library '...'` |
| PT0454 | Dependency URL unreachable | `install_rule_library: dependency '...' could not be fetched` |
| PT0455 | Non-permissive license, no acceptance | `install_rule_library: library '...' uses license '...' — set accept_license => TRUE` |
| PT0456 | Dependent library prevents action | `upgrade/uninstall_rule_library: library '...' is required by '...' — uninstall the dependent first` |
| PT0459 | Name conflicts with a built-in bundle | `install_rule_library: name '...' conflicts with a built-in bundle` |

---

## Relationship to Built-in Bundles

pg_ripple ships with several **built-in bundles** (SKOS, OWL RL, RDFS, DCTERMS,
Schema.org, FOAF) accessible via `load_datalog_bundle()` and
`load_shape_bundle()`. These are compiled into the extension binary.

External **rule libraries** (this feature) are the user-extensible counterpart:
they are Turtle files loaded at runtime and stored in the
`_pg_ripple.rule_libraries` catalog.

The two systems coexist. A rule library may declare a built-in bundle as a
dependency using `pg:dependsOn "skos"` — `install_rule_library` will activate
the bundle via `load_datalog_bundle()` before loading the library's own rules.

Library names that collide with built-in bundle names are rejected with PT0459.
