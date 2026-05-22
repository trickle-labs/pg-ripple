# Incremental R2RML Materialization Plan

**Status:** Proposed  
**Date:** 2026-05-22  
**Target:** Post-v0.127.0, candidate v0.128.0+  
**Related:** [R2RML virtual graph layer](r2rml-virtual.md), [pg-tide relay integration](pg_trickle_relay_integration.md), [pg_trickle ecosystem analysis](ecosystem/pg_trickle.md)

---

## Executive summary

pg_ripple's current R2RML support is **not incremental**. `pg_ripple.r2rml_load(mapping_iri)` performs a full logical-source scan, renders every mapped row to N-Triples, and calls `bulk_load::load_ntriples()`. Re-running the function is idempotent for unchanged triples because the VP storage path uses set-style inserts, but it is not delta-based and it does not retract triples that are no longer produced after source-row updates or deletes.

Incremental R2RML is feasible, and pg_trickle makes the most valuable version possible: R2RML over `rr:sqlQuery`, joins, projections, and aggregates can be maintained by having pg_trickle produce an incrementally maintained relational row stream, then applying the R2RML row-to-triples compiler only to changed rows. For simple `rr:tableName` mappings in the same database, direct PostgreSQL triggers are enough and should not require pg_trickle.

Important terminology update: relay, inbox, and outbox transport moved from pg_trickle to pg_tide. pg_trickle should be used here for IVM over relational logical sources; pg_tide should be used only for external event transport.

---

## Current implementation assessment

### What exists today

- `src/r2rml.rs` implements `r2rml_load(mapping_iri)`.
- `src/maintenance_api.rs` exposes `pg_ripple.r2rml_load(mapping_iri)`.
- The loader discovers `rr:TriplesMap` instances already loaded into the RDF store, reads mapping terms through `lookup_object()` / `lookup_objects()`, builds a source query, and executes it through SPI.
- For `rr:tableName`, the source query is `SELECT * FROM <quoted table>`.
- For `rr:sqlQuery`, the mapping SQL is executed as-is.
- Each source row is expanded into an in-memory N-Triples buffer.
- The generated N-Triples are passed to `crate::bulk_load::load_ntriples(&ntriples, false)`.
- `load_ntriples()` inserts through `storage::batch_insert_encoded()`, which records graph writes in the mutation journal and flushes once at the end of the load.

### What this means operationally

- Initial materialization works for supported R2RML patterns.
- Re-running `r2rml_load()` scans the full source again.
- Inserts in the source table are picked up only after a full rerun.
- Updates in the source table can leave stale triples because the old row's triples are not retracted.
- Deletes in the source table leave stale triples for the same reason.
- There is no mapping registry, source table trigger, source-row watermark, refcount table, or pg_trickle stream-table bridge for R2RML.
- The docs should distinguish “idempotent rerun” from “incremental maintenance”.

---

## Goals

1. Add true incremental R2RML materialization for table-backed mappings.
2. Support insert, update, and delete source-row changes.
3. Avoid stale triples after source updates and deletes.
4. Avoid deleting a triple that is still emitted by another source row in the same mapping.
5. Reuse existing dictionary encoding, VP storage, mutation journal, HTAP merge, and CONSTRUCT writeback paths.
6. Use pg_trickle, when installed, to support incremental `rr:sqlQuery` and joined/derived relational sources.
7. Keep `r2rml_load(mapping_iri)` backward-compatible as the full-snapshot path.
8. Provide a reconciliation command that can repair drift after disabled triggers, missed events, or source schema changes.

---

## Non-goals

1. Do not implement the virtual R2RML graph layer in this work. Query-time virtual mapping remains tracked separately in [r2rml-virtual.md](r2rml-virtual.md).
2. Do not make arbitrary RML non-SQL sources incremental.
3. Do not require pg_trickle for simple base-table mappings.
4. Do not rely on pg_tide for same-database source table changes.
5. Do not make R2RML deletions safe in user-mixed graphs without an explicit ownership model.

