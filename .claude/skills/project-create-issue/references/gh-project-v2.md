# GitHub Projects v2: discovering and setting custom fields via `gh`

Projects v2 (the modern GitHub Projects) has no direct `--field value` shortcut on
`gh issue create`. Adding an issue and setting a custom field (Size, Effort, Status) is a
separate multi-step sequence, and every ID involved (project, field, option) has to be looked up
first - none of them can be guessed from the field's display name.

## 1. Find the project and its node ID

```
gh project list --owner <owner> --format json
```

Returns each project's `number` and `id` (the node ID, needed below). If more than one project
exists, confirm with the user which one before continuing.

## 2. Add the issue to the project

```
gh project item-add <project-number> --owner <owner> --url <issue-url> --format json
```

Returns the item's `id` (its Projects v2 item node ID) - save it, it's needed for every
`item-edit` call.

## 3. List the project's fields to find field IDs and option IDs

```
gh project field-list <project-number> --owner <owner> --format json
```

Returns each field's `id`, `name`, `dataType` (`TEXT`, `NUMBER`, `SINGLE_SELECT`, `DATE`, ...),
and for `SINGLE_SELECT` fields, an `options` array of `{id, name}` - e.g. a Size field's options
might be `Small`/`Medium`/`Large`, each with its own `id`. Match the field by `name` (case
matters), not a guessed slug.

## 4. Set a field value on the item

The flag depends on the field's `dataType`:

```
# SINGLE_SELECT (Size, Effort, Status, ...): pass the option's id, not its name
gh project item-edit --project-id <project-node-id> --id <item-id> \
  --field-id <field-node-id> --single-select-option-id <option-node-id>

# TEXT
gh project item-edit --project-id <project-node-id> --id <item-id> \
  --field-id <field-node-id> --text "<value>"

# NUMBER
gh project item-edit --project-id <project-node-id> --id <item-id> \
  --field-id <field-node-id> --number <value>
```

`--project-id` here is the project's node `id` from step 1, not its `number`. Mixing up
`number` and `id` is the most common failure mode with this command - `gh` gives an opaque
GraphQL error rather than a clear "wrong ID type" message when they're swapped.

## Gotchas

- Field and option names are case-sensitive when matching `field-list` output - `"size"` won't
  match a field literally named `"Size"`.
- A field with no matching option (e.g. the change's estimated effort doesn't cleanly map to any
  existing Size option) is a signal to ask the user which option to use, not to skip the field
  silently or invent a new option.
- Re-running `item-add` on an issue already in the project is a no-op that returns the existing
  item's `id` - safe to call again if the item ID from an earlier step was lost.
