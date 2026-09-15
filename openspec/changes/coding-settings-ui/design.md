## Context

Depends on `settings-tabs-shell` for the Coding tab slot. `source_base_folder`
is a plain `String` field; `agent_options` is a `BTreeMap<String, String>`
keyed by agent type, with no existing accessor methods — `main.rs` already
reads/writes `Settings` collection fields directly elsewhere (e.g.
`self.settings.saved_agents = ...` in `WorkspaceWindow::create_agent`), so
this follows the same convention rather than adding new `Settings` methods
for a single map lookup.

## Goals / Non-Goals

**Goals:**
- Source folder view/choose/clear, backed by `PathPromptOptions` (already
  used elsewhere in `main.rs` for folder pickers).
- Per-agent-type options editor over the existing `agent_options` map.

**Non-Goals:**
- "Open With" / IDE integration — no such feature exists in the Rust port.
- Custom command slots (`custom1`/`custom2`) — no matching field exists in
  `knot_core::Settings`; adding one is a `settings-persistence` change, not
  a UI one.

## Decisions

- **Local `selected_agent_type: String` field on `SettingsWindow`, not a
  new `Settings` scalar** — which agent type's options are currently being
  edited is view-local UI state, not something that needs to persist
  across settings-window sessions (the Swift reference doesn't persist it
  either — it's `@State`, reset each time the view appears).
- **Direct `BTreeMap` read/write over new accessor methods** — a single
  `.get()`/`.insert()` pair doesn't justify new `Settings` API surface;
  matches how `saved_agents`/`saved_workspaces` are already mutated
  directly from `main.rs`.

## Risks / Trade-offs

- [Risk] Free-text options field has no validation (could contain a shell
  metacharacter that breaks `build_agent_command`) → Mitigation: matches
  existing Swift behavior exactly (no validation there either); the
  command-building side already goes through `shell_escape` in
  `knot-agent-launch`, so this isn't a new hazard introduced by adding UI.

## Migration Plan

Additive only — fills in an existing placeholder tab, no data migration.
