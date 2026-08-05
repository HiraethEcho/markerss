# Tasks — config-nav-tags

## 1. Config loader
- [ ] Formats: TOML (`config`, `.toml`), JSON (`.json`), JSONC (`.jsonc`, strip comments), YAML (`.yaml`/`.yml`)
- [ ] `Config` gains: `browser`, `refresh_on_startup`, `refresh_interval_minutes`, `fetch_timeout`, `max_items_per_feed`, `theme_path`, `pane_ratio: [f64;3]`, `nav_presets: Vec<Vec<String>>`, `default_view: Option<String>`
- [ ] Unknown keys ignored; missing → defaults; tests per format + unknown-key + missing-file

## 2. Apply keys
- [ ] `browser` used by `open_browser` (fallback xdg-open)
- [ ] `fetch_timeout` on http client; `max_items_per_feed` cap in refresh
- [ ] `refresh` on/off + interval timer
- [ ] `theme` minimal color mapping

## 3. Nav presets
- [ ] Preset model: default full preset; `nav_presets` override (first = initial)
- [ ] `t` cycles presets (wrap); pane_ratio drives widths

## 4. DB: flags + item tags
- [ ] items table: `read_later`/`favorite`/`saved` INTEGER columns + `tags` TEXT
- [ ] Set/get/toggle flag APIs; TTL cleanup `WHERE saved = 0`
- [ ] Refresh preserves flags + item tags (extend preserve map)

## 5. Virtual nodes
- [ ] Scope::ReadLater / Favorite / Saved — aggregate by flag across feeds
- [ ] Nav renders nodes; counts like All Unread

## 6. Feed tags
- [ ] urls parser: `#tag` → tags, `#`-less → categories (existing)
- [ ] Tags section in nav below Categories; selection filters feeds
- [ ] CRUD preserves tags on rewrite

## 7. Item tags from article
- [ ] `T` in article → input modal → per-item tags; persisted

## 8. Flags keys
- [ ] `Y`/`L`/`S` toggle favourite/read-later/saved in article view

## 9. Verify
- [ ] `cargo build` + `cargo test` green
- [ ] `lightspec validate config-nav-tags --strict`
