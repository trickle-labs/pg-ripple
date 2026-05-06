# Migration Report: grove/pg-ripple → trickle-labs/pg-ripple

**Prepared**: 2026-05-06  
**Scope**: Everything required to migrate the pg-ripple project from the
`grove` GitHub organisation to the `trickle-labs` GitHub organisation.

---

## 1. Executive Summary

Moving a repository between GitHub organisations is a supported first-class
operation ("Transfer repository"). GitHub preserves the full Git history,
issues, pull requests, projects, milestones, and labels, and it automatically
creates HTTP redirects from the old URLs to the new ones for **web traffic**.

However, many artefacts in this repository contain hard-coded references to
the old organisation name (`grove`). Those references must be updated in code
before the move so that CI, container images, Helm charts, and documentation
work correctly under the new org immediately after the transfer.

The table below counts the affected files by category:

| Category | # Files |
|---|---|
| Cargo manifests | 2 |
| Dockerfiles / OCI labels | 3 |
| Docker Compose | 1 |
| GitHub Actions workflows | 2 |
| GitHub meta-files (CODEOWNERS, dependabot.yml) | 2 |
| Copilot agent skill files | 3 |
| Helm chart | 3 |
| Documentation source | 12 |
| README | 1 |
| Kubernetes example manifests | 1 |
| `justfile` | 1 |
| SBOM / CycloneDX files (generated) | 4 |
| Git remote (local workspace) | 1 |

---

## 2. Prerequisites (Before the Transfer)

Complete the following steps **before** initiating the GitHub transfer so that
the codebase is already correct when it lands in the new org.

### 2.1 Create the `trickle-labs` org on GitHub (if not already done)

Ensure the organisation `trickle-labs` exists at `https://github.com/trickle-labs`.

### 2.2 Create GitHub Teams in the new org

CODEOWNERS and dependabot.yml reference three teams that must exist in the
new org:

| Old team | New team to create |
|---|---|
| `@grove/pg-ripple-maintainers` | `@trickle-labs/pg-ripple-maintainers` |
| `@grove/pg-ripple-rust` | `@trickle-labs/pg-ripple-rust` |
| `@grove/pg-ripple-infra` | `@trickle-labs/pg-ripple-infra` |

Add the same members to each team as existed in the `grove` org.

### 2.3 Enable GitHub Container Registry on the new org

The release workflow publishes two container images:
- `ghcr.io/trickle-labs/pg-ripple:<version>`
- `ghcr.io/trickle-labs/pg-ripple-http:<version>`

GHCR is tied to the organisation. Ensure GHCR is enabled in the `trickle-labs`
org settings before the first release runs.

### 2.4 Configure Repository Secrets

After the transfer, the following secrets must be re-configured in the new
repository:

| Secret | Purpose |
|---|---|
| `CARGO_REGISTRY_TOKEN` | Publishing `pg_ripple_http` to crates.io |

`GITHUB_TOKEN` is automatically available and does not need migration.

---

## 3. Code Changes Required

### 3.1 Cargo Manifests

Both workspace members declare `repository` metadata used by crates.io and
SBOM tooling.

**`Cargo.toml` (line 16)**
```toml
# Before:
repository = "https://github.com/grove/pg-ripple"
# After:
repository = "https://github.com/trickle-labs/pg-ripple"
```

**`pg_ripple_http/Cargo.toml` (line 7)**
```toml
# Before:
repository = "https://github.com/grove/pg-ripple"
# After:
repository = "https://github.com/trickle-labs/pg-ripple"
```

### 3.2 Dockerfiles — OCI Labels and Image References

Three Dockerfiles embed the source repository URL as an OCI label, and contain
comments referencing the published image name.

**`Dockerfile` (lines 16, 137)**
```
# Comment (line 16):
# Before:  ghcr.io/grove/pg-ripple:latest
# After:   ghcr.io/trickle-labs/pg-ripple:latest

# OCI label (line 137):
LABEL org.opencontainers.image.source="https://github.com/grove/pg-ripple"
→ LABEL org.opencontainers.image.source="https://github.com/trickle-labs/pg-ripple"
```

