# PLAN_PG_TIDE — pg_ripple Integration with pg_tide v0.1.0

> **Status**: Planning  
> **Triggered by**: pg-trickle v0.46.0 — outbox, inbox, and relay extracted into `pg_tide`  
> **pg_tide repository**: https://github.com/trickle-labs/pg-tide  
> **Affects pg_ripple versions**: v0.93.0

---

## Background

pg-trickle v0.46.0 is a focused extraction release. The full transactional outbox, idempotent
inbox, and relay subsystem (~6 150 Rust LOC + ~2 500 SQL LOC) was removed from `pg_trickle`
and published as the new standalone `pg_tide` extension (`trickle-labs/pg-tide`). After
v0.46.0, `pg_trickle` ships exactly one thing: incremental view maintenance (IVM).

This split has direct consequences for pg_ripple, which depends on both features:

- **IVM** (SPARQL views, Datalog views, CONSTRUCT/DESCRIBE/ASK views) → stays in `pg_trickle`;
  the `pgtrickle.create_stream_table()` / `drop_stream_table()` API is unchanged.
- **Relay + outbox + inbox** (hub-and-spoke integration, BIDI-OUTBOX-01, BIDI-INBOX-01) →
  moved to `pg_tide`; the functions, schema names, and relay binary all changed.

---

## What Changed in pg_trickle v0.46.0 / pg_tide v0.1.0

### Relay catalog tables

| Old (pgtrickle schema) | New (tide schema) |
|---|---|
| `pgtrickle.relay_outbox_config` | `tide.relay_outbox_config` |
| `pgtrickle.relay_inbox_config` | `tide.relay_inbox_config` |
| `pgtrickle.relay_consumer_offsets` | `tide.relay_consumer_offsets` |

The `pgtrickle_relay` role is dropped. The internal tables
`pgtrickle.pgt_outbox_config`, `pgtrickle.pgt_inbox_config`,
`pgtrickle.pgt_consumer_groups`, `pgtrickle.pgt_consumer_offsets`,
`pgtrickle.pgt_consumer_leases`, `pgtrickle.relay_outbox_config`,
`pgtrickle.relay_inbox_config`, `pgtrickle.relay_consumer_offsets`
are all removed from pg_trickle.

### SQL function renames and moves

| Removed from pg_trickle | New pg_tide equivalent |
|---|---|
| `pgtrickle.set_relay_outbox(name, outbox, group, sink)` | `tide.relay_set_outbox(name, config)` |
| `pgtrickle.set_relay_inbox(name, inbox, source)` | `tide.relay_set_inbox(name, config)` |
| `pgtrickle.enable_relay(name)` | `tide.relay_enable(name)` |
| `pgtrickle.disable_relay(name)` | `tide.relay_disable(name)` |
| `pgtrickle.delete_relay(name)` | `tide.relay_delete(name)` |
| `pgtrickle.get_relay_config(name)` | — (query `tide.relay_outbox_config` / `tide.relay_inbox_config` directly) |
| `pgtrickle.list_relay_configs()` | — (query `tide.relay_outbox_config` / `tide.relay_inbox_config` directly) |
| `pgtrickle.enable_outbox(table)` | `tide.outbox_create(outbox_name, retention_hours, inline_threshold)` |
| `pgtrickle.disable_outbox(table)` | `tide.outbox_disable(outbox_name)` |
| `pgtrickle.outbox_status(table)` | `tide.outbox_status(outbox_name)` |
| `pgtrickle.poll_outbox(table, group, batch, timeout)` | consumer via `tide.*` consumer group API |
| `pgtrickle.create_consumer_group(name, table)` | `tide.create_consumer_group(group_name, outbox_name)` |
| `pgtrickle.drop_consumer_group(name)` | `tide.drop_consumer_group(name)` |
| `pgtrickle.commit_offset(group, id)` | `tide.commit_offset(group_name, consumer_id, offset)` |
| `pgtrickle.consumer_heartbeat(group)` | `tide.consumer_heartbeat(group_name, consumer_id)` |
| `pgtrickle.create_inbox(name, ...)` | `tide.inbox_create(inbox_name, ...)` |
| `pgtrickle.drop_inbox(name)` | `tide.inbox_drop(inbox_name)` |
| `pgtrickle.inbox_health()` | `tide.inbox_status(inbox_name)` |

