# Change: Three-pane TUI RSS reader (MVP)

## Why

No functional reader existed in the go branch — only a static skeleton with
placeholder panes. The MVP spec (DESIGN.md §MVP) requires a working
three-pane reader: browse feeds, mark read, fetch full articles on demand,
export posts as markdown.

## What Changes

- Three-pane TUI: nav tree (All Unread, categories → feeds, h/l collapse,
  live preview) / item list / article reader
- Newsboat `urls` file as live subscription source; TUI CRUD rewrites it
- SQLite persistence: items keyed (feed_url, guid); read flags + fetched
  content survive refresh; unread counts; TTL purge
- Refresh: gofeed summary-only, parallel (bounded 8), startup + `r`
- Full-article fetch: go-readability extraction, only on explicit enter
- Rendering: HTML → markdown; links = underlined alt (URL hidden),
  images = `[img]`, sub/sup markers; ANSI-aware truncation
- Export: `e` → YAML frontmatter + markdown to
  `$XDG_DATA_HOME/markerss/<category>/<slug>.md`
- OPML import (merges urls) / export (newsboat-compatible)
- Cache-first startup: UI renders stored items before any HTTP fetch

## Impact

- Affected specs: mvp
- Affected code: main.go, internal/{ui,store,fetch,markdown,export,opml,feedlist,xdg}
- New deps: gofeed, go-readability, html-to-markdown/v2, modernc.org/sqlite
