# Rule-Library Federation Recipe

> Added in v0.120.0

Use this recipe when you already have a versioned rule library and want to share
it across pg_ripple instances. For the full walkthrough, including a two-instance
setup and monitoring examples, see the [Rule Library Federation guide](../guides/rule-library-federation.md).

## When to Use This

Rule-library federation fits teams that maintain shared Datalog or SHACL rule
sets, such as RDFS entailment, customer deduplication, or compliance checks, and
need the same rules installed consistently across environments.

## Minimal Flow

1. Install or create the library on the source instance.
2. Publish the library with the URL where the HTTP companion serves its stream.
3. Record a subscription on the target instance.
4. Ask the target HTTP companion to fetch and install the remote stream.

```sql
-- Source instance: publish an installed library.
SELECT pg_ripple.publish_rule_library(
  'rdfs-entailment',
  'https://instance-a.example.com/rule-libraries/rdfs-entailment/stream'
);

-- Target instance: record subscription intent.
SELECT pg_ripple.subscribe_rule_library(
  'https://instance-a.example.com/rule-libraries/rdfs-entailment/stream',
  'rdfs-entailment'
);
```

```bash
# Target HTTP companion: fetch and install the stream.
curl -X POST https://instance-b.example.com/rule-libraries/rdfs-entailment/subscribe \
  -H "Authorization: Bearer $PG_RIPPLE_HTTP_AUTH_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"source_uri":"https://instance-a.example.com/rule-libraries/rdfs-entailment/stream"}'
```

## Notes

- `publish_rule_library()` and `subscribe_rule_library()` are catalog operations;
  they do not start a background daemon.
- `POST /rule-libraries/{name}/subscribe` performs the remote fetch and installs
  rules into the target instance.
- The current HTTP fetcher does not forward a separate Authorization header to
  the source stream. For protected source streams, use a trusted internal route
  or fetch from an authenticated client.

## See Also

- [Rule Library Federation guide](../guides/rule-library-federation.md)
- [SQL API: publish_rule_library](../reference/sql-api.md#publish_rule_library)
- [SQL API: subscribe_rule_library](../reference/sql-api.md#subscribe_rule_library)
- [HTTP API reference](../reference/http-api.md#complete-endpoint-reference)
