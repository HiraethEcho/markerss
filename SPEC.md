# SPEC

## Goal
TUI RSS reader — browse feeds in terminal; store blog posts as markdown on command.

## What We're Building
- Three-pane TUI: nav pane (category tree) / list pane (item list) / article pane
- Nav tree: categories → feeds; uncategorized at root; h/l collapse/expand; All Unread node at top
- Feed sources: newsboat `urls` format primary (`url "title" tags`); tags = categories; `~` prefix = hidden feed (show only when unread); OPML import/export compatible
- Reading flow: list nav → article pane shows summary preview, stays unread; `<enter>` → mark read + load full content; summary-only feeds → enter fetches full article
- Keys: `o` browser / `e` export / `u` mark-unread in list+article panes; article pane `n/p` next/prev item, `j/k` scroll; auto-advance
- Category CRUD in TUI (create/rename/delete, assign feed); feed CRUD
- Refresh: auto on startup + `r` manual
- Export: YAML frontmatter (title/link/date/feed) + full content; filename = title slug; markdown generated only at export time
- Cache: fetched full articles stored as gzipped raw HTML + metadata; converted to markdown on demand; configurable cleanup TTL
- Paths: XDG — config/state in `$XDG_CONFIG_HOME`, cache in `$XDG_CACHE_HOME`, data in `$XDG_DATA_HOME`
- Post-MVP: tags + read-later strip in lower nav pane
- Minimal MVP — no sync, no background daemon, no accounts

## Decisions
| Decision | Choice | Rationale | Date |
|---|---|---|---|
| Language | 3-way parallel eval: Rust / Go / C++ branches | Performance + MVP speed both explored; C++ for newsboat reference | 2026-08 |
| Branch strategy | main = design docs only; rust/go/cpp worktree branches rebase on main | Single design source, parallel impl | 2026-08 |
| TUI lib | Per branch: ratatui / bubbletea / FTXUI | Parallel eval | 2026-08 |
| RSS parsing | Per branch: feed-rs / gofeed / pugixml+libcurl | Parallel eval | 2026-08 |
| Persistence | Local JSON state file | MVP-simple; no DB dependency | 2026-08 |
| Layout | 3 panes: nav tree / item list / article | Mirrors mail-client pattern | 2026-08 |
| Tree | Categories → feeds, uncategorized at root, h/l collapse | File-tree mental model | 2026-08 |
| Keys | Vim keys + arrows both bound | Familiarity | 2026-08 |
| Post-MVP | Lower nav row: tags + read-later | Deferred, nav strip reserved | 2026-08 |
| Feed source | newsboat `urls` format, live file | Zero migration; coexists with newsboat; single source of truth | 2026-08 |
| Tags | = categories; `~` title prefix = hidden feed | newsboat conventions preserved | 2026-08 |
| OPML | Import/export supported | Interop with other readers | 2026-08 |
| Article flow | Preview summary (unread) → enter = mark read + full content | Read state only changes on commit | 2026-08 |
| Cache | gzipped raw HTML + metadata, markdown on demand | Efficient; TTL configurable | 2026-08 |
| Paths | XDG config/cache/data | Platform convention | 2026-08 |
| Export target | `./out/` markdown, filename from article title | Predictable, user-visible | 2026-08 |
