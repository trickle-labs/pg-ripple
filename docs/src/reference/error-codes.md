# Error Code Registry

> **A13-03 (v0.86.0)**: this page is the authoritative registry for all `PT` error codes used in pg_ripple. Every production error path **must** reference a code from this list. CI enforces `PT` codes on production error paths.

pg_ripple uses structured error codes in the range **PT001–PT799** (extension) and **PT400–PT503** (HTTP companion). Error messages follow PostgreSQL conventions: lowercase first word, no trailing period.

See also: [Error Message Catalog](error-catalog.md) for the full list of error messages by subsystem.

---

## Code Ranges

| Range | Subsystem |
|---|---|
| PT001–PT099 | Dictionary encoding |
| PT100–PT199 | VP storage |
| PT200–PT299 | SPARQL query engine |
| PT300–PT399 | Datalog inference |
| PT400–PT499 | Input validation / HTTP |
| PT500–PT599 | Internal execution |
| PT600–PT699 | SHACL validation |
| PT700–PT799 | External services (LLM, federation) |

---

## Uncertain Knowledge (v0.87.0) — PT0301–PT0307

| Code | Feature | Condition |
|---|---|---|
| PT0301 | Uncertain Knowledge | Fuzzy SPARQL input exceeds `pg_ripple.fuzzy_max_input_length` characters |
| PT0302 | Uncertain Knowledge | `pg_trgm` extension is not installed |
| PT0303 | Uncertain Knowledge | Invalid confidence value (NaN, Inf, or outside `[0.0, 1.0]`) |
| PT0304 | Uncertain Knowledge | `pg:confidence()` called with all three arguments unbound |
| PT0305 | Uncertain Knowledge | `pg:confidence()` used inside a `SERVICE` clause (not supported) |
| PT0306 | Uncertain Knowledge | SHACL score-log table `_pg_ripple.shacl_score_log` does not exist |
| PT0307 | Uncertain Knowledge | Confidence bulk-loader: file path outside allowed directory |

---

## PageRank (v0.88.0) — PT0401–PT0423

| Code | Feature | Condition |
|---|---|---|
| PT0401 | PageRank | Invalid damping factor — must be in `(0, 1)` exclusive |
| PT0402 | PageRank | `max_iterations` must be a positive integer |
| PT0403 | PageRank | `topic` label exceeds `pg_ripple.pagerank_max_topic_length` characters |
| PT0404 | PageRank | `seed_nodes` list exceeds `pg_ripple.pagerank_max_seeds` limit |
| PT0405 | PageRank | `edge_predicates` list is empty (at least one required) |
| PT0406 | PageRank | `convergence_threshold` must be positive and finite |
| PT0407 | PageRank | No pagerank scores found — run `pagerank_run()` first |
| PT0408 | PageRank | `pagerank_scores` table does not exist — run `pagerank_run()` first |
| PT0409 | PageRank | `pagerank_dirty_edges` table does not exist |
| PT0410 | PageRank | Export format unsupported — valid values: `turtle`, `jsonld`, `csv`, `ntriples` |
| PT0411 | PageRank | Centrality metric unsupported — valid values: `betweenness`, `closeness`, `eigenvector`, `katz` |
| PT0412 | PageRank | `katz_beta` (attenuation factor) must be positive |
| PT0413 | PageRank | `explain_pagerank` `top_k` must be a positive integer |
| PT0414 | PageRank | PageRank run aborted — convergence not achieved within `max_iterations` |
| PT0415 | PageRank | Concurrent PageRank run already in progress for this topic |
| PT0416 | PageRank | IRI escaping error in export (malformed IRI in `pagerank_scores`) |
| PT0417 | PageRank | `pg:pagerank()` in SPARQL query triggered on-demand run; timed out |
| PT0418 | PageRank | Betweenness centrality requires at least 3 nodes |
| PT0419 | PageRank | `pagerank_partition` value out of range `[1, 1024]` |
| PT0420 | PageRank | `k_hop_depth` for incremental refresh is out of range `[1, 20]` |
| PT0421 | PageRank | Confidence-weighted PageRank requested but no confidence scores exist |
| PT0422 | PageRank | Temporal decay `half_life_days` must be positive |
| PT0423 | PageRank | `federation_minimum_confidence` outside `[0.0, 1.0]` |

---

## HTTP Companion PT Codes

| Code | HTTP Status | Meaning | Source |
|---|---|---|---|
| PT400 | 400 | Missing or malformed query parameter | `routing/sparql_handlers.rs` |
| PT400_SPARQL_PARSE | 400 | SPARQL syntax error — parse failed | `routing/sparql_handlers.rs`, `spi_bridge.rs` |
| PT401 | 401 | Unauthorized — missing or invalid Bearer token | `common.rs` |
| PT403 | 403 | Forbidden — path outside allowed directory | `bulk_load.rs` |
| PT404 | 413 | Request body exceeds maximum allowed size | `routing/sparql_handlers.rs` |
| PT413 | 413 | Arrow Flight export result is too large | `arrow_encode.rs` |
| PT503 | 503 | Database connection unavailable | `common.rs`, `stream.rs` |

---

## Extension PT Codes (selected)

| Code | Message | Subsystem |
|---|---|---|
| PT001 | dictionary encode failed: hash collision detected | Dictionary |
| PT002 | dictionary decode failed: id not found | Dictionary |
| PT003 | invalid term kind: expected 0/1/2 | Dictionary |
| PT008 | malformed IRI: `<detail>` | Dictionary |
| PT400 | SPARQL parse error: `<detail>` | SPARQL |
| PT403 | file path outside allowed directory | Bulk load |
| PT501 | deprecated GUC: use `<replacement>` | Storage GUC |
| PT512 | strict_dictionary: unknown dictionary id | Dictionary |
| PT600 | SHACL constraint violation: `<detail>` | SHACL |
| PT700 | LLM endpoint unreachable or returned HTTP error | LLM |

---

## CI Enforcement

The CI job `check-pt-codes` (added in v0.86.0) scans all `pgrx::error!` and `tracing::error!` call sites in production code to verify that each one references a PT code in either:

- The error message body (e.g., `"... (PT400)"`), or  
- The error code argument (e.g., `json_error("PT400", ..., StatusCode::BAD_REQUEST)`).

Internal-only errors that use `"internal: <description> — please report"` format are exempted.

To add a new error code, update this file **first**, then reference the code in the implementation.
