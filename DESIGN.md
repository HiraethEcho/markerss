# DESIGN — markerss TUI RSS Reader

Design authority for all implementation branches. Changes land here first, then branches rebase.

## Layout

Three panes:

```
┌─ Nav ────────────────┬─ List ───────────────┬─ Article ───────────┐
│ All Unread (n)       │ 1. item title        │ header:             │
│ ▸ category           │ 2. item title        │   title             │
│   feed (12)          │ 3. item title        │   feed · date · url │
│   feed (0)           │ ...                  │   summary           │
│ ▾ category           │                      │─────────────────────│
│   feed (3)           │                      │ content:            │
│ uncategorized feed   │                      │   full content      │
└──────────────────────┴──────────────────────┴─────────────────────┘
```

- Nav: category tree → feeds. Unread count per feed in parens. Uncategorized feeds at root.
- `All Unread` = virtual node aggregating all feeds, unread-first.
- Article pane split: header (meta + summary) + content (full content only).

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
list nav (article pane shows summary in header, UNREAD kept)
  └─ <enter> → open article in article pane, mark read, load content
       ├─ feed has full content → content area shows it
       └─ summary-only → content area empty until <enter> in article pane

article pane: <enter> → try fetch full article (HTML→text, cache)
```

- `<enter>` in list = open + mark read (only commit point).
- `<enter>` in article pane = ONLY place that fetches full content.
- Read state fully manual afterwards: `u` toggles read/unread anywhere, no need to enter item.
- `A` = mark all unread in current view (feed / category / All Unread) as read.
- Article pane keys: `n/p` next/prev item, `j/k` line scroll, `<c-u>/<c-d>` half-page scroll.
- List + article panes: `o` open browser, `e` export markdown, `u` toggle read.
- Auto-advance to next unread after finishing.

## Cache

- Fetched full articles: gzipped raw HTML + metadata (url, fetched_at, title, feed_id).
- Markdown conversion happens ONLY at export time, never stored.
- Configurable cleanup TTL (age-based purge on startup).

## Export

- `e` → markdown file: YAML frontmatter (title, link, date, feed) + full content.
- Default dir: `$XDG_DATA_HOME/markerss/<category>/<slug>.md` (category = feed's first tag; uncategorized → direct `markerss/<slug>.md`, no subdir). Configurable.

## Refresh

- Auto on startup (background, non-blocking) + `r` manual refresh.

## Paths (XDG)

- Config: `$XDG_CONFIG_HOME/markerss/` — two separate files:
  - `config` (app settings: cache TTL, export dir, refresh behavior)
  - `urls` (newsboat-format subscriptions — kept separate from app config)
- Cache: `$XDG_CACHE_HOME/markerss/` (gzipped fetched articles + metadata)
- State: `$XDG_STATE_HOME/markerss/` (read-state DB)
- Fallbacks per XDG spec when vars unset.

## Keys Summary

| Key | Scope | Action |
|---|---|---|
| h/l | nav | collapse/expand |
| j/k | list | move selection |
| j/k | article | scroll |
| n/p | article | next/prev item |
| <enter> | list | open article (mark read, load content) |
| <enter> | article | fetch full content (summary-only feeds) |
| o | list+article | open in browser |
| e | list+article | export markdown |
| u | list+article | toggle read/unread |
| A | list | mark all unread in view as read |
| <c-u>/<c-d> | article | scroll half page |
| r | global | refresh |
| q | global | quit |
| ? | global | help |

## Out of Scope

- Sync, accounts, push, daemon
- Tags / read-later (post-MVP, lower nav strip)
