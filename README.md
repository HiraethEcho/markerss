# markerss (go branch)

TUI RSS reader — browse feeds in terminal, store blog posts as markdown on command.
Go + [bubbletea](https://github.com/charmbracelet/bubbletea) + [gofeed](https://github.com/mmcdole/gofeed).

## Repo Layout

`main` is design-only (sdd-lite): [SPEC.md](SPEC.md) (intent), [PLAN.md](PLAN.md) (roadmap), [DESIGN.md](DESIGN.md) (TUI design). Five parallel specs — **MVP**, **Config**, **Tags & Favorites**, **Article Polish & Enhancement**, **Advanced** — each a `##` section in all three docs.

Implementation lives on parallel branches, one per language (full sdd):
- `rust` (Rust + ratatui + feed-rs)
- `go` (Go + bubbletea + gofeed)
- `cpp` (C++ + FTXUI + libcurl)

Each branch rebases on main and implements all five specs, in order MVP → Config → Tags & Favorites → Article Polish → Advanced.

## Usage

### Subscriptions
`$XDG_CONFIG_HOME/markerss/urls` — newsboat format: `URL "custom title" category #tag1 #tag2`; quoted title = display name.

### Config
`$XDG_CONFIG_HOME/markerss/config.toml` — JSON / JSONC / TOML / YAML (by extension; plain `config` fallback); keys `cache_ttl_days`, `export_dir`, refresh behavior.

### State & Cache
- DB: `$XDG_CACHE_HOME/markerss/markerss.db` (items + content + read flags)
- Export: `$XDG_DATA_HOME/markerss/<category>/<slug>.md` (uncategorized → root)

### Keys
`?` in-app help. Core: nav `j/k h/l enter`, article `n/p j/k ctrl+u ctrl+d enter`, `o` browser, `e` export, `u` read toggle, `a` mark current list read, `A` mark all feeds read, `r` refresh, `d` delete, `f` link jump, `gi` image toggle, `t` nav layout toggle, `gg/G` top/end, `/` search, `Ctrl+f/b` page, `zt/zz/zb`, `{/}`, `[/]`, `space` read+next, `yy/yn/yp/ys/yc` copy (url/title/feed/summary/content), `st/sn/sf/su` sort stack + `S` reverse.

## Build & Run (per branch)

```sh
go build
./markerss          # TUI; needs a real terminal
go test ./...       # unit + integration tests
```

First run with no subscriptions: press `N` in the nav pane to add a feed.

## Config

`$XDG_CONFIG_HOME/markerss/config.toml` (TOML default; JSON/JSONC/YAML by
extension). Keys (defaults when absent):

```toml
cache_ttl_days = 30        # startup purge of fetched content
export_dir = "..."         # default: $XDG_DATA_HOME/markerss
pane_ratio = [0.15, 0.15, 0.7]
browser = "xdg-open"       # `o` opens links
refresh = true             # or { auto_on_startup = false, interval_minutes = 30 }
nav_presets = [["Unread", "Categories", "Tags"], ["Unread", "Feeds"]]  # `t` cycles
theme = "~/.config/markerss/themes/dark.toml"  # standalone color file
foldlevel = 1              # initial Categories fold depth (0 = all folded)
sort = ["unread", "time"]  # initial sort stack (max 3)
fetch_timeout = 30
max_items_per_feed = 0     # cap per feed on refresh
proxy = ""                 # HTTP proxy
keybindings = { j = "down" }   # custom keymap
default_view = "Category:tech" # startup scope: Feed:<url> / Category:<name>
```

Restart to apply — no hot-reload.

## Subscriptions

`$XDG_CONFIG_HOME/markerss/urls`, newsboat format:

```
https://example.com/feed "~Display" category #tag1 #tag2 !favourite
```

- Quoted title = custom display name (`~` marks it custom)
- First bare token = **category** (single; `a/b` nests)
- `#tag` tokens = feed tags (multi)
- `!favourite` = feed-level favourite (`F` toggles in nav)

TUI CRUD (`N` add / `d` x2 delete / `M` rename / `T` edit tags) rewrites this file.
OPML import/export via `i`/`x`.

## Storage

- DB — `$XDG_CACHE_HOME/markerss/markerss.db`: items keyed (feed_url, guid),
  read flags + fetched content survive refresh. Summary-only on refresh;
  full article fetched only on `enter` in the article pane.
- Export — `e` prompts with the default path prefilled
  (`$XDG_DATA_HOME/markerss/<category>/<slug>.md`); markdown with YAML
  frontmatter (title/link/date/feed), generated only at export time.

## Keys

| Key | Action |
|---|---|
| `h/q/esc/←` | left: article → list → nav; fold → fold parent |
| `l/enter/→` | right: expand (cursor → first child) → list → article+read → fetch full |
| `j/k` | move / scroll (arrows too) |
| `n/p` | list: mark read + jump unread · article: next/prev item |
| `a` / `A` | toggle read / mark all in view read |
| `ctrl+u`/`ctrl+d` | article: half-page scroll (pgup/pgdown) |
| `o` / `e` | open in browser / export (path prompt) |
| `N` / `d` / `M` / `T` | nav: new feed / delete (x2) / rename (context) / edit tags |
| `F` | nav: favourite feed · article: fullscreen |
| `t` / `r` / `R` | cycle nav preset / partial refresh / full refresh |
| `i` / `x` | import / export OPML |
| `tab` / `shift+tab` | focus next / prev pane |
| `?` / `Q` | help / quit |

## Nav Pane

Preset-driven sections (default `[Unread, Read Later, Favourite, Categories,
Tags, Saved]`; `nav_presets` replaces, first = initial, `t` cycles). Unread /
Read Later / Saved are leaf nodes; Favourite / Categories / Tags / Feeds are
foldable. Categories nest (`tech/go` → `go` under `tech`); feeds without a
category land under `No Category`. Top entries highlighted. Read Later /
Saved virtual nodes fill in with the Tags & Favorites spec.

## Status

MVP + Config implemented (go branch). Tags & Favorites, Article Polish,
Advanced pending. Design authority: `DESIGN.md` / `SPEC.md` (from `main`).
Formal spec records: `lightspec/` (see `lightspec/changes/archive/`).
