# PLAN

Progress per branch (rust / go / cpp). `##` = spec, `###` = branch. Each branch implements all five specs, in order MVP → Config → Tags & Favorites → Article Polish → Advanced.

## mvp

### rust

- [x] Phase 1: Scaffold — cargo project, ratatui three-pane layout (nav/list/article), XDG dirs
- [x] Phase 2: Feed source — newsboat `urls` parser, categories from tags, quoted title = display name
- [x] Phase 3: State — read status persistence, All Unread aggregation
- [x] Phase 4: Nav tree — categories → feeds, nested categories, h/l collapse, feed/category CRUD (rewrites urls file)
- [x] Phase 5: Reading flow — summary in header, enter = read + full content, fetch on demand, n/p/j/k/ctrl+u/ctrl+d keys
- [x] Phase 6: Refresh — startup auto + `r` manual, feed-rs, threaded, error handling
- [x] Phase 7: Export — `e` → frontmatter + full content markdown, `<category>/<slug>.md`
- [x] Phase 8: Cache — gzipped HTML + TTL cleanup config
- [x] Phase 9: Polish — empty states, help bar (`?`), README usage

MVP complete — pending user review + archive.

### go

- [x] Phase 1: Scaffold — go module, bubbletea skeleton, three-pane layout (nav/list/article), XDG dirs
- [x] Phase 2: Feed source — newsboat `urls` parser (category + `#tags`), quoted title = display name
- [x] Phase 3: State — read status persistence, All Unread aggregation
- [x] Phase 4: Nav pane — virtual nodes (Unread/Read Later/Favourite/Saved) + Categories tree (nested) + Tags list, h/l collapse, category CRUD (rewrites urls file)
- [x] Phase 5: Reading flow — left/right nav (h/l/q/enter/esc/←/→): expand→list→article+read→fetch full; n/p/j/k keys
- [x] Phase 6: Refresh — startup auto + `r` manual, gofeed, error handling
- [x] Phase 7: Export — `e` → frontmatter + full content markdown, title-slug filename
- [x] Phase 8: Cache — gzipped HTML + metadata, TTL cleanup config
- [x] Phase 9: Polish — empty states, spinner, help bar, README usage

### cpp

- [ ] Phase 1: Scaffold — build system, FTXUI skeleton, three-pane layout (nav/list/article), XDG dirs
- [ ] Phase 2: Feed source — newsboat `urls` parser (category + `#tags`), quoted title = display name
- [ ] Phase 3: State — read status persistence, All Unread aggregation
- [ ] Phase 4: Nav pane — virtual nodes (Unread/Read Later/Favourite/Saved) + Categories tree (nested) + Tags list, h/l collapse, category CRUD (rewrites urls file)
- [ ] Phase 5: Reading flow — left/right nav (h/l/q/enter/esc/←/→): expand→list→article+read→fetch full; n/p/j/k keys
- [ ] Phase 6: Refresh — startup auto + `r` manual, libcurl, error handling
- [ ] Phase 7: Export — `e` → frontmatter + full content markdown, title-slug filename
- [ ] Phase 8: Cache — gzipped HTML + metadata, TTL cleanup config
- [ ] Phase 9: Polish — empty states, spinner, help bar, README usage

## config

### rust

- [ ] Phase 1: Config file parse — JSON / JSONC / TOML / YAML by extension; keys `cache_ttl_days` / `export_dir` / `pane_ratio` / `theme` / `browser` / `refresh` / `nav_pane` / `images`
- [ ] Phase 2: Wire config — cache TTL purge, export path, pane widths, browser, refresh behavior
- [ ] Phase 3: Theme file load + defaults + XDG fallbacks when keys absent
- [ ] Phase 4: Nav pane layouts — full + simple, `t` toggle, `nav_pane` override

### go

- [ ] Phase 1: Config file parse — JSON / JSONC / TOML / YAML by extension; keys `cache_ttl_days` / `export_dir` / `pane_ratio` / `theme` / `browser` / `refresh` / `nav_pane` / `images`
- [ ] Phase 2: Wire config — cache TTL purge, export path, pane widths, browser, refresh behavior
- [ ] Phase 3: Theme file load + defaults + XDG fallbacks when keys absent
- [ ] Phase 4: Nav pane layouts — full + simple, `t` toggle, `nav_pane` override

