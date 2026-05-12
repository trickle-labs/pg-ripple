# PPRL: Cross-Organization Entity Resolution Without Sharing Raw PII

> **What you'll learn:** How to use `bloom_encode()` and `pg:dice_similarity` to find
> records that refer to the same real-world person across two organizations —
> without either side ever seeing the other's raw personal data.

## Why This Matters

Hospitals, banks, and government agencies often need to know whether their records
overlap — the same patient, account holder, or citizen appearing in both systems.
Regulations (GDPR, HIPAA, CCPA) typically prohibit sharing raw identifiers like
full names, dates of birth, or Social Security Numbers.

**Privacy-Preserving Record Linkage (PPRL)** solves this using Bloom-filter
encoding: sensitive identifiers are converted into fixed-length bit vectors that
support similarity comparison without revealing the underlying values.

> **Reference:** Schnell, Bachteler & Reiher (2009) — "Privacy-preserving record
> linkage using Bloom filters." *BMC Medical Informatics and Decision Making* 9:41.

## Architecture

```
Organization A                         Organization B
──────────────                         ──────────────
Raw PII → bloom_encode() → bloomA      Raw PII → bloom_encode() → bloomB
          |                                        |
          └──── RDF triple store ─────────────────┘
                     |
              SPARQL SERVICE federation
                     |
              FILTER(pg:dice_similarity(?bloomA, ?bloomB) > 0.85)
                     |
              Candidate matches (no raw PII exchanged)
```

Both organizations use the **same shared secret key** (established out-of-band
via a secure channel — e.g., TLS-protected key agreement) so that Bloom encodings
of the same value from both sides are identical.

## Step 1: Encode Patient Identifiers

Each organization runs this on their own database. The shared key must match
exactly between organizations.

```sql
-- Enable the extension
CREATE EXTENSION IF NOT EXISTS pg_ripple;

-- Encode a patient record. The 'shared_secret_key' must be agreed upon
-- out-of-band between the two organizations.
INSERT INTO patient_triples (subject, predicate, object)
VALUES (
    '<http://hospitalA.example.org/patient/1001>',
    '<http://pprl.example.org/nameBloom>',
    pg_ripple.bloom_encode(
        'Alice M Smith',          -- concatenate relevant fields
        'shared_secret_key',      -- must match Organization B's key
        30,                       -- hash_count (minimum recommended: 30)
        1024                      -- length in bits (minimum recommended: 1024)
    )
);

-- Store a full patient record as RDF triples
SELECT pg_ripple.insert_triple(
    '<http://hospitalA.example.org/patient/1001>',
    '<http://pprl.example.org/nameBloom>',
    pg_ripple.bloom_encode('Alice M Smith 1985-04-12', 'shared_secret_key')
);
```

## Step 2: Run Cross-Organization Matching

Use SPARQL `SERVICE` federation to query both organizations simultaneously.
The SPARQL `pg:dice_similarity` FILTER compares the Bloom filters without
retrieving the underlying PII.

```sparql
PREFIX pprl: <http://pprl.example.org/>
PREFIX pg:   <http://pg-ripple.org/functions/>

SELECT ?patientA ?patientB ?sim WHERE {
  -- Hospital A's Bloom-encoded identifiers (local graph)
  ?patientA pprl:nameBloom ?bloomA .

  -- Hospital B's Bloom-encoded identifiers (remote SPARQL endpoint)
  SERVICE <http://hospitalB.example.org/sparql> {
    ?patientB pprl:nameBloom ?bloomB .
  }

  -- Find candidates where Bloom filters are highly similar
  FILTER(pg:dice_similarity(?bloomA, ?bloomB) > 0.85)

  -- Compute exact similarity for ranking
  BIND(pg:dice_similarity(?bloomA, ?bloomB) AS ?sim)
}
ORDER BY DESC(?sim)
```

From SQL:

```sql
SELECT * FROM pg_ripple.sparql($$
    PREFIX pprl: <http://pprl.example.org/>
    PREFIX pg:   <http://pg-ripple.org/functions/>

    SELECT ?patientA ?patientB ?sim WHERE {
      ?patientA pprl:nameBloom ?bloomA .
      SERVICE <http://hospitalB.example.org/sparql> {
        ?patientB pprl:nameBloom ?bloomB .
      }
      FILTER(pg:dice_similarity(?bloomA, ?bloomB) > 0.85)
      BIND(pg:dice_similarity(?bloomA, ?bloomB) AS ?sim)
    }
    ORDER BY DESC(?sim)
$$);
```

