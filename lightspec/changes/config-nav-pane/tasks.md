# Tasks — config-nav-pane

## 1. Config loader
- [ ] TOML parse (toml crate) for `config` + `.toml`; JSON for `.json`; JSONC (comments stripped) for `.jsonc`; YAML for `.yaml`/`.yml`
- [ ] `Config` struct gains: `browser`, `refresh_on_startup`, `refresh_interval_minutes`, `fetch_timeout`, `max_items_per_feed`, `theme_path`, `pane_ratio: [f64;3]`, `nav_pane: Option<Vec<String>>`, `default_view: Option<String>`
- [ ] Unknown keys ignored; missing → defaults; unit tests per format + unknown-key + missing-file

## 2. Apply keys
- [ ] `browser` used by `open_browser` (fallback xdg-open)
- [ ] `cache_ttl_days` purge (exists) + `fetch_timeout` on http client
- [ ] `max_items_per_feed` cap in refresh
- [ ] `refresh` on/off + interval (startup auto-refresh gated; interval timer)
- [ ] `theme` minimal color mapping (startup load)
- [ ] `export_dir` (exists)

## 3. Nav pane layout
- [ ] `pane_ratio` drives Layout::horizontal constraints
- [ ] Section list: full default / simple default / `nav_pane` override
- [ ] `t` toggles full↔simple (no-op with override)
- [ ] Empty sections render as headers (Read Later / Favourite / Saved / Tags)

## 4. Default view
- [ ] Parse `Feed:<url>` / `Category:<name>` at startup → initial scope

## 5. Verify
- [ ] `cargo build` + `cargo test` green
- [ ] `lightspec validate config-nav-pane --strict`
