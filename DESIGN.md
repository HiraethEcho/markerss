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
│   ▾ cat1             │                      │─────────────────────│
│     ▸ subcat1        │                      │ content:            │
│       feed1          │                      │   full content      │
│   ▸ cat2             │                      │                     │
│ ▾ Tags               │                      │                     │
│   tag1               │                      │                     │
│   tag2               │                      │                     │
│ Saved (n)            │                      │                     │
└──────────────────────┴──────────────────────┴─────────────────────┘
```

Nav pane structure, top to bottom:
- `All Unread` — virtual node, all items unread-first.
- `Read Later` — virtual node, items flagged `read_later`.
- `Favourite` — virtual node, items flagged `favorite`.
- `Categories` — tree: categories → subcategories → feeds, **nested to any depth**. Unread count per feed in parens. Uncategorized feeds at root.
- `Tags` — flat list of feed tags (`#tag`).
- `Saved` — virtual node, items flagged `saved` (kept in DB, no markdown).
- Article pane split: header (meta + summary) + content (full content only). Body never repeats the summary.
- Pane widths configurable via `pane_ratio` (default ≈ 0.15 / 0.15 / 0.70).

### Feed Source

Feed list from one source, read live:
- newsboat `urls` format (single source of truth):

  ```
  https://example.com "custom title" category #tag1 #tag2 #tag3
  ```

  - First non-`#` token = **category** (single — tree placement).
  - `#tag` tokens = **tags** (multi, optional) — shown in tags strip.
  - Quoted string = **custom display name** (overrides feed-provided title). NOT hidden.

- TUI category/feed CRUD rewrites the urls file.

### Navigation

Left/right movement via `h`/`l`, `q`/`enter`/`esc`, and arrow keys (`←`/`→`).

- Nav pane = virtual nodes (All Unread / Read Later / Favourite / Saved) + Categories tree + Tags list.
- **Right** (`l` / `enter` / `→`):
  - virtual node (All Unread / Read Later / Favourite / Saved) → list pane (its items)
  - folded category → expand it
  - expanded category with children → descend into subcategory on `j/k`; right → list pane (all items in category incl. subcats)
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
| <space> | list | toggle read + move to next item |
| a | nav | add feed (URL → title → category → tags prompts) |
| d | nav | delete feed (press twice to confirm) |
| R | nav | rename category |
| F | global | toggle full-screen focus on article pane |
| Q | global | quit app |
| Tab / Shift+Tab | global | focus next / prev pane |
| r | global | refresh |
| ? | global | help (floating scrollable window) |
| f | article | link jump — highlight links, 1-9/letter hints, press key to open in browser |
| gi | article | toggle kitty image render |
| t | nav | toggle nav pane layout (Full ↔ Simple; disabled when `nav_pane` set) |
| gg / G | nav+list | jump top / bottom |
| Ctrl+f / Ctrl+b | list+article | full page down / up |
| zt / zz / zb | article | scroll cursor top / center / bottom |
| { / } | article | jump prev / next paragraph |
| [ / ] | article | jump prev / next section (h2/h3) |
| / | list | modal search — n = next match, esc exits |
| zr / zm / zR / zM | nav | fold level ±1 / open all / close all |
| yy / yn | list+article | copy item URL / title |
| yf | list+nav | copy feed URL |
| st / sn / sf / su | list | push sort level: time / title / feed / unread-first (last pressed = highest; keep last 3) |
| S | list | reverse sort order |

### Decisions

| Decision | Choice | Rationale | Date |
|---|---|---|---|
| Branch strategy | main = design docs only; parallel impl branches rebase on main | Single design source, parallel impl | 2026-08 |
| Persistence | SQLite (items + content + read flags) | Single-file, queryable, preserves state across refresh | 2026-08 |
| Layout | 3 panes: nav tree / item list / article | Mirrors mail-client pattern | 2026-08 |
| Tree | Virtual nodes (Unread/Read Later/Favourite/Saved) + Categories tree (nested) + Tags list | single nav pane, mail-client pattern | 2026-08 |
| Keys | Vim keys + arrows both bound; h/l/q/enter/esc/←/→ for left/right | Familiarity; explicit nav semantics | 2026-08 |
| Feed source | newsboat `urls` format, read live | Zero migration; coexists with newsboat; single source of truth | 2026-08 |
| Category vs tags | one category (tree placement) + multi `#tags` (optional) | category = structure, tags = cross-cutting | 2026-08 |
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
  - `refresh` — auto-on-startup on/off, interval (`interval_minutes`).
  - `nav_presets` — list of nav section arrays, e.g. `[["Unread", "Feeds"], ["Unread", "Later"]]`; replaces the preset list (first = initial).
  - `images` — kitty image render on/off.
  - `foldlevel` — initial fold depth of nav Categories tree (0 = all folded).
  - `sort` — initial sort stack (max 3), e.g. `["unread", "time"]`, applied left-to-right; never rewritten by keypresses.
  - `fetch_timeout` — per-request timeout.
  - `max_items_per_feed` — cap items kept per feed.
  - `proxy` — HTTP proxy.
  - `keybindings` — custom keymap.
  - `default_view` — startup scope, e.g. `Feed:<url>` / `Category:<name>`.
- Read at startup; defaults + XDG fallbacks when keys absent. No hot-reload in MVP.

### Nav Pane Presets

- Each preset = array of nav sections; one default full preset: `[Unread, Read Later, Favourite, Categories, Tags, Saved]`.
- `nav_presets` replaces the list; first entry is the initial preset; `t` cycles through all presets (wrap).
- Valid sections: `Unread` (All Unread), `Read Later`, `Favourite`, `Saved`, `Categories` (tree), `Tags`, `Feeds`.