## Step 3: Aggregate Reporting with Differential Privacy

If you only need aggregate statistics (e.g., "approximately how many shared
patients do the two organizations have?") without exposing individual identities,
use `dp_noisy_count()`:

```sql
-- Count shared patients with ε=0.1 differential privacy
SELECT pg_ripple.dp_noisy_count(
    'SELECT COUNT(DISTINCT subject) FROM _pg_ripple.vp_rare
     WHERE predicate = pg_ripple.encode_iri(''http://pprl.example.org/nameBloom'')',
    0.1  -- epsilon: smaller = more privacy, more noise
) AS approx_shared_patients;
```

For a histogram of match confidence buckets:

```sql
CREATE TEMP TABLE match_buckets (bucket TEXT, n BIGINT);
INSERT INTO match_buckets VALUES
    ('high (>0.9)',    42),
    ('medium (0.7-0.9)', 17),
    ('low (0.5-0.7)',  8);

SELECT * FROM pg_ripple.dp_noisy_histogram(
    'SELECT bucket, n FROM match_buckets',
    'bucket',
    'n',
    0.5  -- epsilon
);
```

## Step 4: Datalog Rules for Entity Resolution

You can also express the matching as a Datalog rule and use incremental
materialization:

```sql
SELECT pg_ripple.load_rules($$
    -- Generate candidate pairs whose Bloom filters are similar
    pprl:candidate(?x, ?y) :-
        ?x <http://pprl.example.org/nameBloom> ?bx .
        ?y <http://pprl.example.org/nameBloom> ?by .
        ?x != ?y .
        pg:dice_similarity(?bx, ?by) > 0.85 .
$$);

-- Run inference to materialize all candidate pairs
SELECT pg_ripple.infer();
```

## Security Notes

### Recommended Parameters

| Parameter | Minimum Recommended | Default |
|-----------|--------------------:|--------:|
| `hash_count` | 30 | 30 |
| `length` | 1024 bits | 1024 |

Using fewer than 30 hash functions or fewer than 1024 bits significantly
increases the risk of re-identification through graph-based attacks
(Schnell et al. 2009, §4). pg_ripple logs a WARNING when parameters fall
below the recommended minimums.

### Key Management

The shared secret key (`key` parameter) must be:
1. **Agreed out-of-band** — never transmitted with the Bloom-encoded data
2. **Kept secret** — compromised keys allow the encoded data to be reversed
3. **Organization-pair-specific** — use a different key for each pair of
   organizations to limit blast radius

### What the Bloom Filter Protects Against

- **Honest-but-curious adversary**: Cannot infer the original value from the
  bit vector alone (without the key)
- **Eavesdropper**: Sees only bit vectors; without the key, values cannot be
  recovered

### Known Limitations

- Bloom filters with low `hash_count` or small `length` are vulnerable to
  graph-based reconstruction attacks even without the key
- High-frequency values (e.g., very common names) are more susceptible to
  inference attacks
- CLK encodings are not quantum-resistant

### Patent Status

The CLK (Cryptographic Longterm Key) construction described in Schnell et al.
(2009) is published academic work with no known patent encumbrances.

## Verifying Your Encoding

```sql
-- Self-test: encoding the same value twice should give Dice = 1.0
SELECT pg_ripple.dice_similarity(
    pg_ripple.bloom_encode('Alice M Smith', 'testkey', 30, 1024),
    pg_ripple.bloom_encode('Alice M Smith', 'testkey', 30, 1024)
) AS should_be_one;

-- Encoding different values should give Dice < 1.0
SELECT pg_ripple.dice_similarity(
    pg_ripple.bloom_encode('Alice M Smith', 'testkey', 30, 1024),
    pg_ripple.bloom_encode('Bob J Jones',   'testkey', 30, 1024)
) AS should_be_less_than_half;
```

## Further Reading

- Schnell, R., Bachteler, T., & Reiher, J. (2009). Privacy-preserving record
  linkage using Bloom filters. *BMC Medical Informatics and Decision Making*, 9, 41.
  <https://doi.org/10.1186/1472-6947-9-41>
- Christen, P., Ranbaduge, T., & Schnell, R. (2020). *Linking Sensitive Data:
  Methods and Techniques for Practical Privacy-Preserving Information Sharing*.
  Springer.
- pg_ripple SPARQL federation guide: [federation-wikidata.md](federation-wikidata.md)
- pg_ripple differential privacy functions: `pg_ripple.dp_noisy_count()`,
  `pg_ripple.dp_noisy_histogram()`