### New integration hook in pg_trickle (retained)

`pgtrickle.attach_outbox(stream_table, retention_hours, inline_threshold_rows)` —
new in v0.46.0. Requires pg_tide to be installed. Calls `tide.outbox_create()` and
registers the stream table → outbox mapping in the slim `pgtrickle.pgt_outbox_config`
table. Every non-empty IVM refresh automatically calls `tide.outbox_publish()` inside
the same transaction.

### Relay binary renamed

| Old | New |
|---|---|
| `pgtrickle-relay` | `pg-tide-relay` |
| reads from `pgtrickle.relay_outbox_config` | reads from `tide.relay_outbox_config` |
| reads from `pgtrickle.relay_inbox_config` | reads from `tide.relay_inbox_config` |

### Outbox storage model change

Old pg_trickle outbox: each stream table had its own `pgtrickle.outbox_<tablename>` table.
The relay polled the per-table outbox directly.

New pg_tide outbox: all outboxes share one `tide.tide_outbox_messages` table, discriminated
by `outbox_name`. A trigger fires `tide.outbox_publish(outbox_name, payload, headers)` rather
than inserting into a dedicated table. The relay polls `tide.tide_outbox_messages`.

---

## Impact Analysis for pg_ripple

### P0 — Rust source code (no immediate compile break, but semantic changes required)

#### `src/lib.rs` — detection function

`has_pg_trickle()` is used for:
1. **SPARQL/Datalog/CONSTRUCT/ASK views** — still correct; IVM stayed in pg_trickle.
2. **BIDI-OUTBOX-01 / BIDI-INBOX-01** — these comments say "via pg-trickle outbox/inbox"
   but actually pg_ripple creates its own tables (not pg_trickle tables). No functional
   change needed here, only comment correction.

New function needed: `has_pg_tide()` — checks if `pg_tide` is installed. Used by:
- Any future function that calls `tide.outbox_create()`, `tide.relay_set_outbox()`, etc.
- A `pg_ripple.pg_tide_available()` SQL helper function.

#### `src/bidi/mod.rs` — BIDI-OUTBOX-01 / BIDI-INBOX-01 comments

The module doc comments at lines 14–15 say:
```
//! - BIDI-OUTBOX-01: Outbound events via pg-trickle outbox
//! - BIDI-INBOX-01: Receiver feedback via pg-trickle inbox
```
These should be updated to reference `pg_tide`.

#### `src/views/mod.rs` — `PGTRICKLE_HINT`

The hint text for missing pg_trickle is still correct (SPARQL views still need pg_trickle).
Add a companion `PGTIDE_HINT` for relay-related error paths.

### P0 — Documentation updates (user-visible, breaks real setups)

#### `docs/src/operations/pg-trickle-relay.md`

This is the primary integration guide. It contains approximately 20+ references to the
old pg_trickle relay API:

- `pgtrickle.set_relay_inbox(...)` → `tide.relay_set_inbox(...)`
- `pgtrickle.set_relay_outbox(...)` → `tide.relay_set_outbox(...)`
- `pgtrickle.enable_outbox(...)` → `tide.outbox_create(...)` + trigger calls `tide.outbox_publish(...)`
- `pgtrickle-relay` binary → `pg-tide-relay` binary
- Prerequisites section: add pg_tide as a separate install dependency
- Extension install order: `CREATE EXTENSION pg_tide; CREATE EXTENSION pg_trickle; CREATE EXTENSION pg_ripple;`
- Graceful-degradation section: mention both `pg_trickle` (for views) and `pg_tide` (for relay)

