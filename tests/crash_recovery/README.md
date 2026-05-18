# Crash Recovery Tests

This directory contains scripts that verify pg_ripple recovers correctly after
an unclean PostgreSQL shutdown.  Each script uses `pg_ctl -m immediate` to
simulate a crash, then restarts PostgreSQL and asserts that the extension is
fully functional and no data was corrupted.

## PGDATA requirements

All scripts assume a local PostgreSQL 18 data directory managed by `cargo pgrx`.
Before running any crash-recovery test:

1. Start the pgrx-managed cluster:
   ```bash
   cargo pgrx start pg18
   ```
2. Export the data directory location:
   ```bash
   export PGDATA=$(cargo pgrx info pg18 | grep data_dir | awk '{print $2}')
   ```
3. Export the socket directory:
   ```bash
   export PGPORT=28818   # default pgrx port for pg18
   ```

## Invocation pattern

Each script follows the same lifecycle:

```
1. Prepare test data (INSERT triples / run inference / start merge worker)
2. pg_ctl stop -D "$PGDATA" -m immediate   # simulate crash
3. pg_ctl start -D "$PGDATA"               # restart
4. Wait for PostgreSQL to accept connections
5. Run assertion queries via psql
6. Exit non-zero on any assertion failure
```

The `-m immediate` flag sends SIGQUIT to all backend processes and exits
without checkpoint, leaving write-ahead log (WAL) to be replayed on restart.

## Expected outcomes

| Script | What is asserted after recovery |
|--------|---------------------------------|
| `merge_during_kill.sh` | VP main/delta merge is idempotent; no duplicate triples appear after WAL replay |
| `promote_sigkill.sh` | VP table promotion completes or rolls back cleanly; catalog is consistent |
| `dict_during_kill.sh` | Dictionary encode/decode survives mid-transaction crash; no orphaned `hash` entries |
| `merge_kill.sh` | Merge worker restarts and reaches completion within 60 seconds |
| `shacl_during_violation.sh` | SHACL async validator queue clears after restart; no stale violation rows |
| `cdc_slot_cleanup_during_kill.sh` | Replication slot is cleaned up on restart even if the CDC worker was killed mid-drain |
| `pagerank_during_merge.sh` | PageRank computation restarts from last checkpoint; results are consistent |
| `test_construct_view_kill.sh` | CONSTRUCT writeback rule delta is re-derived from scratch; no duplicate derived triples |
| `test_embedding_kill.sh` | Embedding worker resumes from the last processed SID; no embeddings are duplicated |
| `test_federation_spool_kill.sh` | Federation result spool is discarded; next query re-fetches from remote |
| `test_inference_kill.sh` | Datalog derivation table is consistent after recovery; DRed re-runs cleanly |
| `test_parallel_datalog_kill.sh` | Parallel stratum evaluation is atomic per stratum; no half-applied strata |
| `test_promote_kill.sh` | VP promotion lock is released on restart; `_pg_ripple.predicates` catalog is intact |
| `sse_slow_subscriber.sh` | SSE subscriptions are disconnected on crash; no dangling LISTEN sessions |
| `confidence_subxact_rollback.sql` | Confidence sub-transaction rollback: partial confidence updates are reverted |

## Recovery verification steps

After each restart, run the following SQL to confirm the extension is healthy:

```sql
-- 1. Extension is present and version matches expectations.
SELECT extversion FROM pg_extension WHERE extname = 'pg_ripple';

-- 2. Dictionary is internally consistent (no orphaned hash entries).
SELECT COUNT(*) FROM _pg_ripple.dictionary WHERE id IS NULL;

-- 3. VP catalog matches actual table counts.
SELECT name, triple_count FROM _pg_ripple.predicates ORDER BY name LIMIT 10;

-- 4. No stale SHACL validation rows.
SELECT COUNT(*) FROM _pg_ripple.shacl_pending WHERE created_at < now() - interval '1 hour';

-- 5. Datalog rules are intact.
SELECT COUNT(*) FROM _pg_ripple.rules;
```

All queries should return without error.  `COUNT(*) ... WHERE id IS NULL` must
return `0`; other counts depend on the test fixture.
