# DESIGN — markerss TUI RSS Reader

Design authority for all implementation branches. Changes land here first, then branches rebase. This document is language-agnostic — it describes behavior, not specific libraries. Five parallel sections; branches implement all five, in order MVP → Config → Tags & Favorites → Article Polish → Advanced.

## MVP

### Layout

Three panes:

```
┌─ Nav ────────────────┬─ List ───────────────┬─ Article ───────────┐
│ Unread (5)           │ • item title [L]     │ header:             │
│ Read Later (2)       │ • item title [S]     │   title             │
│ Favourite (3)        │   read item          │   feed · date · url │
│ ...                  │                      │─────────────────────│
│                      │                      │ body: summary (+content)│
│ ▾ Categories         │                      │─────────────────────│
│   ▸ cat1             │                      │ content:            │
│   ▾ cat2             │                      │   RSS body / blank  │
│     feed2            │                      │   (list mode: empty)│
│ ▾ No Category (1)    │                      │                     │
│   feed-x             │                      │                     │
│ ▾ Tags               │                      │                     │
│   ▸ tag1             │                      │                     │
│   ▾ tag2             │                      │                     │
│     feed2            │                      │                     │
│ ▾ Feeds              │                      │                     │
│   feed3              │                      │                     │
│ Saved (1)            │                      │                     │
└──────────────────────┴──────────────────────┴─────────────────────┘
```

Nav pane = **preset-driven** (see Config → Nav Pane Presets). Each preset is an ordered array of sections; the default full preset is `[Unread, Read Later, Favourite, Categories, Tags, Saved]` (plus `Feeds`, `No Category` are available sections).

Top entries and their fold behavior:

| Entry | Foldable | Children |
|---|---|---|
| Unread | no | — (list of unread items) |
| Read Later | no | — (items flagged read_later) |
| Saved | no | — (items flagged saved) |
| Favourite | no (aggregate) | items of all favourited feeds (like Read Later/Saved) |
| Categories | yes | category tree → feeds |
| Tags | yes | per-tag entries, each foldable → feeds carrying it |
| Feeds | yes | all feeds (flat) |
| No Category | yes | feeds without a category (flat) |

Tree behavior:
- **All top entries highlighted** (distinct fg + bold).
- Indentation: categories 2 spaces, feeds 4 spaces under a category/tag, 2 spaces under Feeds/Favourite/No Category.
- **Counts are unread counts** on every node (feed / category / tag / Favourite / No Category / virtual nodes).
- **Feed health**: a feed whose last fetch failed shows a `!` marker on its nav row (cleared on next successful fetch).
- **Left (`h`/`q`/`esc`) cascade**: expanded header → fold it; folded header → fold its parent; a top-level folded entry stays (never jumps to Unread); a feed row folds its containing container (category → tag → section).
- **Right (`l`/`enter`)**: folded entry → expand and **jump the cursor to its first child**; expanded/leaf entry → descend to the list pane.
- Article pane split: **3-line header (title / feed·date·read / url)** + body, no separator.
- **No summary truncation**: list preview shows the full summary in the body; article mode shows **summary + feed content in order** (no duplication — summary-only feeds show the summary once).
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

Directional movement: `h`/`q`/`esc` = LEFT, `l`/`enter` = RIGHT (+ arrow keys `←`/`→`).

- **Right** (`l` / `enter` / `→`):
  - folded entry (section / category / tag / Favourite / No Category) → expand + cursor to first child
  - expanded node / feed / tag / virtual node → list pane
  - item in list → article pane + mark read
  - article pane → **always fetch full content** (even when the feed provided a body)
- **Left** (`h` / `q` / `esc` / `←`): article → list → nav; in nav: expanded header → fold, folded → fold parent, top folded → stay; feed row → fold its container
- `j/k` move cursor in nav (live-preview list, no read change).
- List `j/k` moves selection — **does not mark read**. `l`/`enter` opens the article (marks read, clears read-later).
- List `n`/`p` — mark current read, jump to next/prev unread (no reorder).
- **List semantics**: the list is a snapshot taken at startup / manual refresh (`R`) / scope change. Auto fetch (`r`/startup/interval) only **appends new unread items** — read items stay in place, never reordered, until manual refresh or restart.
- **Read-later lifecycle**: marking read-later also marks the item unread; opening/reading an item clears its read-later flag.
- Article pane: preview (list focus) shows the full summary; article mode shows summary + content (blank only when a feed has neither — `enter` fetches); `n/p` parent navigation (article→list cursor, list→nav cursor); `j/k` scroll; ctrl+u/ctrl+d half-page.

