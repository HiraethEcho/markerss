# DESIGN — markerss TUI RSS Reader

Design authority for all implementation branches. Changes land here first, then branches rebase. This document is language-agnostic — it describes behavior, not specific libraries. Five parallel sections; branches implement all five, in order MVP → Config → Tags & Favorites → Article Polish → Advanced.

## MVP

### Layout

Three panes:

```
┌─ Nav ────────────────┬─ List ───────────────┬─ Article ───────────┐
│ All Unread (n)       │ 1. item title        │ header:             │
│ Read Later (n)       │ 2. item title        │   title             │
│ Favourite (n)        │ 3. item title        │   feed · date · url │
│ ▾ Categories         │ ...                  │   summary           │
│   ▸ category         │                      │─────────────────────│
│     feed (12)        │                      │ content:            │
│ ▾ Tags               │                      │   full content      │
│   tag1               │                      │                     │
│   tag2               │                      │                     │
│ Saved (n)            │                      │                     │
└──────────────────────┴──────────────────────┴─────────────────────┘
```

Nav pane structure, top to bottom:
- `All Unread` — virtual node, all items unread-first.
- `Read Later` — virtual node, items flagged `read_later`.
- `Favourite` — virtual node, items flagged `favorite`.
- `Categories` — tree: categories → feeds. Unread count per feed in parens. Uncategorized feeds at root.
- `Tags` — flat list of feed tags (`#tag`).
- `Saved` — virtual node, items flagged `saved` (kept in DB, no markdown).
- Article pane split: header (meta + summary) + content (full content only). Body never repeats the summary.
- Pane widths configurable via `pane_ratio` (default ≈ 0.15 / 0.15 / 0.70).

### Feed Source

Feed list from two sources, both read live:
- newsboat `urls` format (primary, single source of truth):

  ```
  https://example.com/feed.xml "Display Name" category #tag1 #tag2
  ```

  - First non-`#` token = **category** (single — tree placement).
  - `#tag` tokens = **tags** (multi, optional) — shown in tags strip.
  - `~` prefix on title = custom display name (overrides feed-provided title). NOT hidden.
- OPML file — accepted as a feed list source, plus import/export for interop.

- TUI category/feed CRUD rewrites the urls file.

### Navigation

Left/right movement via `h`/`l`, `q`/`enter`/`esc`, and arrow keys (`←`/`→`).

- Nav pane = virtual nodes (All Unread / Read Later / Favourite / Saved) + Categories tree + Tags list.
- **Right** (`l` / `enter` / `→`):
  - virtual node (All Unread / Read Later / Favourite / Saved) → list pane (its items)
  - folded category → expand it
  - expanded category → list pane (all items in category)
  - feed → list pane (all items in that feed)
  - tag → list pane (items from feeds carrying it)
  - item in list → article pane + mark read
  - article pane → fetch full content
- **Left** (`h` / `q` / `esc` / `←`): go back one level — article → list → nav; in nav, feed → fold its category, expanded category → fold.
- `j/k` move cursor in nav (live-preview list, no read change).
- List `j/k` moves selection — **does not mark read**. `Enter`/`→` opens the article (marks read, focuses article pane).
- Article pane shows summary in the header; body shows full content if already fetched, else a fetch hint.
- **Auto-fetch is off**: full content is fetched only when the user presses `Enter`/`→` again in the article pane.
- Displaying content marks the item read (auto on open and on `n/p` navigation). `u` toggles read manually; `A` marks all in view read.
- `n/p` next/prev item; `j/k` scroll; ctrl+u/ctrl+d half-page scroll.

### Storage

- SQLite database at `$XDG_CACHE_HOME/markerss/markerss.db` — items + content + flags.
- Items keyed `(feed_url, guid)`; flags and fetched content preserved across refresh.
- Per-item boolean flags: `read`, `read_later`, `favorite`, `saved` — independent, orthogonal.
- Fetched article content (readability-extracted) stored per item; refresh keeps existing fetched content.
- Markdown generated ONLY at export time, never stored.
- Configurable TTL: startup purge of fetched content older than `cache_ttl_days` — **`saved` items exempt** (content kept, item never deleted).
- Subscriptions stay in the `urls` file (newsboat format) — DB holds items only.

