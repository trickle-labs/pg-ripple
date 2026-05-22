# Documentation Quality Improvement Plan

> Date: 2026-05-22
> Scope: `docs/`, `docs/src/`, and documentation cross-links into `blog/`, `plans/`, code, SQL migrations, and `pg_ripple_http`.
> Current baseline: pg_ripple v0.128.0, PostgreSQL 18, pgrx 0.18, mdBook documentation with 178 source pages under `docs/src/`.

## 1. Goals

The documentation should be trustworthy, easy to navigate, and cheaper to keep correct as the implementation changes. The primary goal is not more prose; it is a tighter loop between the codebase and the docs so users can rely on examples, references, compatibility statements, and operational guidance.

Success means:

- Every page in `docs/src/` is reachable from `SUMMARY.md` and every local source link resolves.
- The docs build locally with `mdbook build docs` on a fresh contributor machine, even when optional preprocessors are absent.
- SQL function, GUC, HTTP route, environment-variable, and error-code references match the implementation.
- Feature pages explain what is implemented now, what is optional or degraded, and what companion extension is required.
- Duplicated pages are either merged, clearly split by audience, or replaced with short index/recipe pages.
- Code examples are executable or explicitly marked as schematic.
- Release work includes a repeatable docs verification step before tagging.

## 2. Baseline Findings From This Audit

This audit found the documentation broadly complete but carrying several kinds of drift:

| Area | Finding | Current action |
|---|---|---|
| Navigation | `docs/src/SUMMARY.md` covers all 178 mdBook source pages; no orphan pages were found. | Preserve this standard with an automated check. |
| Local links | The first source-link scan found 47 missing local targets, mostly links from mdBook pages to top-level `blog/` posts using the wrong relative path. | Rewrote blog links to stable repository URLs and fixed stale local links. |
| Build tooling | `mdbook build docs` failed on machines without `mdbook-admonish`. | Make `mdbook-admonish` optional while preserving admonish rendering when installed. |
| HTTP reference | `docs/src/reference/http-api.md` claimed to be complete but missed newer routes and had stale methods/auth labels for Datalog endpoints. | Updated the endpoint inventory and added JSON mapping writeback endpoints. |
| GUC reference | `docs/gucs.md` had stale defaults/types for several implemented GUCs. | Corrected the identified values and context labels. |
| Companion extensions | Some pages still blurred pg_trickle and pg_tide responsibilities after the relay migration. | Clarified that pg_trickle is IVM-only and pg_tide owns relay/outbox/inbox transport. |
| Duplication | Rule-library federation had overlapping cookbook and guide pages. | Converted the cookbook entry into a short recipe and kept the guide as the detailed walkthrough. |
| Historical docs | `docs/GAP_ANALYSIS.md` and `docs/IMPROVEMENT_PLAN.md` describe v0.99.1-era issues and should not be read as current state. | Mark them as historical and superseded by this plan. |

## 3. Workstream A: Automated Docs Validation

### A1. Add a Source Link Checker

Create `scripts/check_docs_links.py` or a Rust equivalent that scans Markdown files under `docs/` and verifies:

- Internal relative links resolve to an existing source file.
- Links to directories resolve to `README.md` or `index.md` when appropriate.
- Fragment-only anchors are ignored in the first version, then checked in a later pass.
- External links are reported separately and can be checked only in scheduled CI to avoid flaky PR failures.
- Links into top-level `blog/`, `plans/`, `tests/`, and `results/` are either valid source links or deliberate external repository links.

Add a `just docs-check-links` target and run it in CI.

Definition of done:

- The checker exits non-zero on missing local files.
- The checker prints file, line, raw target, and resolved target.
- CI runs it on every pull request that touches Markdown or docs tooling.

### A2. Add a SUMMARY Coverage Check

Create `scripts/check_docs_summary.py` to compare `docs/src/SUMMARY.md` against all `docs/src/**/*.md` files.

Rules:

