## Context

Depends on `settings-tabs-shell` for the Terminal tab slot.
`terminal_font_name`/`terminal_font_size` already exist in
`knot_core::Settings` but are read by nothing in `knot-terminal` today —
confirmed by grep, no other crate references either field. This change
only adds the editor; it does not make Ghostty rendering respect these
values.

## Goals / Non-Goals

**Goals:**
- Font name + size controls, persisting to the existing fields.

**Non-Goals:**
- Engine selection (Ghostty-only per this port's stack mapping).
- Color pickers (no backing fields; Ghostty reads its own config file for
  colors, which this port doesn't parse).
- Wiring these values into actual terminal rendering (`knot-terminal`) —
  separate, later change.

## Decisions

- **Static font shortlist ported from the Swift reference's
  `monospaceFonts` array, not filtered by installed-font detection** — the
  Swift version filters against `NSFontManager.shared.availableFontFamilies`;
  `gpui` has no equivalent system-font-enumeration API surfaced through
  `gpui-kit` today, and adding one is out of scope for a settings field.
  Showing the full static list (some entries may not render if the font
  isn't installed) is the honest minimal option until font enumeration
  exists.
- **Numeric field for size, not a slider** — the Swift reference uses a
  `Slider`, but this codebase has no slider widget in use anywhere yet
  (`gpui-kit`/`gpui-component` inventory checked: no `slider` module);
  a plain numeric input matches every other scalar field in this window
  and avoids introducing a new widget for one control.

## Risks / Trade-offs

- [Risk] Editing a "dead" setting (nothing renders with it yet) may read as
  broken to a user → Mitigation: same situation already accepted for
  `appearance_mode` in `settings-ui-port`; flagged explicitly as a
  Non-Goal in the proposal rather than silently shipped.

## Migration Plan

Additive only — fills in an existing placeholder tab, no data migration.
