# Arrow Flight Reference

pg_ripple exposes an [Apache Arrow Flight](https://arrow.apache.org/docs/format/Flight.html)
bulk-export endpoint via the `pg_ripple_http` companion service.

## Endpoint

```
GET /flight/do_get
```

Streams Arrow IPC record batches from VP tables (or a SPARQL SELECT query result) directly to
the client using the Apache Arrow Flight protocol.

## Authentication

Tickets are HMAC-SHA256 signed with an expiry timestamp and a random nonce to prevent replay
attacks. The secret key is configured via `pg_ripple.arrow_flight_secret`. Unsigned tickets are
rejected unless `pg_ripple.arrow_unsigned_tickets_allowed = on` (disabled by default).

## Ticket Format

A valid Arrow Flight ticket is a JSON object:

```json
{
  "query": "SELECT * FROM pg_ripple.sparql_select($1)",
  "exp": 1735689600,
  "nonce": "a1b2c3d4e5f6",
  "sig": "HMAC-SHA256 hex signature"
}
```

The `sig` field is computed over `query + exp + nonce` using the configured secret.

## Streaming Behavior

The endpoint uses `Transfer-Encoding: chunked` HTTP streaming (via `axum::body::Body::from_stream`)
so that clients can begin decoding Arrow IPC record batches before the full export completes.
Response bytes are sent in 64 KiB chunks as the IPC buffer is produced.

**Memory bound**: The Arrow IPC buffer for the entire export is built in memory before streaming
begins. For very large result sets the RSS of `pg_ripple_http` scales with result-set size
(approximately 32 bytes per row in the IPC buffer plus ~200 bytes per row in PostgreSQL client
memory). The recommended upper bound for a single export call is **10 million rows** (RSS ≲ 512 MB
on a host with 1 GB available to the HTTP companion). For larger exports, partition by named graph
or predicate and call the endpoint in batches.

Clients should use streaming reads (e.g., chunked IPC reader) rather than buffering the full
response body.

## Configuration

| Parameter | Default | Description |
|-----------|---------|-------------|
| `pg_ripple.arrow_flight_secret` | — | HMAC secret for ticket signing (required) |
| `pg_ripple.arrow_unsigned_tickets_allowed` | `off` | Allow unsigned tickets (development only) |
| `ARROW_BATCH_SIZE` env var | `1000` | Rows per Arrow IPC record batch |

## Response Headers

| Header | Description |
|--------|-------------|
| `Content-Type` | `application/vnd.apache.arrow.stream` |
| `X-Arrow-Rows` | Total number of triples exported |
| `X-Arrow-Batches` | Number of Arrow IPC record batches sent |
| `Transfer-Encoding` | `chunked` — response is streamed, not buffered |

## Status

Arrow Flight bulk export is **experimental** in v0.71.0. The HMAC-SHA256 signing,
expiry and nonce checking are fully implemented (v0.67.0 FLIGHT-SEC-02). Chunked HTTP
streaming via `Body::from_stream` is confirmed and validated (v0.71.0 FLIGHT-STREAM-01).

---

## v1 → v2 HMAC Ticket Migration (L16-05)

pg_ripple v0.72.0 introduced **v2 tickets** with a nonce-based replay-protection
cache (`FLIGHT-NONCE-01`).  Older v1 tickets (pre-v0.72.0) lack a `nonce` field
and are therefore accepted only when `arrow_unsigned_tickets_allowed = on`.

### What constitutes a valid v1 ticket

A **v1 ticket** is any ticket JSON that:
- Contains `query` and `exp` fields.
- **Does not** contain a `nonce` field.
- Has a valid HMAC-SHA256 `sig` computed over `query + exp` (no nonce in the signed payload).

v1 tickets are not subject to replay protection because the nonce cache does not
apply to them.  They are therefore **inherently replayable** until expiry.

### Migration path

1. **Generate new v2 tickets** — include a random `nonce` (hex string, ≥ 12 bytes of entropy)
   in every new ticket and sign over `query + exp + nonce`.
2. **Transition window** — v2 tickets are accepted immediately.  v1 tickets continue to be
   accepted for the remainder of their `exp` window when the server is running
   `arrow_unsigned_tickets_allowed = on`.  Set this to `off` once all active clients
   have migrated to v2 tickets.
3. **Identify expired v1 tickets** — a v1 ticket is expired when `exp < now()` (Unix timestamp).
   Use the following to check a raw ticket JSON:
   ```bash
   echo '<ticket_json>' | python3 -c "
   import json, sys, time
   t = json.load(sys.stdin)
   exp = t.get('exp', 0)
   print('expired' if exp < time.time() else f'valid until {time.ctime(exp)}')
   print('v1 ticket (no nonce)' if 'nonce' not in t else 'v2 ticket (has nonce)')
   "
   ```
4. **Disable v1 acceptance** — once all tickets in flight have migrated, set
   `ARROW_UNSIGNED_TICKETS_ALLOWED=false` in the `pg_ripple_http` environment and restart.

### Summary

| Feature | v1 ticket | v2 ticket |
|---------|-----------|-----------|
| Replay protection | None (replayable until expiry) | Nonce cache (one-time use) |
| HMAC input | `query + exp` | `query + exp + nonce` |
| `nonce` field | Absent | Present (≥ 12 bytes hex) |
| Server requirement | `arrow_unsigned_tickets_allowed = on` | Always accepted (default) |

See also: [HTTP API](http-api.md), [Architecture](architecture.md), [Compatibility Matrix](../operations/compatibility.md).
