## MODIFIED Requirements

### Requirement: Window scope

The Terminal tab SHALL show a "Font" section with a font-name picker bound
to `terminal_font_name` and a numeric size field bound to
`terminal_font_size`, persisting on change. It SHALL NOT show an engine
picker or color pickers.

#### Scenario: Changing the font name persists

- **WHEN** the user picks "JetBrains Mono" in the font picker
- **THEN** `terminal_font_name` is saved as `"JetBrains Mono"` immediately

#### Scenario: Changing the font size persists

- **WHEN** the user sets the size field to `14`
- **THEN** `terminal_font_size` is saved as `14.0` immediately