#### `blog/semantic-hub-trickle-relay.md`

Blog post covering the hub-and-spoke pattern. Contains:
- `pgtrickle.set_relay_inbox(...)` at line 48 → update to `tide.relay_set_inbox(...)`
- `pgtrickle.enable_outbox(...)` at line 171 → update to `tide.outbox_create(...)` + `tide.outbox_publish()`
- `pgtrickle.set_relay_outbox(...)` at lines 174, 182 → `tide.relay_set_outbox(...)`

### P1 — Plans documents

#### `plans/pg_trickle_relay_integration.md`

Architectural integration plan. Contains references throughout to the old relay API:
- Lines 63, 167, 170, 178, 186, 484, 555, 562, 571, 580
- Add a prominent header noting the document describes pg_trickle ≤ 0.45.0 and that
  relay has moved to pg_tide. Alternatively, update all API examples to the new API.

### P1 — Roadmap documents

#### `roadmap/v0.52.0.md` (lines 170, 188)

References `pgtrickle.set_relay_inbox` and `pgtrickle.set_relay_outbox`. Add a note that
these examples apply to pg_trickle < 0.46.0; for v0.46.0+ use `tide.*` equivalents.

#### `roadmap/v0.77.0-full.md` (lines 934, 937)

References `pgtrickle.set_relay_outbox`. Add the same note.

### P2 — Compatibility matrix update

#### `docs/src/operations/compatibility.md`

Add a new row documenting that pg_ripple ≥ {next release} requires `pg_tide ≥ 0.1.0`
for relay/outbox/inbox features, while SPARQL views still only require `pg_trickle ≥ 0.46.0`.

### P3 — CDC bridge trigger pattern update

The CDC bridge trigger pattern in `pg-trickle-relay.md` (Approaches A/B/C) uses:
```sql
SELECT pgtrickle.enable_outbox('enriched_events');
```

This must change to:
```sql
-- Outbox is now in pg_tide, not pg_trickle
SELECT tide.outbox_create('enriched-events', retention_hours => 24);

-- Triggers call tide.outbox_publish() instead of inserting into a bridge table
CREATE OR REPLACE FUNCTION bridge_alert_to_tide_outbox()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    PERFORM tide.outbox_publish(
        'enriched-events',
        jsonb_build_object(
            'subject',   pg_ripple.decode_id(NEW.s),
            'predicate', pg_ripple.decode_id(TG_ARGV[0]::bigint),
            'object',    pg_ripple.decode_id(NEW.o),
            'graph',     pg_ripple.decode_id(NEW.g)
        ),
        '{}'::jsonb   -- headers
    );
    RETURN NEW;
END;
$$;
```

Alternatively, for SPARQL view–based outbound paths, use `pgtrickle.attach_outbox()`:
```sql
-- After creating the SPARQL view stream table:
SELECT pg_ripple.create_sparql_view('enriched_alerts', $$ ... $$, '5s', false);

-- Attach the stream table to a pg_tide outbox (pg_trickle v0.46.0+):
SELECT pgtrickle.attach_outbox('pg_ripple.enriched_alerts', retention_hours => 24);

-- Configure the relay pipeline (now in tide schema):
SELECT tide.relay_set_outbox('alerts-to-kafka', config => '{
    "outbox": "pg_ripple.enriched_alerts",
    "group":  "kafka-publisher",
    "sink":   {"type":"kafka","brokers":"${env:KAFKA_BROKERS}","topic":"iot.alerts"}
}'::jsonb);
```

---

## Required Changes — Detailed Work Items

### TIDE-1: Add `has_pg_tide()` to `src/lib.rs`

Add a peer function alongside `has_pg_trickle()`:

```rust
/// The pg_tide version that pg_ripple was tested against.
const PG_TIDE_TESTED_VERSION: &str = "0.1.0";

/// Returns `true` when the pg_tide extension is installed in the current database.
///
/// Relay, outbox, and inbox features gate on this check. pg_ripple's core
/// functionality and SPARQL views work without pg_tide.
pub(crate) fn has_pg_tide() -> bool {
    let exists = pgrx::Spi::get_one::<bool>(
        "SELECT EXISTS(SELECT 1 FROM pg_extension WHERE extname = 'pg_tide')",
    )
    .unwrap_or(None)
    .unwrap_or(false);

    if exists {
        if let Some(installed) = pgrx::Spi::get_one::<String>(
            "SELECT extversion FROM pg_extension WHERE extname = 'pg_tide'",
        )
        .unwrap_or(None)
            && installed.as_str() > PG_TIDE_TESTED_VERSION
        {
            pgrx::warning!(
                "pg_ripple: pg_tide version {} is newer than tested version {}; \
                 relay integration may behave unexpectedly",
                installed,
                PG_TIDE_TESTED_VERSION
            );
        }
    }

    exists
}
```

Also expose it as a SQL function in `views_api.rs` or a new `tide_api.rs`:

```sql
-- pg_ripple.pg_tide_available() → boolean
-- Returns true when pg_tide is installed.
```

### TIDE-2: Update BIDI module doc comments

In `src/bidi/mod.rs`, update:
```rust
//! - BIDI-OUTBOX-01: Outbound events via pg-trickle outbox
//! - BIDI-INBOX-01: Receiver feedback via pg-trickle inbox
```
to:
```rust
//! - BIDI-OUTBOX-01: Outbound events via pg_tide outbox (tide.outbox_publish)
//! - BIDI-INBOX-01: Receiver feedback via pg_tide inbox (tide.relay_set_inbox)
```

### TIDE-3: Update `src/views/mod.rs` install hint

Add a companion constant and hint:
```rust
const PGTIDE_HINT: &str = "Install pg_tide: https://github.com/trickle-labs/pg-tide — \
     then run: CREATE EXTENSION pg_tide";
```

Use this hint in any future relay-dependent error paths.

### TIDE-4: Rewrite `docs/src/operations/pg-trickle-relay.md`

This is the largest single change. The document needs a top-of-file callout:

```
> **Updated for pg_tide v0.1.0 (pg-trickle v0.46.0+):**
> Relay, outbox, and inbox features are now provided by the standalone `pg_tide` extension.
> Install both extensions: `CREATE EXTENSION pg_tide; CREATE EXTENSION pg_trickle;`
> The relay binary is now `pg-tide-relay`.
```

Every SQL example that calls a relay/outbox/inbox function must be updated:

| Location | Old code | New code |
|---|---|---|
| Prerequisites | `CREATE EXTENSION pg_trickle;` | `CREATE EXTENSION pg_tide; CREATE EXTENSION pg_trickle;` |
| Step 1 relay config | `pgtrickle.set_relay_inbox(...)` | `tide.relay_set_inbox(name, config => '{...}'::jsonb)` |
| Step 5 outbox setup | `pgtrickle.enable_outbox('enriched_events')` | `tide.outbox_create('enriched-events', 24)` |
| Step 5 outbox setup | — | Add trigger body calling `tide.outbox_publish(...)` |
| Step 5 relay forward | `pgtrickle.set_relay_outbox(...)` | `tide.relay_set_outbox(name, config => '{...}'::jsonb)` |
| Fan-out examples | `pgtrickle.set_relay_outbox(...)` × 3 | `tide.relay_set_outbox(...)` × 3 |
| Approach A trigger | `pgtrickle.enable_outbox(...)` | `tide.outbox_create(...)` + publish trigger |
| Approach B | — | note on `pgtrickle.attach_outbox()` as convenience wrapper |
| Architecture diagram | "pg-trickle stream tables (inbox → outbox)" | split into "pg-trickle stream tables (IVM)" + "pg_tide (outbox/inbox/relay)" |
| Operations section | `pgtrickle.set_relay_outbox` / `pgtrickle.set_relay_inbox` | `tide.relay_set_outbox` / `tide.relay_set_inbox` |
| relay binary startup | `pgtrickle-relay` | `pg-tide-relay` |

