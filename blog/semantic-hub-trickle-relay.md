[← Back to Blog Index](README.md)

# The Semantic Hub: pg_ripple × pg-tide Relay

## Using a knowledge graph as the integration layer between event sources and consumers

> **Note (v0.93.0)**: pg-trickle v0.46.0 extracted the relay, outbox, and inbox subsystem into
> the standalone `pg_tide` extension (`trickle-labs/pg-tide`). After v0.46.0, pg_trickle provides
> IVM only. This post has been updated to reflect the new architecture. Use pg_tide ≥ 0.33.0 for
> all relay examples shown here.

---

You have Kafka topics with order events. NATS subjects with sensor readings. Webhooks from CRM systems. Each speaks a different schema, uses different identifiers for the same entities, and publishes at different cadences.

Somewhere downstream, consumers need a unified view: the same customer across all sources, enriched with inferred relationships, validated against quality rules, and delivered to Kafka, NATS, or webhooks in a schema they understand.

This is the integration hub pattern. Most teams build it with Kafka Connect, schema registries, stream processors, and a lot of YAML. pg_ripple and pg-tide build it inside PostgreSQL.

---

## The Architecture

```
  INBOUND                    SEMANTIC HUB                    OUTBOUND
  ───────                    ────────────                    ────────

  Kafka ──┐                                               ┌── NATS
           │  pg-tide      ┌─────────────┐  pg-tide   │
  NATS  ──┼── relay ──────▶│  pg_ripple   │──── relay ──┼── Kafka
           │  (reverse)     │             │  (forward)   │
  HTTP  ──┘                 │  Inference  │               └── Webhooks
                            │  Validation │
                            │  Resolution │
                            │  CDC events │
                            └─────────────┘
```

Both extensions live in the same PostgreSQL 18 instance. pg-tide handles the transport: its
`pg-tide` relay process speaks Kafka, NATS, SQS, Redis Streams, and HTTP. pg_ripple handles the
semantics — vocabulary alignment, entity resolution, Datalog inference, SHACL validation, and
SPARQL query. pg_trickle (IVM only since v0.46.0) provides incremental materialized view maintenance.

All three share the same transaction context. A Kafka message that arrives via pg-tide's reverse
relay, gets transformed to RDF, triggers Datalog inference, passes SHACL validation, and lands in
pg-tide's outbox for forward relay — all of that can happen within a single PostgreSQL transaction.

---

## Inbound: Sources to Graph

### Step 1: Relay Delivers Events to an Inbox

pg-tide's relay process (`pg-tide`) runs outside PostgreSQL as a lightweight binary. In reverse mode, it consumes from external sources and writes to inbox tables:

```sql
-- Kafka topic → pg_tide inbox
SELECT tide.relay_set_inbox_v2(jsonb_build_object(
  'name',   'order-events',
  'inbox',  'order_inbox',
  'source', 'kafka',
  'config', jsonb_build_object(
    'brokers', 'kafka:9092',
    'topic',   'orders.events'
  )
));
```

Events arrive as JSON rows in `order_inbox`:

```json
{
  "event_id": "kafka:orders.events:0:1234",
  "event_type": "order_created",
  "payload": {
    "order_id": "ORD-5678",
    "customer_id": "CUST-42",
    "total": 299.99,
    "items": [{"sku": "WIDGET-A", "qty": 3}]
  }
}
```

### Step 2: Transform JSON to RDF

A trigger on the inbox transforms each JSON event into RDF triples:

```sql
CREATE OR REPLACE FUNCTION transform_order_to_rdf()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE
  order_iri TEXT;
  ntriples TEXT;
BEGIN
  order_iri := '<https://example.org/order/' || NEW.payload->>'order_id' || '>';

  ntriples := order_iri || ' a <https://schema.org/Order> .' || E'\n'
    || order_iri || ' <https://schema.org/customer> <https://example.org/customer/'
    || NEW.payload->>'customer_id' || '> .' || E'\n'
    || order_iri || ' <https://schema.org/totalPrice> "'
    || (NEW.payload->>'total')
    || '"^^<http://www.w3.org/2001/XMLSchema#decimal> .';

  PERFORM pg_ripple.load_ntriples(ntriples, false);
  RETURN NEW;
END;
$$;

CREATE TRIGGER order_to_rdf
  AFTER INSERT ON order_inbox
  FOR EACH ROW EXECUTE FUNCTION transform_order_to_rdf();
```

Each Kafka message becomes RDF triples in the knowledge graph — inside the same transaction that consumed the message.

### Step 3: Enrich with Inference

Datalog rules derive new facts from the ingested data:

```sql
-- Infer VIP customers from order history
SELECT pg_ripple.datalog_add_rule(
  'vip_customer(C) :- schema_customer(O, C), schema_totalPrice(O, V), V > 10000.'
);

-- Entity resolution: match customers across sources by email
SELECT pg_ripple.datalog_add_rule(
  'owl_sameAs(C1, C2) :- schema_email(C1, E), schema_email(C2, E), C1 != C2.'
);
```

