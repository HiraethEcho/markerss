## 1. Implementation

- [x] Phase 1: Config file parse — TOML default (`config.toml`) + JSON /
      JSONC / YAML by extension; keys `cache_ttl_days` / `export_dir` /
      `pane_ratio` / `theme` / `browser` / `refresh` / `nav_presets` /
      `images` / `foldlevel` / `sort` / `fetch_timeout` /
      `max_items_per_feed` / `proxy` / `keybindings` / `default_view`
- [x] Phase 2: Wire config — cache TTL purge (startup), export path,
      pane widths, browser, refresh interval, fetch timeout, max items
- [x] Phase 3: Theme file load + defaults + XDG fallbacks when keys absent
- [x] Phase 4: Nav pane rewrite — preset-driven sections, fold cascade
      (left = fold → fold parent; right = expand → first child), top
      entries highlighted, `t` cycles presets, `nav_presets` override
- [x] Phase 5: default_view — startup scope (`Feed:<url>` / `Category:<name>`)

## 2. Validation

- [x] `go build ./...`, `go vet ./...` clean
- [x] Config unit tests: parse each format, defaults, XDG fallback
- [x] Nav pane tests: preset render, fold cascade, `t` cycle
- [x] `go test ./...` all packages green
