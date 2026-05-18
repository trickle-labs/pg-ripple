# Docker Image Pinning Policy (L16-08, v0.117.0)

The `docker-compose.yml` at the repository root pins the `pg_ripple` and
`pg_ripple_http` images to the **current minor version** (e.g., `0.117.0`),
not `latest`.

## Why pin the image tag?

Using `latest` causes silent, untested upgrades every time `docker compose pull`
is run.  This can break deployments when a new release introduces schema changes
or modified SQL semantics.  Pinning to an explicit minor version ensures:

- **Reproducible deployments** — the same image is used across all environments.
- **Safe upgrades** — the image tag is updated deliberately when the operator
  has reviewed the CHANGELOG and tested the upgrade.
- **Audit trail** — the `docker-compose.yml` history in Git records exactly
  which release each environment was running.

## Upgrade procedure

1. Review `CHANGELOG.md` for the target version to understand schema changes.
2. Run `ALTER EXTENSION pg_ripple UPDATE;` against your PostgreSQL instance.
3. Update the `image:` tag in `docker-compose.yml` to the new version.
4. Pull and restart: `docker compose pull && docker compose up -d`.

## Tag convention

Image tags follow the extension version exactly:

```
ghcr.io/trickle-labs/pg_ripple:<major>.<minor>.<patch>
```

Do **not** use `:latest` in production.  The `:latest` tag is published for
convenience in local development only; it tracks the most recent release but
is not guaranteed to be stable.

## Multi-service pinning

Both services in `docker-compose.yml` must be pinned to the same minor version:

```yaml
services:
  pg_ripple:
    image: ghcr.io/trickle-labs/pg_ripple:0.117.0   # ← pin here
  pg_ripple_http:
    image: ghcr.io/trickle-labs/pg_ripple:0.117.0   # ← and here
```

The HTTP companion (`pg_ripple_http`) and the extension must use the same
release to ensure compatibility (see
[Compatibility Matrix](../docs/src/operations/compatibility.md)).
