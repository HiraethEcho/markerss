# SPEC

## Goal
TUI RSS reader — browse feeds in terminal; store blog posts as markdown on command.

## What We're Building
- Three-pane TUI: nav pane (category tree) / list pane (item list) / article pane
- Nav tree: categories → feeds; uncategorized at root; h/l collapse/expand; All Unread node at top
- Feed sources: newsboat `urls` format primary (`url "title" tags`); tags = categories; `~` prefix = custom display name; OPML import/export compatible
- Reading flow: refresh stores summary only; list nav → article header shows summary; `<enter>` list → open + mark read; `<enter>` article pane → fetch full article (never auto-fetch)
- Keys: `o` browser / `e` export / `u` toggle-read in list+article panes; `A` mark-all-read; article pane `n/p` next/prev item (marks read), `j/k` scroll, `<c-u>/<c-d>` half-page; `q` back-nav (article→list→nav→quit)
- Category CRUD in TUI (create/rename/delete, assign feed); feed CRUD
- Refresh: auto on startup + `r` manual
- Export: YAML frontmatter (title/link/date/feed) + full content; default `$XDG_DATA_HOME/markerss/<category>/<slug>.md` (uncategorized → `markerss/<slug>.md`); markdown generated only at export time
- Storage: SQLite in `$XDG_CACHE_HOME/markerss/markerss.db` — items + content + read flags; fetched content preserved across refresh; TTL cleanup
- Rendering: content HTML → markdown → styled text; links = underlined alt (no URL), images = `[img]`; full-article via readability extraction
- Paths: XDG — config (`config` + `urls` files separate) in `$XDG_CONFIG_HOME`, DB in `$XDG_CACHE_HOME`, export in `$XDG_DATA_HOME`
- Post-MVP: tags + read-later strip in lower nav pane
- Minimal MVP — no sync, no background daemon, no accounts

## Decisions
| Decision | Choice | Rationale | Date |
|---|---|---|---|
| Branch strategy | main = design docs only; parallel impl branches rebase on main | Single design source, parallel impl | 2026-08 |
| Persistence | SQLite (items + content + read flags) | Single-file, queryable, preserves state across refresh | 2026-08 |
| Layout | 3 panes: nav tree / item list / article | Mirrors mail-client pattern | 2026-08 |
| Tree | Categories → feeds, uncategorized at root, h/l collapse | File-tree mental model | 2026-08 |
| Keys | Vim keys + arrows both bound | Familiarity | 2026-08 |
| Post-MVP | Lower nav row: tags + read-later | Deferred, nav strip reserved | 2026-08 |
| Feed source | newsboat `urls` format, live file | Zero migration; coexists with newsboat; single source of truth | 2026-08 |
| Tags | = categories; `~` title prefix = custom display name | newsboat conventions preserved | 2026-08 |
| OPML | Import/export supported | Interop with other readers | 2026-08 |
| Article flow | List enter = open + read; article enter = fetch full only (no auto-fetch) | Fetch only on explicit action; summary-only until opened | 2026-08 |
| Storage | SQLite in cache dir; summary-only on refresh; fetched content preserved | Bandwidth/storage efficient; TTL configurable | 2026-08 |
| Rendering | HTML → markdown → styled text; links = underlined alt, no URL | Clean reading; eilmeldung-style | 2026-08 |
| Paths | XDG config/cache/data; config + urls separate files | Platform convention; subscriptions ≠ app config | 2026-08 |
| Export target | `$XDG_DATA_HOME/markerss/<category>/<slug>.md` | Per-category archive; configurable | 2026-08 |
