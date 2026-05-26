# pg_ripple Overall Assessment #18

**Date:** 2026-05-26  
**Version assessed:** v0.128.0  
**Assessor:** GitHub Copilot  
**Baseline:** [PLAN_OVERALL_ASSESSMENT_17.md](PLAN_OVERALL_ASSESSMENT_17.md) (v0.120.0, score 4.57/5.0)  
**Codebase size:** 91,096 Rust LOC across 282 Rust source files (249 extension files in `src/`, 33 HTTP companion files in `pg_ripple_http/src/`); 295 pg_regress SQL files; 25 fuzz targets; 7 concurrency tests; 13 crash-recovery scripts; 11 CI workflows.

---

## Executive Summary

Eight releases have shipped since Assessment #17: v0.121.0 through v0.128.0. The A17 security finding in rule-library subscription is fixed: `subscribe_rule_library()` now calls the shared `resolve_and_check_endpoint()` path instead of the earlier string-contains guard. The A17 RSA advisory concern has also been re-triaged and extended to 2027-01-01 in `audit.toml`. The v0.128.0 release adds the large JSON mapping relational writeback feature, plus the repository now has 295 pg_regress SQL files, 25 fuzz targets, 66 unsafe blocks covered by 87 SAFETY comments, and all core version numbers are in sync at 0.128.0.

The headline negative finding is serious: the new v0.128.0 asynchronous JSON writeback path is silently nonfunctional in important paths. `enable_json_writeback()` looks up predicates with `WHERE iri = $1` even though `_pg_ripple.dictionary` has a `value` column, not `iri`; the error is swallowed as `None`, triggers are not installed, and `writeback_enabled` is still set to true. The queue drain path repeats the same nonexistent-column assumption with `SELECT iri FROM _pg_ripple.dictionary`. The migration script for 0.127.0 -> 0.128.0 also creates the queue table but not the trigger function needed by upgraded installations. This is a Critical v0.128.0 correctness and upgrade finding because the feature can report that automation is enabled while silently failing to propagate RDF changes back to relational rows.

The second systemic issue is release-truth drift. `tests/test_migration_chain.sh` still applies migrations only through 0.96.0, despite the repository containing scripts through 0.128.0. Its sync assertion computes both the highest migration and highest checkpoint from the same `sql/` listing, so it cannot detect that the script never applies the last 32 releases. This allowed the v0.128.0 migration defect above to survive.

Production-readiness verdict: not v1.0.0-ready today. The core engine remains strong and much of the A17 backlog has been remediated, but v0.128.0 introduced a critical feature-integrity problem, migration-chain coverage has regressed against the stated full-upgrade guarantee, and the external audit / 72-hour load-test evidence remains absent. The project is still late-RC quality, but it needs at least one hardening release before GA.

## Top Risks to v1.0.0 Readiness

| # | Risk | Severity | Blocking? |
|---|---|---|---|
| 1 | Async JSON mapping writeback is silently nonfunctional and incomplete | Critical | Yes |
| 2 | Migration-chain test stops at 0.96.0 while repo ships through 0.128.0 | High | Yes |
| 3 | Direct JSON writeback reports incorrect row counts and only emits text-typed parameters | High | Yes for JSON-WRITEBACK-01 quality |
| 4 | New v0.128.0 god module: `src/json_mapping.rs` is 1,245 LOC | High | No, but must be split before API freeze |
| 5 | HTTP companion is fail-open when auth tokens are unset | Medium | Yes for production hardening |
| 6 | Conformance jobs can pass when test data download fails or OWL 2 RL failures occur | Medium | No, but undermines release evidence |
| 7 | External security-review report and 72-hour load-test artifact are still not on file | High | Yes |

## Findings

### Critical Findings (C18-NN)

#### C18-01: Async JSON writeback can be enabled while installing no working triggers

