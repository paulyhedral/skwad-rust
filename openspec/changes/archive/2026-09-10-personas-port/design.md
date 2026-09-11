## Context

`Settings` (`crates/skwad-core/src/settings/mod.rs`) already owns `personas:
Vec<Persona>`, `active_personas()`, and `install_default_personas()`, all
persisted through the existing `persist()` method (see `settings-persistence`
spec). This change adds the remaining CRUD/lookup/restore methods on the same
struct - no new module, no new state.

## Goals / Non-Goals

**Goals:**
- Match the Swift reference (`AppSettings.swift` `addPersona` /
  `updatePersona` / `removePersona` / `restoreDefaultPersonas`) one-to-one in
  behavior, expressed as `Result`-returning methods consistent with the rest
  of `Settings`.

**Non-Goals:**
- Reworking how `personas` is stored or persisted.
- Any caller-facing API beyond `Settings` methods (no service layer, no MCP
  tool, no CLI).

## Decisions

- **`persona(id)` filters through `active_personas()` rather than a raw scan
  of `self.personas`.** The spec requires lookup to resolve only against
  active personas; reusing `active_personas()` keeps the "excludes deleted"
  rule in one place instead of duplicating the filter.
- **`remove_persona` branches on `persona_type`, not on a caller-supplied
  flag.** The spec ties delete semantics to the persona's own type, and the
  Swift reference does the same (`removePersona(_:)` switches on
  `persona.type`) - the caller only ever needs to say which id to remove.
- **`restore_default_personas` iterates the shipped `default_personas()`
  list and, for each, either overwrites the stored record in place (by id)
  or appends it if absent** - including a soft-deleted record, since
  overwriting its `state` back to `enabled` is how the Swift reference
  "un-deletes" a restored default. This is a bytewise copy of the shipped
  `Persona`, not a partial field patch, so name/instructions/state/type all
  reset together.
- **No new error variant.** All four new methods either mutate in place and
  call `self.persist()` (propagating its `Result`) or are a no-op `Ok(())`
  when the id isn't found - matching how `add_bench_agent`/`add_recent_repo`
  already behave in this file.

## Risks / Trade-offs

- [Restoring a default un-deletes it even though the user explicitly deleted
  it] -> This matches the spec ("add back any shipped default that is
  missing") and the Swift reference; restore-defaults is an explicit,
  user-initiated action, not a background sync.
