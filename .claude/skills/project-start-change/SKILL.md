---
name: project-start-change
description: Begin implementation work on an OpenSpec change - asks for the change name, locates its linked GitHub Issue (or finds one by matching the change name), sets up an isolated git worktree, and preps the issue/branch/PR for active work. Use when the user wants to start, pick up, or resume coding on an OpenSpec change - not for writing the proposal itself or for implementing tasks once the worktree already exists (use openspec-apply-change for that).
---

Set up everything needed to start coding on an OpenSpec change: the linked GitHub Issue, an
isolated git worktree, and a branch ready for `openspec-apply-change` to take over.

## Steps

### 1. Get the change name

Ask the user which change to start, unless one is already clear from conversation context.
If unsure, offer to run `openspec list --json` to show options.

### 2. Locate the change

OpenSpec changes live in `openspec/changes/<name>` in this repo. Read that change's
`proposal.md` (and `design.md` if present) for context on scope and affected files.

Resolve `owner/repo` from `git remote -v` (or `gh repo view --json owner,name`) - referred to
below as `<owner>/<repo>`.

### 3. Find the linked GitHub Issue

- Check `proposal.md`/`design.md`/`tasks.md` for an existing issue reference (`#123` or a
  full issue URL).
- If none is found, search for an issue whose title or body names the change:
  `gh issue list --repo <owner>/<repo> --state all --search "<change-name> in:title,body" --json number,title,url,state`
- If multiple candidates come back, show them to the user (AskUserQuestion) rather than
  guessing which one is the match.
- If zero candidates come back, create the issue yourself by invoking the
  `project-create-issue` skill for this change, then continue with the issue it creates - no
  need to stop and ask first.

### 4. Prep the issue for active work

- `gh issue edit <n> --repo <owner>/<repo> --add-assignee @me`
- Record a start date on the issue if the project uses that convention (check existing
  issues for the field before assuming one exists).
- Move the issue's project status to "In Progress" (`gh project item-edit`, or ask the user
  for the project/field IDs if you don't already know them), if the repo uses a Projects v2
  board.

### 5. Create the worktree

Use this repo's fixed worktree convention (do not use `EnterWorktree` - it creates worktrees
under `.claude/worktrees/`, which conflicts with the path used here):

```
git worktree add /Users/paulyhedral/Projects/Code/skwad-rust-worktrees/<branch-slug> -b <branch-name> origin/main
```

- `<branch-name>`: `<issue-number>-<change-name>`, matching GitHub's own suggested linked
  branch name so the branch auto-links to the issue.
- `<branch-slug>`: `<branch-name>` with any `/` replaced by `-`.
- Base ref is `origin/main`.
- If you use `gh issue develop <n> --name <branch-name> --base main` instead (to get GitHub's
  native issue-branch link), it checks the branch out in the current working tree by default -
  immediately switch that working tree back to its original branch, then
  `git worktree add <path> <branch-name>` (no `-b`, the branch already exists).

Report the worktree's absolute path clearly - the user needs it to `cd` there themselves.
You may then call `EnterWorktree` with `path: "<the path just created>"` to switch this
session into it, since that form of the tool accepts any existing worktree of the repo.

### 6. Push and open the PR

- Push the new branch: `git push -u origin <branch-name>` (run inside the worktree).
- `gh pr create --repo <owner>/<repo> --base main --title "<issue title, no Conventional Commits prefix>" --body "..."`
  (never `--draft` - PRs are opened ready for review)
- Set PR assignee, labels, project, and milestone to match the issue.
- Link the PR to the issue (via a `Closes #<n>` line in the body, or `gh issue develop`'s
  auto-link if used in step 5).

### 7. Report back

Summarize: change name, issue number/URL, worktree path, branch name, PR URL.

### 8. Hand off to implementation

Ask the user if they want to begin implementing tasks now. If yes, invoke the
`openspec-apply-change` skill (`/opsx:apply`) in this session - it picks up from the worktree
and branch just created. If the session did not switch into the worktree via `EnterWorktree`
in step 5, tell the user to `cd` there first, since `openspec-apply-change` operates on the
current working tree.

## Gotchas

- Never edit tracked source files in the main working tree for this work - the worktree
  created in step 5 is the only place code changes should happen.
- `gh issue list --search` needs `in:title,body` (not the default, which is title-only) or
  it will miss issues that only mention the change name in the description.
- If GitHub API calls fail with a TLS/certificate error, that's a sandboxed-network
  restriction, not a real GitHub outage - retry the `gh`/`gh api` call outside the sandbox.
