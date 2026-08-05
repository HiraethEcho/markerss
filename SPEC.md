# SPEC

TUI RSS reader — browse feeds in terminal, store blog posts as markdown on command. Five parallel specs, each language-agnostic. Impl branches implement all five, in order MVP → Config → Tags & Favorites → Article Polish → Advanced.

## MVP

### Goal
Three-pane TUI RSS reader: browse feeds, store posts as markdown on command.

### What We're Building
- Three-pane TUI: nav pane / list pane (item list) / article pane
- Nav pane: All Unread / Read Later / Favourite / Saved virtual nodes + Categories tree (uncategorized feeds at root) + Tags list
- Feed sources: newsboat `urls` format primary (`url "custom title" category #tag1 #tag2`) + OPML file; quoted title = custom display name; OPML import/export
- Category vs tags: each feed has exactly one category (tree placement) + 0..n `#tags` (optional)
- Navigation: `h/l`, `q/enter/esc`, `←/→` for left/right — right: folded category→expand, expanded category/feed→list, item→article+read, article→fetch full; left: go back one level
- Reading flow: refresh stores summary only; list nav → article header shows summary; `<enter>` list → open + mark read; `<enter>` article pane → fetch full article (never auto-fetch)
- Keys: `o` browser / `e` export / `u` toggle-read in list+article panes; `A` mark-all-read; article pane `n/p` next/prev item (marks read), `j/k` scroll, `<c-u>/<c-d>` half-page
- Category CRUD in TUI (create/rename/delete, assign feed); feed CRUD
- Refresh: auto on startup + `r` manual
- Export: YAML frontmatter (title/link/date/feed) + full content; default `$XDG_DATA_HOME/markerss/<category>/<slug>.md` (uncategorized → `markerss/<slug>.md`); markdown generated only at export time
- Storage: SQLite in `$XDG_CACHE_HOME/markerss/markerss.db` — items + content + flags (`read` / `read_later` / `favorite` / `saved`); fetched content preserved across refresh; TTL cleanup exempts `saved`
- Rendering: strategy per-language; shared contract — links = underlined alt (no URL), images = `[img]`, headings bold, lists/code rendered; full-article via readability extraction
- Paths: XDG — config (`config` + `urls` files separate) in `$XDG_CONFIG_HOME`, DB in `$XDG_CACHE_HOME`, export in `$XDG_DATA_HOME`
- Minimal MVP — no sync, no background daemon, no accounts

### Decisions
| Decision | Choice | Rationale | Date |
|---|---|---|---|
| Branch strategy | main = design docs only; parallel impl branches rebase on main | Single design source, parallel impl | 2026-08 |
| Persistence | SQLite (items + content + read flags) | Single-file, queryable, preserves state across refresh | 2026-08 |
| Layout | 3 panes: nav tree / item list / article | Mirrors mail-client pattern | 2026-08 |
| Tree | Categories → feeds, uncategorized at root, h/l collapse | File-tree mental model | 2026-08 |
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

### Goal
User-configurable app settings via a config file.

### What We're Building
- Config file at `$XDG_CONFIG_HOME/markerss/config`, separate from `urls` subscriptions file
- Format: JSON, JSONC, TOML, or YAML — detected by extension (`.json`/`.jsonc`/`.toml`/`.yaml`/`.yml`); plain `config` defaults to one (TOML)
- Keys: `cache_ttl_days` (startup purge), `export_dir` (export location), `pane_ratio` (three-pane widths, e.g. 0.15/0.15/0.7), `theme` (standalone color file), `browser` (which browser to open), `refresh` (auto-on-startup on/off, interval), `nav_presets` (list of nav section arrays, e.g. `[["Unread", "Feeds"], ["Unread", "Later"]]`), `images` (kitty image render on/off)
- Optional: `fetch_timeout`, `max_items_per_feed`, `proxy`, `keybindings`, `default_view`
- Nav pane: multiple layout presets, each preset = array of sections; one default full preset (Unread/Read Later/Favourite/Categories/Tags/Saved); `nav_presets` replaces the list (first = initial); `t` cycles presets (wrap)
- Defaults when keys absent; XDG fallbacks per spec
- Read at startup; changes require restart (no hot-reload in MVP)

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

### Goal
Organize beyond feeds: favourites (feed-level) and read-later/saved (item-level); feed tags.

### What We're Building
- **Favourite = feed-level**: `f` on a nav feed row toggles the feed's favourite (persisted in urls file); Favourite node = list of favourited feeds (same presentation as category tree)
- **Read Later / Saved = item-level**: `L`/`S` in article view toggle per-item flags; nodes aggregate flagged items like All Unread; independent, item can carry both
- `saved` = kept in DB without markdown, exempt from TTL cleanup
- Tags: per-feed only, 0..n (`#tag` in urls file) plus exactly one category; tags list in nav (below Categories); select tag → filter list to feeds carrying it; no per-item tags

### Decisions
| Decision | Choice | Rationale | Date |
|---|---|---|---|
| Tags | per-feed only, multi (`#tag`), optional; one category | category = structure, tags = cross-cutting | 2026-08 |
| Favourite | feed-level flag, `f` in nav, urls file marker | favorites = feeds you track | 2026-08 |
| Read Later / Saved | item-level flags, `L`/`S` in article | orthogonal, item can carry both | 2026-08 |
| Virtual nodes | Favourite = feed list; Read Later / Saved = item aggregation | reuses All Unread aggregation pattern | 2026-08 |
| Saved | kept in DB, exempt from TTL cleanup, no markdown | keep without export | 2026-08 |
| Tags placement | tags list in nav pane (below Categories) | second nav region, not new pane | 2026-08 |
| Tag storage | per-feed in urls file | queryable, no item tags | 2026-08 |

## Article Polish

### Goal
Comfortable long-form reading in the article pane — typography, spacing, element styling, performance.

### What We're Building
- Max reading width for readability (e.g. 80 cols); left-aligned, no justification
- Typography: paragraph spacing, heading hierarchy (bold, distinct style), links underlined alt, images `[img]` with alt
- Element rendering: lists indented, code blocks monospace + border, inline code distinct bg, blockquotes indented/colored, tables best-effort aligned, horizontal rules
- Scroll: j/k line, ctrl-u/ctrl-d half-page, n/p item; scrollbar indicator
- Performance: render only visible lines (viewport culling), cap content size, no full-pane redraw on scroll
- Theme-aware colors (respects Config `theme` file)

### Article Enhancement
- Images: render article `<img>` via kitty protocol; terminal-detect → `[img]` fallback; toggle `g` + config `images`; width = article pane
- Link jump: `f` highlights links, 1-9 then letter hints; press key → open in `browser`

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

### Goal
(TBD — user writes details later: advanced functions, more keys, etc.)