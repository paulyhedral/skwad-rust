---
description: Cut a release for this repo - run the changelog check, build the notarized artifacts, and publish the GitHub release
argument-hint: "[version]"
---

Invoke the `release-repo` skill for this repo.

`$ARGUMENTS` is the optional target version (`x.y.z`). If omitted, the skill derives the next
version from `CHANGELOG.md` and the current build number.
