# pg_ripple Documentation Style Guide

> This guide applies to all pages under `docs/src/`. Generated reference pages
> (GUC tables, SQL function lists, HTTP endpoint inventory) are exempt from
> prose-style rules but must follow heading and code-fence conventions.

## 1. Guiding Principles

- **Current behaviour first.** Describe what is true now. Put version history in
  a note or a "Since" line at the bottom of the section.
- **Operational framing.** Write for a practitioner setting up or debugging a
  deployment, not a researcher reading a design document.
- **No false precision.** Do not claim exact throughput numbers unless a
  benchmark file or CI result is linked.
- **Short is good.** A reader who has to scroll past 200 lines of background
  before reaching the first SQL example will give up.

---

## 2. Page Types and Responsibilities

| Type | Purpose | Lives in |
|---|---|---|
| Feature page | Explains a concept and its primary workflow | `docs/src/features/` |
| Reference page | Enumerates exact APIs, signatures, GUCs, limits, error codes | `docs/src/reference/` |
| Guide / walkthrough | End-to-end task with prerequisites and decision points | `docs/src/guides/` |
| Cookbook recipe | Solves exactly one concrete problem, minimal detours | `docs/src/cookbook/` |
| Operations page | Deployment, failure modes, and operational tuning | `docs/src/operations/` |
| Getting-started page | Short path to success for a first-time reader | `docs/src/getting-started/` |

**Do not duplicate content across page types.** Instead, link between them.
A cookbook recipe may reference a feature page for background but must not
re-explain concepts at length.

---

## 3. Extension and Companion Names

| Name | What it refers to | Not used for |
|---|---|---|
| `pg_ripple` | The PostgreSQL extension itself | The HTTP companion |
| `pg_ripple_http` | The standalone HTTP/Flight companion service | The PG extension |
| `pg_trickle` | IVM-backed views, ExtVP, SHACL DAG monitors, live statistics | Relay or CDC delivery |
| `pg_tide` | Relay/outbox/inbox transport, CDC delivery bridge | IVM views |

**Use `pg_trickle` only** when the code path explicitly requires
`pg_ripple.trickle_integration = on` or calls into the pg_trickle extension.

**Use `pg_tide` only** when the code path involves the outbox/inbox relay
protocol or the `pg_tide_available()` function guard.

---

## 4. SQL Formatting

- Use `` sql `` code fences for SQL examples.
- Use `postgresql` fences only for PostgreSQL-dialect-specific content (e.g.
  `EXPLAIN` output, `pg_dump` invocations).
- Qualify function names with the schema on first mention:
  `pg_ripple.sparql_select(...)`, not `sparql_select(...)`.
- Use lowercase SQL keywords (`select`, `insert`, `where`, `from`) in examples
  and uppercase only when quoting PostgreSQL documentation or explaining
  parsing rules.
- Mark examples that cannot run without external services:

  ```sql
  -- requires: pg_tide (relay), pg_trickle (IVM), pgvector, PostGIS
  ```

---

## 5. Code-Block Labels

Every code block should be one of:

| Label | Meaning | How to mark |
|---|---|---|
| Executable | Runs against a fresh pg_ripple DB | No special annotation needed — assumed unless marked otherwise |
| Illustrative | Conceptual; may use ellipsis or placeholder values | Comment `-- illustrative` as the first line |
| Pseudo-output | Shows expected terminal or query output | Use a plain text or `text` fence |

---

## 6. Callouts

Use mdbook-admonish syntax for callouts. Prefer these four:

```
> **Note**: short informational aside.
```

```admonish note
Short informational aside.
```

```admonish warning
Something the reader might do wrong.
```

```admonish tip
Shortcut or best practice.
```

```admonish danger
Data-loss or security risk.
```

Do not use `> **Warning:**` or `> **Note:**` raw blockquotes when
`mdbook-admonish` is available (CI uses it; local builds degrade gracefully).

---

## 7. Headings and Structure

- `#` is reserved for the page title only.
- Use `##` for major sections and `###` for subsections.
- Do not skip heading levels.
- Keep heading text sentence-case: "SPARQL query execution" not "SPARQL Query Execution".

---

## 8. Links

- Use relative links between pages in `docs/src/`.
- Do not use relative paths from `docs/src/` to top-level `blog/` or `plans/`
  directories — use absolute GitHub URLs instead:
  `https://github.com/trickle-labs/pg-ripple/blob/main/blog/pagerank.md`
- External links are checked only in scheduled CI, not on every PR.

---

## 9. Version References

- For feature availability, use a short "Since v0.X.Y" note at the end of the
  relevant section or in a front-matter-style table.
- Do not write "as of v0.X.Y" in running prose — it dates quickly and is
  confusing once a newer version is out.
- When a feature was removed or replaced, state what replaced it.

---

## 10. Optional Dependencies

Every page that describes a feature requiring an optional extension or service
should include a front-matter-style dependency table at the top:

```
| Dependency | Required? | Notes |
|---|---|---|
| `pg_trickle` | Optional | IVM live-view updates only |
| `pg_tide` | Optional | Relay/outbox delivery only |
| `pgvector` | Optional | Vector hybrid search |
| `PostGIS` | Optional | Geospatial SPARQL functions |
| `pg_ripple_http` | Optional | REST/Arrow Flight access |
| Citus | Optional | Horizontal sharding |
```

---

## 11. PR Checklist

When opening a docs PR, verify:

- [ ] `python3 scripts/check_docs_links.py` exits 0.
- [ ] `python3 scripts/check_docs_summary.py` exits 0.
- [ ] `mdbook build docs` succeeds.
- [ ] New routes are in `docs/src/reference/http-api.md`.
- [ ] New GUCs are in `docs/src/reference/guc-reference.md` and `docs/gucs.md`.
- [ ] New SQL functions are in the SQL reference or explicitly marked internal.
- [ ] Optional-dependency tables are present on feature pages that require them.
- [ ] Code examples follow section 5 labelling conventions.