---

## Design overview

The implementation should introduce a reusable R2RML compiler and a delta applicator.

```
R2RML mapping graph
    -> compiled mapping IR
    -> row image(s)
    -> encoded triple set(s)
    -> refcount delta
    -> VP insert/delete batches
    -> mutation_journal::flush()
```

There are three change sources:

| Source kind | Dependency | First milestone behavior |
|---|---|---|
| `rr:tableName` base table | PostgreSQL triggers | Install source-table triggers and apply OLD/NEW row images directly. |
| `rr:sqlQuery` or joined logical source | pg_trickle | Create an incrementally maintained stream table for the SQL result, then trigger on that stream table. |
| External row events | pg_tide or caller API | Accept explicit row-change JSON through a SQL API, with event deduplication and watermarks. |

The core delta logic should be independent of how the row change arrived.

---

## Core data model

### Catalog tables

Add migration SQL for the next release. Candidate tables:

```sql
CREATE TABLE _pg_ripple.r2rml_mappings (
    id               BIGSERIAL PRIMARY KEY,
    name             TEXT NOT NULL UNIQUE,
    mapping_iri      TEXT NOT NULL,
    mapping_hash     TEXT NOT NULL,
    mode             TEXT NOT NULL CHECK (mode IN ('manual', 'trigger', 'pg_trickle', 'external')),
    target_graph     TEXT,
    enabled          BOOLEAN NOT NULL DEFAULT false,
    status           TEXT NOT NULL DEFAULT 'registered',
    last_error       TEXT,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE _pg_ripple.r2rml_triples_maps (
    id               BIGSERIAL PRIMARY KEY,
    mapping_id       BIGINT NOT NULL REFERENCES _pg_ripple.r2rml_mappings(id) ON DELETE CASCADE,
    triples_map_iri  TEXT NOT NULL,
    source_kind      TEXT NOT NULL CHECK (source_kind IN ('table', 'sql')),
    source_schema    TEXT,
    source_table     TEXT,
    source_relid     OID,
    source_sql       TEXT,
    stream_table     TEXT,
    key_columns      TEXT[] NOT NULL DEFAULT '{}',
    required_columns TEXT[] NOT NULL DEFAULT '{}',
    supports_incremental BOOLEAN NOT NULL DEFAULT false,
    UNIQUE (mapping_id, triples_map_iri)
);

CREATE TABLE _pg_ripple.r2rml_triple_refcounts (
    mapping_id       BIGINT NOT NULL REFERENCES _pg_ripple.r2rml_mappings(id) ON DELETE CASCADE,
    triples_map_id   BIGINT NOT NULL REFERENCES _pg_ripple.r2rml_triples_maps(id) ON DELETE CASCADE,
    s                BIGINT NOT NULL,
    p                BIGINT NOT NULL,
    o                BIGINT NOT NULL,
    g                BIGINT NOT NULL DEFAULT 0,
    refcount         BIGINT NOT NULL CHECK (refcount > 0),
    PRIMARY KEY (mapping_id, triples_map_id, s, p, o, g)
);

CREATE TABLE _pg_ripple.r2rml_event_dedup (
    mapping_id       BIGINT NOT NULL REFERENCES _pg_ripple.r2rml_mappings(id) ON DELETE CASCADE,
    event_id         TEXT NOT NULL,
    applied_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (mapping_id, event_id)
);

CREATE TABLE _pg_ripple.r2rml_watermarks (
    mapping_id       BIGINT NOT NULL REFERENCES _pg_ripple.r2rml_mappings(id) ON DELETE CASCADE,
    source_name      TEXT NOT NULL,
    last_lsn         PG_LSN,
    last_event_id    TEXT,
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (mapping_id, source_name)
);
```

### Ownership model

Incremental deletes are dangerous unless pg_ripple knows it owns the triple being retracted. The first release should enforce one of these policies:

1. **Recommended default:** require incremental mappings to write to a mapping-owned named graph, for example `urn:pg-ripple:r2rml:{name}`, unless the R2RML mapping contains explicit graph maps.
2. **Advanced option:** allow shared/default-graph output only when the user passes `allow_shared_graph => true`, and document that pg_ripple may retract triples produced by the incremental mapping.

The refcount table protects triples that are emitted by multiple rows in the same mapping. It does not by itself distinguish a manually inserted triple from a mapped triple in the same graph.

---

## Compiler and row expansion

Refactor `src/r2rml.rs` around a compiled IR instead of the current monolithic full-load function.

Candidate Rust types:

```rust
struct CompiledR2rmlMapping {
    mapping_iri: String,
    triples_maps: Vec<CompiledTriplesMap>,
}

struct CompiledTriplesMap {
    triples_map_iri: String,
    logical_source: LogicalSource,
    subject_map: CompiledSubjectMap,
    predicate_object_maps: Vec<CompiledPredicateObjectMap>,
    graph_map: Option<CompiledTermMap>,
    required_columns: Vec<String>,
    key_columns: Vec<String>,
}

enum LogicalSource {
    Table { schema: Option<String>, table: String, relid: Option<u32> },
    Sql { sql: String },
}

struct EncodedTriple {
    s: i64,
    p: i64,
    o: i64,
    g: i64,
}
```

Add helpers:

- `compile_mapping(mapping_iri: &str) -> Result<CompiledR2rmlMapping, R2rmlError>`
- `compile_triples_map(tm_id: i64) -> Result<CompiledTriplesMap, R2rmlError>`
- `triples_for_row(map: &CompiledTriplesMap, row: &RowImage) -> Vec<EncodedTriple>`
- `triples_for_row_ntriples(...) -> String` only for compatibility/debugging

The full loader should eventually become:

1. compile mapping,
2. scan every source row,
3. call `triples_for_row()`,
4. batch insert encoded triples.

The incremental path should use the same `triples_for_row()` function for OLD and NEW row images.

### R2RML semantic fixes to include

Incremental correctness depends on deterministic row expansion. Fix these while extracting the compiler:

- A NULL-valued column term map should generate no RDF term, not an empty string literal or empty IRI component.
- Update/delete logic should compare triple sets, not raw generated text.
- Template expansion should be shared by full and incremental paths.
- Literal datatype and language support should be represented in the IR even if existing support is partial.
- Graph maps should be represented explicitly; when absent, use the mapping-owned target graph for incremental mappings.

---

## Delta application algorithm

Expose one internal function that every change source calls:

```rust
fn apply_r2rml_row_delta(
    mapping_id: i64,
    triples_map_id: i64,
    old_row: Option<RowImage>,
    new_row: Option<RowImage>,
    event_id: Option<&str>,
) -> Result<R2rmlDeltaSummary, R2rmlError>
```

Algorithm:

1. If `event_id` is present, insert into `_pg_ripple.r2rml_event_dedup`; if already present, return without work.
2. Compile or load the cached `CompiledTriplesMap`.
3. Generate `old_triples` from `old_row`, if present.
4. Generate `new_triples` from `new_row`, if present.
5. Compute row-local set difference:
   - `to_decrement = old_triples - new_triples`
   - `to_increment = new_triples - old_triples`
6. For each triple in `to_decrement`, decrement `_pg_ripple.r2rml_triple_refcounts`.
7. Delete the VP triple only when the refcount reaches zero.
8. For each triple in `to_increment`, increment `_pg_ripple.r2rml_triple_refcounts`.
9. Insert the VP triple only when the refcount transitions from absent to one.
10. Group VP inserts by predicate and call `storage::batch_insert_encoded()`.
11. Group VP deletes by predicate and call a new batch deletion helper, or use `delete_triple_by_ids()` for the first release.
12. Flush the mutation journal once after the delta batch.

Update semantics naturally fall out of this: an UPDATE receives both OLD and NEW row images, retracts only triples that disappeared, and inserts only triples that appeared.

### Needed storage helper