- **Status:** New
- **Area:** Correctness, release engineering, test coverage
- **Location:** [src/json_mapping.rs](../src/json_mapping.rs#L996), [src/json_mapping.rs](../src/json_mapping.rs#L1042), [src/json_mapping.rs](../src/json_mapping.rs#L1188), [src/schema/tables.rs](../src/schema/tables.rs#L25-L28), [src/schema/tables.rs](../src/schema/tables.rs#L904), [sql/pg_ripple--0.127.0--0.128.0.sql](../sql/pg_ripple--0.127.0--0.128.0.sql#L24-L39), [src/storage/ops/mod.rs](../src/storage/ops/mod.rs#L486-L507), [src/storage/ops/mod.rs](../src/storage/ops/mod.rs#L553-L564)
- **Severity:** Critical
- **Blocks v1.0.0:** Yes

`enable_json_writeback_impl()` looks up predicate IDs with:

```sql
SELECT id FROM _pg_ripple.dictionary WHERE iri = $1 LIMIT 1
```

but the dictionary schema defines `id`, `hash`, and `value`; there is no `iri` column. Because the code calls `.unwrap_or(None)`, a SQL error becomes `None`, every predicate is skipped, and the function proceeds to `UPDATE _pg_ripple.json_mappings SET writeback_enabled = true`. The queue drain path has the same defect in reverse, using `SELECT iri FROM _pg_ripple.dictionary WHERE id = $1`, so even manually queued rows cannot decode their subject ID.

There are two additional completeness problems in the same feature. The 0.127.0 -> 0.128.0 migration creates `_pg_ripple.json_writeback_queue` but does not create `_pg_ripple.json_writeback_enqueue_fn()`, even though new installs define that function through `extension_sql!`. Upgraded databases therefore lack the trigger function needed by `enable_json_writeback()`. Finally, the trigger installer only targets existing `vp_{pred_id}_delta` tables. It does not cover `_pg_ripple.vp_rare`, newly promoted predicates after enablement, or tombstone tables used for main-resident deletes. Deletes from main insert tombstones, and rare predicates are deleted directly from `vp_rare`, so important writeback events are never enqueued.

**Recommended fix:**

1. Replace both dictionary references with the real column: `WHERE value = $1 AND kind = KIND_IRI`, and `SELECT value FROM _pg_ripple.dictionary WHERE id = $1 AND kind = KIND_IRI`.
2. Treat predicate lookup errors as fatal inside `enable_json_writeback()`; do not swallow SPI errors with `.unwrap_or(None)`.
3. Only set `writeback_enabled = true` after at least one enqueue path has been installed, or explicitly return a status object with installed trigger count.
4. Add `_pg_ripple.json_writeback_enqueue_fn()` to `sql/pg_ripple--0.127.0--0.128.0.sql`.
5. Install enqueue coverage for `vp_rare`, `vp_{id}_delta`, and `vp_{id}_tombstones`, or move enqueueing into the storage write/delete functions so future VP table creation and promotion cannot bypass it.
6. Add a pg_regress test that enables writeback on an upgraded schema, inserts and deletes triples while predicates are still rare and after promotion, drains the queue, and asserts the relational target changes.

### High Findings (H18-NN)

#### H18-01: Migration-chain test no longer applies the full migration chain

- **Status:** New
- **Area:** Release engineering, upgrade safety
- **Location:** [tests/test_migration_chain.sh](../tests/test_migration_chain.sh#L720-L860), [tests/test_migration_chain.sh](../tests/test_migration_chain.sh#L839-L855), [tests/test_migration_chain.sh](../tests/test_migration_chain.sh#L867-L886)
- **Severity:** High
- **Blocks v1.0.0:** Yes

The migration-chain script is documented as verifying all migrations from v0.1.0 to the current version, but the last explicitly applied migration is `0.95.0 -> 0.96.0`. The repository has migration scripts through `pg_ripple--0.127.0--0.128.0.sql`, but `tests/test_migration_chain.sh` never applies 0.96.0 -> 0.97.0 or anything after it.

The later `MIGCHAIN-SYNC` assertion is ineffective because both `HIGHEST_MIGRATION` and `HIGHEST_CHECKPOINT` are computed from the same list of migration files. That assertion can pass even when no checkpoint or application step exists for the latest release. This is exactly the failure mode behind C18-01: a v0.128 migration can omit its trigger function and still pass migration-chain CI.

**Recommended fix:** Generate an ordered list of migration scripts from `sql/`, apply every script after the base install, and assert the resulting schema reaches `pg_ripple.control`'s `default_version`. Replace the vacuous checkpoint comparison with real post-latest assertions, including `_pg_ripple.json_writeback_enqueue_fn()` existence, v0.128 columns, and a smoke call to `enable_json_writeback()` on an upgraded database.

#### H18-02: Direct JSON writeback returns incorrect row counts and fails common typed schemas

- **Status:** New
- **Area:** Correctness, API ergonomics, test coverage
- **Location:** [src/json_mapping.rs](../src/json_mapping.rs#L805-L821), [src/json_mapping.rs](../src/json_mapping.rs#L895-L926), [src/json_mapping.rs](../src/json_mapping.rs#L739-L759), [docs/src/features/json-mapping.md](../docs/src/features/json-mapping.md#L70-L73), [tests/pg_regress/sql/v0128_json_writeback.sql](../tests/pg_regress/sql/v0128_json_writeback.sql#L268-L287)
- **Severity:** High
- **Blocks v1.0.0:** Yes for JSON-WRITEBACK-01 acceptance

The public contract says `writeback_json_row()` returns affected rows, and the docs say conflict policy `skip` returns 0 rows. The implementation cannot know that. It calls `Spi::run_with_args()` and maps any successful command to `1i64`, so `ON CONFLICT DO NOTHING`, a no-op update, or a zero-row delete all report success as `1`.

The generated INSERT also casts every value to `text` with `SELECT $1::text, $2::text, ...`. That works for the current regression fixture because `contacts_test` uses all `TEXT` columns, but it fails or relies on fragile assignment coercions for the common case of integer, UUID, boolean, JSONB, date, or numeric relational columns. The delete path has the same typed-parameter issue in its `WHERE` clause. The `error` conflict-policy precheck has another edge case: it numbers placeholders before filtering out missing key columns, so a missing first key and present second key can produce `$2` with only one bound parameter.

The regression test even comments that `skip` should return 0, but the expected output accepts 1, so the test enshrines the bug instead of catching it.

**Recommended fix:** Use a statement form that returns actual row counts, for example `WITH upsert AS (...) SELECT count(*) FROM upsert`, `INSERT ... RETURNING 1`, or PostgreSQL command tags via an SPI API that exposes processed rows. Validate that all configured key columns are present before insert/delete. Build casts from `pg_attribute.atttypid::regtype` or avoid forced `::text` and let parameters be typed through explicit target casts. Add tests for integer/UUID key columns, `skip` returning 0, zero-row delete returning 0, and partial-key failure.

#### H18-03: A new 1,245-line `json_mapping.rs` monolith reintroduces god-module risk

- **Status:** New
- **Area:** Code quality, maintainability
- **Location:** [src/json_mapping.rs](../src/json_mapping.rs), [src/storage/ops/scan.rs](../src/storage/ops/scan.rs), [src/maintenance_api.rs](../src/maintenance_api.rs), [src/datalog/parser.rs](../src/datalog/parser.rs), [src/datalog/compiler/mod.rs](../src/datalog/compiler/mod.rs)
- **Severity:** High
- **Blocks v1.0.0:** No, but should be remediated before API freeze

A17's specific god modules were mostly improved: `src/bulk_load/mod.rs` is now 696 LOC and `src/sparql/expr/functions.rs` is 88 LOC. However, v0.128 added a new 1,245-line `src/json_mapping.rs` that combines public SQL API, mapping registration, SHACL warning generation, JSON ingest/export, direct relational writeback, trigger management, status reporting, and queue draining. The concrete C18-01 and H18-02 defects are exactly the kind of cross-concern bug that grows in files at this size.

Current largest files include `src/json_mapping.rs` (1,245 LOC), `src/datalog/parser.rs` (984), `src/datalog/compiler/mod.rs` (983), `src/storage/ops/scan.rs` (975), `src/maintenance_api.rs` (970), `src/storage/merge.rs` (952), `src/export/csv.rs` (949), and `src/schema/tables.rs` (933).

**Recommended fix:** Split JSON mapping into `json_mapping/{registry,ingest,export,writeback,queue,triggers,tests}.rs`. Move SQL generation and row-count handling for writeback into a small testable module. Add a CI soft gate for files over 800 LOC and a hard gate over 1,200 LOC except generated code.

#### H18-04: v1.0.0 external audit and 72-hour load-test evidence are still not on file

- **Status:** Carried from A17
- **Area:** Production readiness, security, scalability
- **Location:** [ROADMAP.md](../ROADMAP.md#L312), [ROADMAP.md](../ROADMAP.md#L320), [roadmap/v1.0.0-full.md](../roadmap/v1.0.0-full.md#L23-L26)
- **Severity:** High
- **Blocks v1.0.0:** Yes

The GA criteria still require an external security-review report and a 72-hour continuous load-test artifact. I found roadmap statements saying these should happen before v1.0.0, but no committed audit report, issue link, artifact, benchmark run history, or `docs/src/benchmarks/` result set that would satisfy the criterion.

**Recommended fix:** Treat these as release blockers independent of code quality. Open a tracked audit engagement item with scope, dates, and expected deliverables. Add a `soak_72h` runner or documented manual release gate that publishes memory, merge latency, query p50/p95/p99, queue depth, and error-count artifacts.

### Medium Findings (M18-NN)

#### M18-01: HTTP companion is fail-open when no auth token is configured

- **Status:** New
- **Area:** Security, deployment ergonomics
- **Location:** [pg_ripple_http/src/common.rs](../pg_ripple_http/src/common.rs#L146-L177), [pg_ripple_http/README.md](../pg_ripple_http/README.md#L34-L40)
- **Severity:** Medium
- **Blocks v1.0.0:** Yes for hardened production defaults

`check_token()` returns `Ok(())` when the expected token is `None`. The README documents `PG_RIPPLE_HTTP_AUTH_TOKEN` as unset by default. In practice, a production deployment that forgets to set the variable exposes SPARQL, admin, JSON writeback, federation status, Datalog, and other endpoints without authentication.

**Recommended fix:** Make production fail closed by default. Require `PG_RIPPLE_HTTP_AUTH_TOKEN` unless `PG_RIPPLE_HTTP_ALLOW_UNAUTHENTICATED=1` is explicitly set, and log a prominent startup warning in development mode. Helm/Docker examples should set secrets or require the explicit opt-out.

#### M18-02: Temporal HTTP Turtle/N-Quads serializers do not escape RDF terms

- **Status:** New
- **Area:** Correctness, interoperability, output safety
- **Location:** [pg_ripple_http/src/routing/temporal_handlers.rs](../pg_ripple_http/src/routing/temporal_handlers.rs#L405-L428), [pg_ripple_http/src/routing/temporal_handlers.rs](../pg_ripple_http/src/routing/temporal_handlers.rs#L513-L540)
- **Severity:** Medium
- **Blocks v1.0.0:** No

The temporal snapshot and diff endpoints build Turtle/N-Quads by hand with `format!("<{s}>")` or `format!("\"{s}\"")`. This does not escape quotes, newlines, backslashes, angle brackets, or invalid IRI characters. It also treats blank nodes starting with `_: `-style prefixes as IRIs by wrapping them in angle brackets. A literal value containing `" . <evil> <p> <o> .` can produce invalid or misleading RDF output.

**Recommended fix:** Use an RDF serializer (`rio_turtle`/`rio_api` or the existing export module) for Turtle and N-Quads. If a minimal local serializer is unavoidable, centralize term escaping and add tests for quotes, newlines, blank nodes, typed literals, language tags, and IRI characters requiring escaping.

#### M18-03: Variable-predicate SPARQL expands to an unbounded UNION over every predicate

- **Status:** New / carried risk, now more visible at scale
- **Area:** Performance, scalability
- **Location:** [src/sparql/sqlgen.rs](../src/sparql/sqlgen.rs#L160-L212)
- **Severity:** Medium
- **Blocks v1.0.0:** No

`build_all_predicates_union()` builds one `UNION ALL` branch per dedicated VP table plus `vp_rare` whenever the predicate position is variable. At thousands of predicates this generates very large SQL strings, stresses PostgreSQL planning time, and can become the dominant cost before execution begins.

**Recommended fix:** Add a configurable maximum expansion limit with a clear error message. Investigate a late-binding strategy using a catalog-driven CTE, partitioned parent table, or executor path that scans candidate predicate tables without embedding thousands of SQL branches. Add a planner-regression benchmark for 1k, 10k, and 50k predicate catalogs.

#### M18-04: Full triple batch scans use OFFSET pagination

- **Status:** New
- **Area:** Performance, scalability
- **Location:** [src/storage/ops/scan.rs](../src/storage/ops/scan.rs#L220-L285)
- **Severity:** Medium
- **Blocks v1.0.0:** No

`for_each_encoded_triple_batch()` paginates VP tables and `vp_rare` with `ORDER BY i LIMIT ... OFFSET ...`. OFFSET pagination becomes O(n^2) for large exports because PostgreSQL must repeatedly walk past earlier rows. This affects full-graph exports, maintenance jobs, and any feature using the batch iterator on large datasets.

**Recommended fix:** Switch to keyset pagination: track the last `i` seen per table and query `WHERE i > $last_i ORDER BY i LIMIT $batch_size`. Ensure each VP table and `vp_rare` have an index that supports `ORDER BY i` (or `(g, i)` when graph-filtered). Add an export benchmark that fails on 2x regression for a multi-million-triple fixture.

#### M18-05: Conformance jobs can silently skip when data fetch fails, and OWL 2 RL is mislabeled as blocking

- **Status:** New
- **Area:** CI, release evidence
- **Location:** [.github/workflows/ci.yml](../.github/workflows/ci.yml#L471-L488), [.github/workflows/ci.yml](../.github/workflows/ci.yml#L568-L585), [.github/workflows/ci.yml](../.github/workflows/ci.yml#L888-L904), [tests/w3c_suite.rs](../tests/w3c_suite.rs#L34-L41), [tests/jena_suite.rs](../tests/jena_suite.rs#L81-L86), [tests/owl2rl_suite.rs](../tests/owl2rl_suite.rs#L136-L145)
- **Severity:** Medium
- **Blocks v1.0.0:** No, but weakens release evidence

The W3C, Jena, and OWL 2 RL fetch steps use `|| echo "... download failed - tests will skip"`. The test binaries then return success when data is missing. The OWL 2 RL job is named "blocking >=95% pass rate" and has `continue-on-error: false`, but the test only panics when `OWL2RL_REQUIRE` is set; the workflow does not set it.

**Recommended fix:** In CI, make data fetch failure fatal for required jobs. For informational jobs, name them accurately and upload a clear skipped artifact. Set `OWL2RL_REQUIRE=1` once the job is meant to be blocking, or rename it to informational until then.

#### M18-06: Compatibility matrix and HTTP README drifted behind recent releases

- **Status:** New
- **Area:** Documentation, operations
- **Location:** [docs/src/operations/compatibility.md](../docs/src/operations/compatibility.md#L20-L24), [pg_ripple_http/README.md](../pg_ripple_http/README.md#L34-L40), [pg_ripple_http/src/main.rs](../pg_ripple_http/src/main.rs#L222-L230), [docs/src/reference/http-api.md](../docs/src/reference/http-api.md#L130-L147)
- **Severity:** Medium
- **Blocks v1.0.0:** No

The compatibility matrix stops at `0.127.x`; there is no row for `0.128.x`. The HTTP README still says the default rate limit is `0` and CORS default is `*`, but the code defaults to rate limit `100` and empty CORS origins. The reference HTTP API page covers the recent temporal, federation auth-status, and JSON writeback endpoints, but the primary `pg_ripple_http/README.md` does not.

**Recommended fix:** Add a 0.128.x compatibility row, update README defaults from code, and either link to `docs/src/reference/http-api.md` as the canonical endpoint reference or mirror the recent endpoint examples in the README.

#### M18-07: JSON writeback configuration requires direct writes to internal catalog tables

- **Status:** New
- **Area:** API ergonomics, privilege boundaries
- **Location:** [docs/src/features/json-mapping.md](../docs/src/features/json-mapping.md#L44-L55), [src/json_mapping.rs](../src/json_mapping.rs#L52-L69)
- **Severity:** Medium
- **Blocks v1.0.0:** No, but should be improved before API freeze

The docs instruct users to configure writeback by directly updating `_pg_ripple.json_mappings`. The public `register_json_mapping()` API does not accept `writeback_table`, `writeback_schema`, `writeback_key_columns`, or `writeback_conflict_policy`, and there is no dedicated `configure_json_writeback()` helper. Direct catalog writes are fragile for users, hard to permission safely, and bypass validation that the target table and key columns exist.

**Recommended fix:** Add `pg_ripple.configure_json_writeback(mapping, schema, table, key_columns, conflict_policy)` and a matching HTTP/admin endpoint if needed. Validate table existence, key columns, conflict policy, and target column types. Deprecate documentation that asks users to update `_pg_ripple` tables directly.

#### M18-08: `pg_ripple.llm_api_key_env` only warns on raw-looking secrets

- **Status:** New
- **Area:** Security, secret handling
- **Location:** [src/gucs/registration/observability.rs](../src/gucs/registration/observability.rs#L12-L42), [src/gucs/registration/observability.rs](../src/gucs/registration/observability.rs#L73-L81)
- **Severity:** Medium
- **Blocks v1.0.0:** No

The assign hook detects raw-looking API keys and warns that they will appear in `pg_settings` and logs, but it still accepts the value. Because the GUC is `Userset`, accidental secret exposure is easy and persistent.

**Recommended fix:** Convert this to a check hook that rejects raw-looking values, or constrain the accepted grammar to environment-variable names (`^[A-Z_][A-Z0-9_]*$`). Keep the warning for legacy compatibility only behind a temporary escape hatch.

### Low Findings (L18-NN)

#### L18-01: `PG_RIPPLE_HTTP_TRUST_PROXY` is stored but not used

- **Status:** New
- **Area:** Configuration, operations
- **Location:** [pg_ripple_http/src/common.rs](../pg_ripple_http/src/common.rs#L56-L60), [pg_ripple_http/src/main.rs](../pg_ripple_http/src/main.rs#L239-L240)
- **Severity:** Low
- **Blocks v1.0.0:** No

The application state stores a trusted proxy list for `X-Forwarded-For`, but no routing or middleware code appears to consume it. Operators may believe rate limiting or access decisions honor proxy headers when they do not.

**Recommended fix:** Either wire this into a vetted `Forwarded`/`X-Forwarded-For` extractor with strict CIDR validation, or remove the variable until implemented.

#### L18-02: Queue-drain status updates ignore SPI errors

- **Status:** New
- **Area:** Robustness, observability
- **Location:** [src/json_mapping.rs](../src/json_mapping.rs#L1193-L1203), [src/json_mapping.rs](../src/json_mapping.rs#L1226-L1236)
- **Severity:** Low
- **Blocks v1.0.0:** No

`drain_json_writeback_queue()` ignores errors when marking queue rows processed or failed. If the status update fails, the worker may retry the same row without any durable explanation.

**Recommended fix:** Log at least a warning with row ID and mapping name; ideally leave the row pending but increment a metric. Avoid `let _ =` for queue state transitions.

#### L18-03: Checked-in conformance report artifacts are stale or ambiguous

- **Status:** New
- **Area:** Documentation, release evidence
- **Location:** [w3c_report/report.json](../w3c_report/report.json#L170-L179), [jena_report/report.json](../jena_report/report.json#L1-L20), [watdiv_report/report.json](../watdiv_report/report.json#L1-L20)
- **Severity:** Low
- **Blocks v1.0.0:** No

The checked-in W3C report says `version: 0.43.0` and 96.0% pass rate, while other report directories contain mixed Jena/WatDiv summaries. It is unclear whether these files are current release evidence or historical artifacts.

**Recommended fix:** Add a generated timestamp/version to every conformance report and publish current artifacts under a stable `results/conformance/<version>/` path. Treat stale report files as historical and label them as such.

## Info / Positive Observations

- H17-01 is resolved: `subscribe_rule_library()` now routes through `resolve_and_check_endpoint()` before writing the subscription catalog.
- H17-02 is mostly resolved: `src/bulk_load/mod.rs` is down to 696 LOC and `src/sparql/expr/functions.rs` is down to 88 LOC, although `src/storage/ops/scan.rs` remains near the threshold and `src/json_mapping.rs` is a new monolith.
- M17-01 is resolved for now: RSA advisories in `audit.toml` have updated rationale and `expires = "2027-01-01"`.
- Workspace clippy still denies undocumented unsafe blocks, and the current scan shows 66 unsafe blocks with 87 SAFETY comments.
- Version constants are synchronized: root `Cargo.toml`, `pg_ripple.control`, and `pg_ripple_http/Cargo.toml` are all `0.128.0`.
- v0.128.0 has a dedicated pg_regress file with 21 JSON writeback tests; the problem is depth and assertions, not total absence.
- SSRF policy coverage has improved: CGNAT, multicast, this-network, and IPv4-mapped IPv6 handling are present in federation policy code.

## Remediation Tracking

| A17 item | Status in A18 | Evidence / note |
|---|---|---|
| H17-01: `subscribe_rule_library()` SSRF string-contains bypass | Resolved | Now calls `resolve_and_check_endpoint()` before catalog write. |
| H17-02: `bulk_load`, `sparql/expr/functions`, `storage/ops/scan` god files | Partly resolved / superseded | `bulk_load` 696 LOC, `expr/functions` 88 LOC, `storage/ops/scan` 975 LOC; new `json_mapping.rs` 1,245 LOC creates H18-03. |
| M17-01: RSA RUSTSEC expiry 2026-12-01 | Resolved for this cycle | `audit.toml` extends RSA advisory review to 2027-01-01 with updated rationale. |
| A17 test gap for v0.119/v0.120 features | Resolved | Later releases added pg_regress/conformance coverage; current new test gap is v0.128 async writeback depth. |
| External security-review report | Carried | Still roadmap/planned; no report artifact found. |
| 72-hour load test | Carried | Still roadmap/planned; no v1.0 soak artifact found. |
| New v0.128 async JSON writeback failure | New | C18-01. |
| Migration-chain cutoff at 0.96.0 | New / recurring class | H18-01. |

## Conformance Suite Status

| Suite | Tests Run / Artifact | Passing | Pass Rate | CI Gate | Notes |
|---|---:|---:|---:|---|---|
| W3C SPARQL 1.1 full | 324 in checked-in `w3c_report/report.json` | 311 | 96.0% | Informational in test code | Artifact says version 0.43.0; workflow name says required, but test does not fail on unexpected failures. |
| W3C smoke | CI job present | Not inspected in artifact | Required | Required | Smoke job is the meaningful blocking W3C gate per repo guidance. |
| Apache Jena | `jena_report/report.json`: 1,088 total | 1,087 plus 1 xfail | ~99.9% | Blocking when data present | Fetch failure can still skip the suite. |
| WatDiv | `watdiv_report/report.json`: 32 templates | 32 | 100% | Non-blocking / benchmark | Current artifact small relative to roadmap's 100-template target. |
| LUBM | 14 documented queries | 14 | 100% | Required | `docs/src/reference/lubm-results.md` reports all 14 pass. |
| OWL 2 RL | CI job present | Not summarized in checked-in artifact | Unknown | Labeled blocking, effectively informational | Workflow does not set `OWL2RL_REQUIRE`; missing data or failures can pass. |

## Scorecard

| Dimension | Score (/5) | Delta from A17 | Cap applied? | Notes |
|---|---:|---:|---|---|
| Correctness | 4.0 | -0.5 | Critical cap | C18-01 and H18-02 directly affect the newest release feature. |
| Robustness | 4.3 | -0.2 | High cap nearby | Unsafe discipline remains good; queue update errors and worker retry patterns need polish. |
| Architecture | 4.2 | -0.3 | High cap | A17 god modules improved, but `json_mapping.rs` reintroduces a 1,245 LOC multi-concern module. |
| Performance | 4.3 | -0.3 | Medium findings | Variable-predicate UNION expansion and OFFSET scans remain scale risks. |
| Security | 4.1 | -0.3 | Medium findings | SSRF is better; fail-open HTTP auth, raw-secret GUC acceptance, and missing external audit remain. |
| Testing | 4.0 | -0.7 | Critical cap | Migration chain stops at 0.96.0; v0.128 tests miss async writeback failure. |
| Documentation | 4.3 | -0.3 | Medium findings | Strong docs overall, but compatibility/README drift and direct internal-catalog instructions are not GA-quality. |
| Release engineering | 4.0 | -0.8 | Critical cap | Migration script missing trigger function and migration-chain truth gap are blockers. |
| Code quality | 4.2 | -0.3 | High cap | 171 `#[allow]` suppressions and new monolith; 68 panic-prone calls mostly test/infallible paths. |
| **Weighted overall** | **4.15 / 5.0** | **-0.42** |  | Regression from A17 due to v0.128 feature-integrity and migration-chain findings. |

## Critical Path to v1.0.0

1. Fix C18-01 completely: dictionary column names, migration trigger function, rare/delta/tombstone enqueue coverage, and end-to-end async writeback tests. Estimated effort: 2-4 days.
2. Rewrite `tests/test_migration_chain.sh` to apply every migration through current `default_version` and assert latest schema. Estimated effort: 1-2 days.
3. Fix direct writeback affected-row semantics, typed column support, and key validation. Estimated effort: 2-3 days.
4. Split `src/json_mapping.rs` before API freeze; isolate writeback and queue code for focused tests. Estimated effort: 2-4 days.
5. Decide production auth default for `pg_ripple_http`; fail closed unless explicitly opted out. Estimated effort: 1 day plus docs/chart updates.
6. Make conformance CI labels match behavior: either hard-fail required jobs on missing data/failures or mark them informational. Estimated effort: 1 day.
7. Complete v1.0.0 external security audit and 72-hour load-test artifact. Estimated effort: 4-6 calendar weeks for audit scheduling/execution plus dedicated soak infrastructure.

## Appendix: Full Finding Index

| ID | Severity | Area | Summary | Blocking? |
|---|---|---|---|---|
| C18-01 | Critical | Correctness / release | Async JSON writeback can be enabled while installing no working triggers | Yes |
| H18-01 | High | Release engineering | Migration-chain test no longer applies migrations after 0.96.0 | Yes |
| H18-02 | High | Correctness / API | Direct JSON writeback reports incorrect row counts and emits text-only parameters | Yes for JSON-WRITEBACK-01 |
| H18-03 | High | Code quality | New 1,245-line `json_mapping.rs` monolith | No |
| H18-04 | High | Production readiness | External audit and 72-hour load-test evidence still missing | Yes |
| M18-01 | Medium | Security | HTTP companion is unauthenticated when auth token is unset | Yes for production hardening |
| M18-02 | Medium | Correctness | Temporal HTTP RDF serializers do not escape Turtle/N-Quads terms | No |
| M18-03 | Medium | Performance | Variable-predicate SPARQL creates unbounded UNION SQL | No |
| M18-04 | Medium | Performance | Full triple batch scans use OFFSET pagination | No |
| M18-05 | Medium | CI | Conformance jobs can skip or pass despite missing data/failures | No |
| M18-06 | Medium | Docs | Compatibility matrix and HTTP README drift behind current code | No |
| M18-07 | Medium | Ergonomics | JSON writeback configuration requires direct internal catalog UPDATE | No |
| M18-08 | Medium | Security | `llm_api_key_env` warns but accepts raw-looking secrets | No |
| L18-01 | Low | Config | `PG_RIPPLE_HTTP_TRUST_PROXY` is stored but unused | No |
| L18-02 | Low | Robustness | Queue-drain status updates ignore SPI errors | No |
| L18-03 | Low | Evidence | Checked-in conformance reports are stale or ambiguous | No |