### Full-Content Fetch

- On explicit `Enter`/`→` in the article pane: fetch the article page, extract the main content (readability-style), store in DB.
- Extraction removes nav/sidebar/ads; falls back to raw page if extraction fails.
- Already-fetched content is reused (no refetch).

### Rendering

- Rendering strategy is **per-language** — each branch chooses its own HTML → styled-text pipeline.
- Behavioral contract (all branches):
  - Headings bold, lists/code blocks rendered.
  - Links as **underlined alt text (URL not shown)**.
  - Images shown as `[img]` placeholder.
  - `<sub>`/`<sup>` → `~x~`/`^x^` markers (rendered as subscript/superscript).
- Export format = the same markdown, written to file on `e`.
- Help = floating opaque window (default colors), scrollable with `j`/`k`/arrows.

### Export

- `e` → markdown file: YAML frontmatter (title, link, date, feed) + full content.
- Default dir: `$XDG_DATA_HOME/markerss/<category>/<slug>.md` (uncategorized → root). Configurable via `export_dir`.

### Refresh

- Auto on startup (background, non-blocking) + `r` manual refresh.

### Paths (XDG)

- Config: `$XDG_CONFIG_HOME/markerss/` — two separate files:
  - `config` (app settings: cache TTL, export dir, pane ratio, browser, refresh)
  - `urls` (newsboat-format subscriptions — kept separate from app config)
- Cache/DB: `$XDG_CACHE_HOME/markerss/markerss.db` (SQLite)
- Export: data dir (configurable)
- Fallbacks per XDG spec when vars unset.

### Keys Summary

| Key | Scope | Action |
|---|---|---|
| h / q / esc / ← | global | go LEFT: article→list→nav; in nav: feed→fold category, expanded category→fold |
| l / enter / → | global | go RIGHT: virtual node/category/feed/tag→list; item→article+read; article→fetch full |
| j/k | nav | move (live-preview list) |
| j/k | list | move selection (no read change) |
| j/k | article | scroll |
| n/p | article | next/prev item (marks read) |
| o | list+article | open in browser |
| e | list+article | export markdown |
| u | list+article | toggle read/unread |
| A | list | mark all unread in view read |
| <c-u>/<c-d> | article | scroll half page |
| a | nav | add feed (URL → title → category → tags prompts) |
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
| Tree | Virtual nodes (Unread/Read Later/Favourite/Saved) + Categories tree + Tags list | single nav pane, mail-client pattern | 2026-08 |
| Keys | Vim keys + arrows both bound; h/l/q/enter/esc/←/→ for left/right | Familiarity; explicit nav semantics | 2026-08 |
| Feed source | newsboat `urls` format + OPML file, read live | Zero migration; coexists with newsboat; single source of truth | 2026-08 |
| Category vs tags | one category (tree placement) + multi `#tags` (optional) | category = structure, tags = cross-cutting | 2026-08 |
| OPML | Import/export + accepted as feed list source | Interop with other readers | 2026-08 |
| Article flow | List enter = open + read; article enter = fetch full only (no auto-fetch) | Fetch only on explicit action; summary-only until opened | 2026-08 |
| Storage | SQLite in cache dir; summary-only on refresh; fetched content preserved | Bandwidth/storage efficient; TTL configurable | 2026-08 |
| Rendering | strategy per-language; shared behavioral contract | each branch picks its own pipeline | 2026-08 |
| Paths | XDG config/cache/data; config + urls separate files | Platform convention; subscriptions ≠ app config | 2026-08 |
| Export target | `$XDG_DATA_HOME/markerss/<category>/<slug>.md` | Per-category archive; configurable | 2026-08 |

## Config

- Config file at `$XDG_CONFIG_HOME/markerss/config`, separate from `urls`.
- Format: JSON, JSONC, TOML, or YAML — detected by extension (`.json`/`.jsonc`/`.toml`/`.yaml`/`.yml`); plain `config` defaults to TOML.
- Keys:
  - `cache_ttl_days` — startup purge of fetched content older than N days.
  - `export_dir` — export location (default `$XDG_DATA_HOME/markerss`).
  - `pane_ratio` — three-pane widths, e.g. `[0.15, 0.15, 0.7]`.
  - `theme` — standalone theme file (colors), separate from config.
  - `browser` — which browser to open (default: `xdg-open`).
  - `refresh` — auto-on-startup on/off, interval.
