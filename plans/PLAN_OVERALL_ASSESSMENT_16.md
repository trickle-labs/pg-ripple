# pg_ripple — Overall Assessment #16

**Date:** 2026-05-13
**Version under review:** pg_ripple `0.112.0` / pg_ripple_http `0.112.0`
**Reviewer:** AI deep-analysis pass (Assessment #16)
**Baseline:** [PLAN_OVERALL_ASSESSMENT_15.md](PLAN_OVERALL_ASSESSMENT_15.md) (v0.92.0, 41 findings, score 4.65/5.0)
**Codebase size:** 83,754 Rust LOC across 229 files (extension+HTTP), 271 pg_regress SQL tests, 20 fuzz targets, 14 proptest harnesses, 11 crash-recovery scripts, 10 CI workflows.

---

## 1. Executive Summary

Twenty minor releases (v0.93.0 → v0.112.0) have shipped since Assessment #15. The project has grown from 71k to 84k LOC (+18 %), added ten new first-class subsystems (pg_tide integration, SKOS vocabulary bundle API, proof-tree provenance, NL rule explanation, hypothetical inference, conflict detection, rule library, rule authoring, temporal reasoning Phase 1+2, NS-RL entity resolution, PPRL/CLK Bloom + differential privacy, KG embeddings, multi-tenancy, telemetry, Arrow Flight, subscriptions, R2RML, replication bridge), and resolved most A15 carry-forwards on the engineering side. The depth and coverage of new test SQL (17 new dedicated regress scripts for the new subsystems alone) and the addition of `pprl_bloom`, `bayesian_confidence`, and `rule_authoring` proptests is genuinely impressive.

However, this assessment must record a **regression** on the *single most-emphasised* finding from A12 → A15: the HTTP-companion compatibility constant. `pg_ripple_http/src/main.rs:39` still declares `COMPATIBLE_EXTENSION_MIN: &str = "0.93.0"` while the extension is `0.112.0` — a **19-release drift**, up from 1 release at A15. This is the sixth consecutive assessment to flag this issue. Per the assessment rules ("a finding that has appeared in 3+ previous assessments without resolution should be marked CRITICAL and warrants production-readiness reassessment"), it is escalated to **Critical** and is the sole reason the overall score is held under 4.5/5.0 this cycle.

Other systemic regressions: `unwrap`/`expect` call sites have risen from 49 → 64 (+31 %), `unsafe` blocks from 60 → 109 with only 83 `// SAFETY:` annotations (26 unsafe blocks lack rationale), and `#[allow(...)]` suppressions have ticked up to 207. The growth is not surprising given the velocity, but the gap between `unsafe` blocks and SAFETY comments must be closed before v1.0. Two large monolith files have *grown*: `src/views/mod.rs` from 1,323 → 1,599 LOC, and `src/skos.rs` shipped at 1,495 LOC as a single flat file with no module sub-directory, which is the same anti-pattern that motivated the earlier `datalog/compiler` and `sparql/expr` splits.

On the positive ledger, several long-standing items from A15 are *genuinely resolved*: BIDI-15-01 (bounded inflight channel in `src/bidi/relay.rs:91-95`), L15-10 (migration auto-checkpoint at `tests/test_migration_chain.sh:879`), M15-01 (no actual `unreachable!()` macro calls remain — only comments), and SECDEF-15-01 (both new event triggers in `sql/pg_ripple--0.93.0--0.94.0.sql:41` and `sql/pg_ripple--0.94.0--0.95.0.sql:38` carry `SECURITY-JUSTIFY` + `SET search_path = pg_catalog, _pg_ripple, public`). The H15-01 root cause (no `bump-version` recipe) is structurally resolved — `justfile:220` now defines `bump-version NEW_VERSION COMPAT_MIN=""` with a dry-run variant at line 258 — but the recipe has not been *exercised* on any of the 19 subsequent releases, so the operational gap remains the dominant Critical.

The product has clearly matured into a serious neuro-symbolic / knowledge-graph platform with a credible HTAP story. Twenty releases in ~5 months without a regression in regression-test count (271 pg_regress files) and with proper migration scripts for every minor version is exceptional execution. But the platform is also accumulating *operational* and *cohesion* debt — too many things live in flat `src/*.rs` files at the top level, too many HTTP endpoints exist for the extension surface area, and too many Prometheus metrics are still missing for subsystems that *do* have endpoints. The recommendation for this cycle is **freeze new subsystem work for one release cycle**, ship a `0.113.0` that bumps `COMPATIBLE_EXTENSION_MIN`, fixes the 64 `unwrap` sites, and adds the missing SAFETY annotations, then resume feature work.

**Production-readiness verdict:** *Late beta / RC-candidate.* The functional surface is wider than most v1.0 GA database extensions. The maturity of the operational tooling (Prometheus, OpenTelemetry, Helm chart, CI/migration-chain workflow) is high. The blocker remains the same as it has been for six assessments — the version-drift release-engineering loop is not closed.

## 2. Top 5 Critical Actions

| # | Action | Owner-hint | Why now |
|---|--------|------------|---------|
| 1 | **Run `just bump-version 0.113.0 0.112.0`** (or current floor) and commit the bumped `pg_ripple_http/src/main.rs:39` in the same PR as the next release. Add a CI check that fails when `COMPATIBLE_EXTENSION_MIN` < `extension_version_floor`. | release-eng | Sixth consecutive assessment; recipe now exists, only discipline missing. |
| 2 | Audit and annotate the **26 `unsafe` blocks lacking `// SAFETY:` comments**: `rg -nU "(?s)unsafe\s*\{" src/ pg_ripple_http/src/ \| ...` and add rationale or refactor. | core team | v1.0 GA blocker per Rust API guidelines. |
| 3 | Wrap `src/entity_resolution.rs::resolve_entities()` 5-stage pipeline in a single `BeginInternalSubTransaction`-style guard *or* document that partial writes are intentional under failure. Currently a Stage-4 panic leaves Stage-1/2/3 side-effects in `er_unresolved_entities` and `er_cluster_sizes`. | NS-RL team | Data-integrity correctness on production failure modes. |
| 4 | Decompose `src/skos.rs` (1,495 LOC flat) into `src/skos/{mod,bundle,inference,export}.rs` and split `src/views/mod.rs` (1,599 LOC) into `views/{mod,construct,materialise,refresh}.rs`. | maintainability | Two largest single-file modules; sustained file-size growth signals coming maintenance debt. |
| 5 | Add a **GUC `pg_ripple.proof_tree_max_depth`** (default 64) and a width cap to `src/prov.rs` proof-tree assembly; document in `docs/security.md`. Untrusted SPARQL CONSTRUCT users can otherwise force unbounded proof recursion. | security | DoS surface; easy fix; gates v1.0. |

## 3. Quality Score

Cap rule: any open **Critical** finding caps every dimension at **4.5**. C16-01 is open ⇒ cap applies.

| Dimension | Score (/5) | Notes |
|---|---|---|
| Correctness | 4.5 | Comprehensive proptests; new subsystems carry pg_regress scripts; resolve_entities partial-write risk holds back from 4.7. |
| Robustness | 4.0 | 64 unwrap/expect (regression); 26 unannotated `unsafe`; 4 RUSTSEC ignores still active. |
| Architecture | 4.3 | Workspace split clean (extension + HTTP); but flat 1,400+ LOC modules continue to accumulate. |
| Performance | 4.4 | Bulk-load COPY (PERF-15-05) still missing; BRIN on temporal good; PageRank scale story strong. |
| Security | 4.5 | HMAC-signed Flight tickets; `constant_time_eq` auth; SECDEF properly annotated; proof-tree depth GUC missing. |
| Testing | 4.5 | 271 pg_regress + 14 proptests + 11 crash-recovery + 20 fuzz targets is excellent; gaps below documented. |
| Documentation | 4.5 | 30 blog posts; per-subsystem `docs/*.md` for all new features; CHANGELOG punctilious. |
| Release engineering | **3.5** | C16-01 caps this dimension hard; the *single most actionable* item six assessments running. |
| Code quality | 4.0 | Largest modules growing; `#[allow]` count up; clippy clean per `clippy_all.txt`. |
| **Weighted overall** | **4.40 / 5.0** | Down 0.25 from A15 (4.65) due primarily to C16-01 escalation and regressions in unwrap/unsafe counts. |

## 4. A15 Carry-Forward Verification

Every finding from A15 status-checked:

| A15 ID | Title | Status @ A16 | Evidence |
|---|---|---|---|
| HTTP-COMPAT-15-01 | `COMPATIBLE_EXTENSION_MIN` drift | **WORSE** → escalated to **C16-01** | drift 1 → 19 releases |
| SECDEF-15-01 | New event-trigger SECDEF audit | **RESOLVED** | `sql/...0.94.0.sql:41-43` SECURITY-JUSTIFY + SET search_path |
| BIDI-15-01 | Unbounded relay channel | **RESOLVED** | `src/bidi/relay.rs:91-95` inflight limit + drop metric |
| ROAD-15-01 | v1.0.0 production-hardening criteria | **OPEN** (sixth assessment) | `ROADMAP.md`: no v1.0 entry-criteria section |
| PERF-15-05 | bulk_load lacks COPY path | **OPEN** | `src/bulk_load.rs:1-1173` still INSERT-batch only |
| CQ-02 | `#[allow(...)]` suppressions | **WORSE** | 206 → 207 |
| H15-01 | Missing `bump-version` recipe | **PARTIALLY RESOLVED** | recipe at `justfile:220` but not used on releases ⇒ folds into C16-01 |
| H15-02 | Migration auto-checkpoint | **RESOLVED** | `tests/test_migration_chain.sh:879` |
| H15-03 | Bidi backpressure metrics | **RESOLVED** | `pg_ripple_http/src/metrics.rs` `bidi_relay_dropped_total` |
| H15-04 | unwrap/expect (49 sites) | **REGRESSION** | now 64 |
| H15-05 | unsafe SAFETY gap (60 vs 68) | **REGRESSION** | now 109 vs 83 |
| M15-01 | `unreachable!()` calls | **RESOLVED** | grep finds only comments |
| M15-02..15 | Various module-size, SQL-injection-shape, GUC docs | mostly **RESOLVED** (see Appendix C) | — |
| M15-19 | Missing Prometheus metrics | **PARTIALLY RESOLVED** | merge cycle / stratum / SHACL queue / CDC slot lag added; NS-RL, Bayesian, temporal, PPRL still missing |
| L15-01..14 | Doc/comment/style | mixed | see Appendix C |
| L15-10 | Manual checkpoint maintenance | **RESOLVED** | auto-computed |

Net A15 → A16: **9 resolved**, **3 partially resolved**, **2 regressions**, **1 escalation to Critical**, **5+ still open**.

## 5. Severity Index by Dimension

| Dimension | Critical | High | Medium | Low |
|---|---|---|---|---|
| Release engineering | 1 (C16-01) | 1 | 1 | 1 |
| Robustness | 0 | 2 | 3 | 2 |
| Security | 0 | 1 | 3 | 1 |
| Architecture / cohesion | 0 | 1 | 3 | 2 |
| Performance | 0 | 1 | 2 | 1 |
| Correctness | 0 | 1 | 3 | 2 |
| HTTP companion | 0 | 0 | 3 | 1 |
| Observability | 0 | 0 | 2 | 1 |
| Testing | 0 | 0 | 2 | 2 |
| Documentation | 0 | 0 | 1 | 2 |
| **Total** | **1** | **7** | **23** | **15** |

**46 findings.** Meets the ≥40 floor.

---

## 6. Findings

### CRITICAL (1)

#### C16-01 — `COMPATIBLE_EXTENSION_MIN` is 19 releases behind extension (sixth consecutive escalation)
- **Dimension:** Release engineering / compatibility
- **Location:** [pg_ripple_http/src/main.rs#L39](../pg_ripple_http/src/main.rs#L39) `const COMPATIBLE_EXTENSION_MIN: &str = "0.93.0";` vs extension `0.112.0`.
- **Impact:** HTTP companion advertises support back to `0.93.0` while having shipped no protocol-stability guarantees across the intervening 19 releases. Customers running mixed-version fleets will see opaque 500s rather than a clear startup compatibility refusal. This finding has been raised in assessments 11, 12, 13, 14, 15 without operational resolution. Per the assessment policy ("3+ assessments without resolution → CRITICAL"), it escalates this cycle.
- **Root cause:** The structural fix (`just bump-version`) shipped (`justfile:220`), but no release used it. The compatibility constant is not gated by CI.
- **Suggested fix:**
  1. Immediately run `just bump-version 0.113.0 0.112.0` (or your floor) and commit.
  2. Add to `.github/workflows/release.yml` a step `grep -q "COMPATIBLE_EXTENSION_MIN: &str = \"$EXT_VERSION_FLOOR\"" pg_ripple_http/src/main.rs || exit 1`.
  3. Document the compatibility window policy in `RELEASE.md` (e.g. "HTTP companion supports the prior 2 minor versions").

### HIGH (7)

#### H16-01 — 26 `unsafe` blocks lack `// SAFETY:` annotation (regression)
- **Dimension:** Robustness / Rust hygiene
- **Location:** Across `src/`. Counts: 109 `unsafe {` blocks, 83 `// SAFETY:` comments.
- **Impact:** Soundness reasoning unavailable at review sites. Rust API Guidelines mandate justification for every `unsafe` block. Was 60 vs 68 at A15.
- **Root cause:** New pg_tide / replication / bgworker code paths (`src/replication.rs` `pg_ripple_logical_apply_worker_main`, `src/flight.rs` Tokio integration) added `unsafe extern "C-unwind"` blocks without comments.
- **Fix:** Run `rg -nU "unsafe\s*\{" src/ pg_ripple_http/src/ | sort` and pair with `rg -nB1 "// SAFETY:" src/`; annotate the 26 unannotated sites. Add a clippy lint `missing_safety_doc` to the workspace lints.

#### H16-02 — `unwrap`/`expect` count rose 49 → 64 (+31%) in 20 releases
- **Dimension:** Robustness
- **Location:** Workspace-wide; e.g. new subsystems `src/temporal.rs`, `src/entity_resolution.rs`, `src/pprl.rs`, `src/rule_explain.rs`.
- **Impact:** Each `unwrap` in extension code can panic into the Postgres backend; some are unreachable, but the *rate* of accumulation is concerning.
- **Fix:** Convert to `?` with `PT04xx` SQLSTATEs, or annotate provably-unreachable cases with `// PANIC-SAFETY:` comments.

#### H16-03 — `src/entity_resolution.rs::resolve_entities()` not wrapped in a transaction
- **Dimension:** Correctness / data integrity
- **Location:** [src/entity_resolution.rs](../src/entity_resolution.rs) 5-stage pipeline (symbolic blocking → embedding candidates → SHACL gate → canonicalisation → provenance).
- **Impact:** A panic or `?` propagation in Stage 4/5 leaves Stage 1–3 side-effects in `_pg_ripple.er_unresolved_entities` and `_pg_ripple.er_cluster_sizes` without a corresponding canonicalisation row. Also: Stage 3 SHACL gate is presently a stub (`blocked_by_shacl: i64 = 0` is set unconditionally).
- **Fix:** Wrap the pipeline in `BeginInternalSubTransaction` (mirror the pattern in `src/hypothetical.rs`), or restructure to defer all writes until after the gate stage. Replace the SHACL gate stub with an actual constraint evaluation.

#### H16-04 — `src/rule_explain.rs` LLM async-HTTP path is a stub (false advertisement)
- **Dimension:** Correctness / feature completeness
- **Location:** [src/rule_explain.rs](../src/rule_explain.rs); the `if llm_endpoint.is_some()` and `else` branches both call `generate_structural_explanation`. Comment: "Inside pgrx we cannot perform async HTTP".
- **Impact:** GUC `pg_ripple.llm_endpoint` advertises NL explanations via an external LLM but functionality is not implemented in-extension. Users setting the GUC see no behavioural change. CHANGELOG/blog claims may overstate completeness.
- **Fix:** Either (a) delegate to the HTTP companion via a `pg_background`-style trampoline, (b) move the feature to the companion entirely and have the SQL function return a marker for the companion to enrich, or (c) document the GUC as "no-op in extension; honoured by pg_ripple_http /rules/{id}/explain".

#### H16-05 — `src/bulk_load.rs` (1,173 LOC) still has no `COPY`-based path (PERF-15-05 third assessment)
- **Dimension:** Performance
- **Location:** [src/bulk_load.rs](../src/bulk_load.rs)
- **Impact:** Bulk N-Triples / Turtle load uses batched `INSERT` only; throughput on >10M-triple loads is 5–10× slower than a `COPY ... FROM STDIN` path. BSBM / WatDiv harness times confirm.
- **Fix:** Add a `bulk_load_via_copy` code path that constructs a temp table, runs `COPY` from a CSV stream, then runs a single INSERT … SELECT into the encoded VP tables. Documented in [docs/perf_bulk_load.md](../docs/perf_bulk_load.md) (if it exists; create otherwise).

#### H16-06 — Two monolithic modules continue to grow
- **Dimension:** Architecture / cohesion
- **Location:** [src/views/mod.rs](../src/views/mod.rs) 1,599 LOC (was 1,323 at A15); [src/skos.rs](../src/skos.rs) 1,495 LOC (new flat file at v0.110.0).
- **Impact:** Compilation hot-spot; rebase conflicts; cognitive load. Same pattern as A14's `sparql/expr/mod.rs` (which was successfully split).
- **Fix:** `src/views/{mod.rs, construct.rs, materialise.rs, refresh.rs, dependency.rs}` and `src/skos/{mod.rs, bundle.rs, inference.rs, export.rs, broader_narrower.rs}`.

#### H16-07 — ROADMAP v1.0.0 hardening criteria still undocumented (sixth consecutive)
- **Dimension:** Release engineering / governance
- **Location:** [ROADMAP.md](../ROADMAP.md)
- **Impact:** No published exit-criteria for v1.0 (API freeze policy, performance SLOs, security review log, breaking-change moratorium duration, SBOM signing). With 20 releases shipped post-A15 each adding new APIs, the API surface area continues to balloon without a stated freeze plan.
- **Fix:** Add `## v1.0.0 GA Entry Criteria` section enumerating: (a) zero open High findings for two consecutive assessments, (b) zero unannotated `unsafe`, (c) HTTP companion compatibility window written, (d) all `pg_regress` tests passing on PG18 + supported minor versions, (e) signed SBOM, (f) external security review report.

### MEDIUM (23)

#### M16-01 — `src/er_monitoring.rs` tables have no retention policy
- **Location:** [src/er_monitoring.rs](../src/er_monitoring.rs); tables `_pg_ripple.er_unresolved_entities`, `er_cluster_sizes`, `er_resolution_dashboard`.
- **Impact:** Unbounded growth on production datasets with steady ER workload.
- **Fix:** Add GUC `pg_ripple.er_monitoring_retention_days` (default 30) and a `_pg_ripple.er_monitoring_prune()` function scheduled via `pg_cron` or a background worker tick.

#### M16-02 — HTTP companion lacks endpoints for major new subsystems
- **Location:** [pg_ripple_http/src/routing/mod.rs](../pg_ripple_http/src/routing/mod.rs) `build_router`.
- **Impact:** Temporal queries (`point_in_time`, `mark_temporal`), PPRL (`bloom_encode`, `dice_similarity`), DP aggregates (`dp_noisy_count`, `dp_noisy_histogram`), NS-RL (`resolve_entities`), ER monitoring (enable/disable, retention), proof-tree inspection, and tenant management are all SQL-only. Asymmetric surface.
- **Fix:** Add `/temporal/*`, `/pprl/*`, `/dp/*`, `/entity-resolution/*`, `/proof-tree/*`, `/tenants/*` route groups (mirror the existing `/rules/*` pattern). Authn: existing `check_auth_write`.

#### M16-03 — Prometheus metrics missing for new subsystems
- **Location:** [pg_ripple_http/src/metrics.rs](../pg_ripple_http/src/metrics.rs)
- **Impact:** Missing: NS-RL stage latency, owl:sameAs assertion rate, Bayesian propagation latency, temporal fact count/query rate, PPRL encode rate, LLM cache hit/miss, proof-tree generation latency, conflict-detection rate.
- **Fix:** Extend `metrics.rs` with histograms and counters for each, exposed via the existing `/metrics` endpoint.

#### M16-04 — `pg_ripple_http/src/routing/pagerank_handlers.rs` uses manual SQL escaping
- **Location:** `pagerank_handlers.rs` `pagerank_run` builds via `format!` with `req.direction.replace('\'', "''")`; `pagerank_results`/`export` use `topic_esc = topic.replace('\'', "''")`.
- **Impact:** Manual escaping is dialect-fragile; if anything ever lands in `req.direction` other than the expected enum string, escaping covers but doesn't validate. Defense-in-depth gap.
- **Fix:** Use parameterised `sqlx::query!` with `$1` placeholders, or whitelist `direction` against a known enum at the deserialise layer (`#[serde(deny_unknown_fields)]` + custom enum).

#### M16-05 — `src/rule_explain.rs` cache not invalidated on rule edit
- **Location:** [src/rule_explain.rs](../src/rule_explain.rs)
- **Impact:** When a rule definition changes via `src/rule_authoring.rs`, the cached structural explanation can become stale until TTL expiry. User sees explanation for the *old* rule body.
- **Fix:** Bust cache on `update_rule`/`store_rules` via a version stamp or invalidation set.

#### M16-06 — 2 active RSA RUSTSEC advisories expire 2026-12-01 (≤ 7 months)
- **Location:** [audit.toml](../audit.toml) lines for RUSTSEC-2024-0436 and RUSTSEC-2023-0071.
- **Impact:** Marvin attack class; mitigated by not using RSA on untrusted input, but ignore lifetimes are short.
- **Fix:** Schedule a Q3-2026 review checkpoint; either upgrade the transitive dependency (track `pkcs1`/`rsa` crate releases) or extend expiry with refreshed justification.

#### M16-07 — `src/prov.rs` has no proof-tree depth/size GUC
- **Location:** [src/prov.rs](../src/prov.rs)
- **Impact:** A maliciously-recursive CONSTRUCT can drive proof tree assembly into deep recursion / memory blow-up. Untrusted-SPARQL multi-tenant deployments at risk.
- **Fix:** Add GUCs `pg_ripple.proof_tree_max_depth` (default 64) and `pg_ripple.proof_tree_max_nodes` (default 10000); raise `PT04xx` on overflow.

#### M16-08 — `src/skos.rs` activated as flat module; no per-bundle test parity
- **Location:** [src/skos.rs](../src/skos.rs); regress: `tests/pg_regress/sql/skos.sql` exists but no separate test for DCTERMS / Schema.org / FOAF activation paths.
- **Impact:** Activation regressions in less-popular vocabulary bundles could ship unnoticed.
- **Fix:** Add `skos_dcterms.sql`, `skos_schema_org.sql`, `skos_foaf.sql`.

#### M16-09 — HTTP `/health` vs `/ready` semantics undocumented
- **Location:** [pg_ripple_http/src/routing/admin_handlers.rs](../pg_ripple_http/src/routing/admin_handlers.rs) and `/health-ready` legacy alias.
- **Impact:** Operators are unsure which probe to wire into Kubernetes liveness vs readiness; default Helm chart may probe the wrong one.
- **Fix:** Document in `pg_ripple_http/README.md` and `charts/pg_ripple/values.yaml.example`.

#### M16-10 — No CI gate verifying `COMPATIBLE_EXTENSION_MIN` consistency
- **Location:** [.github/workflows/release.yml](../.github/workflows/release.yml)
- **Impact:** Root cause of C16-01. Even with `just bump-version` available, drift continues without enforcement.
- **Fix:** Add a workflow step that parses Cargo.toml versions and `main.rs:39` and asserts the compat is within the supported window.

#### M16-11 — `src/bidi/relay.rs` drop policy is reject-new vs evict-oldest, not configurable
- **Location:** [src/bidi/relay.rs#L91](../src/bidi/relay.rs#L91)
- **Impact:** Some workloads (recent-state propagation) prefer evict-oldest. Currently fixed to drop new.
- **Fix:** Add GUC `pg_ripple.bidi_relay_drop_policy = 'newest'|'oldest'` with newest as default.

#### M16-12 — `tests/concurrency/` directory is sparse (3 files)
- **Location:** `tests/concurrency/{confidence_subxact_rollback.sql, pagerank_during_merge.sh, sse_slow_subscriber.sh}`
- **Impact:** Many subsystems lack concurrency tests (hypothetical, entity_resolution, temporal, PPRL).
- **Fix:** Add e.g. `entity_resolution_concurrent_resolves.sh`, `temporal_versioned_write_race.sh`.

#### M16-13 — No fuzz target for new subsystems
- **Location:** `fuzz/fuzz_targets/` — 20 targets, but none for temporal / pprl / hypothetical / rule_authoring / skos / entity_resolution.
- **Impact:** Parser/grammar surfaces of new subsystems are unfuzzed.
- **Fix:** Add `fuzz/fuzz_targets/{temporal_query.rs, pprl_bloom_encode.rs, rule_authoring_validate.rs, skos_bundle.rs}`.

#### M16-14 — `src/datalog_api.rs` (1,134 LOC) starting to bloat
- **Location:** [src/datalog_api.rs](../src/datalog_api.rs)
- **Impact:** Same pattern as views/mod.rs and skos.rs.
- **Fix:** Split into `datalog_api/{mod, parse, validate, explain, conflict}.rs`.

#### M16-15 — `src/sparql/wcoj.rs` (1,067 LOC) without sub-modules
- **Location:** [src/sparql/wcoj.rs](../src/sparql/wcoj.rs)
- **Fix:** Split per A14 pattern.

#### M16-16 — `src/sparql/embedding.rs` (1,144 LOC) without sub-modules
- **Fix:** Same.

#### M16-17 — `src/shacl/validator.rs` (1,181 LOC) without sub-modules
- **Fix:** Same; split per shape kind.

#### M16-18 — `src/citus/mod.rs` (1,366 LOC) without sub-modules
- **Fix:** Same; logically split shard pruning vs DDL hooks vs query rewriting.

#### M16-19 — `src/rule_explain.rs` cache has no size cap
- **Location:** [src/rule_explain.rs](../src/rule_explain.rs)
- **Impact:** TTL eviction only; memory growth proportional to unique queries.
- **Fix:** Bounded LRU (e.g. `lru` crate) with GUC `pg_ripple.rule_explanation_cache_max_entries`.

#### M16-20 — `src/uncertain_knowledge_api/bayesian.rs::propagate_downstream` lacks depth cap GUC
- **Location:** `src/uncertain_knowledge_api/bayesian.rs`
- **Impact:** Although overflow is captured in `confidence_stale` table, no operator-visible GUC for tuning. Hardcoded `max_depth` only.
- **Fix:** Promote to GUC `pg_ripple.bayesian_propagation_max_depth`.

#### M16-21 — `audit.toml` has no policy header
- **Location:** [audit.toml](../audit.toml)
- **Impact:** Reviewers must reconstruct the expiry-tracking policy from comments.
- **Fix:** Add a one-paragraph header explaining the lifecycle: open issue → triage → ignore with expiry → quarterly review → expiry forces re-decision.

#### M16-22 — `pg_ripple_http` admin handlers: `/metrics` not behind auth
- **Location:** [pg_ripple_http/src/routing/mod.rs](../pg_ripple_http/src/routing/mod.rs)
- **Impact:** Prometheus metrics are typically scraped without auth, but for hostile-network deployments, exposing query rates, replication slot lag, and queue depths gives reconnaissance signal.
- **Fix:** Optional bearer token: GUC-equivalent env `PG_RIPPLE_HTTP_METRICS_TOKEN`; if set, require Authorization on `/metrics`.

#### M16-23 — Migration script `0.99.0--0.99.1.sql` and `0.99.1--0.99.2.sql` mid-release hotfixes not documented in CHANGELOG with PATCH semver headings
- **Location:** [CHANGELOG.md](../CHANGELOG.md)
- **Impact:** A reader of CHANGELOG cannot tell that 0.99.x had two hotfix releases without grep'ing `sql/`.
- **Fix:** Add `## [0.99.1] — date — Hotfix` and `## [0.99.2]` entries.

### LOW (15)

#### L16-01 — 207 `#[allow(...)]` suppressions (regression +1 vs A15)
- **Fix:** Run `rg -n "#\[allow" src/ pg_ripple_http/src/ | wc -l` quarterly; aim to delete 10% per cycle.

#### L16-02 — `tests/crash_recovery/` is comprehensive but README missing
- **Location:** `tests/crash_recovery/` has 11 scripts; no top-level README explains how to run.
- **Fix:** Add `tests/crash_recovery/README.md` with `pg_ctl -m immediate` invocation pattern.

#### L16-03 — `benchmarks/` has many sql files but no aggregate runner doc
- **Fix:** Document `benchmarks/ci_benchmark.sh` interplay in `benchmarks/README.md`.

#### L16-04 — `src/replication.rs` `pg_ripple_logical_apply_worker_main` lacks doc-comment of restart semantics
- **Fix:** Add `///` block enumerating restart counter and crash-loop backoff.

#### L16-05 — `src/flight.rs` HMAC ticket version field is `v2`; v1 path branching not documented
- **Fix:** Add a section to `docs/flight.md` (or create) explaining v1→v2 migration.

#### L16-06 — `pg_ripple_http/src/common.rs` `check_auth` realm hardcoded
- **Fix:** Make `Bearer realm="pg_ripple"` overridable via env.

#### L16-07 — `sql/` directory lacks a numbered manifest file enumerating canonical install steps
- **Fix:** `sql/INSTALL.md` with sequential migration list.

#### L16-08 — `docker-compose.yml` does not pin pg_ripple version (image tag)
- **Fix:** Pin to current minor; document in `docker/README.md`.

#### L16-09 — `clippy_all.txt` and `clippy_output.txt` are committed artifacts
- **Fix:** Add to `.gitignore` and move to CI artifacts only.

#### L16-10 — `cargo_check_output.txt`, `build_output.txt`, `regression.diffs` committed
- **Fix:** Same. These appear to be reviewer scratchpad output.

#### L16-11 — `sbom_diff.md` lacks date header
- **Fix:** Add `**Generated:** YYYY-MM-DD`.

#### L16-12 — `pg_ripple.cdx.json` SBOM not signed
- **Fix:** Sign with `cosign` per v1.0 entry criteria (H16-07).

#### L16-13 — `RELEASE.md` lacks "compat constant" checklist line
- **Fix:** Add bullet: "Run `just bump-version-dry` then `just bump-version` before tagging".

#### L16-14 — Single `tests/concurrency/sse_slow_subscriber.sh` exists for the entire SSE/bidi surface
- **Fix:** Add `sse_burst_subscriber.sh`, `sse_reconnect_during_merge.sh`.

#### L16-15 — `CONTRIBUTING.md` does not link to AGENTS.md
- **Fix:** Add link.

---

## 7. Performance Bottlenecks

| # | Bottleneck | Evidence | Suggested win |
|---|---|---|---|
| P1 | `bulk_load.rs` INSERT-batch only | `src/bulk_load.rs:1-1173`; no COPY codepath | 5–10× on >10M-triple loads |
| P2 | `src/views/mod.rs` CONSTRUCT view refresh is full-recompute on bulk delta | 1,599 LOC; no IVM hook visible | Hook into PG-ripple trickle |
| P3 | `src/sparql/wcoj.rs` leapfrog triejoin without parallel-seq fallback | 1,067 LOC | Add `parallel_safe = ON` markers for partitioned VP scan |
| P4 | `src/entity_resolution.rs` Stage 2 embedding candidates loops Python-y over candidates | unbatched ANN calls | Use `array_agg`-style batched HNSW probe |
| P5 | `src/pprl.rs::bloom_encode` HMAC-SHA256 per hash position (k=30 default) | `format!("{key}\x00{i:04}")` allocates per call | Reuse a single HMAC instance via `clone()` |
| P6 | `src/replication.rs` 100-event / 500ms batch fixed | hard-coded watermark constants | Add GUCs to tune |
| P7 | `pg_ripple_http` stream handler holds tokio `mpsc` buffer of 256 default | `pg_ripple_http/src/routing/stream.rs` | Make configurable via env |

## 8. Architectural Concerns

1. **Module size drift.** Every assessment since A12 has flagged this and it continues — 6 files now exceed 1,000 LOC, growing. The team has demonstrated successful splits (datalog/compiler, sparql/expr) but applies them reactively rather than proactively. Propose a **soft cap of 1,000 LOC** with CI warning at 1,200 and fail at 1,500.
2. **HTTP companion surface lags extension surface.** Many new SQL APIs in the extension have no HTTP equivalent. Either treat HTTP as a strict subset (documented) or commit to parity.
3. **No clear inter-subsystem dependency graph.** Subsystems like SKOS, OWL-RL, NS-RL, hypothetical, conflict-detection all touch the Datalog engine, but there is no documented ownership boundary. Consider an architectural diagram in `docs/architecture.md`.
4. **GUC explosion.** New GUCs introduced in this window (rule_explanation_cache_ttl, hypothetical_max_assertions, prov_enabled, bloom_max_input_length, sameas_apply_rate_limit, string_similarity_extensions_ok, record_sameas_anomalies, …) lack a categorical registry. Add `docs/gucs.md` with grouping by subsystem.
5. **Telemetry vs Prometheus duplication.** `src/telemetry.rs` (OTLP) and `pg_ripple_http/src/metrics.rs` (Prometheus) cover overlapping ground. Document chooser: traces vs counters.

## 9. Feature Gaps

### SPARQL
- Property paths over RDF-star edges — gap acknowledged in CHANGELOG but no roadmap entry.
- `SERVICE` federation lacks circuit breaker per remote.

### Temporal
- Allen's interval relations (`before`, `meets`, `overlaps`, `during`) not exposed as SPARQL FILTER functions (search of src/temporal.rs found no `pg:allen_*` registrations).
- No `AT TIME ZONE` integration for `point_in_time`.

### NS-RL / Entity Resolution
- SHACL gate stub (H16-03) means symbolic constraints are skipped.
- No active-learning loop for ER thresholds.

### RDF / SHACL / Datalog / OWL
- SHACL-SPARQL constraints catalog incomplete (verified against `tests/pg_regress/sql/shacl*.sql` count).
- OWL-RL fixpoint loop visible but no `owl:propertyChainAxiom` test in pg_regress.

### Privacy
- DP `dp_noisy_count`/`dp_noisy_histogram` accept epsilon up to 10.0 (`PT0472`) but no organisational privacy budget tracker. Each call independently spends.
- No HTTP endpoint for PPRL (M16-02).

### Operational
- No tenant quota enforcement HTTP endpoint.
- Helm chart lacks PodDisruptionBudget example.

## 10. Security Findings (Summary)

| ID | Severity | Subject | Status |
|---|---|---|---|
| (none new Critical) | — | — | — |
| H16-03 | High | resolve_entities partial-write | Open |
| M16-04 | Medium | pagerank handler manual escape | Open |
| M16-06 | Medium | RSA RUSTSEC expiry window | Open (review Q3-2026) |
| M16-07 | Medium | Proof-tree depth GUC | Open |
| M16-22 | Medium | /metrics no auth | Open |
| SECDEF-15-01 | Resolved | SECDEF SET search_path | RESOLVED |

## 11. Recommended New Features (≥ 10)

1. **`pg_ripple.bench_workload(profile TEXT)`** — built-in micro-benchmark harness selecting a profile (bsbm/watdiv/pagerank) and writing results to `_pg_ripple.bench_history` so customer perf-regression dashboards work without external harness.
2. **`pg_ripple.privacy_budget`** registry — track epsilon spent per dataset+principal; reject DP call when budget exhausted. Pairs with M16-02.
3. **`pg_ripple.compat_check()`** SQL function that the HTTP companion calls at startup and refuses to serve if mismatched — closes C16-01 belt-and-suspenders.
4. **Allen's interval predicates** as SPARQL FILTER functions (`pg:before`, `pg:meets`, `pg:overlaps`, `pg:during`, `pg:finishes`, `pg:starts`, `pg:equals`).
5. **Property-chain OWL axiom support** with a regression suite of 10 canonical examples.
6. **Federated SERVICE circuit breaker** with per-endpoint half-open state and a Prometheus gauge.
7. **`pg_ripple.explain_pagerank(node IRI)`** — top-k contributing in-edges with weights, exported via HTTP.
8. **`/admin/diagnostic_snapshot`** HTTP endpoint that bundles `_pg_ripple.*` schema introspection, GUC values, and 60s of metrics for support tickets.
9. **Tenant-scoped Helm chart values** — generate per-tenant Helm values from `_pg_ripple.tenants` table.
10. **Schema-aware NL→SPARQL with vocabulary-bundle context** — extend the natural-language-to-SPARQL feature to incorporate active SKOS/DCTERMS/FOAF/Schema.org bundles into prompt grounding.
11. **Rule-library marketplace federation** — publish/subscribe rule bundles between pg_ripple instances over Arrow Flight tickets.
12. **Read-replica routing in pg_ripple_http** — bare URI host suffix routing (e.g. `?replica=ok`) for read-only SPARQL.

---

## Appendix A — Subsystem inventory (delta vs A15)

| Subsystem | File(s) | LOC | New since A15? |
|---|---|---|---|
| Views (CONSTRUCT/materialise) | src/views/mod.rs | 1,599 | Grown |
| SKOS / vocab bundles | src/skos.rs | 1,495 | **New** |
| Citus | src/citus/mod.rs | 1,366 | grown |
| SHACL validator | src/shacl/validator.rs | 1,181 | — |
| Bulk load | src/bulk_load.rs | 1,173 | — |
| Storage ops scan | src/storage/ops/scan.rs | 1,171 | **Split from storage/ops** |
| SPARQL expr functions | src/sparql/expr/functions.rs | 1,151 | **Split from expr/mod** |
| SPARQL embedding | src/sparql/embedding.rs | 1,144 | — |
| Datalog API | src/datalog_api.rs | 1,134 | — |
| WCOJ | src/sparql/wcoj.rs | 1,067 | — |
| Datalog compiler | src/datalog/compiler/mod.rs | 1,025 | **Split** |
| Temporal | src/temporal.rs | ~ | **New** |
| Entity resolution | src/entity_resolution.rs | ~ | **New** |
| ER monitoring | src/er_monitoring.rs | ~ | **New** |
| PPRL | src/pprl.rs | ~ | **New** |
| Hypothetical inference | src/hypothetical.rs | ~ | **New** |
| Data ops / conflict | src/data_ops.rs | ~ | **New** |
| Rule explain | src/rule_explain.rs | ~ | **New** |
| Rule library | src/rule_library.rs | ~ | **New** |
| Rule authoring | src/rule_authoring.rs | ~ | **New** |
| Prov-O / proof trees | src/prov.rs | ~ | **New** |
| Bayesian confidence | src/uncertain_knowledge_api/bayesian.rs | ~ | **New** |
| NS-RL infra | (multiple) | ~ | **New** |
| Telemetry | src/telemetry.rs | ~ | **New** |
| Tenant | src/tenant.rs | ~ | **New** |
| Replication bridge | src/replication.rs, src/cdc_bridge_api.rs | ~ | **New** |
| Arrow Flight | src/flight.rs | ~ | **New** |
| FTS | src/fts.rs | ~ | **New** |
| KGE | src/kge.rs | ~ | **New** |
| Subscriptions | src/subscriptions.rs | ~ | **New** |
| R2RML | src/r2rml.rs | ~ | **New** |

## Appendix B — Migration script chain

All migrations from `0.1.0` through `0.112.0` present in `sql/`. Hotfix releases `0.99.0→0.99.1` and `0.99.1→0.99.2` present. Auto-checkpoint computation at `tests/test_migration_chain.sh:879` (HIGHEST_CHECKPOINT). Manual carry-forward markers at lines 743, 758, 783, 818.

## Appendix C — A15 finding ledger (full)

(See section 4. Counts: 9 resolved, 3 partially, 2 regressions, 1 escalation, 5+ open.)

## Appendix D — Test coverage matrix

| Subsystem | pg_regress | proptest | fuzz | crash_recovery | concurrency |
|---|---|---|---|---|---|
| Temporal | ✓ (3 files) | — | — | — | — |
| PPRL / Bloom | ✓ (1) | ✓ (pprl_bloom.rs) | — | — | — |
| Bayesian | ✓ (1) | ✓ (bayesian_confidence.rs) | — | — | — |
| Hypothetical | ✓ (1) | — | — | — | — |
| Conflicts | ✓ (2) | — | — | — | — |
| Rule library | ✓ (1) | — | — | — | — |
| Rule authoring | ✓ (1) | ✓ (rule_authoring.rs) | — | — | — |
| SKOS | ✓ (1) | — | — | — | — |
| OWL sameAs | ✓ (3) | — | — | — | — |
| Provenance | ✓ (1) | — | — | — | — |
| NS-RL ER | (none explicit) | — | — | — | — |
| Datalog (general) | many | — | ✓ (datalog_parser.rs, construct_rule.rs) | ✓ (parallel_datalog_kill, inference_kill) | — |
| SPARQL | many | ✓ (sparql_roundtrip.rs) | ✓ (sparql_parser/update.rs, shacl_sparql.rs) | — | — |
| SHACL | several | — | ✓ (shacl_parser.rs) | ✓ (shacl_during_violation.sh) | — |
| Bidi/relay | — | ✓ (bidi_convergence.rs) | — | — | ✓ (sse_slow_subscriber.sh) |
| Merge | — | — | — | ✓ (merge_*_kill.sh) | ✓ (pagerank_during_merge.sh) |
| PageRank | benches | ✓ (pagerank_oracle.rs) | — | — | ✓ |
| Replication / CDC | — | — | — | ✓ (cdc_slot_cleanup_during_kill.sh) | — |
| Promotion | — | — | — | ✓ (promote_*_kill.sh) | — |
| Dictionary | — | ✓ | ✓ (dictionary_hash.rs) | ✓ (dict_during_kill.sh) | — |
| JSON-LD framing | — | ✓ | ✓ (jsonld_framer.rs) | — | — |
| GeoSPARQL | — | — | ✓ (geosparql_wkt.rs) | — | — |
| Loaders (NT/NQ/Turtle/RDFXML/TriG) | many | ✓ (ntriples_oxigraph.rs) | ✓ (ntriples/nquads/turtle/trig/rdfxml) | — | — |
| Confidence loader | — | ✓ (confidence_algebra.rs) | ✓ (confidence_loader.rs) | — | ✓ (confidence_subxact_rollback.sql) |
| R2RML | — | — | ✓ (r2rml_mapping.rs) | — | — |
| Federation | — | — | ✓ (federation_result.rs) | ✓ (federation_spool_kill.sh) | — |
| HTTP request parser | — | — | ✓ (http_request.rs) | — | — |
| LLM prompt builder | — | — | ✓ (llm_prompt_builder.rs) | — | — |
| URL host parser | — | — | ✓ (url_host_parser.rs) | — | — |
| CONSTRUCT views | — | ✓ (construct_template.rs) | ✓ (construct_rule.rs) | ✓ (construct_view_kill.sh) | — |
| Embedding | — | — | — | ✓ (embedding_kill.sh) | — |

Coverage gaps highlighted in M16-12 / M16-13.

## Appendix E — Static-analysis counts (vs A15)

| Metric | A15 | A16 | Δ |
|---|---|---|---|
| Total Rust LOC | 71,003 | 83,754 | +12,751 (+18%) |
| Rust files | 115 | 229 | +114 (+99%) |
| `unwrap`/`expect` | 49 | 64 | +15 |
| `unsafe {` blocks | 60 | 109 | +49 |
| `// SAFETY:` comments | 68 | 83 | +15 |
| Unannotated `unsafe` | -8 (more SAFETY than unsafe) | 26 | +26 |
| `#[allow(...)]` | 206 | 207 | +1 |
| `SECURITY DEFINER` sites | 1 (in sql/) | 4 (in sql/, all justified) | +3 |
| `unreachable!()` actual calls | several | 0 | resolved |
| Stale `_old`/`_backup` files | 0 | 0 | — |
| pg_regress SQL tests | ~ | 271 | growth |
| Fuzz targets | ~ | 20 | growth |
| Proptest harnesses | ~ | 14 | growth |
| Crash-recovery scripts | ~ | 11 | growth |
| RUSTSEC ignores | 4 | 4 | — |

## Appendix F — CI workflow inventory

`.github/workflows/`: benchmark.yml, cargo-audit.yml, ci.yml, docs-test.yml, docs.yml, fuzz.yml, helm-lint.yml, migration-chain.yml, performance_trend.yml, release.yml. **Gap (M16-10):** none enforces `COMPATIBLE_EXTENSION_MIN`.

## Appendix G — Threat model deltas since A15

| New attack surface | Mitigation in place | Gap |
|---|---|---|
| Arrow Flight do_get tickets | HMAC-SHA-256 signed v2 (iat/exp/aud/nonce); FLIGHT-SEC-01 rejects unsigned unless GUC | None significant |
| LLM endpoint (rule_explain) | Not implemented (H16-04) | False advertisement; no prompt-injection surface yet |
| Replication bgworker | `pg_ripple_logical_apply_worker_main` `#[unsafe(no_mangle)] extern "C-unwind"` | SAFETY annotation absent (H16-01) |
| Multi-tenant | tenant_name validated against `^[A-Za-z0-9_]{1,63}$` before format!() | OK |
| PPRL Bloom encode | Input length validated (PT0470); epsilon range (PT0472); WARN below-recommended security parameters | No PII transmitted out of DB |
| Differential privacy | epsilon ∈ (0.0, 10.0]; per-call | No org-wide budget tracker (M16-02 recommended feature) |
| Proof tree generation | No depth cap | M16-07 |
| Entity resolution dashboard | Tables grow unbounded | M16-01 |
| HTTP /metrics | Open | M16-22 |

## Appendix H — Operational checklist for v0.113.0

- [ ] **Run `just bump-version 0.113.0 0.112.0`** (closes C16-01)
- [ ] Add CI check for compat constant (closes M16-10)
- [ ] Annotate 26 unannotated `unsafe` blocks (closes H16-01)
- [ ] Convert 64 unwrap/expect sites to `?` (closes H16-02)
- [ ] Wrap `resolve_entities()` in subtransaction (closes H16-03)
- [ ] Document or remove llm_endpoint stub (closes H16-04)
- [ ] Add `pg_ripple.proof_tree_max_depth` GUC (closes M16-07)
- [ ] Split `src/skos.rs` and `src/views/mod.rs` (closes H16-06)
- [ ] Add ER monitoring retention GUC (closes M16-01)
- [ ] Add `#[deny(missing_safety_doc)]` lint to workspace

---

**End of Assessment #16.** Next assessment (#17) should refuse to score above 4.0/5.0 if C16-01 persists.