Add `storage::batch_delete_encoded(p_id, rows)` for performance and symmetry with `batch_insert_encoded()`.

First implementation can call `delete_triple_by_ids()` per triple to reduce risk, but the public design should aim for a batched path that handles:

- dedicated VP delta deletes,
- tombstone inserts for main-resident triples,
- `vp_rare` deletes,
- predicate `triple_count` updates,
- mutation journal delete records.

---

## Public SQL API

Add a small API surface that separates registration, initial snapshot, incremental enablement, and repair.

```sql
SELECT pg_ripple.register_r2rml_mapping(
    name         => 'customers_r2rml',
    mapping_iri  => 'https://example.org/CustomerMap',
    mode         => 'trigger',
    target_graph => 'urn:pg-ripple:r2rml:customers'
);

SELECT pg_ripple.r2rml_refresh('customers_r2rml', mode => 'full');
SELECT pg_ripple.enable_r2rml_incremental('customers_r2rml');
SELECT pg_ripple.disable_r2rml_incremental('customers_r2rml');
SELECT * FROM pg_ripple.r2rml_mappings();
SELECT pg_ripple.r2rml_reconcile('customers_r2rml');
```

For external transport and tests:

```sql
SELECT pg_ripple.r2rml_apply_change(
    mapping_name => 'customers_r2rml',
    source_name  => 'public.customer',
    op           => 'update',
    old_row      => '{"id":1,"email":"old@example.org"}'::jsonb,
    new_row      => '{"id":1,"email":"new@example.org"}'::jsonb,
    event_id     => 'source-lsn-or-relay-id'
);
```

Backward compatibility:

- Keep `pg_ripple.r2rml_load(mapping_iri)`.
- Internally, make it compile the mapping through the new IR and perform a full one-shot materialization.
- Do not automatically enable incremental maintenance from `r2rml_load()`.

---

## Direct trigger mode for `rr:tableName`

For base tables in the same PostgreSQL database, direct triggers are the simplest and lowest-latency option.

### Trigger installation

For each table-backed `CompiledTriplesMap`:

1. Validate the source table exists and resolve its OID.
2. Validate all required columns exist.
3. Choose key columns:
   - explicit API argument, if supplied,
   - primary key columns, if present,
   - otherwise a hash of all required columns for insert/update, with a warning that external deletes need full OLD row images.
4. Install one AFTER trigger on the source table.
5. The trigger calls an internal SQL function with `to_jsonb(OLD)` and/or `to_jsonb(NEW)`.

Shape:

```sql
CREATE TRIGGER pg_ripple_r2rml_<mapping_id>_<triples_map_id>
AFTER INSERT OR UPDATE OR DELETE ON public.customer
FOR EACH ROW EXECUTE FUNCTION _pg_ripple.r2rml_source_trigger(<mapping_id>, <triples_map_id>);
```

The trigger function should:

- pass `NULL, to_jsonb(NEW)` for INSERT,
- pass `to_jsonb(OLD), to_jsonb(NEW)` for UPDATE,
- pass `to_jsonb(OLD), NULL` for DELETE.

### Bootstrap without missing writes

Enabling incremental mode must avoid the classic snapshot/trigger race.

Recommended sequence in one transaction:

1. Take a mapping-level advisory lock.
2. Lock every source table in `SHARE ROW EXCLUSIVE` mode.
3. Install or replace triggers.
4. Clear prior refcounts for this mapping.
5. Run `r2rml_refresh(name, mode => 'full')` to build the initial snapshot.
6. Mark mapping `enabled = true`.
7. Commit.

Writers block while the initial snapshot runs, but no changes are missed. A later optimization can queue changes during snapshot instead of taking a stronger lock.

---

## pg_trickle mode for `rr:sqlQuery` and joins

This is where pg_trickle materially improves R2RML.

### Why pg_trickle helps

`rr:sqlQuery` can represent joins, filters, projections, aggregates, and views. It is not practical for pg_ripple to infer the exact base tables and delta rules for arbitrary SQL. pg_trickle already provides that machinery: it can maintain a stream table for the logical SQL result and update only rows whose result changed.

