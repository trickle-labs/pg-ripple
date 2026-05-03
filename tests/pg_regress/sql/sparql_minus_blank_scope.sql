-- pg_regress test: SPARQL MINUS blank-node scope (v0.90.0 CB-05)
--
-- Ensures MINUS correctly handles blank-node scoping.
-- Per SPARQL 1.1 §18.6 (compatible mappings), blank nodes in the MINUS
-- operand are existentially quantified independently from the outer pattern.

-- Setup: insert test triples using a dedicated namespace to avoid interference.
SELECT pg_ripple.load_ntriples(
    '<https://minus.test/x> <https://minus.test/a> <https://minus.test/val1> .' || E'\n' ||
    '<https://minus.test/x> <https://minus.test/b> <https://minus.test/val2> .' || E'\n' ||
    '<https://minus.test/y> <https://minus.test/a> <https://minus.test/val3> .'
) = 3 AS three_triples_loaded;

-- Test 1: MINUS with blank nodes excludes nodes with the MINUS predicate.
-- <minus.test/x> has both :a and :b predicates so is excluded by MINUS.
-- <minus.test/y> has only :a so is included. Expected: 1 row.
SELECT COUNT(*) = 1 AS minus_blank_scope_count
FROM pg_ripple.sparql($$
  SELECT ?x WHERE {
    ?x <https://minus.test/a> [] .
    MINUS { ?x <https://minus.test/b> [] . }
  }
$$);

-- Test 2: MINUS with named variables correctly excludes by binding.
SELECT COUNT(*) = 1 AS minus_named_var_count
FROM pg_ripple.sparql($$
  SELECT ?x WHERE {
    ?x <https://minus.test/a> ?v .
    MINUS { ?x <https://minus.test/b> ?w . }
  }
$$);

-- Test 3: Without MINUS, both :a nodes are returned.
SELECT COUNT(*) = 2 AS without_minus_returns_both
FROM pg_ripple.sparql($$
  SELECT ?x WHERE {
    ?x <https://minus.test/a> ?v .
  }
$$);