### cpp

- [ ] Phase 1: Config file parse — JSON / JSONC / TOML / YAML by extension; keys `cache_ttl_days` / `export_dir` / `pane_ratio` / `theme` / `browser` / `refresh` / `nav_pane` / `images`
- [ ] Phase 2: Wire config — cache TTL purge, export path, pane widths, browser, refresh behavior
- [ ] Phase 3: Theme file load + defaults + XDG fallbacks when keys absent
- [ ] Phase 4: Nav pane layouts — full + simple, `t` toggle, `nav_pane` override

## tags-favorites

### rust

- [ ] Phase 1: Flags — per-item `read_later` / `favorite` / `saved` in DB, toggle from article view; virtual nodes Read Later / Favourite / Saved in nav
- [ ] Phase 2: Tags list in nav pane (below Categories) — from feed `#tags`; select tag → filter list; assign tags to items from article view
- [ ] Phase 3: Saved semantics — exempt from TTL cleanup, kept in DB without markdown

### go

- [ ] Phase 1: Flags — per-item `read_later` / `favorite` / `saved` in DB, toggle from article view; virtual nodes Read Later / Favourite / Saved in nav
- [ ] Phase 2: Tags list in nav pane (below Categories) — from feed `#tags`; select tag → filter list; assign tags to items from article view
- [ ] Phase 3: Saved semantics — exempt from TTL cleanup, kept in DB without markdown

### cpp


- [ ] Phase 1: Flags — per-item `read_later` / `favorite` / `saved` in DB, toggle from article view; virtual nodes Read Later / Favourite / Saved in nav
- [ ] Phase 2: Tags list in nav pane (below Categories) — from feed `#tags`; select tag → filter list; assign tags to items from article view
- [ ] Phase 3: Saved semantics — exempt from TTL cleanup, kept in DB without markdown

## article-polish

### rust

- [ ] Phase 1: Reading width ~80 cols, paragraph spacing, heading hierarchy
- [ ] Phase 2: Element styling — lists, code blocks, inline code, blockquotes, tables, hr, links, images
- [ ] Phase 3: Scroll (j/k, ctrl-u/d, n/p) + scrollbar + viewport culling + theme colors
- [ ] Phase 4: Enhancement — images via kitty protocol, `g` toggle + `images` config, `[img]` fallback
- [ ] Phase 5: Enhancement — link jump `f` + hints (1-9/letters) + open in browser

### go

- [ ] Phase 1: Reading width ~80 cols, paragraph spacing, heading hierarchy
- [ ] Phase 2: Element styling — lists, code blocks, inline code, blockquotes, tables, hr, links, images
- [ ] Phase 3: Scroll (j/k, ctrl-u/d, n/p) + scrollbar + viewport culling + theme colors
- [ ] Phase 4: Enhancement — images via kitty protocol, `g` toggle + `images` config, `[img]` fallback
- [ ] Phase 5: Enhancement — link jump `f` + hints (1-9/letters) + open in browser

### cpp

- [ ] Phase 1: Reading width ~80 cols, paragraph spacing, heading hierarchy
- [ ] Phase 2: Element styling — lists, code blocks, inline code, blockquotes, tables, hr, links, images
- [ ] Phase 3: Scroll (j/k, ctrl-u/d, n/p) + scrollbar + viewport culling + theme colors
- [ ] Phase 4: Enhancement — images via kitty protocol, `g` toggle + `images` config, `[img]` fallback
- [ ] Phase 5: Enhancement — link jump `f` + hints (1-9/letters) + open in browser

## advanced

### rust
- [ ] Phase 1: OPML import — nested folders → nested categories, `category` attr → feed tags
- [ ] Phase 2: OPML export — categories → nested folders, tags → `category` attr (round-trip)

### go
- [ ] Phase 1: OPML import — nested folders → nested categories, `category` attr → feed tags
- [ ] Phase 2: OPML export — categories → nested folders, tags → `category` attr (round-trip)

### cpp
- [ ] Phase 1: OPML import — nested folders → nested categories, `category` attr → feed tags
- [ ] Phase 2: OPML export — categories → nested folders, tags → `category` attr (round-trip)