- Every mdBook page except `SUMMARY.md` must be linked exactly once unless explicitly allowlisted.
- Allowlisted pages must explain why they are intentionally hidden.
- Duplicate entries should fail CI unless one is a deliberate redirect/index entry.

Definition of done:

- The current tree reports 0 orphan pages.
- The check catches new pages that are not added to navigation.

### A3. Make mdBook Build Reproducible

Keep `mdbook-admonish` optional for local builds, but document the enhanced build path:

```bash
cargo install mdbook mdbook-admonish
mdbook build docs
```

Add `just docs-build` to run `mdbook build docs` and a CI job that installs `mdbook-admonish` so production output keeps styled callouts.

Definition of done:

- `mdbook build docs` succeeds without `mdbook-admonish`.
- CI still tests the styled admonish path.
- `docs/book/` generated output is either consistently ignored or regenerated only by a documented release step.

## 4. Workstream B: Generated Reference Sources

### B1. Generate the GUC Reference

The repo has two GUC reference surfaces: `docs/gucs.md` and `docs/src/reference/guc-reference.md`. They should be generated or checked from `src/gucs/**` and `src/gucs/registration/**`.

Implementation steps:

1. Parse GUC registration calls in `src/gucs/registration/**/*.rs`.
2. Extract name, type, default, min/max where available, context, and short description.
3. Cross-check defaults against `GucSetting::new(...)` declarations.
4. Emit a machine-readable intermediate file such as `target/docs/gucs.json`.
5. Generate the tabular `docs/gucs.md` from that intermediate file.
6. For `guc-reference.md`, either generate full sections or insert generated tables between stable markers.

Definition of done:

- A CI check fails when a registered GUC is missing from the docs.
- A CI check fails when a documented default/type/context differs from code.
- Deprecated or legacy names such as `trickle_integration` and `citus_trickle_compat` are documented as legacy without hiding that they remain active GUC names.

### B2. Generate or Check the HTTP Endpoint Inventory

`pg_ripple_http/src/routing/mod.rs` is the source of truth for HTTP routes. Build a small route extractor that emits method, path, handler, and auth expectation.

Implementation steps:

1. Parse Axum `.route(...)` declarations from `pg_ripple_http/src/routing/mod.rs`.
2. Map handler functions to auth mode by scanning for `check_auth`, `check_auth_write`, metrics-token checks, or no check.
3. Emit `target/docs/http-routes.json`.
4. Compare it to the table in `docs/src/reference/http-api.md`.
5. Add route documentation requirements for new handlers in PR review.

Definition of done:

- Every implemented route appears in `http-api.md`.
- Method mismatches fail CI, such as documenting `POST` where the router uses `PUT`.
- Auth labels are generated from the handler or explicitly overridden in a small allowlist.

### B3. Generate SQL Function Coverage

The SQL reference should be checked against pgrx exports.

Implementation options:

- Parse `#[pg_extern]` functions in `src/**/*.rs` and compare names to docs.
- Prefer `cargo pgrx schema` output if it is reliable in CI, because it reflects actual SQL names and defaults after pgrx expansion.

Minimum metadata per function:

- SQL name and schema.
- Argument names and defaults.
- Return type.
- Source file.
- Docs page or section that owns it.
- Stability tier: public, legacy alias, internal/admin, or deprecated.

Definition of done:

- Public SQL functions are either documented or intentionally marked internal.
- Legacy aliases such as `trickle_available()` are documented as compatibility aliases.
- The docs do not claim network I/O or background behavior for SQL functions that only record catalog state.

### B4. Check Error Code Coverage

Build a scanner for `PT[0-9]{3,4}` occurrences in Rust, SQL, tests, and docs.

Definition of done:

- Every user-facing error code emitted by code appears in `docs/src/reference/error-catalog.md` or `error-codes.md`.
- Error docs include cause, likely fix, and owning subsystem.
- Duplicate meanings for the same code fail CI.

## 5. Workstream C: Truth Audit by Feature Area

Run a page-by-page truth audit using the implementation as source of truth. Each feature page should have a small front-matter or first-section status block with:

- Availability version.
- Required extension(s), if any.
- SQL entry points.
- HTTP entry points, if any.
- Degraded behavior when optional dependencies are absent.
- Link to the relevant reference page.

Audit order:

1. Getting Started and Evaluate pages.
2. Operations and deployment pages.
3. SQL/API references.
4. Feature deep dives.
5. Cookbook recipes.
6. AI/RAG pages.
7. Research and historical pages.

High-priority checks:

- pg_tide vs pg_trickle responsibilities.
- HTTP routes and auth modes.
- GUC defaults and ranges.
- JSON mapping writeback behavior.
- Federation credential and SSRF policy behavior.
- Rule-library federation SQL vs HTTP responsibilities.
- Conformance claims and CI gate language.
- Performance claims that imply a benchmark source.

Definition of done:

- Every feature page states optional dependencies accurately.
- No page describes a function, endpoint, or GUC that does not exist.
- Historical release notes remain historical and do not masquerade as current guidance.

## 6. Workstream D: Executable Examples

Examples are the highest-trust part of the docs and should be treated like tests.

Implementation steps:

1. Mark code blocks with one of these labels in nearby prose: executable, illustrative, or pseudo-output.
2. Extract executable SQL blocks from Getting Started, core Feature pages, and Cookbook pages.
3. Run them in a temporary pg_ripple database through `cargo pgrx regress` or a lightweight psql harness.
4. Store expected outputs for short examples where stable.
5. Skip examples requiring external services unless a mock or local fixture exists.

Priority pages:

- `docs/src/getting-started/hello-world.md`
- `docs/src/getting-started/tutorial.md`
- `docs/src/features/loading-data.md`
- `docs/src/features/querying-with-sparql.md`
- `docs/src/features/validating-data-quality.md`
- `docs/src/features/reasoning-and-inference.md`
- `docs/src/features/json-mapping.md`
- `docs/src/reference/http-api.md`

Definition of done:

- Hello World SQL is regression-tested.
- At least one executable example per major feature area runs in CI.
- Examples that require pgvector, PostGIS, pg_tide, pg_trickle, Citus, or external LLM services are clearly labeled.

## 7. Workstream E: Deduplication and Information Architecture

Use the existing structure, but make page responsibilities explicit.

Rules:

- Feature pages explain concepts and primary workflows.
- Reference pages enumerate exact APIs, signatures, GUCs, limits, and error codes.
- Cookbook pages solve one concrete problem with minimal detours.
- Operations pages describe deployment and failure modes.
- Blog links provide background, not required operational steps.

Known consolidation candidates:

| Area | Current issue | Target shape |
|---|---|---|
| Live views, live subscriptions, CDC subscriptions | Three pages overlap but cover different APIs. | Keep all three, but add a comparison table and clear first paragraph on each page. |
| Rule-library federation | Cookbook and guide overlapped. | Cookbook stays a short recipe; guide owns the full walkthrough. |
| SQL API vs SQL function reference vs user-guide SQL reference | Multiple reference surfaces can disagree. | Establish one canonical generated SQL reference and make other pages task-oriented indexes. |
| GUC docs | `docs/gucs.md` and `guc-reference.md` duplicate data. | Generate both from the same metadata or make one redirect to the other. |
| Historical plans and gap analyses | Old docs describe obsolete version state. | Mark historical docs as superseded and keep current plans under `plans/`. |

Definition of done:

- A reader can tell why each similarly named page exists.
- No cookbook page duplicates a guide page line-for-line.
- Reference tables are not manually copied into multiple places without a generated source.

## 8. Workstream F: Style and Consistency Pass

Create a short `docs/STYLE.md` and apply it during page audits.

Style rules:

- Prefer direct, operational wording over release-note phrasing.
- Put current behavior first; version history belongs in notes.
- Use `pg_tide` only for relay/outbox/inbox transport.
- Use `pg_trickle` only for IVM-backed views, ExtVP, SHACL DAG monitors, and live statistics where code requires it.
- Spell SQL function names with `pg_ripple.` prefix on first mention.
- Use `postgresql` or `sql` code fences consistently.
- Use `Warning`, `Note`, and `Tip` callouts consistently through mdbook-admonish syntax.
- Avoid promising exact performance unless a benchmark file or CI result is linked.