### Storage

- SQLite database at `$XDG_CACHE_HOME/markerss/markerss.db` — items + content + flags; WAL journal + `(feed_url, read)` / `(read_later)` / `(saved)` indexes.
- Items keyed `(feed_url, guid)`; flags and content preserved across refresh.
- Per-item boolean flags: `read`, `read_later`, `saved` — independent. **Favourite is feed-level** (stored in the urls file as `!favourite` marker, not in the DB).
- **Feed-provided content is kept on refresh** (arrives with the feed — no extra request); full-article fetch (readability) may replace it.
- Markdown generated ONLY at export time, never stored.
- Configurable TTL: startup purge of fetched content older than `cache_ttl_days` — **`saved` items exempt**.
- Subscriptions stay in the `urls` file (newsboat format) — DB holds items only.

### Full-Content Fetch

- On `l`/`enter` in the article pane: fetch the article page, extract the main content (readability-style), store in DB.
- **Always attempted** — even when the feed provided a body (may replace it).
- Extraction removes nav/sidebar/ads; falls back to raw page if extraction fails.

### Rendering

- Rendering strategy is **per-language** — each branch chooses its own HTML → styled-text pipeline.
- Behavioral contract (all branches):
  - Headings bold, lists/code blocks rendered.
  - Links as **underlined alt text, URL never shown** (urls stripped at display; export keeps them).
  - Images shown as `[img]` placeholder.
  - `<sub>`/`<sup>` → `~x~`/`^x^` markers (rendered as subscript/superscript).
- Body markdown = summary + feed content in order (each rendered once; no duplication).
- Nav feed rows display the **feed's own title** (fetched from the feed XML, persisted to the urls file) — priority: custom name > feed title > url.
- Export format = the same markdown, written to file on `e`.
- Help = floating opaque window (default colors), scrollable with `j`/`k`/arrows.

### Export

- `e` → prompt with the **default path prefilled** as placeholder (`$XDG_DATA_HOME/markerss/<category>/<slug>.md`, uncategorized → root); enter accepts, or type a custom path.
- Markdown: YAML frontmatter (title, link, date, feed) + full content.

### Refresh

- **Partial** (`r`): fetch new items for the **current scope's feeds only** (upsert — never removes read items; new unread appended to the list top).
- **Full** (`R`): fetch every feed, rebuild the list snapshot, re-apply read state (unread only in All Unread).
- Auto fetch on startup (background, non-blocking) + optional interval — both behave like partial refresh.

### Paths (XDG)

- Config: `$XDG_CONFIG_HOME/markerss/` — two separate files:
  - `config.toml` (app settings: cache TTL, export dir, pane ratio, browser, refresh, nav_presets) — TOML assumed; other formats by extension only
  - `urls` (newsboat-format subscriptions — kept separate from app config)
- Cache/DB: `$XDG_CACHE_HOME/markerss/markerss.db` (SQLite)
- Export: data dir (configurable)
- Fallbacks per XDG spec when vars unset.

### Keys Summary