**`Dockerfile.http` (lines 16, 71)**
```
# Comment (line 16):
# Before:  ghcr.io/grove/pg-ripple-http:latest
# After:   ghcr.io/trickle-labs/pg-ripple-http:latest

# OCI label (line 71):
LABEL org.opencontainers.image.source="https://github.com/grove/pg-ripple"
→ LABEL org.opencontainers.image.source="https://github.com/trickle-labs/pg-ripple"
```

**`docker/Dockerfile.cnpg` (lines 13, 17, 104)**
```
# Comments (lines 13, 17):
# Before:  ghcr.io/grove/pg-ripple:0.98.0-cnpg
# After:   ghcr.io/trickle-labs/pg-ripple:0.98.0-cnpg

# OCI label (line 104):
LABEL org.opencontainers.image.source="https://github.com/grove/pg-ripple"
→ LABEL org.opencontainers.image.source="https://github.com/trickle-labs/pg-ripple"
```

### 3.3 docker-compose.yml

**Lines 29 and 50** — two service definitions pin the image:
```yaml
# Before:
image: ghcr.io/grove/pg-ripple:0.99.0
# After:
image: ghcr.io/trickle-labs/pg-ripple:0.99.0
```

### 3.4 GitHub Actions Workflows

#### `.github/workflows/release.yml`

Nine lines reference the old org across two container build/push jobs and one
Trivy security scan:

| Line | Content to update |
|---|---|
| 419 | `images: ghcr.io/grove/pg-ripple` |
| 429 | `org.opencontainers.image.documentation=https://github.com/grove/pg-ripple/...` |
| 445 | `annotation-index.org.opencontainers.image.source=https://github.com/grove/pg-ripple` |
| 473 | `image-ref: ghcr.io/grove/pg-ripple@${{ steps.digest.outputs.digest }}` |
| 506 | `images: ghcr.io/grove/pg-ripple-http` |
| 516 | `org.opencontainers.image.documentation=https://github.com/grove/pg-ripple/...` |
| 529 | `annotation-index.org.opencontainers.image.source=https://github.com/grove/pg-ripple` |
| 551 | `image-ref: ghcr.io/grove/pg-ripple-http@${{ steps.digest.outputs.digest }}` |

All occurrences of `ghcr.io/grove/` → `ghcr.io/trickle-labs/` and
`https://github.com/grove/pg-ripple` → `https://github.com/trickle-labs/pg-ripple`.

#### `.github/workflows/docs.yml`

Three `sed` substitutions (lines 56, 59, 67) inject raw GitHub blob URLs into
the built documentation:
```bash
# Before:
sed -i 's|](roadmap/|](https://github.com/grove/pg-ripple/blob/main/roadmap/|g'
sed -i 's|](plans/|](https://github.com/grove/pg-ripple/blob/main/plans/|g'
sed -i 's|](AGENTS\.md|](https://github.com/grove/pg-ripple/blob/main/AGENTS.md|g'
# After: replace grove/pg-ripple → trickle-labs/pg-ripple
```

### 3.5 GitHub Meta-files

#### `.github/CODEOWNERS`

All team references use the old org prefix. Every `@grove/pg-ripple-*` token
must become `@trickle-labs/pg-ripple-*`. Affected lines: 10, 13–19, 22–24, 27,
30–31, 34, 37, 40–42, 45–46.

Three distinct teams:
- `@grove/pg-ripple-maintainers` → `@trickle-labs/pg-ripple-maintainers`
- `@grove/pg-ripple-rust` → `@trickle-labs/pg-ripple-rust`
- `@grove/pg-ripple-infra` → `@trickle-labs/pg-ripple-infra`

#### `.github/dependabot.yml`

Lines 12 and 38 assign PR reviewers using the old team slug:
```yaml
# Before:
- grove/pg-ripple-maintainers
# After:
- trickle-labs/pg-ripple-maintainers
```

### 3.6 Copilot Agent Skill Files

These files contain documentation and script snippets that embed the old repo
path. They do not affect CI execution but will direct contributors to wrong URLs
if left unchanged.

