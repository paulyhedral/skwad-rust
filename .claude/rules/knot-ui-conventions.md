---
paths:
  - "crates/knot/src/**"
---

# Knot GPUI settings/dialog conventions

These conventions came out of repeated user feedback on the settings window
and dialogs; follow them for any new pane, dialog, or window in `crates/knot`.

- Labels: right-aligned in a fixed-width column (`SettingsWindow::LABEL_WIDTH`
  via `row`/`text_row`), not left-aligned or inline.
- Row alignment: use `row` (`items_center`) for switches/buttons/inputs, and
  `text_row` (`items_baseline`) when both the label and the value are plain
  text (e.g. a computed model name, a URL), so text baselines line up.
- Hints: use the shared `hint()` helper, never a raw `div()` with muted text.
  It aligns under the control column and wraps instead of overflowing.
- Sectioning: group related rows in a titled `GroupBox` (`SettingsWindow::group`),
  not a bare `v_flex`. Section titles must be visually larger than row content.
- Scroll regions: only the specific list/content that can grow unboundedly
  should scroll (e.g. the persona list). Never nest that scroll region inside
  another scrolling container - the outer scroll steals gesture priority and
  makes the section header/border appear to drift with the content.
- Buttons: prefer an icon + tooltip (`SettingsWindow::icon_button`) over a
  text label for actions with an obvious icon (add/edit/delete/copy/restore).
  Tint the icon red for destructive actions - keep the button `.ghost()`,
  never `.danger()` (that variant replaces `.ghost()` outright, since
  `ButtonVariants` is a single mutually-exclusive field).
- Destructive/restore actions: always confirm via `window.open_alert_dialog`
  before applying (delete persona, restore defaults, delete workspace, etc).
- Monospace: use `cx.theme().mono_font_family` (or `SettingsWindow::mono_text`)
  for anything code/identifier-like - API keys, model names, URLs, commands.
- Flex overflow: any text/row meant to wrap or scroll inside a flex row needs
  `min_w_0()` on the immediate flex child, not just `flex_1()` - `flex_1()`
  alone keeps the browser/GPUI default `min-width: auto` and the content
  overflows its container instead of wrapping.
- Native pickers/panels over in-app equivalents where the OS provides one
  (e.g. `NSFontManager`/`NSFontPanel` for font choice, folder pickers via
  `cx.prompt_for_paths`, `NSApplication.orderFrontCharacterPalette` for an
  emoji/character picker - not a curated list of a handful of options).
  When bridging an AppKit callback that doesn't fire reliably (e.g.
  `changeFont:` can be intercepted by AppKit's own responder chain), poll
  the relevant `NSFontManager`/AppKit property directly instead of relying
  on the target/action message. `orderFrontCharacterPalette` needs no such
  bridging - it inserts the chosen character straight into whatever text
  field has keyboard focus, so just focus the target `InputState` first.
- Dialog windows: put the window's purpose in the OS titlebar (`TitlebarOptions.title`),
  not as an in-body heading. Autofocus the first meaningful input on open
  (`InputState::focus`/`TextareaState::focus`). Anchor action buttons to the
  bottom-right via `justify_end()` on a `flex_shrink_0()` row, with any
  growing content area (e.g. a multi-line textarea) as the `flex_1()` sibling
  above it. Use "Create" (not "Save") for a brand-new-item confirm button, and
  "Save" only when editing an existing item.
- Title bars: prefer gpui-kit's integrated `TitleBar` (`TitleBar::window_options()`
  + `TitleBar::new()` as the render's first child) over the native OS titlebar
  for app windows (workspace manager, workspace window). Use a native
  `TitlebarOptions` title only for simple utility dialogs (settings, persona
  editor) that don't need custom titlebar content. `TitleBar` draws its own
  `border_b_1` by default - override it with `.border_color(gpui_kit::transparent_black())`
  when the window body should read as one continuous surface with no seam.
  Don't duplicate the window's title as an in-body heading once it's shown in
  a `TitleBar` or `TitlebarOptions.title` - one title per window, not two.
- Form validation: a primary submit button (e.g. "Add Agent") should be
  `.disabled(true)` until every required field is filled, not just rejected
  with an error after the click. Wire a `Change` subscription on any input
  the disabled check depends on so the button's enabled state updates live
  as the user types, not only on the next unrelated re-render. Save the
  click-time check too (defence in depth, and it produces the error message).
  Error text uses `cx.theme().danger`, not the default text color, and reads
  as an instruction ("Choose a folder.") not a restatement of the constraint
  ("Choose an existing agent folder.").
- Swift-parity dialogs (agent editor, and similar): when asked to match the
  Swift reference (`Skwad/Views/...`) rather than follow the Settings
  window's own conventions, check the actual Swift source and, if present,
  a matching screenshot under `images/screenshots/` before rewriting -
  don't guess the layout from memory. That reference uses left-aligned
  labels with a trailing control (`LabeledContent` style, via
  `AgentEditor::dialog_row`) and borderless filled card sections
  (`GroupBox::new().fill()`, via `AgentEditor::dialog_section`), which is
  deliberately different from the Settings window's right-aligned label
  column and titled/outlined `GroupBox` - the two dialogs are allowed to
  diverge because they're copying different references.
- Agent list rows (workspace sidebar): avatar at a fixed size with no
  background box (plain emoji glyph, matching Swift's `AvatarView`), name
  bold, persona line prefixed with an icon when the agent has one, then a
  status/title line and a folder-name line (last path component only, not
  the full path), with a small state-colored dot trailing when the agent
  isn't a shell agent. `AgentState` dot colors (diverges from the Swift
  reference, which uses red for both): idle=green, working=orange,
  awaiting-input=blue, error=red - awaiting-input needs its own color since
  it isn't a failure state.
- Single-character fields (an avatar/emoji slot): back them with a real
  `InputState`, but clamp on `Change` to the first extended grapheme cluster
  (`unicode_segmentation::UnicodeSegmentation::graphemes(s, true).next()`,
  not `.chars()` - a naive char split breaks multi-codepoint emoji). Before
  invoking a native picker that inserts at the focus point (character
  palette, etc.), clear the field first so the insertion replaces rather
  than appends.
