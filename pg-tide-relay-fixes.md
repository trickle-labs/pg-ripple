# pg-tide Relay Fix Plan

Date: 2026-05-21

## Executive Summary

The project still carries the phrase "pg-trickle relay" and several pg_trickle-era relay assumptions. That terminology is now wrong for relay, outbox, and inbox features: pg_trickle is still relevant for incremental view maintenance (IVM), but the relay subsystem has moved to the separate pg_tide project at https://github.com/trickle-labs/pg-tide.

The fix should not be a blind search-and-replace. Some pg_trickle references are still correct, especially SPARQL/Datalog/CONSTRUCT/DESCRIBE/ASK views and ExtVP stream tables. The work is to draw a hard boundary:

- pg_trickle: IVM only.
- pg_tide: relay pipelines, transactional outbox, idempotent inbox, consumer groups, relay binary, relay deployment, relay monitoring.
- pg_ripple: RDF/triplestore logic, JSON/RDF transforms, subscriptions, and bridge code that should publish to pg_tide outboxes when relay transport is needed.

The plan below updates docs, examples, runtime checks, GUC names, CDC bridge behavior, regression tests, Docker/dependency pins, and release metadata so the repo no longer implies that a pg-trickle relay exists.

## Research Findings

### Current pg_tide project facts

From the pg_tide repository and documentation as of 2026-05-21:

- Repository: https://github.com/trickle-labs/pg-tide
- Purpose: transactional outbox, idempotent inbox, consumer groups, and relay pipelines for PostgreSQL 18+.
- Extension name: `pg_tide`.
- SQL schema: `tide`.
- User-facing relay process/binary: `pg-tide`.
- Source package/crate name for cargo install: `pg-tide-relay`, but the installed command shown in docs is `pg-tide`.
- Container image: `ghcr.io/trickle-labs/pg-tide:latest` or versioned tags, not `ghcr.io/trickle-labs/pg-tide-relay`.
- Primary relay environment variable: `PG_TIDE_POSTGRES_URL`.
- Relay process exposes Prometheus metrics and health on the configured metrics address, defaulting to `0.0.0.0:9090`.
- Pipeline configuration lives in PostgreSQL catalog tables and hot-reloads through LISTEN/NOTIFY. It is not managed through static relay TOML pipeline sections.

### Current pg_tide SQL API shape

Use named arguments in pg_ripple docs/examples where there is any ambiguity.
For relay pipeline configuration, prefer the stable JSONB v2 APIs introduced by
pg_tide (`relay_set_outbox_v2` and `relay_set_inbox_v2`) because the older
multi-argument forms emit deprecation warnings in pg_tide 0.33.0.

```sql
CREATE EXTENSION pg_tide;

SELECT tide.outbox_create(
  p_name             := 'orders',
  p_retention_hours  := 48,
  p_inline_threshold := 10000
);

SELECT tide.outbox_publish(
  p_name    := 'orders',
  p_payload := '{"order_id": 42}'::jsonb,
  p_headers := '{"event_type": "order.created"}'::jsonb
);

SELECT tide.inbox_create(
  p_name        := 'payment-webhooks',
  p_schema      := 'tide',
  p_max_retries := 5
);

SELECT tide.relay_set_outbox_v2(jsonb_build_object(
  'name',       'orders-to-nats',
  'outbox',     'orders',
  'sink_type',  'nats',
  'config',     jsonb_build_object('url', 'nats://localhost:4222', 'subject', 'orders.{event_type}'),
  'batch_size', 200,
  'enabled',    true
));

SELECT tide.relay_set_inbox_v2(jsonb_build_object(
  'name',       'stripe-webhooks',
  'inbox',      'payment-webhooks',
  'source',     'webhook',
  'config',     jsonb_build_object('port', 8080, 'path', '/webhooks/stripe'),
  'batch_size', 50,
  'idempotent', true
));

SELECT tide.relay_disable('orders-to-nats');
SELECT tide.relay_enable('orders-to-nats');
SELECT tide.relay_delete('orders-to-nats');
SELECT tide.relay_get_config('orders-to-nats');
SELECT tide.relay_list_configs();
```

