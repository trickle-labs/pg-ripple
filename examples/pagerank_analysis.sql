-- examples/pagerank_analysis.sql
-- L15-02 (v0.97.0): PageRank computation and analysis demonstration.
--
-- This example demonstrates how to compute PageRank over a knowledge graph,
-- use incremental view maintenance (IVM) for live updates, and integrate
-- PageRank scores with SPARQL hybrid search.
--
-- Prerequisites:
--   - pg_ripple extension installed
--   - pgvector extension installed (for hybrid search section)
--
-- Usage:
--   psql -f examples/pagerank_analysis.sql

-- ── 1. Load a citation graph ──────────────────────────────────────────────────

SELECT pg_ripple.load_turtle($ttl$
  @prefix ex:   <https://example.org/papers/> .
  @prefix cites: <https://example.org/ontology/cites> .
  @prefix dc:   <http://purl.org/dc/elements/1.1/> .

  ex:paper1 dc:title "Foundations of Semantic Web" ;
            cites:cites ex:paper2, ex:paper3 .
  ex:paper2 dc:title "RDF Schema and the Web" ;
            cites:cites ex:paper1, ex:paper4 .
  ex:paper3 dc:title "SPARQL Query Language" ;
            cites:cites ex:paper2 .
  ex:paper4 dc:title "OWL Web Ontology Language" ;
            cites:cites ex:paper1, ex:paper3 .
$ttl$);

-- ── 2. Compute PageRank ───────────────────────────────────────────────────────
-- compute_pagerank() runs the Datalog-native iterative PageRank algorithm.
-- Parameters: predicate IRI, damping factor (default 0.85), max iterations.

SELECT pg_ripple.compute_pagerank(
  'https://example.org/ontology/cites',
  0.85,  -- damping factor
  100    -- max iterations
);

-- ── 3. Query PageRank scores ──────────────────────────────────────────────────

SELECT * FROM pg_ripple.sparql($q$
  PREFIX ex:    <https://example.org/papers/>
  PREFIX dc:    <http://purl.org/dc/elements/1.1/>
  PREFIX pr:    <https://pg-ripple.io/pagerank/>

  SELECT ?paper ?title ?score WHERE {
    ?paper dc:title ?title .
    ?paper pr:score ?score .
  }
  ORDER BY DESC(?score)
$q$);

-- ── 4. Incremental PageRank update (IVM) ─────────────────────────────────────
-- After adding a new citation, recompute only the affected portion.

SELECT pg_ripple.insert_triple(
  'https://example.org/papers/paper5',
  'https://example.org/ontology/cites',
  'https://example.org/papers/paper1'
);

-- Incremental recompute (faster than full recompute for sparse updates)
SELECT pg_ripple.compute_pagerank_incremental('https://example.org/ontology/cites');

-- ── 5. Hybrid SPARQL + PageRank search ───────────────────────────────────────
-- Combine vector similarity with PageRank centrality for ranked retrieval.

SELECT * FROM pg_ripple.sparql($q$
  PREFIX pr:  <https://pg-ripple.io/pagerank/>
  PREFIX dc:  <http://purl.org/dc/elements/1.1/>

  SELECT ?paper ?title ?score WHERE {
    ?paper dc:title ?title .
    ?paper pr:score ?score .
    FILTER(?score > 0.1)
  }
  ORDER BY DESC(?score)
  LIMIT 10
$q$);

-- ── 6. PageRank statistics ────────────────────────────────────────────────────

SELECT
  count(*)   AS scored_nodes,
  avg(score) AS mean_score,
  max(score) AS max_score,
  min(score) AS min_score
FROM pg_ripple.pagerank_scores();
