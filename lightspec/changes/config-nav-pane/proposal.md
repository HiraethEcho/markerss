# Change Proposal: config-nav-pane

Config + Nav-pane phase of the rust branch, per SPEC.md `## Config` and the
nav-pane parts of `## MVP`.

## Why

MVP hardcodes: three-pane widths (15/15/70), xdg-open browser, startup
refresh, TTL, export dir. Users need control over these. Nav pane needs
configurable layout (full/simple) and an adjustable pane ratio.

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
Given a config containing unrecognized keys, then they are ignored and the
app still starts.

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
Given `theme = "~/.config/markerss/themes/dark.toml", then colors load from
that file (standalone; MVP: minimal color mapping).

### Nav pane layout

#### Scenario: pane_ratio
Given `pane_ratio = [0.2, 0.2, 0.6]`, then nav/list/article widths use those
ratios.

#### Scenario: two defaults + t toggle
Full default = Unread / Read Later / Favourite / Categories / Tags / Saved;
simple default = Unread / Feeds. `t` toggles between them. (Read Later /
Favourite / Saved / Tags render as empty sections until Tags & Favorites
lands; Categories tree and All Unread always render.)

#### Scenario: nav_pane array override
Given `nav_pane = ["Unread", "Feeds"]`, then exactly those sections show;
`t` no-ops.

### Default view

#### Scenario: default_view
Given `default_view = "Feed:https://x.com/f"`, then startup selects that
feed's list. Given `default_view = "Category:blog"`, startup selects that
category. Default: All Unread.

## Out of Scope

- Tags / Read Later / Favourite / Saved item logic (Tags & Favorites spec)
- Hot reload, in-app config editing
- Keybinding remapping UI (config key accepted, applied at startup)
