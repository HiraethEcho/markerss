# Change Proposal: config-nav-tags

Config + Nav-pane presets + Tags & Favorites, per SPEC.md `## Config` and
`## Tags & Favorites` and the nav-pane parts of `## MVP`.

## Why

MVP hardcodes pane widths, browser, refresh, TTL, export dir. This change
makes settings user-configurable, adds multiple nav layout presets, and
implements item organization (read-later / favorite / saved flags, feed
tags, tags list in nav, per-item tags).

## ADDED Requirements

### Config file

#### Scenario: Read config from XDG config dir
Given `$XDG_CONFIG_HOME/markerss/config`, then app settings load at startup.
Missing file → defaults, no error.

#### Scenario: Formats by extension
Given `config` (no extension), then TOML is assumed. Given
`config.toml` / `config.json` / `config.jsonc` / `config.yaml` / `config.yml`,
then the matching format is parsed.

#### Scenario: Unknown keys ignored
Given a config with unrecognized keys, then they are ignored; app still starts.

#### Scenario: Startup-only reload
Config is read once at startup; changes require restart.

### Config keys

#### Scenario: cache_ttl_days
Given `cache_ttl_days = 7`, then startup content purge uses 7 days.

#### Scenario: export_dir
Given `export_dir = "/data/out"`, then exports write under that dir.

#### Scenario: browser
Given `browser = "firefox"`, then `o` opens the article URL with firefox
(instead of the platform default).

#### Scenario: refresh
Given `refresh = false`, then no auto-refresh at startup. Given
`refresh = { interval_minutes = 30 }`, then auto-refresh runs on that
interval; `r` manual refresh always works.

#### Scenario: fetch_timeout / max_items_per_feed
Given `fetch_timeout = 15` and `max_items_per_feed = 50`, then fetches time
out at 15s and per-feed storage caps at 50 items.

#### Scenario: theme
Given `theme = "~/.config/markerss/themes/dark.toml"`, then colors load from
that file (MVP: minimal color mapping).

### Nav pane presets

#### Scenario: pane_ratio
Given `pane_ratio = [0.2, 0.2, 0.6]`, then nav/list/article widths use those
ratios.

#### Scenario: default full preset
Default nav preset = `["Unread", "Read Later", "Favourite", "Categories",
"Tags", "Saved"]`.

#### Scenario: nav_presets override
Given `nav_presets = [["Unread", "Feeds"], ["Unread", "Later"]]`, then the
preset list is replaced; the first entry becomes the initial preset.

#### Scenario: t cycles presets
Given multiple presets, then `t` cycles through them in order (wrap).

### Favourite (feed-level)

#### Scenario: toggle from nav
`f` on a feed row toggles that feed's favourite; persists in urls file.

#### Scenario: Favourite node lists feeds
Favourite = virtual node listing favourited feeds (like the category tree).

### Read Later / Saved (item-level)

#### Scenario: aggregation
Read Later / Saved aggregate items with their flag set, across all feeds —
same pattern as All Unread.

#### Scenario: toggle from article view
`L` toggles read-later, `S` toggles saved on the current item. Flags are
independent; an item can carry both.

#### Scenario: saved exempt from TTL
Startup content purge skips items with `saved` set.

#### Scenario: saved kept without markdown
`saved` items stay in the DB with content; no markdown is generated for them
unless exported.

### Feed tags

#### Scenario: #tag in urls file
Given line `https://x.com/f "T" category #tech #rust`, then the feed carries
tags tech, rust plus category `category`; `#`-less words are categories.

#### Scenario: tags list in nav
The nav pane shows a Tags section (below Categories) listing all feed tags.

#### Scenario: tag filters feeds
Selecting a tag in nav filters the list to items of feeds carrying that tag.


## Out of Scope

- TUI config editing (file editing only)
- Keybinding remapping UI (config key accepted, applied at startup)
- Hot reload
- Theme beyond minimal color mapping
