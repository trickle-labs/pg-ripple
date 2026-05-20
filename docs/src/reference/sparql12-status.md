# SPARQL 1.2 Property Path Status

This page documents the execution status of all eight SPARQL 1.2 property path
algebra operators as implemented in pg_ripple v0.124.0.

```admonish success title="All operators supported"
As of v0.124.0, pg_ripple executes all eight `PropertyPathExpression` variants
defined by spargebra 0.4.6 (features `sparql-12` + `sep-0006`).  A
PATH-BNODE-01 bug was fixed in this release: sequence paths decomposed by
spargebra into two `GraphPattern::Path` nodes connected by an anonymous blank
node now produce correct INNER JOINs instead of Cartesian products.
```

---

## Operator coverage table

| SPARQL syntax | spargebra variant | SQL strategy | Status | Since |
|---|---|---|---|---|
| `<p>` | `NamedNode` | Direct VP table scan | ✅ Full | v0.5.0 |
| `^p` | `Reverse` | `SELECT o AS s, s AS o` swap | ✅ Full | v0.5.0 |
| `a/b` | `Sequence` | Subquery INNER JOIN on mid-node | ✅ Full | v0.5.0 |
| `a\|b` | `Alternative` | `UNION ALL` | ✅ Full | v0.5.0 |
| `p+` | `OneOrMore` | `WITH RECURSIVE` + `CYCLE` (PG 18) | ✅ Full | v0.5.0 |
| `p*` | `ZeroOrMore` | `WITH RECURSIVE` + `CYCLE` + zero-hop `UNION ALL` | ✅ Full | v0.5.0 |
| `p?` | `ZeroOrOne` | Direct `UNION ALL` with identity row | ✅ Full | v0.5.0 |
| `!(p1\|p2)` | `NegatedPropertySet` | `vp_rare WHERE p NOT IN (...)` | ✅ Full | v0.5.0 |

---

## Bug fix: PATH-BNODE-01 — Sequence paths via blank node join (v0.124.0)

**Root cause.** The spargebra/sparopt optimizer sometimes decomposes a Sequence
path expression such as `hop*/hop` into two separate `GraphPattern::Path` algebra
nodes connected by an anonymous blank node:

```
{ <a> (hop)* _:b0 . _:b0 hop ?x . }
```

Before v0.124.0, the `GraphPattern::Path` translator in `sqlgen.rs` only handled
`TermPattern::Variable` for subject/object binding.  Blank nodes were silently
ignored, so the two path subqueries were joined by a Cartesian product in the SQL
`FROM` clause instead of an `INNER JOIN` on the shared column.  This produced
N × M duplicate rows (e.g. 30 rows instead of 5 for a 5-hop chain).

**Fix.** The path translator now calls `bgp::bind_term()` for both subject and
object, which handles `Variable`, `BlankNode`, `NamedNode`, and `Literal`
uniformly.  When the blank node appears in both path fragments, `Fragment::merge`
adds the join condition (e.g. `_t0.o = _t1.s`).

**Affected patterns (examples).**

| SPARQL path | Decomposed by spargebra | Before fix | After fix |
|---|---|---|---|
| `hop*/hop` | `hop* _:b . _:b hop` | 30 rows (×6) | 5 rows ✅ |
| `hop?/hop` | `hop? _:b . _:b hop` | 10 rows (×5) | 2 rows ✅ |
| `^hop/!hop` | `^hop _:b . _:b !hop` | 6 rows (×2) | 3 rows ✅ |

---

## Regression test coverage

The regression test suite includes two dedicated test files added in v0.124.0:

| File | Tests | Coverage |
|---|---|---|
| `tests/pg_regress/sql/sparql12_property_paths.sql` | 20 | All 8 operators + 5 compound combinations |
| `tests/pg_regress/sql/sparql12_owl_chain_nhop.sql` | 5 | OWL `owl:propertyChainAxiom` n=4 and n=5 hop chains |

All 25 tests pass as of v0.124.0.

---

## Known limitations

| Limitation | Ticket | Workaround |
|---|---|---|
| `GRAPH ?g { path }` with nested recursive CTEs may not propagate graph context through all hops | PATH-G-01 | Use named graph filter: `GRAPH <uri> { path }` |
| Max path depth is controlled by `pg_ripple.max_path_depth` GUC (default 100) | — | Increase GUC for very deep graphs |