- Read at startup; defaults + XDG fallbacks when keys absent. No hot-reload in MVP.

### Suggested additional keys (optional)

- `fetch_timeout` — per-request timeout for full-content fetch.
- `max_items_per_feed` — cap items kept per feed.
- `proxy` — HTTP proxy for fetch.
- `keybindings` — custom keymap file.
- `default_view` — startup scope (e.g. All Unread, a category).

### Decisions

| Decision | Choice | Rationale | Date |
|---|---|---|---|
| Config file | separate from `urls` | app settings ≠ subscriptions | 2026-08 |
| Format | JSON / JSONC / TOML / YAML, by extension | user preference; ecosystem standard | 2026-08 |
| Theme | standalone file, referenced by `theme` key | colors ≠ app settings; swappable | 2026-08 |
| Pane ratio | `pane_ratio` key, default 0.15/0.15/0.7 | user-adjustable layout | 2026-08 |
| Browser | `browser` key, default xdg-open | user choice | 2026-08 |
| Reload | startup only | MVP simplicity | 2026-08 |

## Tags & Favorites

### Virtual nodes

- `Read Later`, `Favourite`, `Saved` = virtual nodes in nav, like `All Unread` — aggregate items by flag, no new storage.
- `saved` = items kept in DB without markdown; exempt from TTL cleanup.

### Tags

- Tags are **per-feed**: each feed has 0..n tags (`#tag` in urls file), plus exactly one category.
- Tags list in nav pane (below Categories).
- Selecting a tag filters the list pane to items from feeds carrying that tag.
- Tags can also be assigned to individual items from the article view.

### Flags

- Per-item boolean flags: `read_later`, `favorite`, `saved` — independent, toggle from article view.

### Decisions

| Decision | Choice | Rationale | Date |
|---|---|---|---|
| Tags | per-feed, multi (`#tag`), optional; one category | category = structure, tags = cross-cutting | 2026-08 |
| Virtual nodes | Read Later / Favourite / Saved, like All Unread | reuses All Unread aggregation; no new storage | 2026-08 |
| Flags | independent booleans `read_later` / `favorite` / `saved` | orthogonal, item can have several | 2026-08 |
| Saved | kept in DB, exempt from TTL cleanup, no markdown | keep without export | 2026-08 |
| Tags placement | tags list in nav pane (below Categories) | second nav region, not new pane | 2026-08 |
| Tag storage | per-feed in urls file; per-item in DB | queryable | 2026-08 |

## Article Polish

### Reading Width

- Content column capped at ~80 cols (configurable via theme? no — fixed), left-aligned, no justification.
- Blank line between paragraphs; headings bold with hierarchy (H1 > H2 > H3 by size/emphasis).

### Element Rendering

- Lists: indented, proper markers.
- Code blocks: monospace, bordered box, no wrap (or horizontal scroll).
- Inline code: distinct background.
- Blockquotes: indented + distinct color.
- Tables: best-effort aligned columns.
- Horizontal rules: full-width line.
- Links: underlined alt text (URL not shown); images `[img]` with alt.
- `<sub>`/`<sup>` → `~x~`/`^x^` markers.

### Scrolling & Performance

- `j/k` line scroll, `ctrl-u`/`ctrl-d` half-page, `n/p` prev/next item.
- Scrollbar indicator at right edge.
- Render only visible lines (viewport culling); cap content size; no full-pane redraw on scroll.
- Colors follow Config `theme` file (fallback default palette).

### Decisions

| Decision | Choice | Rationale | Date |
|---|---|---|---|
| Width | max reading width ~80 cols | readability | 2026-08 |
| Alignment | left, no justification | terminal readability | 2026-08 |
| Performance | viewport culling | long articles stay smooth | 2026-08 |
| Theme | follows Config `theme` | consistent look | 2026-08 |

## Advanced

(TBD — user writes details later: advanced functions, more keys, etc.)