### Design

For each SQL-backed `CompiledTriplesMap`:

1. Generate a stable relational query for the logical source.
2. Ensure the projection contains every column used by subject, predicate, object, and graph maps.
3. Ensure there is a stable key:
   - use user-supplied key columns when available,
   - otherwise synthesize `_pg_ripple_row_hash` from all projected columns,
   - reject mappings where neither option is deterministic.
4. Create a pg_trickle stream table for the logical source query.
5. Attach the same R2RML source trigger to the stream table.
6. Let pg_trickle maintain the row delta; let pg_ripple maintain the triple delta.

Conceptually:

```sql
SELECT pgtrickle.create_stream_table(
    name     => '_pg_ripple.r2rml_src_customers_active',
    query    => $$
        SELECT id, full_name, email, country
        FROM public.customer
        WHERE deleted_at IS NULL
    $$,
    schedule => 'IMMEDIATE'
);
```

Then:

```sql
CREATE TRIGGER pg_ripple_r2rml_src_customers_active
AFTER INSERT OR UPDATE OR DELETE ON _pg_ripple.r2rml_src_customers_active
FOR EACH ROW EXECUTE FUNCTION _pg_ripple.r2rml_source_trigger(<mapping_id>, <triples_map_id>);
```

### Modes

| Mode | Freshness | Use case |
|---|---|---|
| `IMMEDIATE` | Same transaction as source DML, if pg_trickle supports it for the query | Operational sync and validation. |
| short schedule, e.g. `1s` | Near-real-time | Dashboards, feeds, bulk update tolerance. |
| longer schedule | Batch incremental | Heavy aggregate mappings. |

### pg_trickle absence

If pg_trickle is not installed:

- `rr:tableName` mappings can still use direct trigger mode.
- `rr:sqlQuery` incremental registration should fail with a clear hint to install pg_trickle.
- `r2rml_load(mapping_iri)` full-snapshot mode should continue to work.

---

## External event mode with pg_tide

pg_tide is the relay/outbox/inbox transport. It is useful when source row changes arrive from another system or database.

The SQL entry point should be `pg_ripple.r2rml_apply_change(...)`, not a pg_tide-specific API. pg_tide triggers or inbox processors can call it.

Requirements:

- Each event should include an `event_id` or LSN for deduplication.
- DELETE and UPDATE events must include OLD values for every required mapping column.
- If old row images are not available, require a row-state cache or force `REPLICA IDENTITY FULL` on PostgreSQL sources.
- Store progress in `_pg_ripple.r2rml_watermarks`.
- Failed events should be visible through status rows and logs; a later release can add a dead-letter table.

This mode is complementary to pg_trickle mode. pg_tide moves change events into the database; pg_ripple still performs the row-to-triple delta calculation.

---

## Mapping support matrix

### Milestone 1: base-table incremental

Support:

- `rr:logicalTable [ rr:tableName ... ]`
- `rr:subjectMap` with `rr:template`, `rr:column`, `rr:constant`
- `rr:class`
- `rr:predicateObjectMap`
- `rr:predicate` / constant predicate maps
- object maps with `rr:column`, `rr:template`, `rr:constant`
- `rr:termType` for IRI and literal
- mapping-owned default graph or simple graph map

Reject or full-refresh only:

- `rr:sqlQuery`, unless pg_trickle mode is requested
- referencing object maps with parent triples maps
- volatile SQL expressions
- RML non-SQL sources

### Milestone 2: pg_trickle SQL-source incremental

Support:

- `rr:sqlQuery` that pg_trickle accepts as a stream-table query
- SQL views represented as `rr:sqlQuery`
- joins and projections
- aggregates when the stream table exposes stable OLD/NEW row images
- filtered sources, including soft-delete filters

### Milestone 3: richer R2RML

Support:

- referencing object maps through a pg_trickle-maintained logical source join, or through explicit dependency tracking between child and parent triples maps
- dynamic graph maps
- datatype and language maps with complete W3C semantics
- schema-change recompile automation

---

## Tests

Add focused regression coverage before broadening support.

### Rust/unit tests

- Compile a simple R2RML mapping into the new IR.
- Expand one row into encoded triples.
- NULL column produces no term.
- UPDATE set difference keeps unchanged triples untouched.
- Template expansion is shared by full and incremental paths.

### pg_regress tests

Add a new file, for example `tests/pg_regress/sql/r2rml_incremental.sql`, with expected output.

Cases:

1. Register a table-backed customer mapping.
2. Initial full refresh produces expected triples.
3. INSERT source row adds only that row's triples.
4. UPDATE source row retracts old value triples and inserts new value triples.
5. DELETE source row retracts its triples.
6. Two source rows emit the same triple; deleting one row leaves the triple, deleting both removes it.
7. Incremental mapping writes to a mapping-owned graph.
8. `r2rml_reconcile()` repairs drift after triggers are disabled and re-enabled.
9. CONSTRUCT writeback fires after incremental R2RML inserts/deletes via the mutation journal.

### pg_trickle-gated tests

Add optional tests that skip when `pg_ripple.pg_trickle_available()` is false.

Cases:

1. `rr:sqlQuery` with a filter over one table.
2. `rr:sqlQuery` with a join between customer and purchase.
3. Aggregate source row update, if pg_trickle emits usable OLD/NEW stream-table rows.
4. Scheduled mode eventually applies deltas.

### Performance tests

Add a benchmark under `benchmarks/`:

- 1M source rows, 5 triples per row.
- Compare full rerun vs direct-trigger incremental for 1%, 0.1%, and single-row changes.
- Compare pg_trickle SQL-source mode against full rerun for joined mappings.
- Track p50/p95/p99 update latency and VP rows touched.

---

## Documentation updates

Update [docs/src/features/r2rml.md](../docs/src/features/r2rml.md):

- Replace “Re-running requires a full export” / “Re-runs incrementally” wording with precise current behavior until this feature ships.
- Add a section named “Incremental R2RML” once implemented.
- Explain that full `r2rml_load()` is snapshot materialization.
- Explain that incremental mode needs mapping registration and an owned target graph.
- Explain pg_trickle vs pg_tide responsibilities.

Update [blog/r2rml-relational-to-graph.md](../blog/r2rml-relational-to-graph.md) or add a note if the blog remains intentionally aspirational.

Add SQL reference entries for the new functions.

---

## Migration and release checklist

For the release that implements this:

1. Add `sql/pg_ripple--0.127.0--0.128.0.sql` or the appropriate next-version migration.
2. Create the R2RML catalog tables.
3. Add internal trigger functions in the extension SQL output.
4. Add catalog indexes:
   - `r2rml_triples_maps(mapping_id)`
   - `r2rml_triples_maps(source_relid)`
   - `r2rml_triple_refcounts(mapping_id, triples_map_id)`
   - `r2rml_event_dedup(applied_at)` for cleanup.
5. Update `pg_ripple.control` for the release.
6. Update `CHANGELOG.md`.
7. Add `feature_status()` rows for R2RML incremental mode and pg_trickle-backed SQL-source mode.

---

## Failure modes and mitigations

| Risk | Mitigation |
|---|---|
| Missed writes during initial snapshot | Lock source tables while installing triggers and running the first refresh. |
| Deleting triples owned by another producer | Use mapping-owned graphs by default; require explicit opt-in for shared/default graphs. |
| Duplicate source rows emit the same triple | Maintain `_pg_ripple.r2rml_triple_refcounts`; delete from VP only at zero. |
| Source schema changes break mappings | Store source relid and required columns; validate on trigger call; add `r2rml_reconcile()` and status reporting. |
| pg_trickle unavailable | Keep base-table trigger mode working; reject SQL-source incremental mode with a clear install hint. |
| pg_tide unavailable | External relay mode unavailable; direct and pg_trickle modes continue to work. |
| OLD row image unavailable from external CDC | Require full old row image, row-state cache, or `REPLICA IDENTITY FULL`. |
| Large source update creates huge delta | Batch by predicate, reuse `batch_insert_encoded()`, add `batch_delete_encoded()`, flush mutation journal once. |
| Mapping graph itself changes | Mark registered mapping `status = 'needs_recompile'`; require `register_r2rml_mapping(..., replace => true)` or add `r2rml_recompile()`. |

