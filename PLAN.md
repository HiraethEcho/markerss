# PLAN

Progress per branch (rust / go / cpp). `##` = spec, `###` = branch. Each branch implements all five specs, in order MVP → Config → Tags & Favorites → Article Polish → Advanced.

## mvp

### rust

- [x] Phase 1: Scaffold — cargo project, ratatui three-pane layout (nav/list/article), XDG dirs
- [x] Phase 2: Feed source — newsboat `urls` parser, categories from tags, quoted title = display name
- [x] Phase 3: State — read status persistence, All Unread aggregation
- [x] Phase 4: Nav tree — categories → feeds, nested categories, h/l collapse, feed/category CRUD (rewrites urls file)
- [x] Phase 5: Reading flow — summary in header, enter = read + full content, fetch on demand, n/p/j/k/ctrl+u/ctrl+d/space keys
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
- [x] Phase 5: Reading flow — left/right nav (h/l/q/enter/esc/←/→): expand→list→article+read→fetch full; n/p/j/k/space keys
- [x] Phase 6: Refresh — startup auto + `r` manual, gofeed, error handling
- [x] Phase 7: Export — `e` → frontmatter + full content markdown, title-slug filename
- [x] Phase 8: Cache — gzipped HTML + metadata, TTL cleanup config
- [x] Phase 9: Polish — empty states, spinner, help bar, README usage

### cpp

- [ ] Phase 1: Scaffold — build system, FTXUI skeleton, three-pane layout (nav/list/article), XDG dirs
- [ ] Phase 2: Feed source — newsboat `urls` parser (category + `#tags`), quoted title = display name
- [ ] Phase 3: State — read status persistence, All Unread aggregation
- [ ] Phase 4: Nav pane — virtual nodes (Unread/Read Later/Favourite/Saved) + Categories tree (nested) + Tags list, h/l collapse, category CRUD (rewrites urls file)
- [ ] Phase 5: Reading flow — left/right nav (h/l/q/enter/esc/←/→): expand→list→article+read→fetch full; n/p/j/k/space keys
- [ ] Phase 6: Refresh — startup auto + `r` manual, libcurl, error handling
- [ ] Phase 7: Export — `e` → frontmatter + full content markdown, title-slug filename
- [ ] Phase 8: Cache — gzipped HTML + metadata, TTL cleanup config
- [ ] Phase 9: Polish — empty states, spinner, help bar, README usage

## config

### rust

- [x] Phase 1: Config file (`config.toml`) parse — JSON / JSONC / TOML / YAML by extension; keys `cache_ttl_days` / `export_dir` / `pane_ratio` / `theme` / `browser` / `refresh` / `nav_presets` / `images` / `proxy` / `keybindings` / `default_view` / `fetch_timeout` / `max_items_per_feed`
- [x] Phase 2: Wire config — TTL purge, export path, pane widths, browser, refresh interval, fetch timeout, max items
- [x] Phase 3: Nav presets — one full default (`[Unread, Read Later, Favourite, Categories, Tags, Saved]`), `nav_presets` override (first = initial), `t` cycles; No Category node; top entries highlighted
- [x] Phase 4: default_view (`Feed:<url>` / `Category:<name>`); DB auto-migration for old schemas

Config complete (rust).

### go

- [x] Phase 1: Config file parse — JSON / JSONC / TOML / YAML by extension; keys `cache_ttl_days` / `export_dir` / `pane_ratio` / `theme` / `browser` / `refresh` / `nav_pane` / `images` / `foldlevel`
- [x] Phase 2: Wire config — cache TTL purge, export path, pane widths, browser, refresh behavior
- [x] Phase 3: Theme file load + defaults + XDG fallbacks when keys absent
- [x] Phase 4: Nav pane layouts — full + simple, `t` toggle, `nav_pane` override (implemented as `nav_presets`)

### cpp

- [ ] Phase 1: Config file parse — JSON / JSONC / TOML / YAML by extension; keys `cache_ttl_days` / `export_dir` / `pane_ratio` / `theme` / `browser` / `refresh` / `nav_pane` / `images` / `foldlevel`
- [ ] Phase 2: Wire config — cache TTL purge, export path, pane widths, browser, refresh behavior
- [ ] Phase 3: Theme file load + defaults + XDG fallbacks when keys absent
- [ ] Phase 4: Nav pane layouts — full + simple, `t` toggle, `nav_pane` override

## tags-favorites

### rust

- [x] Phase 1: Flags — per-item `read_later` / `saved` in DB, toggle from list+article (`L`/`S`); favourite = feed-level (`F` in nav, `!favourite` marker in urls file)
- [x] Phase 2: Virtual nodes — Read Later / Saved aggregate items; Favourite lists favourited feeds; feed `#tags` in urls, per-tag fold + filter
- [x] Phase 3: Saved semantics — exempt from TTL cleanup, kept in DB without markdown
- [x] Phase 4: Read-later lifecycle — `L` marks unread; reading clears read-later; list snapshot keeps read items until manual refresh

Tags & Favorites complete (rust). No per-item tags (decided).

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
- [ ] Phase 1: OPML import — nested folders → `cat/subcat` categories, `category` attr → feed tags
- [ ] Phase 2: OPML export — categories → nested folders, tags → `category` attr (round-trip)
- [x] Phase 3: Vim keys — `gg`/`G`, `Ctrl+f/b`, `zt/zz/zb`, `{/}`, `[/]`, `/` modal search (n next), `gi` image toggle; copy `yy/yn/yf`; sort `st/sn/sf/su` push (last pressed = highest, keep last 3) + `s` reverse + `sort` config init; `foldlevel` + `sort` config

### go
- [ ] Phase 1: OPML import — nested folders → `cat/subcat` categories, `category` attr → feed tags
- [ ] Phase 2: OPML export — categories → nested folders, tags → `category` attr (round-trip)
- [ ] Phase 3: Vim keys — `gg`/`G`, `Ctrl+f/b`, `zt/zz/zb`, `{/}`, `[/]`, `/` modal search (n next), `gi` image toggle; copy `yy/yn/yf`; sort `st/sn/sf/su` push (last pressed = highest, keep last 3) + `S` reverse + `sort` config init; `foldlevel` + `sort` config

### cpp
- [ ] Phase 1: OPML import — nested folders → `cat/subcat` categories, `category` attr → feed tags
- [ ] Phase 2: OPML export — categories → nested folders, tags → `category` attr (round-trip)
- [ ] Phase 3: Vim keys — `gg`/`G`, `Ctrl+f/b`, `zt/zz/zb`, `{/}`, `[/]`, `/` modal search (n next), `gi` image toggle; copy `yy/yn/yf`; sort `st/sn/sf/su` push (last pressed = highest, keep last 3) + `S` reverse + `sort` config init; `foldlevel` + `sort` config
