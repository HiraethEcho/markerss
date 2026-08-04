# PLAN

Progress per branch (rust / go / cpp). `##` = spec, `###` = branch. Each branch implements all three specs, in order MVP → Config → Tags & Favorites.

## mvp

### rust
- [ ] Phase 1: Scaffold — crate, ratatui skeleton, three-pane layout (nav/list/article), XDG dirs
- [ ] Phase 2: Feed source — newsboat `urls` parser, categories from tags, `~` display name
- [ ] Phase 3: State — read status persistence, All Unread aggregation
- [ ] Phase 4: Nav tree — categories → feeds, h/l collapse, category CRUD (rewrites urls file)
- [ ] Phase 5: Reading flow — preview summary (unread) → enter = read + full content, fetch on demand, n/p/j/k keys
- [ ] Phase 6: Refresh — startup auto + `r` manual, feed-rs, error handling
- [ ] Phase 7: Export — `e` → frontmatter + full content markdown, title-slug filename
- [ ] Phase 8: Cache — gzipped HTML + metadata, TTL cleanup config
- [ ] Phase 9: OPML — import + export
- [ ] Phase 10: Polish — empty states, spinner, help bar, README usage

### go
- [ ] Phase 1: Scaffold — go module, bubbletea skeleton, three-pane layout (nav/list/article), XDG dirs
- [ ] Phase 2: Feed source — newsboat `urls` parser, categories from tags, `~` display name
- [ ] Phase 3: State — read status persistence, All Unread aggregation
- [ ] Phase 4: Nav tree — categories → feeds, h/l collapse, category CRUD (rewrites urls file)
- [ ] Phase 5: Reading flow — preview summary (unread) → enter = read + full content, fetch on demand, n/p/j/k keys
- [ ] Phase 6: Refresh — startup auto + `r` manual, gofeed, error handling
- [ ] Phase 7: Export — `e` → frontmatter + full content markdown, title-slug filename
- [ ] Phase 8: Cache — gzipped HTML + metadata, TTL cleanup config
- [ ] Phase 9: OPML — import + export
- [ ] Phase 10: Polish — empty states, spinner, help bar, README usage

### cpp
- [ ] Phase 1: Scaffold — build system, FTXUI skeleton, three-pane layout (nav/list/article), XDG dirs
- [ ] Phase 2: Feed source — newsboat `urls` parser, categories from tags, `~` display name
- [ ] Phase 3: State — read status persistence, All Unread aggregation
- [ ] Phase 4: Nav tree — categories → feeds, h/l collapse, category CRUD (rewrites urls file)
- [ ] Phase 5: Reading flow — preview summary (unread) → enter = read + full content, fetch on demand, n/p/j/k keys
- [ ] Phase 6: Refresh — startup auto + `r` manual, libcurl, error handling
- [ ] Phase 7: Export — `e` → frontmatter + full content markdown, title-slug filename
- [ ] Phase 8: Cache — gzipped HTML + metadata, TTL cleanup config
- [ ] Phase 9: OPML — import + export
- [ ] Phase 10: Polish — empty states, spinner, help bar, README usage

## config

### rust
- [ ] Phase 1: Config file parse — JSON / JSONC / TOML / YAML by extension, keys `cache_ttl_days` / `export_dir` / refresh behavior
- [ ] Phase 2: Wire config — cache TTL purge, export path override, refresh behavior
- [ ] Phase 3: Defaults + XDG fallbacks when keys absent

### go
- [ ] Phase 1: Config file parse — JSON / JSONC / TOML / YAML by extension, keys `cache_ttl_days` / `export_dir` / refresh behavior
- [ ] Phase 2: Wire config — cache TTL purge, export path override, refresh behavior
- [ ] Phase 3: Defaults + XDG fallbacks when keys absent

### cpp
- [ ] Phase 1: Config file parse — JSON / JSONC / TOML / YAML by extension, keys `cache_ttl_days` / `export_dir` / refresh behavior
- [ ] Phase 2: Wire config — cache TTL purge, export path override, refresh behavior
- [ ] Phase 3: Defaults + XDG fallbacks when keys absent

## tags-favorites

### rust
- [ ] Phase 1: Lower nav strip — tags list below category tree
- [ ] Phase 2: Tag assignment from article view + filter list by tag
- [ ] Phase 3: Favorites special category (virtual node like All Unread) + toggle from article view

### go
- [ ] Phase 1: Lower nav strip — tags list below category tree
- [ ] Phase 2: Tag assignment from article view + filter list by tag
- [ ] Phase 3: Favorites special category (virtual node like All Unread) + toggle from article view

### cpp
- [ ] Phase 1: Lower nav strip — tags list below category tree
- [ ] Phase 2: Tag assignment from article view + filter list by tag
- [ ] Phase 3: Favorites special category (virtual node like All Unread) + toggle from article view