### Step 4: Validate with SHACL

Quality rules catch problems before they propagate downstream:

```turtle
ex:OrderShape a sh:NodeShape ;
  sh:targetClass schema:Order ;
  sh:property [
    sh:path schema:customer ;
    sh:minCount 1 ;
    sh:maxCount 1 ;
    sh:class schema:Customer ;
  ] ;
  sh:property [
    sh:path schema:totalPrice ;
    sh:minCount 1 ;
    sh:datatype xsd:decimal ;
    sh:minExclusive 0 ;
  ] .
```

Orders that fail validation are flagged, not silently ingested.

---

## Outbound: Graph to Consumers

### Step 5: CDC Captures Changes

pg_ripple's CDC subscriptions detect new and changed triples — including inferred ones:

```sql
SELECT pg_ripple.cdc_subscribe(
  name      => 'enriched_orders',
  predicate => 'schema:Order',
  include_inferred => true
);
```

The CDC bridge writes events to a table compatible with pg_trickle's outbox:

```sql
SELECT pg_ripple.cdc_enable_bridge(
  subscription => 'enriched_orders',
  target_table => 'enriched_events'
);
```

### Step 6: Relay Delivers to Consumers

pg-tide's outbox + relay distributes the enriched events:

```sql
-- Create a pg_tide outbox so the relay can poll it.
SELECT tide.outbox_create(
  'enriched-events',
  p_retention_hours  := 24,
  p_inline_threshold := 0
);

-- Enriched data → Kafka
SELECT tide.relay_set_outbox_v2(jsonb_build_object(
  'name',      'enriched-to-kafka',
  'outbox',    'enriched-events',
  'sink_type', 'kafka',
  'config',    jsonb_build_object(
    'brokers', 'kafka:9092',
    'topic',   'enriched.orders'
  )
));

-- Alerts → NATS
SELECT tide.relay_set_outbox_v2(jsonb_build_object(
  'name',      'alerts-to-nats',
  'outbox',    'enriched-events',
  'sink_type', 'nats',
  'config',    jsonb_build_object(
    'url',     'nats://localhost:4222',
    'subject', 'alerts.{event_type}'
  )
));
```

---

## Why This Works

### Zero-Copy Data Flow

Both extensions operate on the same PostgreSQL tables. There's no serialization/deserialization between pg-tide and pg_ripple — the inbox trigger writes triples directly to VP tables. The CDC bridge writes events directly to the outbox table. No intermediate queue, no network hop, no format conversion.

### Transactional Consistency

The entire pipeline — ingest, transform, infer, validate, publish — can run in a single PostgreSQL transaction. If SHACL validation fails, the transaction rolls back. The Kafka offset isn't committed. The message is retried. No partial state, no inconsistency.

### Schema Evolution Without Downtime

When the upstream Kafka schema changes (new field, renamed field), you update the trigger function. When the downstream schema changes, you update the CDC subscription or the outbox format. The RDF graph in the middle absorbs schema differences — that's what ontologies are for.

### Entity Resolution Across Sources

The killer feature: when Kafka, NATS, and webhook sources all refer to the same customer with different IDs, Datalog rules and `owl:sameAs` canonicalization unify them. Downstream consumers see one customer, regardless of how many sources contributed data about them.

---

## The Alternative: Kafka Connect + ksqlDB

The standard integration stack:

| Component | Purpose | Equivalent in pg_ripple + pg-tide |
|-----------|---------|-----------------------------------|
| Kafka Connect | Source/sink connectors | pg-tide relay (reverse/forward) |
| Schema Registry | Schema validation | SHACL shapes |
| ksqlDB | Stream processing | Datalog rules + SPARQL |
| Debezium | CDC from databases | pg_ripple CDC subscriptions |
| Kafka Streams | Entity resolution | owl:sameAs + Datalog |

The pg_ripple stack replaces 5 separate systems with 2 PostgreSQL extensions and a relay binary. The operational complexity difference is significant — one database to back up, one transaction model to reason about, one set of monitoring metrics.

---

## When This Pattern Doesn't Fit

- **Throughput above 100K events/second sustained.** At that volume, a dedicated stream processor (Flink, Kafka Streams) is more appropriate. pg_ripple handles 10–50K events/second comfortably; beyond that, the single-writer PostgreSQL model becomes the bottleneck.

- **No semantic enrichment needed.** If you're just routing events from source to sink with simple transformations, pg-tide alone (without pg_ripple) is simpler. The knowledge graph adds value when you need inference, entity resolution, or vocabulary alignment.

- **Truly global distribution.** PostgreSQL is a single-region system. If sources and consumers span continents and need sub-50ms latency, a globally distributed message bus is the right choice.

For most enterprise integration use cases — connecting 5–20 sources, unifying customer/product/entity identifiers, enforcing data quality, enriching with business rules — the semantic hub pattern inside PostgreSQL is simpler, cheaper, and more correct than the multi-system alternative.