| File | Change needed |
|---|---|
| `.github/skills/create-pull-request/SKILL.md` | Line 124: PR URL; lines 138–140: team references |
| `.github/skills/fix-ci/SKILL.md` | Line 4: example Actions run URL |
| `.github/skills/implement-version/SKILL.md` | Line 255: `gh issue list -R grove/pg-ripple` (×2) |

### 3.7 Helm Chart

#### `charts/pg_ripple/Chart.yaml`

```yaml
# Before:
home: https://github.com/grove/pg-ripple
sources:
  - https://github.com/grove/pg-ripple
maintainers:
  - name: grove
    url: https://github.com/grove
# After:
home: https://github.com/trickle-labs/pg-ripple
sources:
  - https://github.com/trickle-labs/pg-ripple
maintainers:
  - name: trickle-labs
    url: https://github.com/trickle-labs
```

#### `charts/pg_ripple/values.yaml`

Lines 9 and 64 set the default image repository used when deploying via Helm:
```yaml
# Before:
repository: ghcr.io/grove/pg-ripple
# After:
repository: ghcr.io/trickle-labs/pg-ripple
```

#### `docs/src/operations/kubernetes.md`

The Helm installation command embeds the GitHub Pages URL for the chart repo
(line 18):
```bash
# Before:
helm repo add pg-ripple https://grove.github.io/pg-ripple/charts
# After:
helm repo add pg-ripple https://trickle-labs.github.io/pg-ripple/charts
```
> **Note**: The GitHub Pages site URL is derived from the org name. After the
> repository transfer, GitHub Pages will automatically serve at
> `https://trickle-labs.github.io/pg-ripple/` (the old URL redirects for web
> traffic but the Helm `helm repo add` command needs the canonical URL).

### 3.8 Documentation Source Files

The following files in `docs/src/` contain direct GitHub URL references:

| File | Lines | Change |
|---|---|---|
| `docs/book.toml` | 17–18 | `git-repository-url` and `edit-url-template` |
| `docs/src/operations/docker.md` | 5, 23 | Image name `ghcr.io/grove/pg-ripple:*` |
| `docs/src/operations/cloudnativepg.md` | 25, 26, 55 | Image references in table and YAML example |
| `docs/src/operations/kubernetes.md` | 18, 49 | Helm repo URL; values table |
| `docs/src/operations/pg-trickle-relay.md` | 916 | `ghcr.io/grove/pg-ripple:latest` in docker-compose snippet |
| `docs/src/reference/w3c-conformance.md` | 5 | Actions page link |
| `docs/src/reference/roadmap.md` | 3 | ROADMAP.md blob URL |
| `docs/src/reference/release-process.md` | 3 | RELEASE.md blob URL |
| `docs/src/reference/changelog.md` | 3 | CHANGELOG.md blob URL |
| `docs/src/reference/degradation.md` | 233 | README link |
| `docs/src/evaluate/performance-results.md` | 40 | benchmarks/ tree link |
| `docs/src/evaluate/architecture-glance.md` | 105 | plans/ blob link |
| `docs/src/features/uncertain-knowledge.md` | 138 | discussions link |
| `docs/src/research/postgresql-deepdive.md` | 3, 202, 203 | Three blob links |
| `docs/src/user-guide/playground.md` | 10, 81 | Image name; `git clone` URL |
| `docs/spec/rdf-bidi-integration-v1.md` | 3, 371, 442 | Three references |

All changes follow the pattern:
```
https://github.com/grove/pg-ripple → https://github.com/trickle-labs/pg-ripple
ghcr.io/grove/pg-ripple           → ghcr.io/trickle-labs/pg-ripple
```

### 3.9 README.md

