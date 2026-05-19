# Rule-Library Federation

> Added in v0.120.0

Rule-Library Federation lets you publish a named Datalog/SPARQL rule set from one
pg_ripple instance and subscribe to it from another — enabling a rule-library
marketplace pattern across a fleet of pg_ripple deployments.

## Publishing a Rule Library

On the source instance, mark a named rule library as publicly accessible:

```sql
SELECT pg_ripple.publish_rule_library(
  'my-owl-rl-rules',
  'https://ripple-hub.example.com'
);
```

This records the library in `_pg_ripple.rule_library_federation` and makes it
available via the HTTP companion's streaming endpoint.

## Streaming Rules via HTTP

Any client can fetch all rules from a published library as NDJSON:

```bash
curl https://ripple-hub.example.com/rule-libraries/my-owl-rl-rules/stream \
  -H "Authorization: Bearer $TOKEN"
```

Each line in the response is a JSON object:

```json
{"name":"my-owl-rl-rules","rule":"rdfs:subClassOf rule body...","version":"0.120.0"}
```

## Subscribing from Another Instance

On the target instance, subscribe to a remote rule library:

```sql
SELECT pg_ripple.subscribe_rule_library(
  'https://ripple-hub.example.com',
  'my-owl-rl-rules'
);
```

This records the subscription intent. The HTTP companion can then fetch rules:

```bash
curl -X POST https://my-instance.example.com/rule-libraries/my-owl-rl-rules/subscribe \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"source_uri": "https://ripple-hub.example.com/rule-libraries/my-owl-rl-rules/stream"}'
```

## Security

- Publishing requires `check_auth_write` (HTTP write token).
- Subscribing also requires `check_auth_write`.
- The source library endpoint validates authentication via the standard Bearer
  token mechanism.
- HTTPS is strongly recommended for all federation endpoints.

## Environment Variables

| Variable | Description |
|----------|-------------|
| `PG_RIPPLE_HTTP_AUTH_TOKEN` | Bearer token for authenticating requests |
| `PG_RIPPLE_HTTP_URL` | Base URL of the HTTP companion |

## Monitoring

The `GET /admin/diagnostic-snapshot` endpoint includes `rule_library_federation`
table row counts, making it easy to audit which libraries are published or
subscribed on each instance.
