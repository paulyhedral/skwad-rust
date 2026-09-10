---
name: project-create-issue
description: Create a single GitHub Issue for an already-scoped OpenSpec change - derive title/labels/type/effort from proposal.md, set project, milestone, and link back to the change. Use when the user wants to file or open a GitHub Issue for one OpenSpec change - not for splitting a plan into issues (to-issues) or resuming work on an existing issue (project-start-change).
---

Create the GitHub Issue that `project-start-change` later locates and `project-finish-change`
later closes. This is the step that doesn't exist yet when a change has only been proposed - one
OpenSpec change becomes one issue, fully tagged.

## Steps

### 1. Identify the change and its repo

Ask which change, unless already clear from context. OpenSpec changes live in
`openspec/changes/<name>` in this repo. Resolve `owner/repo` from `git remote -v` (or
`gh repo view --json owner,name`) - referred to below as `<owner>/<repo>`.

### 2. Check for an existing issue first

Search before creating, so this skill never produces a duplicate:

```
gh issue list --repo <owner>/<repo> --state all --search "<change-name> in:title,body" --json number,title,url,state
```

If a match already exists, stop and tell the user - point them at `project-start-change` instead
of filing a second issue for the same change.

### 3. Read the change

Read `proposal.md` (required) and `design.md`/`tasks.md` if present. Pull from `proposal.md`'s
`## Why` and `## What Changes` sections:
- a title (a short imperative summary - the proposal rarely states one verbatim, so write one)
- a body summary (condense `Why` + `What Changes`, don't paste the whole file)
- a rough type signal: new capability -> feature, `MODIFIED`/`REMOVED` requirements or "fix" in
  the wording -> bug or chore, docs-only change -> documentation

`tasks.md`'s task-group count is a rough proxy for size/effort if the repo's project tracks that
as a field - more groups and cross-cutting groups trend toward a larger size than a single-file,
single-group change.

### 4. Discover the repo's actual taxonomy - never guess a label, type, milestone, or project

Taxonomies differ per repo and may not exist yet. Pull the real options before mapping anything:

```
gh label list --repo <owner>/<repo> --json name,description
gh issue create --repo <owner>/<repo> --help | grep -A3 -- '--type'
gh api repos/<owner>/<repo>/milestones --jq '.[] | {title, number, state}'
gh project list --owner <owner> --format json
```

Native GitHub Issue Types is an org-level feature and not every account has it enabled - if
`--type` isn't available on `gh issue create` in this environment, fall back to a `type: <kind>`
label if one exists in the discovered label list, otherwise skip type rather than inventing a
label that isn't part of this repo's taxonomy.

For the project's Size/Effort field (a Projects v2 custom field, not a label in most setups),
see `references/gh-project-v2.md` - discovering and setting custom fields needs a specific `gh
project field-list` / `item-edit` sequence that's easy to get wrong on the first try.

### 5. Map the change to real values, asking when it's genuinely ambiguous

- **Type**: use the signal from step 3 against the discovered types/labels. If more than one
  plausible type fits, ask (AskUserQuestion) rather than picking arbitrarily.
- **Labels**: match discovered label names/descriptions against the change's content by keyword
  overlap. Apply what clearly fits; don't force a label onto every category if nothing matches
  well.
- **Size/Effort**: if the project has a Size/Effort custom field, propose a value from the
  `tasks.md` heuristic in step 3 and confirm with the user rather than silently committing to a
  guess.
- **Milestone**: if exactly one open milestone exists, use it. If several are open, ask which
  one. If none exist, proceed without a milestone rather than creating one speculatively.
- **Project**: if the repo has exactly one associated Projects v2 project, use it. If several,
  ask.

### 6. Create the issue

The body MUST reference the OpenSpec change it came from - this is the traceability link the
rest of the OpenSpec workflow (`project-start-change`, `project-finish-change`) depends on to
find their way back to the spec. Use a heredoc so multi-line formatting survives:

```
gh issue create --repo <owner>/<repo> --title "<title>" --body "$(cat <<'EOF'
<condensed Why + What Changes summary>

---
OpenSpec change: `openspec/changes/<change-name>/proposal.md`
EOF
)" --label "<label1>" --label "<label2>"
```

Then, if type or milestone weren't settable at creation time, apply them:

```
gh issue edit <n> --repo <owner>/<repo> --milestone "<milestone>"
gh issue edit <n> --repo <owner>/<repo> --type "<type>"   # only if native Issue Types is enabled
```

### 7. Add to the project and set custom fields

Follow `references/gh-project-v2.md` for the add + field-edit sequence (item add, then
project/field/option IDs, then `item-edit`). Set Status to whatever the project's default
"not started" column is - `project-start-change` moves it to "In Progress" later, this skill
should not.

### 8. Report back

Summarize: issue number/URL, repo, title, labels applied, type (or note if skipped - no native
Issue Types support), milestone, project + any custom fields set, and confirm the OpenSpec change
reference is present in the body.

## Gotchas

- Always run the discovery calls in step 4 before creating the issue - a repo's labels,
  milestones, and projects are never assumed, only ever read live. A label or milestone that
  looks obviously right (`bug`, `v1.0`) may not exist in this specific repo.
- Projects v2 field edits need the project's node ID and the field's ID, not just a project
  number or field name - `gh project item-edit` will reject a bare name. See
  `references/gh-project-v2.md`.
- If GitHub API calls fail with a TLS/certificate error, that's a sandboxed-network restriction,
  not a real GitHub outage - retry the `gh`/`gh api` call outside the sandbox rather than
  reporting GitHub as down.
- Don't assign the issue or move its project status to "In Progress" here - assignment and
  status changes happen "when starting on a change", which is `project-start-change`'s job.
- Search for a duplicate issue (step 2) before creating - re-running this skill on a change that
  already has an issue should stop and redirect, not file a second one.