Definition of done:

- New docs PRs have a concise style checklist.
- Existing high-traffic pages follow the style guide.
- Generated reference pages are exempt from prose style except for intro sections.

## 9. Workstream G: Release Process Integration

Docs quality should be part of release readiness, not a cleanup after release.

Add to the release checklist:

1. Run `just docs-build`.
2. Run `just docs-check-links`.
3. Run `just docs-check-summary`.
4. Run generated-reference drift checks for GUCs, SQL functions, HTTP routes, env vars, and error codes.
5. Confirm `CHANGELOG.md` links to any new user-facing docs.
6. Confirm new HTTP routes appear in `docs/src/reference/http-api.md`.
7. Confirm new GUCs appear in `docs/src/reference/guc-reference.md`.
8. Confirm new SQL functions appear in the SQL reference or are marked internal.
9. Confirm any new migration script has upgrade docs if it changes user-visible schema.

Definition of done:

- Release PRs cannot merge with broken docs checks.
- The changelog template includes a documentation checklist.
- `plans/documentation-2.md` is updated or closed out as workstreams complete.

## 10. Suggested Execution Order

### Phase 1: Guardrails (1-2 days)

- Add link checker.
- Add SUMMARY coverage checker.
- Add `just docs-build`.
- Add CI job for docs checks.
- Mark historical docs as superseded.

### Phase 2: Generated Drift Checks (3-5 days)

- Implement GUC metadata extractor.
- Implement HTTP route extractor.
- Implement environment-variable extractor for `pg_ripple_http`.
- Add non-failing report mode first, then make new drift fail CI after the initial backlog is fixed.

### Phase 3: High-Traffic Truth Audit (3-7 days)

- Audit Getting Started and Evaluate pages.
- Audit Operations pages for pg_tide/pg_trickle, Docker versions, compatibility, security, and deployment commands.
- Audit `http-api.md`, `guc-reference.md`, and SQL references against generated reports.

### Phase 4: Executable Examples (5-10 days)

- Build SQL extraction harness.
- Add tests for Hello World and the guided tutorial.
- Add representative examples for SHACL, Datalog, JSON mapping, SPARQL Update, and export.

### Phase 5: Deduplication and Polish (ongoing)

- Consolidate overlapping pages.
- Add comparison tables where similar APIs exist.
- Normalize style and callouts.
- Add per-page status/dependency blocks for feature pages.

## 11. Acceptance Criteria for v1.0 Documentation

Before v1.0.0, documentation should meet these criteria:

- `mdbook build docs` succeeds in CI.
- Link checker reports 0 missing local links.
- SUMMARY checker reports 0 orphan pages and 0 accidental duplicate entries.
- HTTP route checker reports 0 undocumented implemented routes.
- GUC checker reports 0 missing public GUCs and 0 default/type/context mismatches.
- SQL function checker reports 0 undocumented public functions, excluding approved internal/admin allowlist entries.
- Error-code checker reports 0 undocumented user-facing PT codes.
- Hello World and the guided tutorial examples run successfully in CI.
- Optional dependency pages clearly distinguish pg_tide, pg_trickle, pgvector, PostGIS, Citus, and pg_ripple_http.
- Historical analyses in `docs/` are clearly marked as historical or moved under `plans/archive/`.

## 12. Open Questions

- Should `docs/gucs.md` remain as a separate root-level artifact, or should it be generated from and linked to `docs/src/reference/guc-reference.md`?
- Should top-level `blog/` posts be published into the mdBook, or should docs continue linking to the repository copy on GitHub?
- Should `docs/book/` generated output be committed, or should CI publish it as a build artifact only?
- Should the HTTP API table be generated directly into Markdown, or should generated OpenAPI become the canonical reference with Markdown summaries?
- Should historical planning documents under `docs/` be moved to `plans/archive/` to keep `docs/` focused on user-facing documentation?
