---
name: release-repo
description: Cut a release for this repo - check the changelog, bump the build number, and trigger the release build (GitHub Actions or a local notarized build). Use when the user asks to release, cut a release, ship, or publish a new version.
metadata:
  type: engineering
---

# Release this repo

The release flow is driven by the `Makefile`. There are two paths - a GitHub Actions build
(normal) and a local notarized build (fallback when Actions is unavailable or a signed local
artifact is needed).

## Preconditions

- Working tree is clean (`git diff --quiet`). `make prerelease`/`make latest` refuse to run
  otherwise.
- `gh` is authenticated for the GitHub Actions path.
- For the local path: Xcode, a `Developer ID Application` certificate, and notarization
  credentials in `.env` (`TEAM_ID`, `APPLE_ID`, app-specific password). See the Makefile header.
- `CHANGELOG.md` has an `[Unreleased]` section with real entries - `make check-changelog` fails
  if it is empty.

## Steps

### 1. Decide the version

`make get-version` / `make get-build` print the current marketing version and build number.
If the user named a version, set it: `make set-version VERSION=x.y.z`, commit that change on
its own (`chore: version prep`). Otherwise the marketing version stays and only the build
number increments.

### 2. Verify before releasing

```bash
make check-changelog
make test
```

Do not proceed on untested claims - run these and read the output. Stop and report if either
fails.

### 3a. GitHub Actions path (normal)

```bash
make prerelease   # -> triggers build.yml with release_type=prerelease
# or
make latest       # -> triggers build.yml with release_type=latest
```

Both run `check-changelog`, assert a clean tree, run `increment-build`, then
`gh workflow run build.yml --ref <current-branch> -f release_type=<type>`.

Watch the run: `gh run watch $(gh run list --workflow build.yml --limit 1 --json databaseId --jq '.[0].databaseId')`.
Report the run URL and its outcome. If it fails, read the failed logs
(`gh run view <id> --log-failed`) and report the failing step - do not blindly re-run.

The `increment-build` commit is left in the working tree by the Makefile - commit and push it
(`chore: increment build number`) so the tag and the built artifact agree.

### 3b. Local notarized build path (fallback)

```bash
make release      # increment-build + archive + export + sign + notarize + dmg
```

Produces `build/Skwad.dmg` (and `build/Skwad.zip`). Then generate the Sparkle appcast and
publish a GitHub Release:

```bash
make appcast                       # -> build/appcast.xml
gh release create v<version> build/Skwad.zip build/Skwad.dmg build/appcast.xml \
  --title "v<version>" --notes-file <changelog excerpt or ->
```

### 4. Update the changelog

Move the `[Unreleased]` entries under a new `[x.y.z] - <date>` heading in `CHANGELOG.md`,
commit (`chore: changelog for x.y.z`), and push.

### 5. Report back

Version released, build number, which path was used, run/release URL, and confirmation the
build-number and changelog commits landed on the branch.

## When it fails

- **`check-changelog` fails**: the `[Unreleased]` section is empty - add entries first.
- **`make prerelease`/`latest` aborts on "uncommitted changes"**: commit or stash, then retry.
- **Actions run fails at notarization**: credentials in the workflow secrets are wrong or
  expired - not something to retry blindly.
- **`gh` TLS/certificate error**: sandboxed-network restriction, not a GitHub outage - retry
  the `gh` call outside the sandbox.

Report which step failed and why, not just "the release didn't work."
