## ADDED Requirements

### Requirement: Config File Location and Format

The system SHALL read app settings from `$XDG_CONFIG_HOME/markerss/config.toml`
(default TOML). JSON (`.json`), JSONC (`.jsonc`) and YAML (`.yaml`/`.yml`)
SHALL be supported and detected by file extension. The config file SHALL be
separate from the `urls` subscriptions file.

#### Scenario: TOML default

- **WHEN** no config file exists or it is named `config.toml`
- **THEN** settings are read from TOML with defaults for absent keys

#### Scenario: Format by extension

- **WHEN** the file is `config.yaml` or `config.json`
- **THEN** it is parsed as YAML or JSON respectively

### Requirement: Config Keys

The system SHALL support keys: `cache_ttl_days`, `export_dir`,
`pane_ratio` (three floats, default `[0.15, 0.15, 0.7]`), `theme` (theme
file path), `browser` (default `xdg-open`), `refresh`
(auto-on-startup bool, `interval_minutes`), `nav_presets` (array of section
arrays), `images` (bool), `foldlevel` (initial fold depth), `sort` (initial
sort stack, max 3), `fetch_timeout`, `max_items_per_feed`, `proxy`,
`keybindings`, `default_view`.

#### Scenario: Absent keys

- **WHEN** a key is absent
- **THEN** the documented default applies (XDG fallbacks per spec)

### Requirement: Config Wiring

The system SHALL apply config at startup: TTL purge of fetched content older
than `cache_ttl_days`; export writes to `export_dir`; pane widths from
`pane_ratio`; browser from `browser`; refresh interval from `refresh`;
per-request timeout from `fetch_timeout`; per-feed item cap from
`max_items_per_feed`.

#### Scenario: Export dir override

- **WHEN** `export_dir` is set
- **THEN** `e` writes markdown under that directory instead of the default

#### Scenario: Pane widths

- **WHEN** `pane_ratio` is `[0.2, 0.3, 0.5]` at 100 cols
- **THEN** panes are ~20 / 30 / 50 columns

### Requirement: Theme File

The system SHALL load colors from the `theme` file (standalone, separate
from config). Absent theme SHALL fall back to the default palette.

#### Scenario: Theme load

- **WHEN** `theme` points to an existing file
- **THEN** UI colors come from that file

### Requirement: Nav Pane Presets

The nav pane SHALL be preset-driven: each preset is an ordered array of
sections; the default full preset is `[Unread, Read Later, Favourite,
Categories, Tags, Saved]`. Available sections: `Unread`, `Read Later`,
`Favourite`, `Saved`, `Categories` (tree), `Tags`, `Feeds`; `No Category`
renders automatically at the end of Categories. `nav_presets` replaces the
list; the first entry is the initial preset; `t` cycles all presets
(wrapping).

#### Scenario: Default preset

- **WHEN** no `nav_presets` configured
- **THEN** the nav pane shows the full default preset

#### Scenario: Custom presets

- **WHEN** `nav_presets = [["Unread", "Feeds"], ["Unread", "Later"]]`
- **THEN** the app starts with `[Unread, Feeds]` and `t` toggles to
  `[Unread, Later]`

### Requirement: Nav Fold Semantics

Sections Unread / Read Later / Saved SHALL NOT fold. Favourite, Categories,
Tags, Feeds, No Category SHALL be foldable. Left (`h`/`q`/`esc`): expanded
header folds; folded header folds its parent; top-level folded entry stays.
Right (`l`/`enter`): folded entry expands and jumps the cursor to its first
child; expanded node or leaf descends to the list pane. All top entries SHALL
be highlighted (distinct fg + bold).

#### Scenario: Fold cascade

- **WHEN** cursor is on a folded category inside Categories and `h` is pressed
- **THEN** Categories folds; a second `h` at the folded top stays

#### Scenario: Expand jumps to first child

- **WHEN** `l` is pressed on a folded tag
- **THEN** the tag expands and the cursor lands on its first feed

### Requirement: Startup View

The system SHALL accept `default_view` (`Feed:<url>` / `Category:<name>`)
as the startup scope.

#### Scenario: Category start

- **WHEN** `default_view = "Category:tech"`
- **THEN** the app opens with the tech category list shown