### Things already partially correct in pg_ripple

- `src/lib.rs` already has `PG_TIDE_TESTED_VERSION`, `has_pg_tide()`, and version warning logic.
- `src/views_api.rs` exposes `pg_ripple.pg_tide_available()`.
- `src/bidi/mod.rs` already says BIDI outbox/inbox use pg_tide APIs.
- `docs/src/operations/compatibility.md` already has a pg_tide / pg_trickle compatibility section.
- `docs/src/operations/pg-trickle-relay.md` has been partly rewritten to pg_tide concepts and `tide.*` calls.
- `plans/PLAN_PG_TIDE.md` captured the original extraction from pg_trickle v0.46.0, but it is stale against current pg_tide docs and still says the relay binary is `pg-tide-relay`.

### Main remaining mismatch

The repo still treats the CDC bridge as a pg_trickle feature in source and tests. `src/storage/cdc_bridge.rs` gates bridge functions on `TRICKLE_INTEGRATION` and `has_pg_trickle()`, and its trigger function inserts directly into a caller-provided table with `(event_id, payload)`. That matches the old pg_trickle relay/table-outbox model, not pg_tide's named outbox and `tide.outbox_publish()` model.

This is the root functional issue to fix.

## Scope Boundaries

### Keep pg_trickle references where they are still true

Do not remove or rename pg_trickle references related to:

- SPARQL views.
- Datalog views.
- CONSTRUCT, DESCRIBE, ASK views.
- ExtVP stream tables.
- `pg_ripple.pg_trickle_available()`.
- pg_trickle dependency docs for IVM.
- Historical roadmap/release notes where the text clearly refers to the state at that release and already includes a migration note.

### Replace or rewrite pg_trickle references where they describe relay transport

Fix references to:

- "pg-trickle relay" as a current component.
- `pgtrickle-relay` or `pg-tide-relay` as an operator command.
- `ghcr.io/trickle-labs/pg-tide-relay` as a container image.
- `PG_TIDE_RELAY_POSTGRES_URL` as the relay database URL environment variable.
- `pgtrickle.set_relay_*`, `pgtrickle.enable_outbox`, `pgtrickle.pause_subscription`, and `pgtrickle.resume_subscription` as current relay APIs.
- Any bridge code or docs that say relay events are inserted into a plain table for pg_trickle to consume.

## Affected Inventory

### Source code

- `src/storage/cdc_bridge.rs`
  - Module title says `CDC -> pg-trickle Outbox Bridge`.
  - `require_trickle()` checks `TRICKLE_INTEGRATION` and `has_pg_trickle()`.
  - Trigger function inserts into an arbitrary table with `INSERT INTO %I (event_id, payload)`.
  - Catalog column is `outbox_table`, but pg_tide relay should use a named outbox.

- `src/cdc_bridge_api.rs`
  - `trickle_available()` says CDC bridge requires pg_trickle.
  - `enable_cdc_bridge_trigger()` docs say PT800 is raised when pg-trickle is absent.

- `src/schema/views.rs`
  - Fresh install schema creates `_pg_ripple.cdc_bridge_triggers(outbox_table text)`.
  - Embedded trigger function still inserts into an outbox table.
  - Comments say v0.52 was pg-trickle relay integration.

- `src/gucs/registration/storage.rs` and `src/gucs/storage.rs`
  - `pg_ripple.cdc_bridge_outbox_table` describes a target table with `(event_id, payload)`.
  - `pg_ripple.trickle_integration` describes pg-trickle bridge integration.

- `src/views/mod.rs`
  - `PGTIDE_HINT` exists, but the minimum version text should be reviewed against the current supported pg_tide floor.

### Tests and expected output

- `tests/pg_regress/sql/trickle_graceful_degradation.sql`
  - Tests `trickle_available()` and old GUC behavior.

- `tests/pg_regress/sql/trickle_integration.sql`
  - Creates `mock_outbox(event_id, payload)` to mimic old pg-trickle outbox schema.
  - Uses `pg_ripple.trickle_available()` to decide whether to install the bridge trigger.