### TIDE-5: Update `blog/semantic-hub-trickle-relay.md`

- Line 48: `pgtrickle.set_relay_inbox(...)` → `tide.relay_set_inbox(...)`
- Line 171: `pgtrickle.enable_outbox('enriched_events')` → `tide.outbox_create('enriched-events', 24)` + `tide.outbox_publish()` trigger pattern
- Lines 174, 182: `pgtrickle.set_relay_outbox(...)` → `tide.relay_set_outbox(...)`
- Update prerequisites section to list pg_tide alongside pg_trickle

### TIDE-6: Add backward-compatibility note to `plans/pg_trickle_relay_integration.md`

Add a header box:
```markdown
> **⚠ Note (pg-trickle v0.46.0+):** This plan was written against pg-trickle ≤ 0.45.0.
> In v0.46.0, relay / outbox / inbox were extracted into `pg_tide`.
> All examples using `pgtrickle.set_relay_outbox()`, `pgtrickle.set_relay_inbox()`,
> and `pgtrickle.enable_outbox()` must use the `tide.*` equivalents.
> See [PLAN_PG_TIDE.md](PLAN_PG_TIDE.md) for the migration guide.
```

### TIDE-7: Update roadmap documents

- `roadmap/v0.52.0.md` lines 170, 188: add inline comments `-- pg-trickle < 0.46.0 only; use tide.* for v0.46.0+`
- `roadmap/v0.77.0-full.md` lines 934, 937: same treatment

### TIDE-8: Update `docs/src/operations/compatibility.md`

Add rows to the compatibility table:

```markdown
| pg_trickle version | pg_ripple minimum | Notes |
|---|---|---|
| ≥ 0.46.0 | ≥ 0.91.0 | IVM only; relay/outbox/inbox moved to pg_tide |
| ≥ 0.25.0, < 0.46.0 | ≥ 0.52.0 | IVM + relay/outbox/inbox in pg_trickle |

| pg_tide version | pg_ripple minimum | Notes |
|---|---|---|
| ≥ 0.1.0 | ≥ 0.92.0 (next) | Standalone relay/outbox/inbox; replaces pgtrickle relay |
```

### TIDE-9: Update architecture diagram in `docs/src/operations/architecture.md` (if present)

Any diagram showing "pg-trickle" as the single integration layer should now show two layers:
- pg_trickle (IVM, stream tables, scheduled refresh)
- pg_tide (relay catalog, outbox messages, inbox, consumer groups)

---

## Migration Guide for Existing pg_ripple + pg_trickle Deployments

Operators upgrading from pg-trickle ≤ 0.45.0 to ≥ 0.46.0 need to:

### Step 1 — Install pg_tide

```sql
CREATE EXTENSION pg_tide;
```

### Step 2 — Migrate relay pipeline configs

Old:
```sql
SELECT pgtrickle.set_relay_outbox(
    'alerts-to-kafka',
    outbox => 'enriched_events',
    group  => 'kafka-publisher',
    sink   => '{"type":"kafka","brokers":"...","topic":"iot.alerts"}'
);
```

New:
```sql
SELECT tide.relay_set_outbox('alerts-to-kafka', config => '{
    "outbox": "enriched-events",
    "group":  "kafka-publisher",
    "sink":   {"type":"kafka","brokers":"...","topic":"iot.alerts"}
}'::jsonb);
```

Old:
```sql
SELECT pgtrickle.set_relay_inbox(
    'sensor-readings',
    inbox  => 'sensor_inbox',
    source => '{"type":"kafka","brokers":"...","topic":"iot.sensors"}'
);
```

