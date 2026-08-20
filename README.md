# markerss

TUI RSS reader — browse feeds in the terminal, store blog posts as markdown on command.

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
# rust
git checkout rust && cargo build --release && cargo run

# go
git checkout go && go build && ./markerss

# cpp
git checkout cpp && make && ./build/markerss
```

## Status
MVP in progress — see PLAN.md.