- `tests/pg_regress/expected/trickle_graceful_degradation.out`
- `tests/pg_regress/expected/trickle_integration.out`
- `tests/pg_regress/results/trickle_graceful_degradation.out`
- `tests/pg_regress/results/trickle_integration.out`
  - Expected output still encodes old names.

### Documentation

- `docs/src/SUMMARY.md`
  - Link title still says `pg-trickle Relay: Hub-and-Spoke`.

- `docs/src/operations/pg-trickle-relay.md`
  - Filename is stale even though the title says pg-tide.
  - Requires line uses `pg-tide >= 0.4.0`; this may be too old for the current docs/examples and should match the tested version policy.
  - Docker snippet uses `ghcr.io/trickle-labs/pg-tide-relay:0.15.0` and `PG_TIDE_RELAY_POSTGRES_URL`; both are stale.
  - Deployment text calls the binary `pg-tide-relay`; current docs use command `pg-tide`.

- `docs/src/operations/bidi-production-checklist.md`
  - Section `## 11. pg-trickle relay` and signoff row should become pg_tide relay.

- `docs/src/operations/bidi-runbook.md`
  - Intro links to the pg-trickle relay guide.
  - Queue draining procedure says pg-trickle delivery stalls.
  - Pause/resume examples call nonexistent `pg_trickle.pause_subscription()` / `pg_trickle.resume_subscription()`.

- `docs/src/operations/docker.md`
  - Preinstalled versions table says pg_ripple 0.98.0, pg_trickle 0.48.0, pg_tide 0.15.0, which is stale relative to current repo pins.

- `docs/src/reference/guc-reference.md`
  - CDC bridge GUC descriptions still say pg-trickle outbox bridge and table target.

- `docs/src/reference/error-catalog.md`
  - PT800 category and fix point to pg_trickle rather than pg_tide for bridge features.

- `docs/src/reference/sql-functions.md`
  - Contains pg_trickle availability docs, which are valid for IVM. Add or cross-link pg_tide availability and relay availability docs.

- Generated docs under `docs/book/`
  - Should be regenerated after source docs change, or excluded if the project does not commit generated mdBook output.

### Examples and blog

- `examples/bidi_relay_setup.sql`
  - Header says synchronizing knowledge graphs using pg-trickle.
  - Prerequisites say pg_trickle replication setup.
  - Section title says `Check pg-trickle relay prerequisites`.

- `blog/semantic-hub-trickle-relay.md`
  - Title is already pg-tide, but file name remains trickle.
  - Text says CLI is `pg-tide-relay`; current command should be `pg-tide`.

### Roadmap and plans

- `plans/PLAN_PG_TIDE.md`
  - Useful historical migration analysis, but stale with current pg_tide API and binary naming.
  - Several examples use older `tide.relay_set_outbox(name, config)` style instead of current argument list.

- `plans/pg_trickle_relay_integration.md`
  - Historical plan still has old Docker images like `trickle-labs/pgtrickle-relay:0.25.0`.
  - It already has a migration warning. Either keep as historical with a stronger deprecation banner, or replace examples with links to the new pg_tide relay guide.

- `roadmap/v0.77.0-full.md`
  - Contains current-looking statements that pg-trickle is a hard dependency for BIDI relay and mentions a pg-trickle relay webhook handler.
  - Add explicit update notes or revise the text if this roadmap is presented as current truth.

- `roadmap/v0.52.0.md` and `roadmap/v0.52.0-full.md`
  - Historical release title is `pg-trickle Relay Integration`; leave as historical if wrapped by a v0.93 migration note.

### Dependency and deployment metadata

- `.versions.toml`
  - Current `pg_tide = "0.16.0"`; upstream pg_tide latest observed from GitHub is v0.33.0.

- `Dockerfile`
  - Current `ARG PG_TIDE_VERSION=0.16.0`.

- `scripts/check_dep_versions.sh`
  - Ensures Dockerfile and `.versions.toml` stay aligned. Keep and use it after version changes.

## Desired End State