---

## Suggested implementation sequence

### R2RML-INC-01 — Correct docs and status wording

- Update R2RML docs to say current support is full-snapshot materialization.
- Mention that idempotent reruns do not equal incremental updates.
- Add this plan to roadmap references.

### R2RML-INC-02 — Extract compiler IR

- Refactor `src/r2rml.rs` to compile mapping graph data into Rust structs.
- Add row-to-encoded-triples expansion.
- Make `r2rml_load()` use the compiler while preserving behavior.

### R2RML-INC-03 — Add catalogs and registration API

- Add `_pg_ripple.r2rml_mappings` and related tables.
- Expose `register_r2rml_mapping()`, `r2rml_mappings()`, and `r2rml_refresh()`.
- Add source-column validation.

### R2RML-INC-04 — Implement delta applicator and refcounts

- Implement `apply_r2rml_row_delta()`.
- Add refcount transitions.
- Use `batch_insert_encoded()` for inserts.
- Add initial delete support through `delete_triple_by_ids()`.
- Flush the mutation journal once per delta batch.

### R2RML-INC-05 — Direct trigger mode

- Generate source-table triggers for `rr:tableName` mappings.
- Implement enable/disable functions.
- Add race-free bootstrap with source table locks.

### R2RML-INC-06 — pg_trickle SQL-source mode

- Detect pg_trickle with `pg_ripple.pg_trickle_available()` / `crate::has_pg_trickle()`.
- Create pg_trickle stream tables for supported `rr:sqlQuery` sources.
- Attach R2RML delta triggers to stream tables.
- Add pg_trickle-gated regression tests.

### R2RML-INC-07 — External event API

- Add `r2rml_apply_change()`.
- Add event deduplication and watermarks.
- Document pg_tide inbox trigger examples.

### R2RML-INC-08 — Reconciliation and observability

- Add `r2rml_reconcile()` full diff.
- Add status rows and counters.
- Add cleanup for old event dedup rows.

### R2RML-INC-09 — Optimize deletes

- Add `storage::batch_delete_encoded()`.
- Add benchmarks for large update/delete batches.

---

## Open questions

1. Should incremental mappings require an explicit named graph, or is an automatically generated graph acceptable by default?
2. Should `r2rml_load(mapping_iri)` remain pure snapshot mode forever, or should it warn when an incremental registration exists for the same mapping?
3. Should refcounts be scoped per triples map or per whole mapping? Per triples map gives better diagnostics; per mapping may reduce rows.
4. How much of W3C R2RML should be fixed before incremental mode ships, especially `rr:joinCondition`, `rr:datatype`, `rr:language`, and `rr:graphMap`?
5. Does pg_trickle expose stream-table OLD/NEW row images consistently for every query shape we want to support, especially aggregates?
6. Should source schema changes auto-disable the mapping, or should trigger calls fail until `r2rml_reconcile()` runs?

---

## Recommendation

Implement incremental R2RML in two layers:

1. **Base-table trigger mode first.** This gives immediate value, requires no pg_trickle dependency, and forces the correct compiler/refcount/delete semantics.
2. **pg_trickle SQL-source mode second.** This unlocks the more interesting R2RML use cases: joins, filtered views, aggregates, and arbitrary logical tables maintained from relational deltas.

Do not build the feature as “rerun full R2RML faster”. Build it as “row delta to triple delta”, with output ownership and refcounts from day one. That is the difference between a fresh graph and a quietly accumulating pile of stale facts.