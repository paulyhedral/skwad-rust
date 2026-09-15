## Context

`SettingsWindow` (from `settings-ui-port`) currently renders General's three
sections directly in `Render::render`. Six sibling pane changes are queued
up behind this one; each needs a tab to render into without touching how
the others are wired.

## Goals / Non-Goals

**Goals:**
- A `SettingsTab` enum and selection field on `SettingsWindow`, with a tab
  strip rendered above the pane body.
- General's existing render logic moves into a `render_general` method
  unchanged; other tabs get a one-line placeholder method until their
  change lands.

**Non-Goals:**
- Any real content for Coding/Personas/Autopilot/Voice/MCP/Terminal — six
  separate changes.
- Persisting which tab was last selected — always opens on General, per
  the Swift reference's default `@State private var selectedTab: SettingsTab = .general`.

## Decisions

- **`SettingsTab` enum + `selected_tab` field, not seven separate windows**
  — matches the Swift reference's single `TabView`; a settings window per
  pane would be a worse UX (seven windows to manage) with no compensating
  benefit.
- **Tab strip built from `Button`/`h_flex` row with `Selectable`, not a
  dedicated tab-bar component** — `gpui_kit::base::Selectable` is already
  imported in `main.rs` (used elsewhere for row selection state); reusing
  it avoids pulling in a new tab-bar widget for seven buttons.
- **Placeholder panes render inline, not a "coming soon" popup or disabled
  tab** — keeps every tab clickable and visible from day one so the full
  shape of Settings is visible in review even before each pane ships,
  matching how `settings-ui-port`'s task list already flagged "no settings
  screen exists yet" as a recurring gap other changes hit.

## Risks / Trade-offs

- [Risk] Six placeholder tabs ship a visibly incomplete window → Mitigation:
  explicitly acceptable per the phased plan; each pane change fills one in
  and the placeholder text sets that expectation for anyone testing the
  build in between.

## Migration Plan

Additive/refactor only. `settings-ui-port`'s "Window scope" requirement is
superseded per the Modified Capabilities note; no data migration.