1. No current docs or examples say a pg-trickle relay exists.
2. pg_trickle is documented only as IVM for live/materialized stream views.
3. pg_tide is documented as the only relay/outbox/inbox transport.
4. The canonical operator command is `pg-tide`, not `pg-tide-relay`.
5. The canonical relay container is `ghcr.io/trickle-labs/pg-tide:<version>`, not `ghcr.io/trickle-labs/pg-tide-relay:<version>`.
6. The canonical database URL env var is `PG_TIDE_POSTGRES_URL`, not `PG_TIDE_RELAY_POSTGRES_URL`.
7. CDC bridge trigger code publishes to a pg_tide named outbox via `tide.outbox_publish()` rather than inserting into a pg_trickle-style table.
8. Public compatibility is preserved where practical through deprecated aliases and migration notes.
9. Tests cover both graceful degradation when pg_tide is absent and the pg_tide publish path when pg_tide is installed.
10. Dependency pins and docs agree on the tested pg_tide version.

## Implementation Plan

### Phase 0: Confirm version policy

1. Decide the pg_tide version floor for the next pg_ripple release.
   - Conservative choice: support `pg_tide >= 0.16.0` because that is what the current Dockerfile tests against.
   - Better current-doc choice: update to latest observed `pg_tide = "0.33.0"`, rebuild, and make that the tested version.
   - Compatibility-doc choice: keep a minimum such as `>= 0.4.0` only if the current APIs used by pg_ripple are verified against that version.

2. Update source-of-truth version files together if the tested version changes.
   - `.versions.toml`
   - `Dockerfile` `ARG PG_TIDE_VERSION`
   - `CHANGELOG.md`
   - release/roadmap file for the new pg_ripple version

3. Run `scripts/check_dep_versions.sh` after any version change.

### Phase 1: Fix runtime bridge ownership

1. Rename internal bridge requirement helper.
   - From: `require_trickle(fn_name)`
   - To: `require_tide(fn_name)` or `require_relay_transport(fn_name)`

2. Change bridge gating.
   - Current: `TRICKLE_INTEGRATION && has_pg_trickle()`
   - Target: relay bridge requires pg_tide.
   - Keep pg_trickle gating only for IVM view code.

3. Introduce a canonical relay availability function.
   - Preferred: add `pg_ripple.relay_available() RETURNS bool` returning `relay_integration_enabled && has_pg_tide()`.
   - Keep `pg_ripple.pg_tide_available() RETURNS bool` as the raw extension check.
   - Keep `pg_ripple.trickle_available()` as a deprecated compatibility alias for v0.52 callers, but change its docs. Because this function originally represented relay/bridge availability, it should no longer check pg_trickle for relay features.
   - Keep `pg_ripple.pg_trickle_available()` untouched for IVM.

4. Update CDC trigger function to use pg_tide.
   - Current behavior: dynamic insert into an arbitrary outbox table.
   - Target behavior:

```sql
PERFORM tide.outbox_publish(
    outbox_name,
    payload,
    jsonb_build_object(
        'event_id', dedup_key,
        'event_type', 'pg_ripple.triple.insert',
        'predicate_id', pred_id,
        'subject_id', NEW.s,
        'object_id', NEW.o,
        'graph_id', NEW.g
    )
);
```

5. Validate the target outbox before trigger install.
   - Require users to create the outbox explicitly with `tide.outbox_create(...)`.
   - During `enable_cdc_bridge_trigger`, call a lightweight validation such as `SELECT tide.outbox_status($1)` or check the pg_tide catalog table.
   - If the outbox is missing, raise a clear error that says to run `SELECT tide.outbox_create(...)`.

6. Avoid dynamic table-name SQL in the publish path.
   - This removes the `EXECUTE format('INSERT INTO %I ...')` pattern.
   - It aligns with pg_tide's single shared outbox table, `tide.tide_outbox_messages`.

7. Preserve idempotence.
   - Keep the `ripple:<statement_id>` dedup key in headers.
   - Confirm how pg_tide handles idempotence for outbox publish. If the outbox does not enforce `event_id` uniqueness, ensure duplicate suppression lives at the consumer/relay layer or add a pg_ripple-side guard.

