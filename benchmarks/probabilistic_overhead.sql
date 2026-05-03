-- pg_ripple benchmark: probabilistic Datalog overhead (v0.87.0 CONF-PERF-01a)
--
-- Measures the additional cost of probabilistic Datalog when @weight
-- annotations are present. Run with:
--   pgbench -f benchmarks/probabilistic_overhead.sql -c 4 -j 2 -T 30 pg_ripple_test
--
-- Prerequisites:
--   CREATE EXTENSION IF NOT EXISTS pg_ripple;
--   SELECT pg_ripple.load_rules('bench_prob', '
--     parent(X, Y) :- father(X, Y). @weight(0.9)
--     parent(X, Y) :- mother(X, Y). @weight(0.85)
--     ancestor(X, Z) :- parent(X, Z). @weight(0.9)
--   ');

\set VERBOSITY terse

-- Toggle probabilistic mode for the benchmark
SET pg_ripple.probabilistic_datalog = on;

-- Run a single-stratum inference cycle with probabilistic scoring.
SELECT pg_ripple.run_inference('bench_prob');

-- Reset
SET pg_ripple.probabilistic_datalog = off;

-- ─────────────────────────────────────────────────────────────────────────────
-- CON-02 — Confidence Hot-Row Benchmark (v0.90.0)
--
-- Simulates concurrent noisy-OR writes on the same narrow key range.
-- This exercises confidence table contention: many writers updating the
-- same stmt_id forces ON CONFLICT merges on the same heap page.
--
-- Run with high concurrency to trigger the hot-row scenario:
--   pgbench -f benchmarks/probabilistic_overhead.sql -c 16 -j 8 -T 60 pg_ripple_test
-- ─────────────────────────────────────────────────────────────────────────────

-- Narrow key range (1–100) to force hot-row contention
\set sid random(1, 100)

-- Noisy-OR merge: each concurrent writer contributes an independent evidence source
-- Formula: P(A ∨ B) = 1 - (1 - P(A)) × (1 - P(B))
INSERT INTO _pg_ripple.confidence (stmt_id, confidence)
VALUES (:sid, random())
ON CONFLICT (stmt_id) DO UPDATE
  SET confidence = 1.0 - (1.0 - _pg_ripple.confidence.confidence)
                       * (1.0 - excluded.confidence),
      updated_at = now();

