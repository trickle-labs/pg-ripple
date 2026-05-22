# JSON-LD Reverse Mapping: Writing RDF Changes Back to Relational Tables

One of the most requested features in `pg_ripple` has been **closing the loop** between
RDF knowledge-graph mutations and the underlying relational tables that originally sourced
that data. Version 0.128.0 delivers exactly this with the **JSON-LD Reverse Mapping**
(relational writeback) feature.

## The Round-Trip Problem

Before v0.128.0, `register_json_mapping` was already a powerful bidirectional bridge:

```
REST API payload (JSON)
    │
    ▼  ingest_json()
_pg_ripple VP tables (RDF triples)
    │
    ▼  export_json_node()
JSON document
```

But once SPARQL rules, SHACL validation, or Datalog inference enriched the RDF graph,
there was no automatic way to propagate those changes back into the originating relational
table (e.g. `contacts`, `products`, `orders`). You had to poll the graph and apply changes
yourself.

## Introducing `writeback_json_row()`

v0.128.0 adds a direct relational write-back path:

```sql
-- 1. Configure the mapping with a writeback target.
SELECT pg_ripple.register_json_mapping(
    'contacts',
    '{"contact_id": "http://schema.org/identifier",
      "full_name":  "http://schema.org/name",
      "email_addr": "http://schema.org/email"}'::jsonb
);

UPDATE _pg_ripple.json_mappings
SET writeback_table        = 'contacts',
    writeback_schema       = 'public',
    writeback_key_columns  = ARRAY['contact_id'],
    writeback_conflict_policy = 'replace'
WHERE name = 'contacts';

-- 2. Ingest a JSON payload.
SELECT pg_ripple.ingest_json(
    '{"contact_id": "c001", "full_name": "Alice Smith", "email_addr": "alice@example.com"}'::jsonb,
    'https://example.com/contacts/c001',
    'contacts'
);

-- 3. Run SPARQL/Datalog enrichment...
--    (e.g. a rule adds a canonical email derived from another predicate)

-- 4. Write the enriched graph state back to the relational table.
SELECT pg_ripple.writeback_json_row('contacts', 'https://example.com/contacts/c001');
-- Returns: 1 (rows affected)
```

The function exports the subject as JSON using the mapping context, looks up the
target table's columns from `information_schema`, and executes a parameterised
`INSERT … ON CONFLICT` — never constructing SQL from raw user input.

## Conflict Policies

Three conflict policies control how the writeback handles existing rows:

| Policy | Behaviour |
|--------|-----------|
| `'replace'` (default) | `ON CONFLICT (key_cols) DO UPDATE SET …` — overwrites all non-key columns |
| `'skip'` | `ON CONFLICT DO NOTHING` — leaves existing row unchanged, returns 0 rows |
| `'error'` | Raises `PT0551` when a conflicting row exists — use when you need strict idempotency guarantees |

```sql
-- Skip conflicts silently:
UPDATE _pg_ripple.json_mappings
SET writeback_conflict_policy = 'skip'
WHERE name = 'contacts';

-- Strict mode — error on conflict:
UPDATE _pg_ripple.json_mappings
SET writeback_conflict_policy = 'error'
WHERE name = 'contacts';
```

## Deletes: `writeback_json_row_delete()`

When an RDF subject should be removed from the relational table:

```sql
SELECT pg_ripple.writeback_json_row_delete('contacts', 'https://example.com/contacts/c001');
-- Returns: 1 (rows deleted)
```

This decodes key-column values from the VP tables and executes
`DELETE FROM contacts WHERE contact_id = 'c001'`.

## Trigger-Based Automation: `enable_json_writeback()`

For continuous synchronisation without polling, enable VP delta triggers:

```sql
SELECT pg_ripple.enable_json_writeback('contacts');
```

This installs `AFTER INSERT OR DELETE FOR EACH ROW` triggers on every
`_pg_ripple.vp_*_delta` table whose predicate IRI appears in the mapping context.
Each trigger call enqueues a row in `_pg_ripple.json_writeback_queue`.

The background merge worker drains the queue automatically (controlled by the
`pg_ripple.json_writeback_batch_size` GUC, default 100 rows per tick):

```sql
-- Monitor the queue:
SELECT * FROM pg_ripple.json_writeback_status();
--  mapping_name | pending | errors | last_error | last_processed_at
-- ─────────────┼─────────┼────────┼────────────┼──────────────────
--  contacts     |       0 |      0 | NULL       | 2026-05-22 10:30:00+00
```

To stop the triggers:

```sql
SELECT pg_ripple.disable_json_writeback('contacts');
```

Both `enable_json_writeback()` and `disable_json_writeback()` are idempotent.

## End-to-End Pattern: REST API → RDF → Relational Write-Back

```
┌───────────────────┐
│  REST API client  │
│  POST /contacts   │
└────────┬──────────┘
         │ JSON payload
         ▼
  pg_ripple.ingest_json()
         │ RDF triples
         ▼
  _pg_ripple VP tables
         │
         │ SPARQL / Datalog rules enrich the graph
         │ (e.g. canonical IRI derivation, deduplication)
         ▼
  pg_ripple.writeback_json_row()   ← direct call
  OR
  json_writeback_queue             ← trigger-based async
         │
         ▼
  contacts (relational table)
```

## Security

`writeback_json_row()` never builds SQL strings from user-supplied table or column
names. It resolves the target table OID via PostgreSQL's `quote_ident()` and fetches
column metadata from `information_schema`. All data values are passed as
parameterised `$N` arguments — eliminating SQL injection risk.

## Backward Compatibility

All changes are additive. The five new columns on `_pg_ripple.json_mappings`
(`writeback_table`, `writeback_schema`, `writeback_key_columns`,
`writeback_conflict_policy`, `writeback_enabled`) default to NULL/`'public'`/`{}`/
`'replace'`/`false` respectively. Existing `register_json_mapping()` call sites
require no modification.

The migration script `sql/pg_ripple--0.127.0--0.128.0.sql` applies the `ALTER TABLE`
and `CREATE TABLE` changes automatically when running `ALTER EXTENSION pg_ripple UPDATE`.