8. Preserve payload shape.
   - Keep existing JSON-LD payload shape for downstream compatibility.
   - Move transport metadata into headers where possible: `event_type`, `dedup_key`, `predicate_id`, source graph, and schema version.

### Phase 2: Migrate schema and GUC naming safely

1. Add new canonical catalog naming.
   - Current column: `_pg_ripple.cdc_bridge_triggers.outbox_table`
   - New semantic name: `outbox_name`

2. Use a backward-compatible migration.
   - Add `outbox_name TEXT` nullable.
   - Backfill `outbox_name = outbox_table`.
   - Update source to write both columns for one release, or keep `outbox_table` as a compatibility alias if dropping columns is too disruptive before v1.0.
   - Update `cdc_bridge_triggers()` output column from `outbox` or `outbox_table` to describe it as a pg_tide outbox name.

3. Add new canonical GUC names.
   - Preferred new name: `pg_ripple.relay_integration` or `pg_ripple.tide_integration`.
   - Preferred new name: `pg_ripple.cdc_bridge_outbox_name`.

4. Keep compatibility aliases for old GUCs until v1.0.
   - `pg_ripple.trickle_integration` should keep working but be documented as deprecated.
   - `pg_ripple.cdc_bridge_outbox_table` should keep working but be documented as deprecated and interpreted as an outbox name when pg_tide is used.

5. Update error text.
   - Current: pg_trickle not installed or trickle_integration disabled.
   - Target: pg_tide not installed or relay integration disabled.

6. Update error catalog.
   - Rename PT800 category from pg-trickle CDC bridge to pg_tide relay bridge.
   - Fix cause and remediation text.

### Phase 3: Correct docs and examples

1. Rename the operations guide.
   - Add `docs/src/operations/pg-tide-relay.md` as the canonical guide.
   - Keep `docs/src/operations/pg-trickle-relay.md` as a short compatibility stub that points to `pg-tide-relay.md`, unless external links are not a concern.
   - Update `docs/src/SUMMARY.md` to `pg-tide Relay: Hub-and-Spoke`.

2. Update `docs/src/operations/pg-tide-relay.md` content.
   - Use command `pg-tide --postgres-url ...`.
   - Use image `ghcr.io/trickle-labs/pg-tide:<version>`.
   - Use env var `PG_TIDE_POSTGRES_URL`.
  - Use stable `tide.relay_set_outbox_v2` and `tide.relay_set_inbox_v2` JSONB configuration APIs.
   - Say pg_trickle is needed only if the walkthrough uses IVM stream views. If the walkthrough only uses raw triggers and pg_tide outbox/inbox, pg_trickle should be optional.

3. Update BIDI runbook.
   - Replace "pg-trickle delivery stalls" with "pg_tide relay delivery stalls".
   - Replace nonexistent pause/resume calls with pg_tide lifecycle calls:

```sql
SELECT tide.relay_disable('<pipeline_name>');
SELECT tide.relay_enable('<pipeline_name>');
```

   - Replace `pg_trickle_paused` wording if that field still exists in `bidi_status()`. If the SQL API still exposes `pg_trickle_paused`, plan a compatibility rename to `relay_paused` with old alias retained.

4. Update BIDI production checklist.
   - Section 11 becomes `pg_tide relay`.
   - Checklist should verify `pg_ripple.pg_tide_available()`, relay health endpoint, pipeline health metrics, and consumer lag.

5. Update examples.
   - `examples/bidi_relay_setup.sql` should mention pg_tide relay and pg_tide setup.
  - Include `CREATE EXTENSION pg_tide;` and a minimal `tide.outbox_create` / `tide.relay_set_outbox_v2` example if relevant.

6. Update Docker docs.
   - Preinstalled version table must match `.versions.toml` and Dockerfile.
   - Include the current relay image and env var in compose snippets.

7. Update blog post.
   - Change user-facing command from `pg-tide-relay` to `pg-tide`.
   - Decide whether to rename the file for permalink correctness. If keeping the old filename, add a short note that the old filename is historical.

### Phase 4: Update roadmap/plans without erasing history