| Key | Scope | Action |
|---|---|---|
| h / q / esc / ← | global | go LEFT: article→list→nav; in nav: fold, then fold parent (top folded stays) |
| l / enter / → | global | go RIGHT: expand (cursor → first child) → list → article+read → fetch full |
| j/k | nav | move (live-preview list) |
| j/k | list | move selection (no read change) |
| j/k | article | scroll |
| n/p | list | mark current read + jump to next/prev unread (no reorder) |
| n/p | article | next/prev item (marks read, clears read-later) |
| u | list+article | toggle read/unread |
| a | list | mark all items in current list read |
| A | list | mark all items in all feeds read |
| <c-u>/<c-d> | list+article | scroll half page |
| o | list+article | open in browser |
| e | list+article | export markdown (rename prompt, default prefilled) |
| N | nav | add feed (URL → title → category → tags prompts) |
| d | nav | delete feed (press twice to confirm) |
| M | nav | modify — feed custom title / category / tags (contextual) |
| T | nav | edit feed tags (prefilled) |
| F | nav / article | nav: favourite feed · article: toggle fullscreen (f freed) |
| D | nav | delete feed (press twice to confirm) |
| n/p | global | parent navigation: article → list cursor, list → nav cursor |
| J/K | list | next/prev unread (mark read + jump) |
| gg | nav+list+article | jump top |
| yy/yn/yp/ys/yc | list+nav | copy item url / title / feed url / summary / full content (markdown) |
| L / S | list+article | toggle read-later / saved (again to cancel; L marks unread) |
| t | nav | cycle nav preset |
| r / R | global | partial refresh (current scope) / full refresh (rebuild) |
| i / x | global | import / export OPML |
| Q | global | quit app |
| Tab / Shift+Tab | global | focus next / prev pane |
| ? | global | help (floating scrollable window) |

Advanced keys (planned, unbound or remapped — see Advanced): `gg/G`, `Ctrl+f/b`, `zt/zz/zb`, `{/}`, `[/]`, `/` search, `yy/yn/yf`, `st/sn/sf/su` (+`sT/sN/sF/sU` reversed), `gi` images.

### Decisions

| Decision | Choice | Rationale | Date |
|---|---|---|---|
| Branch strategy | main = design docs only; parallel impl branches rebase on main | Single design source, parallel impl | 2026-08 |
| Persistence | SQLite (items + content + read flags) | Single-file, queryable, preserves state across refresh | 2026-08 |
| Layout | 3 panes: nav tree / item list / article | Mirrors mail-client pattern | 2026-08 |
| Tree | Preset-driven sections: Unread/Read Later/Favourite/Categories/Tags/Saved/Feeds/No Category; foldable except Unread/Read Later/Saved | single nav pane, mail-client pattern | 2026-08 |
| Keys | h/q/esc left, l/enter right; u/a/A/N/M/F/L/S/t/r/R/i/x/Q | Explicit directional nav + per-action keys | 2026-08 |
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
  - `foldlevel` — initial fold depth of nav Categories tree (0 = all folded).
  - `sort` — initial sort stack (max 3), e.g. `["unread", "time"]`, applied left-to-right; never rewritten by keypresses.
  - `fetch_timeout` — per-request timeout.
  - `max_items_per_feed` — cap items kept per feed.
  - `reading_width` — max article body columns (0 = fill pane).
  - `keybindings` — map of action → key string or list (single keys, combos like `gg`, specials like `<enter>`), in config.toml `[keybindings]` or standalone `keybindings.toml` (replaces the config map). Combos match via a prefix buffer; ctrl chords are never rebindable. Actions: open back quit refresh refresh_all toggle_read mark_read mark_all_read (alias mark_read_all) export browser favourite read_later saved new_feed delete rename edit_tags help focus_next focus_prev search jump_top jump_bottom next_unread prev_unread parent_next parent_prev copy_item_url copy_item_title copy_feed_url copy_item_summary copy_item_content sort_time sort_title sort_feed sort_unread sort_*_rev cycle_preset import_opml export_opml.
  - `default_view` — startup scope, e.g. `Feed:<url>` / `Category:<name>`.
- Read at startup; defaults + XDG fallbacks when keys absent. No hot-reload in MVP.

### Nav Pane Presets

- Each preset = array of nav sections; one default full preset: `[Unread, Read Later, Favourite, Categories, Tags, Saved]`.
- `nav_presets` replaces the list; first entry is the initial preset; `t` cycles through all presets (wrap).
- Valid sections: `Unread`, `Read Later`, `Favourite`, `Saved`, `Categories` (tree), `Tags`, `Feeds`.
- `No Category` renders automatically at the end of the Categories section.
- Section rendering: single-node sections (Unread / Read Later / Saved / Favourite) render as the node itself; list sections (Categories / Tags / Feeds) render a foldable header row + children.

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
- **Read Later / Saved = item-level**: `L` / `S` in the list or article view toggle (again to cancel); nodes aggregate flagged items across feeds (All Unread pattern); independent, item can carry both.
- **Read-later lifecycle**: `L` also marks the item unread; opening/reading an item clears its read-later flag.
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

