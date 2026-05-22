# Rule Library Federation

This guide walks through a complete end-to-end example of publishing a rule
library on one pg_ripple instance (instance A) and subscribing to it from a
second instance (instance B).

## Prerequisites

- Two running pg_ripple 0.120.0+ instances with pg_ripple_http
- Both HTTP companions accessible from each other over HTTPS
- `PG_RIPPLE_HTTP_AUTH_TOKEN` configured for read access to protected stream
  endpoints. If `PG_RIPPLE_HTTP_DATALOG_WRITE_TOKEN` is set on the subscribing
  instance, use it for the subscribe call; otherwise the main auth token is used.
- The source stream must be reachable by the subscribing HTTP companion. The
  current subscribe handler fetches `source_uri` directly and does not forward a
  separate source Authorization header, so protect private streams with a
  trusted internal network or gateway when source auth is required.

## Step 1 — Install a rule library on instance A

```sql
-- Connect to instance A
\c ripple_a

-- Install the RDFS entailment rule set
SELECT pg_ripple.install_rule_library(
    'rdfs-entailment',
    '1.0.0',
    'RDFS entailment rules: rdfs:subClassOf, rdfs:subPropertyOf, rdfs:domain, rdfs:range'
);

-- Add the core RDFS rules
SELECT pg_ripple.add_rule(
    'rdfs-entailment',
    '?x rdf:type ?C :- ?x rdf:type ?D, ?D rdfs:subClassOf ?C .'
);
SELECT pg_ripple.add_rule(
    'rdfs-entailment',
    '?x ?q ?y :- ?x ?p ?y, ?p rdfs:subPropertyOf ?q .'
);
```

## Step 2 — Publish the library on instance A

```sql
-- Publish via the SQL function (records in the federation catalog)
SELECT pg_ripple.publish_rule_library(
    'rdfs-entailment',
    'https://instance-a.example.com/rule-libraries/rdfs-entailment/stream'
);
```

The HTTP companion immediately begins serving the stream at
`GET /rule-libraries/rdfs-entailment/stream`.

## Step 3 — Subscribe from instance B

```sql
-- Connect to instance B
\c ripple_b

-- Record subscription intent. The SQL function validates the URI and catalog
-- state, but the HTTP companion performs the actual fetch/install step.
SELECT pg_ripple.subscribe_rule_library(
    'https://instance-a.example.com/rule-libraries/rdfs-entailment/stream',
    'rdfs-entailment'
);
-- (void — raises PT046x on error)
```

Alternatively, use the HTTP API from instance B's companion:

```bash
curl -X POST https://instance-b.example.com/rule-libraries/rdfs-entailment/subscribe \
  -H "Authorization: Bearer $PG_RIPPLE_HTTP_AUTH_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"source_uri": "https://instance-a.example.com/rule-libraries/rdfs-entailment/stream"}'
```

## Step 4 — Verify shared inference

Load some triples on instance B and verify that RDFS entailment fires using
the subscribed rules:

```sql
\c ripple_b

-- Load a small ontology fragment
SELECT pg_ripple.load_turtle('
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix ex:   <https://example.org/> .

ex:GraduateStudent rdfs:subClassOf ex:Student .
ex:alice            a ex:GraduateStudent .
');

-- Enable Datalog inference
SELECT pg_ripple.enable_datalog('rdfs-entailment');

-- Verify entailment: alice should now be inferred as a Student
SELECT pg_ripple.sparql_query('
  SELECT ?type WHERE {
    <https://example.org/alice> a ?type .
  }
');
-- Expected: rows include ex:GraduateStudent (asserted) AND ex:Student (inferred)
```

## Step 5 — Monitor with Prometheus

After step 3 the following metrics will be populated on instance B's HTTP
companion:

```
# Stream duration on instance A (cumulative seconds serving stream requests)
pg_ripple_rule_library_stream_duration_seconds 0.012345

# Subscribe errors on instance B (should be 0 for a successful subscription)
pg_ripple_rule_library_subscribe_errors_total 0
```

Add an alert rule for failed subscriptions:

```yaml
- alert: RuleLibrarySubscribeFailed
  expr: increase(pg_ripple_rule_library_subscribe_errors_total[5m]) > 0
  labels:
    severity: warning
  annotations:
    summary: "Rule library subscribe errors detected"
    description: "{{ $value }} subscribe errors in the last 5 minutes."
```

## Troubleshooting

| Symptom | Likely cause | Fix |
|---------|-------------|-----|
| `PT0466: SSRF blocked` | `source_uri` resolves to a private IP | Use a public HTTPS endpoint; private addresses are blocked by the SSRF guard |
| `remote returned HTTP 401` | Source stream requires auth that the subscribing fetcher is not forwarding | Expose the source stream through a trusted internal route, or install the library manually from an authenticated client |
| `PT0467: catalog write failed` | Insufficient DB permissions | Grant USAGE on `_pg_ripple` schema to the app role |
| Inference not firing | Datalog engine not enabled | Run `SELECT pg_ripple.enable_datalog('rdfs-entailment');` |

## See also

- [`publish_rule_library` API reference](../reference/sql-api.md#publish_rule_library)
- [`subscribe_rule_library` API reference](../reference/sql-api.md#subscribe_rule_library)
- [Rule library Prometheus metrics](../operations/read-replicas.md)
- [Blog: Rule Library Federation](https://github.com/trickle-labs/pg-ripple/blob/main/blog/rule-library-federation.md)
