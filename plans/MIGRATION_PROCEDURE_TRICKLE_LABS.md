# Migration Procedure: grove/pg-ripple → trickle-labs/pg-ripple

**Prepared**: 2026-05-07  
**Reference**: [`plans/REPORT_TRICKLE_LABS.md`](REPORT_TRICKLE_LABS.md) — full analysis and rationale  
**Status**: Step 1 complete (PR #79 merged / pending merge)

---

## Overview

The migration has eight steps. Steps 1–2 happen before the GitHub transfer.
Step 3 is the transfer itself. Steps 4–8 happen after.

```
[Step 1] Codebase changes merged  ← DONE (PR #79)
[Step 2] New org pre-staged
[Step 3] GitHub repository transfer
[Step 4] Restore CI settings
[Step 5] Container image migration
[Step 6] Regenerate SBOM files
[Step 7] Verify everything works
[Step 8] Communicate / announce
```

**Estimated total time**: 1–2 hours (dominated by waiting for CI after the transfer).

---

## Step 1 — Merge the codebase changes [DONE]

PR #79 (`chore: migrate org references from grove to trickle-labs`) updated all
60 source files to reference `trickle-labs` instead of `grove`. It must be merged
into `main` before proceeding.

**Checklist:**
- [ ] PR #79 is approved
- [ ] PR #79 is merged into `main`

After merging, update your local clone:
```bash
git checkout main
git pull
```

---

## Step 2 — Pre-stage the `trickle-labs` organisation [Before transfer]

These actions are done in the GitHub web UI. Complete all of them before
initiating the transfer.

### 2.1 — Create the `trickle-labs` organisation

If it does not already exist, go to:  
`https://github.com/organizations/plan` → create `trickle-labs`.

### 2.2 — Create the three GitHub Teams

Navigate to `https://github.com/orgs/trickle-labs/teams` and create:

| Team name | Members to add |
|---|---|
| `pg-ripple-maintainers` | same members as `grove/pg-ripple-maintainers` |
| `pg-ripple-rust` | same members as `grove/pg-ripple-rust` |
| `pg-ripple-infra` | same members as `grove/pg-ripple-infra` |

> The teams must exist in `trickle-labs` before the transfer, otherwise CODEOWNERS
> evaluation will fail on the first PR after the move.

### 2.3 — Enable GitHub Container Registry on `trickle-labs`

1. Go to `https://github.com/organizations/trickle-labs/settings/packages`
2. Confirm "Container registry" is enabled (it is on by default for new orgs).
3. Set default package visibility to **Private** or **Public** as needed — the
   release workflow pushes public images.

### 2.4 — Note the `CARGO_REGISTRY_TOKEN` secret

The secret is currently stored in `grove/pg-ripple` Settings → Secrets.
You cannot read its value, so retrieve the token from wherever it was originally
generated (crates.io → Account Settings → API Tokens) before the transfer. You
will re-add it in Step 4.

---

## Step 3 — Transfer the repository [The transfer]

> **This is the point of no return.** Complete Step 2 first.

1. Go to `https://github.com/grove/pg-ripple/settings`
2. Scroll to **Danger Zone**
3. Click **Transfer repository**
4. Type `grove/pg-ripple` to confirm
5. Select `trickle-labs` as the destination organisation
6. Click **I understand, transfer this repository**

GitHub will:
- Move all Git history, issues, PRs, projects, milestones, labels, and wiki
- Create HTTP 301 redirects from `github.com/grove/pg-ripple` to
  `github.com/trickle-labs/pg-ripple` for all web traffic
- Invalidate existing `GITHUB_TOKEN`-based deploy keys (the token is re-issued
  in the new org context automatically)
- **Not** redirect GHCR image pulls (handle in Step 5)
- **Not** transfer branch protection rules (handle in Step 4)

> The redirect for `git clone https://github.com/grove/pg-ripple.git` also works,
> but contributors should update their remotes as soon as convenient.

---

## Step 4 — Restore CI settings [Immediately after transfer]

Branch protection rules and Actions permissions are lost during the transfer.
Restore them before the next push to `main`.

### 4.1 — Update your local remote

```bash
git remote set-url origin git@github.com:trickle-labs/pg-ripple.git
git fetch origin
```

### 4.2 — Re-apply branch protection on `main`

Go to `https://github.com/trickle-labs/pg-ripple/settings/branches`
→ Add rule → Branch name pattern: `main`

| Setting | Value |
|---|---|
| Require a pull request before merging | ✅ enabled |
| Required approving reviews | 1 (or more) |
| Dismiss stale PR approvals when new commits are pushed | ✅ enabled |
| Require status checks to pass before merging | ✅ enabled |
| Required status checks | `ci`, `docs-test`, `cargo-audit`, `helm-lint` |
| Require conversation resolution before merging | ✅ enabled |
| Allow force pushes | ❌ disabled |
| Allow deletions | ❌ disabled |

### 4.3 — Re-configure Actions permissions

Go to `https://github.com/trickle-labs/pg-ripple/settings/actions`

| Setting | Value |
|---|---|
| Actions permissions | Allow all actions and reusable workflows |
| Workflow permissions | Read and write permissions |
| Allow GitHub Actions to create and approve PRs | ✅ enabled |

### 4.4 — Re-add the `CARGO_REGISTRY_TOKEN` secret

Go to `https://github.com/trickle-labs/pg-ripple/settings/secrets/actions`
→ New repository secret

| Name | Value |
|---|---|
| `CARGO_REGISTRY_TOKEN` | (token retrieved in Step 2.4) |

### 4.5 — Re-add the GHCR write permission for Actions

The release workflow pushes to GHCR using `GITHUB_TOKEN`. For the first release
after the transfer:

1. Go to `https://github.com/orgs/trickle-labs/settings/packages`
2. Find `pg-ripple` (if it has been pushed before) or confirm Actions can create
   new packages: **Allow GitHub Actions to create packages**.

---

## Step 5 — Migrate container images [After transfer]

Images already published under `ghcr.io/grove/pg-ripple` are **not** automatically
moved. GHCR does not follow the GitHub redirect for `docker pull`.

### 5.1 — Authenticate to GHCR

```bash
echo $GITHUB_TOKEN | docker login ghcr.io -u <your-github-username> --password-stdin
# or:
gh auth token | docker login ghcr.io -u <your-github-username> --password-stdin
```

### 5.2 — Re-publish the last two releases

Run for each version you want to preserve (adjust `VERSIONS` as needed):

```bash
VERSIONS="0.98.0 0.99.0"
for V in $VERSIONS; do
  # Main extension image
  docker pull ghcr.io/grove/pg-ripple:$V
  docker tag  ghcr.io/grove/pg-ripple:$V ghcr.io/trickle-labs/pg-ripple:$V
  docker push ghcr.io/trickle-labs/pg-ripple:$V

  # HTTP companion image
  docker pull ghcr.io/grove/pg-ripple-http:$V
  docker tag  ghcr.io/grove/pg-ripple-http:$V ghcr.io/trickle-labs/pg-ripple-http:$V
  docker push ghcr.io/trickle-labs/pg-ripple-http:$V
done

# CloudNativePG image (adjust version as needed)
docker pull ghcr.io/grove/pg-ripple:0.98.0-cnpg
docker tag  ghcr.io/grove/pg-ripple:0.98.0-cnpg ghcr.io/trickle-labs/pg-ripple:0.98.0-cnpg
docker push ghcr.io/trickle-labs/pg-ripple:0.98.0-cnpg
```

### 5.3 — Set package visibility

After pushing, go to:
- `https://github.com/orgs/trickle-labs/packages/container/pg-ripple`
- `https://github.com/orgs/trickle-labs/packages/container/pg-ripple-http`

Set visibility to **Public** so unauthenticated users can pull them.

### 5.4 — Annotate the old `grove` packages (optional but courteous)

If you still have admin access to the `grove` org packages, add a description note
to each package pointing to the new location, for example:
> "This image is no longer updated. Please pull from ghcr.io/trickle-labs/pg-ripple."

---

## Step 6 — Regenerate SBOM files [After transfer]

The four CycloneDX SBOM files were intentionally not edited in PR #79 — they
are generated artefacts and should be regenerated rather than manually patched.

```bash
# Ensure you are on main with the latest code
git checkout main && git pull

# Install cargo-cyclonedx if not already installed
cargo install cargo-cyclonedx

# Regenerate
cargo cyclonedx

# Commit the updated files
git add sbom.json pg_ripple.cdx.json \
        pg_ripple_http/sbom.json pg_ripple_http/pg_ripple_http.cdx.json
git commit -m "chore: regenerate SBOM files after org transfer to trickle-labs"
git push
```

The four files that will be updated are:
- `sbom.json`
- `pg_ripple.cdx.json`
- `pg_ripple_http/sbom.json`
- `pg_ripple_http/pg_ripple_http.cdx.json`

---

## Step 7 — Verify everything works

### 7.1 — Trigger CI

Either push an empty commit or trigger the workflow manually:

```bash
# Empty commit to trigger CI
git commit --allow-empty -m "ci: trigger post-transfer verification run"
git push
```

Or in the GitHub UI: Actions → CI → Run workflow → branch `main`.

### 7.2 — Check the documentation site

After the docs workflow runs, confirm the docs site is live at:  
`https://trickle-labs.github.io/pg-ripple/`

If the GitHub Pages source is not configured yet:
1. Go to `https://github.com/trickle-labs/pg-ripple/settings/pages`
2. Set Source: **Deploy from a branch** → branch `gh-pages` (or Actions, depending
   on the current `docs.yml` configuration)

### 7.3 — Verify Helm chart access

```bash
helm repo remove pg-ripple 2>/dev/null || true
helm repo add pg-ripple https://trickle-labs.github.io/pg-ripple/charts
helm repo update
helm search repo pg-ripple
```

### 7.4 — Verify GHCR pull

```bash
docker pull ghcr.io/trickle-labs/pg-ripple:0.99.0
```

### 7.5 — Verify Renovate

If the Renovate GitHub App was installed via the marketplace, check whether it
automatically follows the repository transfer. If not, install it on
`trickle-labs/pg-ripple` from `https://github.com/apps/renovate`. The
`renovate.json` configuration file requires no changes.

---

## Step 8 — Communicate the migration

### 8.1 — Notify contributors

Post in your team channel / mailing list:

> pg-ripple has moved to https://github.com/trickle-labs/pg-ripple
>
> Update your local clone:
> ```bash
> git remote set-url origin git@github.com:trickle-labs/pg-ripple.git
> ```
> Old `github.com/grove/pg-ripple` URLs continue to redirect for web traffic.
> GHCR image users should update to `ghcr.io/trickle-labs/pg-ripple`.

### 8.2 — Re-index DeepWiki

1. Go to `https://deepwiki.com/trickle-labs/pg-ripple` and trigger indexing.
2. Once live, update the README badge in a follow-up commit:

```markdown
# Before (already updated in PR #79 — will work once trickle-labs is indexed):
[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/trickle-labs/pg-ripple)
```

No code change needed — PR #79 already updated the badge URL. Just make sure
the DeepWiki project is registered.

### 8.3 — Add a CHANGELOG entry

Add a note to `CHANGELOG.md` under `Unreleased` or the next version heading:

```markdown
### Infrastructure
- Migrated repository from `grove/pg-ripple` to `trickle-labs/pg-ripple`.
  Container images are now published to `ghcr.io/trickle-labs/pg-ripple`.
  Old `github.com/grove/pg-ripple` URLs continue to redirect.
```

### 8.4 — Notify Helm users

If the Helm chart is listed in an external chart museum or Artifact Hub, update
the entry to point to `https://trickle-labs.github.io/pg-ripple/charts`.

---

## Post-migration checklist

Use this checklist to confirm the migration is complete:

- [ ] PR #79 merged into `main`
- [ ] Three teams created in `trickle-labs` org
- [ ] GHCR enabled on `trickle-labs` org
- [ ] `CARGO_REGISTRY_TOKEN` secret value in hand
- [ ] Repository transferred via GitHub UI
- [ ] Local remote updated to `git@github.com:trickle-labs/pg-ripple.git`
- [ ] Branch protection rules restored on `main`
- [ ] Actions permissions restored (read/write + allow PRs)
- [ ] `CARGO_REGISTRY_TOKEN` re-added as Actions secret
- [ ] Recent GHCR tags re-published to `ghcr.io/trickle-labs/`
- [ ] Package visibility set to Public on new GHCR packages
- [ ] SBOM files regenerated by `cargo cyclonedx` and committed
- [ ] CI workflow passes on `main` in the new org
- [ ] Docs site live at `trickle-labs.github.io/pg-ripple/`
- [ ] Helm repo accessible at `trickle-labs.github.io/pg-ripple/charts`
- [ ] GHCR pull of `ghcr.io/trickle-labs/pg-ripple:0.99.0` succeeds
- [ ] Renovate app installed on `trickle-labs/pg-ripple`
- [ ] Contributors notified with new clone URL
- [ ] DeepWiki re-indexed at `deepwiki.com/trickle-labs/pg-ripple`
- [ ] CHANGELOG entry added
