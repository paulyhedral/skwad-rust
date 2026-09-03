---
name: project-finish-change
description: Wrap up work on an OpenSpec change already in progress - commits code and docs, marks remaining tasks complete, opens or updates the linked PR, merges it once checks pass, closes the GitHub Issue, updates the local main worktree, and removes the feature worktree. Use when the user says a change is done, ready to merge, or wants to close out/finish/wrap up a change - not for starting a change (use project-start-change) or archiving OpenSpec artifacts on their own (use openspec-archive-change).
---

Close out an OpenSpec change that was set up with `project-start-change`: land the code, merge
the PR, close the issue, and clean up the worktree.

Resolve `owner/repo` from `git remote -v` - referred to below as `<owner>/<repo>`.

## Steps

### 1. Identify the change and its worktree

Ask which change to finish, unless clear from context. Find its worktree: `git worktree list`,
or the fixed path convention
`/Users/paulyhedral/Projects/Code/skwad-rust-worktrees/<branch-slug>`. All remaining work
happens inside that worktree, not the main working tree.

### 2. Verify the work before declaring anything done

Run the repo's build, lint, and test commands inside the worktree (`make test`, `cargo test`,
`cargo clippy`, etc. as the stack requires). Do not proceed to committing/merging on the basis
of untested claims - confirm by actually running these.

### 3. Mark tasks complete

Open the change's `tasks.md` and flip any task actually finished from `- [ ]` to `- [x]`.
If tasks remain genuinely incomplete, stop and ask the user whether to finish them first or
proceed anyway (mirroring `openspec-archive-change`'s incomplete-task warning) - don't mark a
task done that wasn't done.

### 4. Commit code and docs

Stage only the files that changed for this work (never a blanket `git add -A`/`.`). Commit
using Conventional Commits (`<type>(<scope>): <description>`, per the root `AGENTS.md`),
splitting code and doc/task-list changes into separate commits only if that matches the
repo's existing commit history style - otherwise one commit is fine.

### 5. Archive the change

Use the `openspec-archive-change` skill to archive the change.

### 6. Push and ensure the PR is in order

`git push` the branch. If `project-start-change` already opened a PR, confirm it's still
pointed at the right base and not marked draft. If no PR exists yet, create one now:

```
gh pr create --repo <owner>/<repo> --base main --title "<issue title, no Conventional Commits prefix>" --body "Closes #<issue-number>"
```

The `Closes #<n>` (or `Fixes #<n>`) line is what links the PR to the issue and auto-closes it
on merge - confirm it's present in the PR body even if the PR already existed.

### 7. Wait for checks

`gh pr checks <n> --watch` (or poll `gh pr view <n> --json statusCheckRollup`). If checks
fail, report the failure and stop - do not merge a red PR. Fix and re-push if the fix is
clear; otherwise hand back to the user.

### 8. Merge

Confirm with the user before merging, unless they've already authorized auto-merge for this
task - merging is a shared, visible action. Default to squash merge, or ask if the repo's
convention differs:

```
gh pr merge <n> --squash --delete-branch
```

`--delete-branch` removes the remote branch; it does not touch the local worktree.

### 9. Close out the Issue

If the merge's `Closes #<n>` didn't auto-close it, close it explicitly:

```
gh issue close <n> --repo <owner>/<repo>
```

Then set the end date if the project tracks one, and move the issue's project status to
"Done" (`gh project item-edit`), if the repo uses a Projects v2 board.

### 10. Update the local main worktree

In the repo's main working tree (not the feature worktree):

```
git checkout main
git pull
```

### 11. Remove the feature worktree

```
git worktree remove /Users/paulyhedral/Projects/Code/skwad-rust-worktrees/<branch-slug>
git branch -D <branch-name>
```

### 12. Report back

Summarize: change name, PR number/URL and merge method, issue number and closed state, and
confirmation the worktree was removed.

## Gotchas

- Don't skip step 2 - "tests pass" and "lint is clean" must be observed by actually running
  them this session, not assumed from earlier context.
- A PR merged into a non-default branch does not auto-close the issue - always verify the
  issue actually closed (step 9) rather than trusting the `Closes #n` line alone.
- Never force-merge past failing checks or skip hooks to make a merge go through - stop and
  report instead.
