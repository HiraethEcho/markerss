# DESIGN — markerss TUI RSS Reader

Design authority for all implementation branches. Changes land here first, then branches rebase.

## Layout

Three panes:

```
┌─ Nav ────────────────┬─ List ───────────────┬─ Article ───────────┐
│ All Unread (n)       │ 1. item title        │ header:             │
│ ▸ category           │ 2. item title        │   title             │
│   feed (12)          │ 3. item title        │   feed · date · url │
│   feed (0)           │ ...                  │─────────────────────│
│ ▾ category           │                      │ content:            │
│   feed (3)           │                      │   summary or full   │
│ uncategorized feed   │                      │   (scrollable)      │
└──────────────────────┴──────────────────────┴─────────────────────┘
```

- Nav: category tree → feeds. Unread count per feed in parens. Uncategorized feeds at root.
- `All Unread` = virtual node aggregating all feeds, unread-first.
- Article pane split: header (meta) + content. Header: title, feed name, date, url.

## Feed Source

Primary: newsboat `urls` format, read live — single source of truth:

```
https://example.com/feed.xml "Display Name" category1 category2
```

- Tags = categories (feed belongs to first tag for tree placement; multi-tag supported)
- `~` prefix on title = custom display name (overrides feed-provided title). NOT hidden.
- TUI category/feed CRUD rewrites this file.
- OPML import/export for interop.

## Reading Flow

State machine per article:

```
list nav (preview summary, UNREAD kept)
  └─ <enter> → mark read + load full content
       ├─ feed has full content → show it
       └─ summary-only → fetch full article (HTML→text), cache
```

- `<enter>` is the only commit point for read state.
- Article pane keys: `n/p` next/prev item, `j/k` line scroll, `<c-u>/<c-d>` half-page scroll.
- List + article panes: `o` open browser, `e` export markdown, `u` mark unread.
- Auto-advance to next unread after finishing.

## Cache

- Fetched full articles: gzipped raw HTML + metadata (url, fetched_at, title, feed_id).
- Markdown conversion happens ONLY at export time, never stored.
- Configurable cleanup TTL (age-based purge on startup).

## Export

- `e` → markdown file: YAML frontmatter (title, link, date, feed) + full content.
- Filename: title slug. Dir: configurable, default `./out/`.

## Refresh

- Auto on startup (background, non-blocking) + `r` manual refresh.

## Paths (XDG)

- Config: `$XDG_CONFIG_HOME/markerss/` (config, feeds file)
- Cache: `$XDG_CACHE_HOME/markerss/`
- Data: `$XDG_DATA_HOME/markerss/` (read state, db)
- Fallbacks per XDG spec when vars unset.

## Keys Summary

| Key | Scope | Action |
|---|---|---|
| h/l | nav | collapse/expand |
| j/k | list | move selection |
| j/k | article | scroll |
| n/p | article | next/prev item |
| <enter> | list | open article (mark read, load full) |
| o | list+article | open in browser |
| e | list+article | export markdown |
| u | list+article | toggle unread |
| <c-u>/<c-d> | article | scroll half page |
| r | global | refresh |
| q | global | quit |
| ? | global | help |

## Out of Scope (MVP)

- Sync, accounts, push, daemon
- Tags / read-later (post-MVP, lower nav strip)
