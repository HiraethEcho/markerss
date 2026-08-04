# PLAN

## Change: mvp-tui-rss-reader
- [ ] Phase 1: Scaffold — go module, bubbletea skeleton, three-pane layout (nav/list/article), XDG dirs
- [ ] Phase 2: Feed source — newsboat `urls` parser, categories from tags, `~` hidden feeds
- [ ] Phase 3: State — read status persistence, All Unread aggregation
- [ ] Phase 4: Nav tree — categories → feeds, h/l collapse, category CRUD (rewrites urls file)
- [ ] Phase 5: Reading flow — preview summary (unread) → enter = read + full content, fetch on demand, n/p/j/k keys
- [ ] Phase 6: Refresh — startup auto + `r` manual, gofeed, error handling
- [ ] Phase 7: Export — `e` → frontmatter + full content markdown, title-slug filename
- [ ] Phase 8: Cache — gzipped HTML + metadata, TTL cleanup config
- [ ] Phase 9: OPML — import + export
- [ ] Phase 10: Polish — empty states, spinner, help bar, README usage

## Change: post-mvp-tags-readlater
- [ ] Phase 1: Lower nav strip — tags list + read-later views (deferred, after MVP)
- [ ] Phase 2: Tag assignment + read-later toggle from article view