Lines 3, 4, 13, and 336 contain CI/Release badge URLs and the DeepWiki badge,
plus the `git clone` example:
```markdown
# Before:
[![CI](https://github.com/grove/pg-ripple/actions/...)]
[![Release](https://github.com/grove/pg-ripple/actions/...)]
[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/grove/pg-ripple)
git clone https://github.com/grove/pg-ripple.git

# After:
[![CI](https://github.com/trickle-labs/pg-ripple/actions/...)]
[![Release](https://github.com/trickle-labs/pg-ripple/actions/...)]
[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/trickle-labs/pg-ripple)
git clone https://github.com/trickle-labs/pg-ripple.git
```

> **Note on DeepWiki**: The DeepWiki badge URL (`deepwiki.com/grove/pg-ripple`)
> points to an externally-hosted index. After the transfer, re-index the project
> at `deepwiki.com/trickle-labs/pg-ripple` to keep the badge functional.

### 3.10 Kubernetes Example Manifests

**`examples/cloudnativepg_cluster.yaml` (line 41)**
```yaml
# Before:
image: ghcr.io/grove/pg-ripple:0.98.0-cnpg
# After:
image: ghcr.io/trickle-labs/pg-ripple:0.98.0-cnpg
```

### 3.11 `justfile`

The `release-bump` recipe contains inline `sed` commands that update
`docker-compose.yml`. Lines 216 (×2), 250, and 283 reference the old image path:

```makefile
# Before:
sed -i '' "s|ghcr.io/grove/pg-ripple:$$OLD_VERSION|ghcr.io/grove/pg-ripple:{{NEW_VERSION}}|g"
echo "[docker-compose.yml]  ghcr.io/grove/pg-ripple:{{NEW_VERSION}}";
DC_VER=$(grep 'ghcr.io/grove/pg-ripple:' docker-compose.yml ...)

# After: replace grove/pg-ripple → trickle-labs/pg-ripple
```

---

## 4. Generated / Derived Artefacts

### 4.1 SBOM Files

The following CycloneDX SBOM files are **generated artefacts** produced by
`cargo cyclonedx`. They contain the VCS URL pulled from `Cargo.toml`:

- `sbom.json` (line 31)
- `pg_ripple_http/sbom.json` (line 31)
- `pg_ripple.cdx.json` (line 31)
- `pg_ripple_http/pg_ripple_http.cdx.json` (line 31)

**Action**: After updating `Cargo.toml`, regenerate all four files by running
`cargo cyclonedx` (or let CI regenerate them on the next release). Do not
manually edit them; they are generated output.

---

## 5. GitHub Platform Actions (Post-Transfer)

These actions are performed in the GitHub UI/API, not in the codebase itself.

### 5.1 Transfer the Repository

1. Navigate to `https://github.com/grove/pg-ripple` → Settings → Danger Zone → Transfer.
2. Transfer to `trickle-labs`.
3. GitHub will create HTTP redirects from `github.com/grove/pg-ripple` to
   `github.com/trickle-labs/pg-ripple`. These redirects are best-effort and
   do **not** apply to GHCR image pulls.

### 5.2 Re-configure Branch Protection Rules

Branch protection rules (required status checks, required reviewers, etc.) are
**not transferred** with the repository. Re-apply the following to `main` in
the new location:
- Require pull request reviews (1 or more approvals)
- Required status checks: `ci`, `docs-test`, `cargo-audit`, `helm-lint`
- Dismiss stale pull request approvals when new commits are pushed
- Require conversation resolution before merging
- Do not allow force pushes

### 5.3 Re-configure Actions Permissions

In the new org, set:
- Actions → General → Workflow permissions: "Read and write permissions"
- Actions → Allow GitHub Actions to create and approve pull requests: enabled

### 5.4 GitHub Pages

GitHub Pages will continue to work at the new URL
`https://trickle-labs.github.io/pg-ripple/` after the transfer. Update any
external links (helm repo add, documentation references) to use the new URL.

### 5.5 GHCR — Container Image Migration

> This is the most critical operational concern for existing users.

After the transfer, new releases will push to `ghcr.io/trickle-labs/pg-ripple`.
Images already published under `ghcr.io/grove/pg-ripple` will **not** be
automatically moved. Options:

