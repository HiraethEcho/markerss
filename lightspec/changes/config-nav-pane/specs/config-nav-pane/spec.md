# config-nav-pane Spec

## ADDED Requirements

### Requirement: Config file

The app SHALL read settings from `$XDG_CONFIG_HOME/markerss/config` at startup, with TOML assumed for the extension-less file and format detected by extension otherwise. Missing file SHALL yield defaults without error; unknown keys SHALL be ignored; reload SHALL happen at startup only.

#### Scenario: Read config from XDG config dir
Given `$XDG_CONFIG_HOME/markerss/config`, then app settings load at startup. Missing file → defaults, no error.

#### Scenario: Formats by extension
Given `config` (no extension), then TOML is assumed. Given `config.toml` / `config.json` / `config.jsonc` / `config.yaml` / `config.yml`, then the matching format is parsed.

#### Scenario: Unknown keys ignored
Given a config containing unrecognized keys, then they are ignored and the app still starts.

#### Scenario: Startup-only reload
Config is read once at startup; changes require restart.

### Requirement: Config keys

The app SHALL honor `cache_ttl_days`, `export_dir`, `browser`, `refresh`, `fetch_timeout`, `max_items_per_feed`, `theme`, `pane_ratio`, `nav_pane`, `default_view`, `images`, `proxy`, `keybindings` when present.

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

### Requirement: Nav pane layout

The nav pane SHALL render sections per `pane_ratio` / `nav_pane` config with two defaults (full, simple) toggled by `t`.

#### Scenario: pane_ratio
Given `pane_ratio = [0.2, 0.2, 0.6]`, then nav/list/article widths use those ratios.

#### Scenario: two defaults + t toggle
Full default = Unread / Read Later / Favourite / Categories / Tags / Saved; simple default = Unread / Feeds. `t` toggles between them. (Read Later / Favourite / Saved / Tags render as empty sections until Tags & Favorites lands; Categories tree and All Unread always render.)

#### Scenario: nav_pane array override
Given `nav_pane = ["Unread", "Feeds"]`, then exactly those sections show; `t` no-ops.

### Requirement: Default view

The app SHALL select the initial list scope per `default_view`.

#### Scenario: default_view
Given `default_view = "Feed:https://x.com/f"`, then startup selects that feed's list. Given `default_view = "Category:blog"`, startup selects that category. Default: All Unread.
