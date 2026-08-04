# DESIGN — markerss TUI RSS Reader

Design authority for all implementation branches. Changes land here first, then branches rebase. This document is language-agnostic — it describes behavior, not specific libraries.

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
- Article pane split: header (meta + summary) + content (full content only). Body never repeats the summary.
- Nav/list panes narrow (≈15% each); article wide (≈70%).

## Feed Source

Primary: newsboat `urls` format, read live — single source of truth:

```
https://example.com/feed.xml "Display Name" category1 category2
```

- Tags = categories (feed belongs to first tag for tree placement).
- `~` prefix on title = custom display name (overrides feed-provided title). NOT hidden.
- TUI category/feed CRUD rewrites this file.
- OPML import/export for interop.

## Reading Flow

- Refresh stores **summary only** — full content is never fetched for unopened items.
- Nav `j/k` moves the cursor and live-preview the list (no read change). `Enter` opens the scope.
- List `j/k` moves selection — **does not mark read**. `Enter` opens the article (marks read, focuses article pane).
- Article pane shows summary in the header; body shows full content if already fetched, else a fetch hint.
- **Auto-fetch is off**: full content is fetched only when the user presses `Enter` again in the article pane.
- Displaying content marks the item read (auto on open and on `n/p` navigation). `u` toggles read manually; `A` marks all in view read.
- `n/p` next/prev item; `j/k` scroll; ctrl+u/ctrl+d half-page scroll.

## Storage

- SQLite database at `$XDG_CACHE_HOME/markerss/markerss.db` — items + content + read flags.
- Items keyed `(feed_url, guid)`; read flag and fetched content preserved across refresh.
- Fetched article content (readability-extracted) stored per item; refresh keeps existing fetched content.
- Markdown generated ONLY at export time, never stored.
- Configurable TTL: startup purge of fetched content older than `cache_ttl_days`.
- Subscriptions stay in the `urls` file (newsboat format) — DB holds items only.

## Full-Content Fetch

- On explicit `Enter` in the article pane: fetch the article page, extract the main content (readability-style), store in DB.
- Extraction removes nav/sidebar/ads; falls back to raw page if extraction fails.
- Already-fetched content is reused (no refetch).

## Rendering

- Content HTML → markdown → styled terminal text.
- Headings bold, lists/code blocks rendered, links as **underlined alt text (URL not shown)**.
- Images shown as `[img]` placeholder.
- Export format = the same markdown, written to file on `e`.

## Export

- `e` → markdown file: YAML frontmatter (title, link, date, feed) + full content.
- Default dir: `$XDG_DATA_HOME/markerss/<category>/<slug>.md` (uncategorized → root). Configurable.

## Refresh

- Auto on startup (background, non-blocking) + `r` manual refresh.

## Paths (XDG)

- Config: `$XDG_CONFIG_HOME/markerss/` — two separate files:
  - `config` (app settings: cache TTL, export dir, refresh behavior)
  - `urls` (newsboat-format subscriptions — kept separate from app config)
- Cache/DB: `$XDG_CACHE_HOME/markerss/markerss.db` (SQLite)
- Export: data dir (configurable)
- Fallbacks per XDG spec when vars unset.

## Keys Summary

| Key | Scope | Action |
|---|---|---|
| h / esc | global | go LEFT: article→list→nav→parent in tree (feed → fold its category) |
| l / enter | global | go RIGHT: expand tree→list→article→fetch full |
| F | global | toggle full-screen focus on article pane |
| q | global | quit (press twice to confirm) |
| j/k | nav | move (live-preview list) |
| j/k | list | move selection (no read change) |
| j/k | article | scroll |
| n/p | article | next/prev item (marks read) |
| o | list+article | open in browser |
| e | list+article | export markdown |
| u | list+article | toggle read/unread |
| A | list | mark all unread in view read |
| <c-u>/<c-d> | article | scroll half page |
| Q | global | quit app |
| Tab / Shift+Tab | global | focus next / prev pane |
| r | global | refresh |
| ? | global | help |

## Out of Scope

- Sync, accounts, push, daemon
- Tags / read-later (post-MVP, lower nav strip)