| Option | Effort | Impact |
|---|---|---|
| Re-publish existing tagged images to `ghcr.io/trickle-labs/pg-ripple` using `docker pull` + `docker push` | Medium | Users on old tags can pull from new location |
| Add a pinned note to the `grove` org GHCR registry pointing to the new location | Low | Requires `grove` org admin access |
| Accept that old tags are orphaned and announce the new location | Low | Existing deployments using `ghcr.io/grove/pg-ripple:0.x.y` continue to work until those images expire |

**Recommendation**: Re-publish the most recent 2–3 versions to the new registry,
then announce the migration.

### 5.6 Crates.io

`pg_ripple_http` is published to crates.io. The `repository` field in
`Cargo.toml` is informational metadata only — it does not affect crate ownership.
No ownership transfer is needed at crates.io. The updated `repository` URL will
take effect on the next `cargo publish`.

### 5.7 Renovate

`renovate.json` does not contain hard-coded org references; it relies on the
`GITHUB_TOKEN` and the repository context. After the transfer, ensure the
Renovate GitHub App is installed on `trickle-labs/pg-ripple` (either via the
app marketplace or the Renovate bot account). The configuration file itself
requires no changes.

### 5.8 DeepWiki

The README badge points to `https://deepwiki.com/grove/pg-ripple`. Register
and re-index the project at `https://deepwiki.com/trickle-labs/pg-ripple` to
restore the badge, then update the badge URL in `README.md`.

---

## 6. Communication Checklist

Before completing the migration, prepare the following communications:

- [ ] **Internal announcement**: Notify all contributors of the new clone URL
  and that existing PRs/forks will need a `git remote set-url origin` update.
- [ ] **CHANGELOG entry**: Add a note in `CHANGELOG.md` documenting the org transfer.
- [ ] **Docker Hub / GHCR notice**: Add a description note to the old
  `ghcr.io/grove/pg-ripple` packages pointing to the new location.
- [ ] **Helm users**: If the Helm chart is published to a standalone chart
  museum, update the index. Users with `helm repo add pg-ripple https://grove.github.io/pg-ripple/charts`
  in their scripts will need to run `helm repo remove pg-ripple` and `helm repo add`
  with the new URL.

---

## 7. Prioritised Change Order

Execute changes in this order to minimise disruption:

1. **[Prepare codebase]** Apply all code changes from §3 in a single PR to
   `grove/pg-ripple` before initiating the transfer.
2. **[Prepare platform]** Create teams in `trickle-labs` (§2.2), enable GHCR
   (§2.3), and stage secrets (§2.4).
3. **[Transfer]** Initiate GitHub repository transfer (§5.1).
4. **[Update remote]** Run `git remote set-url origin git@github.com:trickle-labs/pg-ripple.git`
   in all local clones.
5. **[Restore CI]** Re-apply branch protection rules (§5.2) and Actions
   permissions (§5.3).
6. **[Container images]** Re-publish recent image tags to `ghcr.io/trickle-labs/`
   (§5.5).
7. **[Regenerate SBOMs]** Run `cargo cyclonedx` and commit the updated files.
8. **[Verify]** Trigger a CI run, confirm all workflows pass, and check the docs
   site at the new URL.
9. **[Communicate]** Send announcement, update DeepWiki (§5.8), update Helm
   users (§6).

---

## 8. Files Not Requiring Changes

The following files were reviewed and contain no `grove` references that need
updating:

- All files under `src/` (Rust source) — `trickle-labs` references already
  present for `pg_tide` dependency; no `grove` org refs.
- `sql/` migration scripts — no org references.
- `tests/` directory — only `tests/w3c/known_failures.txt` line 27 references
  `grove/pg-ripple/issues/460`; this is a historical tracking comment. Update
  opportunistically but not urgently (the redirect will work).
- `roadmap/` and `plans/` markdown files — these contain historical references
  to old image tags (e.g., `ghcr.io/grove/pg-ripple:0.54.0` in assessment
  documents). These are archived historical records; update them only if they
  are actively surfaced in documentation builds.
- `blog/` markdown files — contain no `grove` org references.
- `CONTRIBUTING.md`, `LICENSE`, `RELEASE.md` — no org references.