New:
```sql
SELECT tide.relay_set_inbox('sensor-readings', config => '{
    "inbox":  "sensor_inbox",
    "source": {"type":"kafka","brokers":"...","topic":"iot.sensors"}
}'::jsonb);
```

### Step 3 — Migrate outbox setup

Old (called once at setup time):
```sql
SELECT pgtrickle.enable_outbox('enriched_events');
```

New — two options:

**Option A: Use pg_tide's outbox directly (for custom bridge triggers):**
```sql
-- Create the outbox in pg_tide
SELECT tide.outbox_create('enriched-events', 24, 10000);

-- Change the bridge trigger to publish via tide.outbox_publish()
CREATE OR REPLACE FUNCTION _pg_ripple.bridge_to_tide_outbox()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    PERFORM tide.outbox_publish(
        'enriched-events',
        jsonb_build_object(
            'subject',   pg_ripple.decode_id(NEW.s),
            'predicate', pg_ripple.decode_id(TG_ARGV[0]::bigint),
            'object',    pg_ripple.decode_id(NEW.o),
            'graph',     pg_ripple.decode_id(NEW.g)
        ),
        '{}'::jsonb
    );
    RETURN NEW;
END;
$$;
```

**Option B: Use `pgtrickle.attach_outbox()` for SPARQL view stream tables:**
```sql
-- Only for pg_trickle stream tables (IVM-backed views):
SELECT pgtrickle.attach_outbox('pg_ripple.enriched_alerts', retention_hours => 24);
```

### Step 4 — Replace the relay binary

```bash
# Old
pgtrickle-relay --postgres-url "$PG_URL"

# New
pg-tide-relay --postgres-url "$PG_URL"
```

The binary has the same flags and configuration file format; only the name changed.

### Step 5 — Verify

```sql
-- Check pg_tide outbox status
SELECT tide.outbox_status('enriched-events');

-- Check relay pipeline configs are visible to the relay
SELECT name, enabled, config FROM tide.relay_outbox_config;
SELECT name, enabled, config FROM tide.relay_inbox_config;

-- Check consumer lag
SELECT * FROM tide.consumer_lag;
```

---

## Extension Dependency Declaration

pg_ripple should not hard-declare `pg_tide` or `pg_trickle` as required extensions
in `pg_ripple.control` because:
1. Core triple store functionality works without either.
2. Both are optional integrations gated at runtime.
3. Declaring them required would break installs that do not need IVM or relay.

The correct pattern is **soft detection** at runtime:
- `has_pg_trickle()` — gates SPARQL view creation
- `has_pg_tide()` — gates relay pipeline configuration helpers (new, TIDE-1)

Relevant section of `pg_ripple.control` (no change needed to `requires` field):
```
# pg_ripple.control
requires = ''   # no hard dependency on pg_trickle or pg_tide
```

---

## New pg_ripple Functions to Consider (Optional, Future Work)

These are convenience wrappers that could be added to pg_ripple to simplify
hub-and-spoke setup for users who do not want to call pg_tide directly:

### `pg_ripple.create_outbox_pipeline(name, outbox_name, sink_config)` (future)

Combines:
1. `tide.outbox_create(outbox_name, ...)`
2. `tide.relay_set_outbox(name, ...)`
3. Installing a CDC bridge trigger that calls `tide.outbox_publish()`

### `pg_ripple.create_inbox_pipeline(name, inbox_table, source_config)` (future)

Combines:
1. Creating the inbox target table
2. `tide.relay_set_inbox(name, ...)`
3. Installing the dispatch trigger

These wrappers would require pg_tide to be installed and would call `has_pg_tide()`
before proceeding, failing gracefully with a helpful error if pg_tide is absent.

---

## Test Coverage Required

### Regression tests (new SQL files)

