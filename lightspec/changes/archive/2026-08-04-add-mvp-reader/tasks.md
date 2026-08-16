## 1. Implementation

- [x] Phase 1: Scaffold — module, bubbletea skeleton, three-pane layout, XDG dirs
- [x] Phase 2: Feed source — newsboat `urls` parser, categories from tags, `~` display name
- [x] Phase 3: State — SQLite items (feed_url, guid), read persistence, All Unread aggregation
- [x] Phase 4: Nav tree — categories → feeds, h/l collapse, category/feed CRUD (rewrites urls)
- [x] Phase 5: Reading flow — summary preview, enter = read, fetch on demand, n/p/j/k keys
- [x] Phase 6: Refresh — startup auto + `r`, gofeed, parallel, error handling
- [x] Phase 7: Export — frontmatter + full content markdown, slug filename
- [x] Phase 8: Cache — fetched content preserved, TTL purge
- [x] Phase 9: OPML — import + export
- [x] Phase 10: Polish — empty states, spinner, help window, README

## 2. Validation

- [x] `go build ./...`, `go vet ./...` clean
- [x] `go test ./...` — 6 packages, incl. end-to-end ui tests (httptest feed)
- [x] pty smoke test: real run loads feed, lists items, unread counts, refresh status