1. Add a new remediation roadmap entry for the next release.
   - Suggested item name: `TIDE-RELAY-NAME-01` or `PGTIDE-RELAY-01`.
   - Include source migration, docs rename, test updates, Docker/dependency refresh, and grep guard.

2. Update `plans/PLAN_PG_TIDE.md`.
   - Mark it as superseded by this plan or refresh its API tables to current pg_tide.
   - Fix `pg-tide-relay` binary references to `pg-tide` for operator commands.
  - Fix `tide.relay_set_outbox(name, config)` examples to the current `relay_set_outbox_v2(config JSONB)` signature.

3. Update `plans/pg_trickle_relay_integration.md`.
   - Keep as historical exploration, but add a stronger top banner:
     - pg-trickle relay no longer exists.
     - For current work use pg_tide and the new operations guide.
   - Remove or clearly mark old Docker examples using `trickle-labs/pgtrickle-relay`.

4. Update `roadmap/v0.77.0-full.md` carefully.
   - Since it describes a released version, prefer footnotes/update notes instead of rewriting historical claims without context.
   - Any current guidance inside that file should say BIDI relay transport is pg_tide.

### Phase 5: Tests

1. Rename or add tests for pg_tide availability.
   - Add `tests/pg_regress/sql/tide_graceful_degradation.sql`.
   - Keep old `trickle_graceful_degradation.sql` only if it is still needed for deprecated function coverage.

2. Update graceful degradation coverage.
   - `pg_ripple.pg_tide_available()` returns boolean.
   - New `pg_ripple.relay_available()` returns boolean if added.
   - Bridge trigger install errors clearly when pg_tide is absent or relay integration is off.
   - `pg_ripple.pg_trickle_available()` remains covered by view tests.

3. Update bridge integration test.
   - Stop mocking a pg-trickle-style `mock_outbox` table for the canonical path.
   - If pg_tide is unavailable in default regress runs, keep a no-pg_tide degradation test in normal CI.
   - Add an optional pg_tide-enabled test job that:
     - creates `CREATE EXTENSION pg_tide;`
     - creates `SELECT tide.outbox_create('ripple-events', ...)`
     - installs `pg_ripple.enable_cdc_bridge_trigger(...)`
     - inserts a triple
     - verifies a row exists in `tide.tide_outbox_messages` or via a pg_tide view/status API
     - verifies headers include event type and dedup key

4. Update expected output files.
   - Regenerate expected files after SQL changes.
   - Avoid committing `tests/pg_regress/results/*` unless this repository intentionally tracks them.

5. Add a grep guard.
   - A small script or CI step should fail on current-tense forbidden strings outside known historical files.
   - Suggested forbidden patterns:
     - `pg-trickle relay`
     - `pg_trickle relay`
     - `pgtrickle-relay`
     - `ghcr.io/trickle-labs/pg-tide-relay`
     - `PG_TIDE_RELAY_POSTGRES_URL`
     - `pgtrickle.set_relay_`
     - `pgtrickle.enable_outbox`
     - `pg_trickle.pause_subscription`
     - `pg_trickle.resume_subscription`

   - Allowlist historical changelog/roadmap sections only when they also include a migration note.

### Phase 6: Release artifacts

1. Add a migration script from the current version to the next version.
   - Current repo appears to be at v0.126.0, so the likely file is `sql/pg_ripple--0.126.0--0.127.0.sql`.
   - If catalog columns/GUC compatibility objects change, include those DDL changes.
   - If code/docs only, add a comment-only migration explaining the pg_tide relay terminology and API cleanup.

2. Update `pg_ripple.control` default version for the release.

3. Update `CHANGELOG.md`.
   - Document behavior changes and compatibility aliases.
   - Include a clear migration note for operators.

4. Update `ROADMAP.md` and add a release file if this project expects one per release.

5. Update generated docs if they are committed.
   - Run mdBook or the repository's doc generation command.

## Concrete Code Change Sketch

### New bridge requirement helper