Status per key — **[done]** rust implemented · **[dropped]** user removed · *(plain)* planned/not bound.

- `gg` / `G` — jump to top / bottom (nav + list + article). **[done]**
- `Ctrl+f` / `Ctrl+b` — full page down / up (list + article). **[done]**
- `/` — modal search in list pane: live filter over title+summary; `enter` keeps the filter active (new appends still re-filter), `esc` or **left (`h`/`q`/`esc`) stops it** — the pre-search list is restored (first left cancels search and stays in the list; a second left goes back to nav). **[done]**
- `yy` / `yn` — copy item URL / title (list + article). `yf` — copy feed URL. `ys` / `yc` — copy summary / full content (summary + content, markdown). **[done]** (OSC52, no dependency)
- `st` / `sn` / `sf` / `su` — push a sort level (time / title / feed / unread-first). **Last pressed = highest priority** (front of array): `st` → `["time"]`, then `sf` → `["feed", "time"]`. Keep only the last 3 presses — a 4th drops the oldest. **[done]**
- `sT`/`sN`/`sF`/`sU` — push the same level **reversed** (per-level direction: `sT` = time ascending). **[done]**
- `s` / `y` alone show their combo help in the status bar. **[done]**
- `keybindings` config — remap single-key actions (see Config). **[done]**
- `foldlevel` config — initial fold depth of nav Categories tree: `0` = all folded, `N` = levels 1..N open. **[done]**
- `zr` / `zm` / `zR` / `zM` — fold level controls. **[dropped]** (user: not needed)
- `zt` / `zz` / `zb` — scroll cursor to top / center / bottom (article). *(planned — scroll-only view, no cursor)*
- `{` / `}` — jump prev / next paragraph (blank line) (article). *(planned)*
- `[` / `]` — jump prev / next section (article headings h2/h3). *(planned)*
- ~~`gi` — kitty image render~~ — **[removed]** (unstable, default-off; removed entirely).
- `zr` / `zm` — decrease / increase fold level by 1 (unfold/fold one level).
- `zR` / `zM` — open all folds / close all folds.
- `Ctrl+f` / `Ctrl+b` — full page down / up (list + article).
- `zt` / `zz` / `zb` — scroll cursor to top / center / bottom (article).
- `{` / `}` — jump prev / next paragraph (blank line) (article).
- `[` / `]` — jump prev / next section (article headings h2/h3 etc).
- `/` — modal search in list pane: live filter over title+summary; `enter` keeps the filter active (new appends still re-filter), `esc` or **left (`h`/`q`/`esc`) stops it** — the pre-search list is restored (first left cancels search and stays in the list; a second left goes back to nav).
- `gi` — toggle kitty image render (was `g`; `g` freed for `gg` top).
- `yy` / `yn` — copy item URL / title (list + article). `yf` — copy feed URL.
- `st` / `sn` / `sf` / `su` — push a sort level (time / title / feed / unread-first). **Last pressed = highest priority** (front of array): `st` → `["time"]`, then `sf` → `["feed", "time"]`.
- Keep only the last 3 presses — a 4th drops the oldest.
- `sT`/`sN`/`sF`/`sU` — push the same level **reversed** (per-level direction: `sT` = time ascending).
- `s` / `y` alone show their combo help in the status bar.
- Keypresses affect in-memory sort only — **config file is never modified**; `sort` config array (max 3) = initial stack, applied left-to-right (first = highest priority).
- `foldlevel` config — initial fold depth of nav Categories tree: `0` = all folded, `N` = levels 1..N open.
- Folding applies to Categories tree (nested); flat sections (Tags, virtual nodes) unaffected.

### Decisions

| Decision | Choice | Rationale | Date |
|---|---|---|---|
| OPML folders | → nested categories | preserves tree structure | 2026-08 |
| OPML category attr | → feed tags | OPML has no multi-tag; comma-separated | 2026-08 |
| Nested categories | supported in tree | deep hierarchies | 2026-08 |
