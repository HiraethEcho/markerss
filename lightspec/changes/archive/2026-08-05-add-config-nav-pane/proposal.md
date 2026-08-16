# Change: Config file + preset-driven nav pane

## Why

The go branch has no config support: cache TTL, export path, pane widths,
browser and refresh behavior are hardcoded. DESIGN.md (Config section + MVP
Layout) defines a configurable app (`config.toml` + optional formats, ~13
keys) and a preset-driven nav pane (sections, fold cascade, `t` cycling).
The current go nav pane predates that design (All Unread only, flat
categories) and must be rebuilt to the preset model as part of wiring
`nav_presets`.

## What Changes

- Config file at `$XDG_CONFIG_HOME/markerss/config.toml` (TOML default;
  JSON / JSONC / YAML by extension), separate from `urls`
- Keys: `cache_ttl_days`, `export_dir`, `pane_ratio`, `theme`, `browser`,
  `refresh` (auto-on-startup, interval), `nav_presets`, `images`,
  `foldlevel`, `sort`, `fetch_timeout`, `max_items_per_feed`, `proxy`,
  `keybindings`, `default_view`
- Wire config: TTL purge, export path, pane widths, browser, refresh
  interval, fetch timeout, max items
- Theme: standalone color file, loaded via `theme` key
- Nav pane: preset-driven sections (Unread / Read Later / Favourite /
  Categories / Tags / Saved / Feeds / No Category); foldable headers with
  left-cascade (h/q/esc) and right expand→first-child (l/enter); top
  entries highlighted; `t` cycles presets; `nav_presets` replaces the
  default list (first = initial)
- Read at startup; defaults + XDG fallbacks; no hot-reload

## Impact

- Affected specs: config (new), mvp (nav pane rebuilt to preset model)
- Affected code: internal/ui (nav pane rewrite, layout), new internal/config,
  main.go wiring, internal/store (TTL purge), internal/export (dir),
  internal/fetch (timeout, proxy, max items)
- New deps: TOML parser (e.g. BurntSushi/toml or pelletier/go-toml/v2)
- Note: go code currently implements the pre-preset nav design; virtual
  nodes (Read Later / Favourite / Saved) belong to Tags & Favorites spec —
  this change defines their place in presets, not their behavior