```rust
pub(crate) fn require_relay_transport(fn_name: &str) {
    if !crate::RELAY_INTEGRATION.get() {
        pgrx::error!(
            "{fn_name}(): pg_ripple.relay_integration is off; set it to on to use relay bridge features"
        );
    }
    if !crate::has_pg_tide() {
        pgrx::error!(
            "{fn_name}(): pg_tide extension is not installed; install pg_tide from https://github.com/trickle-labs/pg-tide and run CREATE EXTENSION pg_tide"
        );
    }
}
```

If introducing a new GUC is too much for the immediate patch, use the existing `TRICKLE_INTEGRATION` setting internally for one release, but rename user-facing docs and mark the GUC deprecated.

### New trigger function shape

```sql
CREATE OR REPLACE FUNCTION _pg_ripple.cdc_bridge_trigger_fn()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE
    pred_id     BIGINT := TG_ARGV[0]::bigint;
    outbox_name TEXT   := TG_ARGV[1];
    s_iri       TEXT;
    p_iri       TEXT;
    o_iri       TEXT;
    payload     JSONB;
    headers     JSONB;
    dedup_key   TEXT;
BEGIN
    SELECT value INTO s_iri FROM _pg_ripple.dictionary WHERE id = NEW.s;
    SELECT value INTO p_iri FROM _pg_ripple.dictionary WHERE id = pred_id;
    SELECT value INTO o_iri FROM _pg_ripple.dictionary WHERE id = NEW.o;

    dedup_key := 'ripple:' || NEW.i::text;

    payload := jsonb_build_object(
        '@context', 'https://schema.org/',
        '@id', COALESCE(s_iri, '_:' || NEW.s::text),
        p_iri, COALESCE(o_iri, NEW.o::text)
    );

    headers := jsonb_build_object(
        'event_type', 'pg_ripple.triple.insert',
        'dedup_key', dedup_key,
        'predicate_id', pred_id,
        'statement_id', NEW.i,
        'graph_id', NEW.g
    );

    PERFORM tide.outbox_publish(outbox_name, payload, headers);
    RETURN NEW;
END;
$$;
```

### Canonical documentation snippet

```yaml
services:
  relay:
    image: ghcr.io/trickle-labs/pg-tide:0.33.0
    environment:
      PG_TIDE_POSTGRES_URL: postgres://relay:pw@postgres/hub
      PG_TIDE_LOG_FORMAT: json
      PG_TIDE_LOG_LEVEL: info
      PG_TIDE_GROUP_ID: production
      KAFKA_BROKERS: kafka:9092
    ports:
      - "9090:9090"
```

```sql
SELECT tide.relay_set_outbox_v2(jsonb_build_object(
  'name',      'alerts-to-kafka',
  'outbox',    'enriched-events',
  'sink_type', 'kafka',
  'config',    jsonb_build_object(
    'brokers', '${env:KAFKA_BROKERS}',
    'topic',   'iot.alerts'
  )
));
```

## Verification Checklist

### Static verification

- `git grep -n "pg-trickle relay" -- .` returns only historical files with explicit migration notes.
- `git grep -n "pgtrickle-relay" -- .` returns only historical files with explicit migration notes, or nothing.
- `git grep -n "pg-tide-relay" -- docs examples blog Dockerfile docker-compose.yml charts` returns nothing for user-facing commands/images unless it is explicitly describing the cargo package name.
- `git grep -n "PG_TIDE_RELAY_POSTGRES_URL" -- .` returns nothing.
- `git grep -n "pgtrickle.set_relay\|pgtrickle.enable_outbox\|pg_trickle.pause_subscription\|pg_trickle.resume_subscription" -- .` returns nothing outside historical migration notes.

### Build and test verification

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo pgrx test pg18`
- `cargo pgrx regress pg18`
- `bash tests/test_migration_chain.sh`
- `scripts/check_dep_versions.sh`

### pg_tide integration verification

Run in an environment with pg_tide installed:

```sql
CREATE EXTENSION pg_tide;
CREATE EXTENSION pg_ripple;

SELECT pg_ripple.pg_tide_available();
SELECT tide.outbox_create('ripple-events', 24, 10000);

SELECT pg_ripple.insert_triple(
  '<https://example.org/s>',
  '<https://example.org/p>',
  '"o"'
);

