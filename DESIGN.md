# DESIGN — markerss TUI RSS Reader

Design authority for all implementation branches. Changes land here first, then branches rebase. This document is language-agnostic — it describes behavior, not specific libraries. Three parallel sections; branches implement all three, in order MVP → Config → Tags & Favorites.

## MVP

### Layout

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

### Feed Source

Primary: newsboat `urls` format, read live — single source of truth:

```
https://example.com/feed.xml "Display Name" category1 category2
```

- Tags = categories (feed belongs to first tag for tree placement).
- `~` prefix on title = custom display name (overrides feed-provided title). NOT hidden.
- TUI category/feed CRUD rewrites this file.
- OPML import/export for interop.

### Reading Flow

- Refresh stores **summary only** — full content is never fetched for unopened items.
- Nav `j/k` moves the cursor and live-preview the list (no read change). `Enter` opens the scope.
- List `j/k` moves selection — **does not mark read**. `Enter` opens the article (marks read, focuses article pane).
- Article pane shows summary in the header; body shows full content if already fetched, else a fetch hint.
- **Auto-fetch is off**: full content is fetched only when the user presses `Enter` again in the article pane.
- Displaying content marks the item read (auto on open and on `n/p` navigation). `u` toggles read manually; `A` marks all in view read.
- `n/p` next/prev item; `j/k` scroll; ctrl+u/ctrl+d half-page scroll.

### Storage

- SQLite database at `$XDG_CACHE_HOME/markerss/markerss.db` — items + content + read flags.
- Items keyed `(feed_url, guid)`; read flag and fetched content preserved across refresh.
- Fetched article content (readability-extracted) stored per item; refresh keeps existing fetched content.
- Markdown generated ONLY at export time, never stored.
- Configurable TTL: startup purge of fetched content older than `cache_ttl_days`.
- Subscriptions stay in the `urls` file (newsboat format) — DB holds items only.

### Full-Content Fetch

- On explicit `Enter` in the article pane: fetch the article page, extract the main content (readability-style), store in DB.
- Extraction removes nav/sidebar/ads; falls back to raw page if extraction fails.
- Already-fetched content is reused (no refetch).

### Rendering

- Content HTML → markdown → styled terminal text.
- Headings bold, lists/code blocks rendered, links as **underlined alt text (URL not shown)**.
- Images shown as `[img]` placeholder.
- `<sub>`/`<sup>` → `~x~`/`^x^` markdown markers (rendered as subscript/superscript).
- Export format = the same markdown, written to file on `e`.
- Help = floating opaque window (default colors), scrollable with `j`/`k`/arrows.

### Export

- `e` → markdown file: YAML frontmatter (title, link, date, feed) + full content.
- Default dir: `$XDG_DATA_HOME/markerss/<category>/<slug>.md` (uncategorized → root). Configurable.

### Refresh

- Auto on startup (background, non-blocking) + `r` manual refresh.

### Paths (XDG)

- Config: `$XDG_CONFIG_HOME/markerss/` — two separate files:
  - `config` (app settings: cache TTL, export dir, refresh behavior)
  - `urls` (newsboat-format subscriptions — kept separate from app config)
- Cache/DB: `$XDG_CACHE_HOME/markerss/markerss.db` (SQLite)
- Export: data dir (configurable)
- Fallbacks per XDG spec when vars unset.

### Keys Summary

| Key | Scope | Action |
|---|---|---|
| h / q / esc | global | go LEFT: article→list→nav→parent in tree (feed → fold its category) |
| l / enter | global | go RIGHT: expand tree→list→article→fetch full |
| j/k | nav | move (live-preview list) |
| j/k | list | move selection (no read change) |
| j/k | article | scroll |
| n/p | article | next/prev item (marks read) |
| o | list+article | open in browser |
| e | list+article | export markdown |
| u | list+article | toggle read/unread |
| A | list | mark all unread in view read |
| <c-u>/<c-d> | article | scroll half page |
| a | nav | add feed (URL → title → category prompts) |
| d | nav | delete feed (press twice to confirm) |
| R | nav | rename category |
| i / x | global | import / export OPML |
| F | global | toggle full-screen focus on article pane |
| Q | global | quit app |
| Tab / Shift+Tab | global | focus next / prev pane |
| r | global | refresh |
| ? | global | help (floating scrollable window) |

### Decisions

| Decision | Choice | Rationale | Date |
|---|---|---|---|
| Branch strategy | main = design docs only; parallel impl branches rebase on main | Single design source, parallel impl | 2026-08 |
| Persistence | SQLite (items + content + read flags) | Single-file, queryable, preserves state across refresh | 2026-08 |
| Layout | 3 panes: nav tree / item list / article | Mirrors mail-client pattern | 2026-08 |
| Tree | Categories → feeds, uncategorized at root, h/l collapse | File-tree mental model | 2026-08 |
| Keys | Vim keys + arrows both bound | Familiarity | 2026-08 |
| Feed source | newsboat `urls` format, live file | Zero migration; coexists with newsboat; single source of truth | 2026-08 |
| Tags | = categories; `~` title prefix = custom display name | newsboat conventions preserved | 2026-08 |
| OPML | Import/export supported | Interop with other readers | 2026-08 |
| Article flow | List enter = open + read; article enter = fetch full only (no auto-fetch) | Fetch only on explicit action; summary-only until opened | 2026-08 |
| Storage | SQLite in cache dir; summary-only on refresh; fetched content preserved | Bandwidth/storage efficient; TTL configurable | 2026-08 |
| Rendering | HTML → markdown → styled text; links = underlined alt, no URL | Clean reading; eilmeldung-style | 2026-08 |
| Paths | XDG config/cache/data; config + urls separate files | Platform convention; subscriptions ≠ app config | 2026-08 |
| Export target | `$XDG_DATA_HOME/markerss/<category>/<slug>.md` | Per-category archive; configurable | 2026-08 |

## Config

- Config file at `$XDG_CONFIG_HOME/markerss/config`, separate from `urls`.
- Format: JSON, JSONC, TOML, or YAML — detected by extension (`.json`/`.jsonc`/`.toml`/`.yaml`/`.yml`); plain `config` defaults to TOML.
- Keys:
  - `cache_ttl_days` — startup purge of fetched content older than N days.
  - `export_dir` — override default export path (`$XDG_DATA_HOME/markerss`).
  - refresh behavior — auto-on-startup on/off, interval.
- Read at startup; defaults + XDG fallbacks when keys absent. No hot-reload in MVP.

### Decisions

| Decision | Choice | Rationale | Date |
|---|---|---|---|
| Config file | separate from `urls` | app settings ≠ subscriptions | 2026-08 |
| Format | JSON / JSONC / TOML / YAML, by extension | user preference; ecosystem standard | 2026-08 |
| Reload | startup only | MVP simplicity | 2026-08 |

## Tags & Favorites

### Tags

- Lower nav strip below the category tree: list of tags.
- Assign tags to items from the article view.
- Selecting a tag filters the list pane to items with that tag.

### Favorites

- Favorites = special category = virtual node (like `All Unread`), shown in the nav tree.
- Toggle from article view adds/removes the item from favorites.
- Aggregates favorited items across all feeds.
- A category view, not separate storage — reuses the item read/flag model.

### Decisions

| Decision | Choice | Rationale | Date |
|---|---|---|---|
| Favorites | special category / virtual node | reuses All Unread aggregation; no new storage | 2026-08 |
| Tags placement | lower nav strip | second nav region, not new pane | 2026-08 |
| Tag storage | per-item in DB | queryable | 2026-08 |