## Why

`skwad-core`'s settings store already holds the `Persona` model, the
`personas` collection, `active_personas()`, and `install_default_personas()`
(from `settings-persistence-port`). The `personas` spec also requires
create/update, type-dependent delete (soft for system, hard for user),
id lookup restricted to active personas, and a restore-defaults action - none
of which exist yet. This change fills that gap so agent creation (a later
change) has a complete persona API to attach a `persona_id` to an agent.

## What Changes

- Add to `Settings` in `crates/skwad-core/src/settings/mod.rs`:
  - `add_persona(name, instructions) -> &Persona` - appends a new `user`/
    `enabled` persona and persists.
  - `update_persona(id, name, instructions) -> Result<()>` - renames/rewrites
    an existing persona by id (any type); no-op `Ok(())` if the id is absent.
  - `remove_persona(id) -> Result<()>` - soft delete (`state = deleted`,
    record retained) for `system` personas; hard delete (record removed) for
    `user` personas; no-op if the id is absent.
  - `persona(id) -> Option<&Persona>` - looks up by id restricted to the
    active list (excludes deleted), matching the spec's lookup requirement.
  - `restore_default_personas() -> Result<()>` - resets every shipped system
    persona already present (matched by id) to its shipped name,
    instructions, and `enabled` state; appends any shipped default that is
    entirely missing (including previously soft-deleted, since deletion
    keeps the record - restore un-deletes it, matching the Swift reference);
    leaves user personas untouched.
- Unit tests alongside the existing `settings` tests: add/update/lookup
  round-trip, system-delete is soft, user-delete is hard, restore reverts a
  renamed/disabled system persona while leaving a user persona alone, restore
  re-adds a fully-missing default.

Non-goals:

- Any GUI (`PersonasSettingsView`/`PersonaSheet` equivalents) - a later UI
  change consumes this API.
- Attaching `persona_id` to an agent or resolving it into a system prompt -
  that is `agent-lifecycle`/`agent-launch-command` work.
- Changing the `Persona`/`PersonaType`/`PersonaState` shapes, the shipped
  `DEFAULT_PERSONAS` table, or `install_default_personas()` - all already
  correct per spec.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. `openspec/specs/personas/spec.md` is the unchanged contract; this
change completes the implementation already partially in place. `skip_specs:
true`.

## Impact

- Modified: `crates/skwad-core/src/settings/mod.rs` (new methods + tests).
- No new dependencies, no new crates, no changes to `skwad-git`,
  `skwad-discovery`, `skwad-history`, `skwad`, or the Swift app.
