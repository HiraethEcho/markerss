# Tasks — config-nav-tags

## 1. Config loader
- [x] Formats: TOML (`config`, `.toml`), JSON (`.json`), JSONC (`.jsonc`, strip comments), YAML (`.yaml`/`.yml`)
- [x] `Config` gains: `browser`, `refresh_on_startup`, `refresh_interval_minutes`, `fetch_timeout`, `max_items_per_feed`, `theme_path`, `pane_ratio: [f64;3]`, `nav_presets: Vec<Vec<String>>`, `default_view: Option<String>`
- [x] Unknown keys ignored; missing → defaults; tests per format + unknown-key + missing-file

## 2. Apply keys
- [x] `browser` used by `open_browser` (fallback xdg-open)
- [x] `fetch_timeout` on http client; `max_items_per_feed` cap in refresh
- [x] `refresh` on/off + interval timer
- [x] `theme` minimal color mapping

## 3. Nav presets
- [x] Preset model: default full preset; `nav_presets` override (first = initial)
- [x] `t` cycles presets (wrap); pane_ratio drives widths

## 4. DB: flags + item tags
- [x] items table: `read_later`/`favorite`/`saved` INTEGER columns + `tags` TEXT
- [x] Set/get/toggle flag APIs; TTL cleanup `WHERE saved = 0`
- [x] Refresh preserves flags (extend preserve map)
- [x] Feed favourite flag round-trips through urls file

## 5. Virtual nodes
- [x] Scope::ReadLater / Favorite / Saved — aggregate by flag across feeds
- [x] Nav renders nodes; counts like All Unread

## 6. Feed tags
- [x] urls parser: `#tag` → tags, `#`-less → categories (existing)
- [x] Tags section in nav below Categories; selection filters feeds
- [x] CRUD preserves tags on rewrite

## 8. Flags keys
- [x] `f` on nav feed row toggles favourite; `L`/`S` in article toggle read-later/saved

## 9. Verify
- [x] `cargo build` + `cargo test` green
- [x] `lightspec validate config-nav-tags --strict`
