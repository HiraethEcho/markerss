# config-nav-tags Spec

## ADDED Requirements

### Requirement: Config file

The app SHALL read settings from `$XDG_CONFIG_HOME/markerss/config` at startup, with TOML assumed for the extension-less file and format detected by extension otherwise. Missing file SHALL yield defaults without error; unknown keys SHALL be ignored; reload SHALL happen at startup only.

#### Scenario: Read config from XDG config dir
Given `$XDG_CONFIG_HOME/markerss/config`, then app settings load at startup. Missing file → defaults, no error.

#### Scenario: Formats by extension
Given `config` (no extension), then TOML is assumed. Given `config.toml` / `config.json` / `config.jsonc` / `config.yaml` / `config.yml`, then the matching format is parsed.

#### Scenario: Unknown keys ignored
Given a config with unrecognized keys, then they are ignored; app still starts.

#### Scenario: Startup-only reload
Config is read once at startup; changes require restart.

### Requirement: Config keys

The app SHALL honor `cache_ttl_days`, `export_dir`, `browser`, `refresh`, `fetch_timeout`, `max_items_per_feed`, `theme`, `pane_ratio`, `nav_presets`, `default_view`, `images`, `proxy`, `keybindings` when present.

#### Scenario: cache_ttl_days
Given `cache_ttl_days = 7`, then startup content purge uses 7 days.

#### Scenario: export_dir
Given `export_dir = "/data/out"`, then exports write under that dir.

#### Scenario: browser
Given `browser = "firefox"`, then `o` opens the article URL with firefox (instead of the platform default).

#### Scenario: refresh
Given `refresh = false`, then no auto-refresh at startup. Given `refresh = { interval_minutes = 30 }`, then auto-refresh runs on that interval; `r` manual refresh always works.

#### Scenario: fetch_timeout / max_items_per_feed
Given `fetch_timeout = 15` and `max_items_per_feed = 50`, then fetches time out at 15s and per-feed storage caps at 50 items.

#### Scenario: theme
Given `theme = "~/.config/markerss/themes/dark.toml"`, then colors load from that file (MVP: minimal color mapping).

### Requirement: Nav pane presets

The nav pane SHALL render sections per preset; presets are arrays of section names. Default preset = `["Unread", "Read Later", "Favourite", "Categories", "Tags", "Saved"]`; `nav_presets` replaces the list (first entry = initial); `t` cycles presets.

#### Scenario: pane_ratio
Given `pane_ratio = [0.2, 0.2, 0.6]`, then nav/list/article widths use those ratios.

#### Scenario: default full preset
Default nav preset = `["Unread", "Read Later", "Favourite", "Categories", "Tags", "Saved"]`.

#### Scenario: nav_presets override
Given `nav_presets = [["Unread", "Feeds"], ["Unread", "Later"]]`, then the preset list is replaced; the first entry becomes the initial preset.

#### Scenario: t cycles presets
Given multiple presets, then `t` cycles through them in order (wrap).

### Requirement: Feed tags

Feeds SHALL carry 0..n tags from `#tag` words in the urls file plus exactly one category (`#`-less words). The nav SHALL list feed tags below Categories; selecting a tag SHALL filter the list to feeds carrying it. (No per-item tags.)

#### Scenario: #tag in urls file
Given line `https://x.com/f "T" category #tech #rust`, then the feed carries tags tech, rust plus category `category`.

#### Scenario: tags list in nav
The nav pane shows a Tags section (below Categories) listing all feed tags.

#### Scenario: tag filters feeds
Selecting a tag in nav filters the list to items of feeds carrying that tag.

### Requirement: Feed favourite

Feeds SHALL carry a favourite flag, toggled with `f` on a nav feed row. The Favourite virtual node SHALL list favourited feeds (same presentation as the category tree). The flag persists in the urls file.

#### Scenario: favourite toggle from nav
`f` on a feed row toggles that feed's favourite flag; the flag persists in the urls file.

#### Scenario: favourite node lists feeds
The Favourite node shows the favourited feeds; entering one opens its list.

### Requirement: Virtual item nodes

Read Later and Saved SHALL appear as virtual nav nodes aggregating items with the corresponding flag, across all feeds, like All Unread.

#### Scenario: read-later / saved aggregation
Each node aggregates items with its flag set, across all feeds.

#### Scenario: toggle from article view
`L` toggles read-later on the current item, `S` toggles saved. Flags are independent; an item can carry both.

#### Scenario: saved exempt from TTL
Startup content purge skips items with `saved` set.

#### Scenario: saved kept without markdown
`saved` items stay in the DB with content; no markdown is generated for them unless exported.

### Requirement: Item flags persistence

Item flags SHALL be stored in the DB per item and survive restart/refresh.

#### Scenario: persisted per item
Item flags survive restart and feed refresh.
