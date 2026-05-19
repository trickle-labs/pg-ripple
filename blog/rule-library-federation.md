# Rule Library Federation: Share Reasoning Rules Across pg_ripple Instances

pg_ripple 0.120.0 introduced *rule library federation* — the ability to publish
a named Datalog/SPARQL rule set on one instance and subscribe to it from
another, using the existing Arrow Flight streaming infrastructure.

This post walks through the federation mechanics, the security model, and the
Prometheus metrics added in v0.123.0 to give operators full visibility into the
health of their federation topology.

## Why federated rule libraries?

In a multi-tenant or multi-region deployment, teams often maintain shared
ontology and inference rules (RDFS entailment, domain-specific constraints,
entity-resolution rules). Without federation, each instance must manually
re-apply rule sets every time they are updated — a fragile, error-prone process.

Rule library federation solves this:

1. **Curating team** maintains rules on the *publisher* instance
2. **Consuming instances** *subscribe* and receive an up-to-date snapshot of
   the rule set
3. Changes are propagated by re-subscribing (or via a scheduled subscription
   refresh)

## Architecture

```
┌─────────────────────┐        HTTPS Arrow-Flight stream
│  Instance A          │ ──────────────────────────────────►  Instance B
│  (publisher)         │    GET /rule-libraries/rdfs/stream   (subscriber)
│                      │
│  _pg_ripple.rules    │                                       _pg_ripple.rules
│  rule_set='rdfs'     │    ◄ NDJSON ({"rule_set","body"}) ►  rule_set='rdfs'
└─────────────────────┘
```

The stream endpoint serialises each rule as a JSON object on a single line.
The subscriber deserialises and inserts rules via `ON CONFLICT DO NOTHING`,
making subscription idempotent.

## Security model

- **Authentication:** the stream endpoint re-uses the Arrow Flight HMAC secret
  (`ARROW_FLIGHT_SECRET`). Unauthenticated requests receive HTTP 401.
- **SSRF protection:** the `subscribe_rule_library()` SQL function and
  `POST /rule-libraries/{name}/subscribe` HTTP endpoint both validate the
  `source_uri` through the SSRF blocklist introduced in v0.121.0. Private
  ranges (RFC 1918, CGNAT, multicast, loopback, IPv4-mapped IPv6) are rejected
  with `PT0466`.
- **Write authorisation:** the subscribe HTTP endpoint requires
  `check_auth_write`, preventing read-only API keys from triggering remote
  fetches.

## Prometheus monitoring (v0.123.0)

Two new metrics were added in v0.123.0 to give operators visibility:

| Metric | Description |
|--------|-------------|
| `pg_ripple_rule_library_stream_duration_seconds` | Cumulative time spent serving stream responses (counter) |
| `pg_ripple_rule_library_subscribe_errors_total` | Total failed subscribe calls — network errors, non-200 responses from the remote (counter) |

Example alert for subscribe failures:

```yaml
- alert: RuleLibrarySubscribeFailed
  expr: increase(pg_ripple_rule_library_subscribe_errors_total[15m]) > 0
  for: 0m
  labels:
    severity: warning
  annotations:
    summary: "Rule library subscribe errors on {{ $labels.instance }}"
    description: "{{ $value }} subscribe errors in the last 15 minutes."
```

## Complete worked example

See the full step-by-step guide:
[Rule Library Federation Guide](../docs/src/guides/rule-library-federation.md)

## What's next

- **v0.126.0** adds per-endpoint federation credentials (OAuth2 Bearer, API key)
  which will extend the rule-library stream endpoint with token-based auth
  beyond the current HMAC approach.
- A future version will add *push-based* federation: the publisher will proactively
  notify subscribers when rules change, rather than requiring a manual re-subscribe.

## See also

- [Rule library federation guide](../docs/src/guides/rule-library-federation.md)
- [SQL API: publish_rule_library, subscribe_rule_library](../docs/src/reference/sql-api.md)
- [Federation circuit breaker](federation-circuit-breaker.md)
