-- tests/concurrency/confidence_subxact_rollback.sql
-- v0.90.0 TEST-05: Confidence table sub-transaction rollback consistency
--
-- Verifies that rolling back a sub-transaction (SAVEPOINT) that inserted
-- confidence values correctly reverts the confidence entries without
-- corrupting the confidence table or leaving orphaned rows.
--
-- Run with: psql -f tests/concurrency/confidence_subxact_rollback.sql

-- Setup
SELECT pg_ripple.insert_triple(
    '<https://subxact.test/s>',
    '<https://subxact.test/p>',
    '<https://subxact.test/o>'
) AS base_sid;

-- Capture baseline confidence count
SELECT count(*) AS baseline_confidence_count
FROM _pg_ripple.confidence;

-- Test 1: SAVEPOINT + ROLLBACK TO SAVEPOINT
BEGIN;
  SAVEPOINT sp1;

  -- Insert with confidence inside savepoint
  SELECT pg_ripple.load_ntriples_with_confidence($nt$
    <https://subxact.test/s2> <https://subxact.test/p> <https://subxact.test/o2> .
  $nt$, 0.75, 'https://subxact.test/graph');

  -- Count inside savepoint
  SELECT count(*) AS inside_savepoint
  FROM _pg_ripple.confidence;

  -- Roll back the savepoint
  ROLLBACK TO SAVEPOINT sp1;

  -- Count after rollback — should match baseline
  SELECT count(*) AS after_rollback
  FROM _pg_ripple.confidence;

COMMIT;

-- Test 2: Verify the rolled-back confidence values are gone
SELECT count(*) = 0 AS rolled_back_values_gone
FROM _pg_ripple.confidence c
JOIN _pg_ripple.dictionary d ON d.id = (
    SELECT s FROM _pg_ripple.vp_rare
    WHERE p = (SELECT id FROM _pg_ripple.dictionary
               WHERE iri = 'https://subxact.test/p')
    LIMIT 1
)
WHERE d.iri LIKE 'https://subxact.test/s2%';

-- Test 3: Normal commit path still works
BEGIN;
  SAVEPOINT sp2;

  SELECT pg_ripple.load_ntriples_with_confidence($nt$
    <https://subxact.test/s3> <https://subxact.test/p> <https://subxact.test/o3> .
  $nt$, 0.9, 'https://subxact.test/graph');

  RELEASE SAVEPOINT sp2;
COMMIT;

-- After RELEASE + COMMIT, the confidence should persist
SELECT count(*) >= 1 AS committed_confidence_persists
FROM _pg_ripple.confidence;