### Decisions

| Decision | Choice | Rationale | Date |
|---|---|---|---|
| Config file | separate from `urls` | app settings ≠ subscriptions | 2026-08 |
| Format | JSON / JSONC / TOML / YAML, by extension | user preference; ecosystem standard | 2026-08 |
| Theme | standalone file, referenced by `theme` key | colors ≠ app settings; swappable | 2026-08 |
| Pane ratio | `pane_ratio` key, default 0.15/0.15/0.7 | user-adjustable layout | 2026-08 |
| Browser | `browser` key, default xdg-open | user choice | 2026-08 |
| Nav layout | multiple presets (arrays), one full default, `t` cycles | user control | 2026-08 |
| Reload | startup only | MVP simplicity | 2026-08 |

## Tags & Favorites

### Virtual nodes

- **Favourite = feed-level**: `f` on a nav feed row toggles the feed's favourite; marker persisted in the urls file; Favourite node lists favourited feeds (category-tree presentation).
- **Read Later / Saved = item-level**: `L` / `S` in the article view toggle; nodes aggregate flagged items across feeds (All Unread pattern); independent, item can carry both.
- `saved` = items kept in DB without markdown; exempt from TTL cleanup.

### Tags

- Tags are **per-feed only**: each feed has 0..n tags (`#tag` in urls file), plus exactly one category.
- Tags list in nav pane (below Categories).
- Selecting a tag filters the list pane to items from feeds carrying that tag.
- **No per-item tags.**

### Decisions

| Decision | Choice | Rationale | Date |
|---|---|---|---|
| Tags | per-feed, multi (`#tag`), optional; one category | category = structure, tags = cross-cutting | 2026-08 |
| Virtual nodes | Read Later / Favourite / Saved, like All Unread | reuses All Unread aggregation; no new storage | 2026-08 |
| Favourite | feed-level flag, `f` in nav, urls file marker | favourites = feeds you track | 2026-08 |
| Read Later / Saved | item-level flags, `L`/`S` in article | orthogonal, item can carry both | 2026-08 |
| Saved | kept in DB, exempt from TTL cleanup, no markdown | keep without export | 2026-08 |
| Tags placement | tags list in nav pane (below Categories) | second nav region, not new pane | 2026-08 |
| Tag storage | per-feed in urls file only | queryable; no item tags | 2026-08 |

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

### Article Enhancement

#### Images

- Render article `<img>` via kitty graphics protocol.
- Detect terminal support; unsupported → `[img]` placeholder.
- Toggle: key `g` in article + config `images` (bool).
- Width constrained to article pane.

#### Link Jump

- `f` in article pane: highlight links, assign key hint per link — `1-9` then letters.
- Press hint key → open link in `browser` (Config key).
- Links stay underlined alt text; hints shown inline while in jump mode.

### Decisions

| Decision | Choice | Rationale | Date |
|---|---|---|---|
| Width | max reading width ~80 cols | readability | 2026-08 |
| Alignment | left, no justification | terminal readability | 2026-08 |
| Performance | viewport culling | long articles stay smooth | 2026-08 |
| Theme | follows Config `theme` | consistent look | 2026-08 |
| Images | kitty protocol; `g` toggle + `images` config | terminal graphics | 2026-08 |
| Link jump | `f` → hints → browser | fast link open | 2026-08 |

## Advanced

### OPML Mapping

- **Import**:
  - Nested `<outline>` without `xmlUrl` = folders → category hierarchy. Folder/subfolder → category/subcategory; **nested categories supported**.
  - `type="rss"` outline = feed; `category` attribute (comma-separated) → feed tags; `text`/`title` → display name; `htmlUrl` kept.
- **Export**: reverse — categories (incl. nested) → nested folders; feed tags → `category` attr (comma-joined).
- Round-trip preserves hierarchy + tags.

(more advanced functions/keys TBD by user)

### Vim-like Keys

- `gg` / `G` — jump to top / bottom (nav + list).
- `zr` / `zm` — decrease / increase fold level by 1 (unfold/fold one level).
- `zR` / `zM` — open all folds / close all folds.
- `Ctrl+f` / `Ctrl+b` — full page down / up (list + article).
- `zt` / `zz` / `zb` — scroll cursor to top / center / bottom (article).
- `{` / `}` — jump prev / next paragraph (blank line) (article).
- `[` / `]` — jump prev / next section (article headings h2/h3).
- `/` — modal search in list pane; while active `n` = next match, `esc` exits; search owns `n` only in search mode.
- `gi` — toggle kitty image render (was `g`; `g` freed for `gg` top).
- `yy` / `yn` — copy item URL / title (list + article). `yf` — copy feed URL.
- `st` / `sn` / `sf` / `su` — push a sort level (time / title / feed / unread-first). **Last pressed = highest priority** (front of array): `st` → `["time"]`, then `sf` → `["feed", "time"]`.
- Keep only the last 3 presses — a 4th drops the oldest.
- `S` — reverse the full ordering.
- Keypresses affect in-memory sort only — **config file is never modified**; `sort` config array (max 3) = initial stack, applied left-to-right (first = highest priority).
- `foldlevel` config — initial fold depth of nav Categories tree: `0` = all folded, `N` = levels 1..N open.
- Folding applies to Categories tree (nested); flat sections (Tags, virtual nodes) unaffected.

### Decisions

| Decision | Choice | Rationale | Date |
|---|---|---|---|
| OPML folders | → nested categories | preserves tree structure | 2026-08 |
| OPML category attr | → feed tags | OPML has no multi-tag; comma-separated | 2026-08 |
| Nested categories | supported in tree | deep hierarchies | 2026-08 |