SELECT pg_ripple.enable_cdc_bridge_trigger(
  'p_bridge',
  '<https://example.org/p>',
  'ripple-events'
);

SELECT pg_ripple.insert_triple(
  '<https://example.org/s2>',
  '<https://example.org/p>',
  '"o2"'
);

SELECT tide.outbox_status('ripple-events');
```

Expected result: the status shows at least one pending message, and the stored message includes the pg_ripple JSON-LD payload and headers with a stable dedup key.

### Deployment verification

- Build the Docker image with the updated pg_tide pin.
- Confirm `pg-tide --version` works inside the relay image/container.
- Confirm `CREATE EXTENSION pg_tide;` works inside the batteries-included pg_ripple image if that image is supposed to bundle pg_tide.
- Start a relay with `PG_TIDE_POSTGRES_URL` and verify `/health` and `/metrics`.
- Configure a test pipeline with `tide.relay_set_outbox_v2(...)` and confirm hot reload without restart.

## Migration Notes for Users

Users with old pg_trickle relay deployments should:

1. Install pg_tide.

```sql
CREATE EXTENSION pg_tide;
```

2. Keep pg_trickle only for IVM features.

```sql
CREATE EXTENSION pg_trickle;
```

3. Replace old relay config calls.

```sql
-- Old, no longer current:
-- SELECT pgtrickle.set_relay_outbox(...);

-- New:
SELECT tide.relay_set_outbox_v2(jsonb_build_object(
  'name',      'pipeline-name',
  'outbox',    'outbox-name',
  'sink_type', 'kafka',
  'config',    jsonb_build_object(
    'brokers', '${env:KAFKA_BROKERS}',
    'topic',   'events'
  )
));
```

4. Replace old outbox table assumptions.

```sql
SELECT tide.outbox_create('outbox-name', 24, 10000);
SELECT tide.outbox_publish('outbox-name', '{"hello":"world"}'::jsonb, '{"event_type":"example"}'::jsonb);
```

5. Replace relay process startup.

```bash
PG_TIDE_POSTGRES_URL="postgres://relay:secret@db/app" pg-tide
```

6. Use `pg_ripple.pg_tide_available()` for relay transport checks and `pg_ripple.pg_trickle_available()` for IVM checks.

## Suggested Work Order

1. Add the grep guard first so new stale strings do not keep landing.
2. Migrate `src/storage/cdc_bridge.rs` from table insert to `tide.outbox_publish()`.
3. Add or update migration SQL for catalog/GUC compatibility.
4. Update regression tests and expected files.
5. Rename the operations guide and update docs links.
6. Update examples, runbook, checklist, Docker docs, and blog wording.
7. Refresh `.versions.toml` and Dockerfile if choosing a newer pg_tide tested version.
8. Update changelog, roadmap, and release migration script.
9. Run full verification.

## Open Decisions

1. Should the next release bump pg_tide from 0.16.0 to latest observed 0.33.0, or only fix naming/API assumptions against the current 0.16.0 pin?
2. Should `pg_ripple.trickle_available()` remain as a deprecated relay alias that now checks pg_tide, or should it keep old behavior and be documented as obsolete?
3. Should the CDC bridge auto-create pg_tide outboxes, or require explicit `tide.outbox_create()` for clearer operational ownership?
4. Should `_pg_ripple.cdc_bridge_triggers.outbox_table` be renamed before v1.0, or kept forever as a compatibility column whose semantics are now "outbox name"?
5. Should historical roadmap files be rewritten, or should they retain history with stronger migration banners?

## Recommended Decisions

1. Bump pg_tide to the latest tested version during the fix, because docs currently point to APIs from a newer pg_tide than the repo pins.
2. Add `pg_ripple.relay_available()` and deprecate `pg_ripple.trickle_available()`.
3. Require explicit `tide.outbox_create()`; do not auto-create outboxes from pg_ripple bridge functions.
4. Add `outbox_name` while keeping `outbox_table` as a compatibility alias through v1.0.
5. Rewrite current docs and examples, but keep historical roadmap files intact with clear update notes.