- `tests/pg_regress/sql/pg_tide_detection.sql`: `pg_ripple.pg_tide_available()` returns `false` when pg_tide is not installed (since the regress suite runs without pg_tide).
- No changes to existing regression tests since none call the old `pgtrickle.enable_outbox()` etc.

### Integration tests (manual / CI with pg_tide)

The existing CI does not install pg_tide. A new CI job or docker-compose service
should be added that:
1. Installs both `pg_trickle ≥ 0.46.0` and `pg_tide ≥ 0.1.0`
2. Runs through the hub-and-spoke example with the new API
3. Verifies `tide.outbox_pending` shows expected rows after triple inserts
4. Verifies `pg-tide-relay` reads configs from `tide.*`

---

## Release Planning

These changes should be bundled into a single pg_ripple version because they affect
the public documentation and operator instructions as a unit.

Suggested version: **v0.93.0** — “pg_tide integration & documentation modernisation”

Migration script: `sql/pg_ripple--0.92.0--0.93.0.sql` (schema-only comment, no DDL changes):
```sql
-- Migration 0.92.0 → 0.93.0: pg_tide integration
-- Schema changes: None
-- Notes:
--   - pg_tide v0.1.0 (trickle-labs/pg-tide) is now the required extension for
--     relay, outbox, and inbox features (previously provided by pg_trickle).
--   - pg_trickle ≥ 0.46.0 retains IVM/stream-table functionality.
--   - Install pg_tide before using relay-dependent features:
--       CREATE EXTENSION pg_tide;
--   - Replace pgtrickle-relay with pg-tide-relay in your deployment.
```

---

## Summary of All Files to Change

### Rust source

| File | Change | Priority |
|---|---|---|
| `src/lib.rs` | Add `has_pg_tide()`, `PG_TIDE_TESTED_VERSION` | P0 |
| `src/bidi/mod.rs` | Update BIDI-OUTBOX-01 / BIDI-INBOX-01 doc comments | P0 |
| `src/views/mod.rs` | Add `PGTIDE_HINT` constant | P1 |

### SQL migration

| File | Change | Priority |
|---|---|---|
| `sql/pg_ripple--0.92.0--0.93.0.sql` | Create (comment-only migration) | P0 |

### Documentation

| File | Change | Priority |
|---|---|---|
| `docs/src/operations/pg-trickle-relay.md` | Full API update (20+ call sites) | P0 |
| `docs/src/operations/compatibility.md` | Add pg_tide rows to compat table | P0 |
| `blog/semantic-hub-trickle-relay.md` | Update 4 API call sites | P1 |
| `plans/pg_trickle_relay_integration.md` | Add backward-compat header | P1 |
| `roadmap/v0.52.0.md` | Add inline notes on 2 call sites | P2 |
| `roadmap/v0.77.0-full.md` | Add inline note on 1 call site | P2 |

### Changelog

| File | Change | Priority |
|---|---|---|
| `CHANGELOG.md` | Add v0.92.0 entry describing pg_tide integration | P0 |

---

## What Does NOT Change

The following are unaffected by the pg_tide split:

- **Core triple store**: VP tables, dictionary encoding, SPARQL→SQL translation — no change.
- **SPARQL views**: `create_sparql_view()`, `drop_sparql_view()` — still use `pgtrickle.create_stream_table()`, no change.
- **Datalog views**: same as SPARQL views.
- **CONSTRUCT/DESCRIBE/ASK views**: same.
- **`has_pg_trickle()`**: still correct; IVM gating is unchanged.
- **`pg_ripple_http`**: no relay-related code; no changes needed.
- **Regression test suite**: all 242 existing tests pass unchanged (none test relay/outbox).
- **HTAP storage, merge workers, federation, SHACL, Datalog engine**: no change.
- **Citus integration**: NOTIFY signals to pg_trickle (`merge_start`, `merge_end`) are IVM
  concerns and stay in pg_trickle; unaffected.
