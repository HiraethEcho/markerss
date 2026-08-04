# markerss

TUI RSS reader — browse feeds in the terminal, store blog posts as markdown on command.

## Repo Layout

`main` is design-only (sdd-lite): [SPEC.md](SPEC.md) (intent), [PLAN.md](PLAN.md) (roadmap), [DESIGN.md](DESIGN.md) (TUI design). Five parallel specs — **MVP**, **Config**, **Tags & Favorites**, **Article Polish**, **Advanced** — each a `##` section in all three docs.

Implementation lives in parallel worktrees, one per language (full sdd):
- `rust` → `../markerss-rust` (Rust + ratatui + feed-rs)
- `go` → `../markerss-go` (Go + bubbletea + gofeed)
- `cpp` → `../markerss-cpp` (C++ + FTXUI + libcurl)

Each branch rebases on main and implements all five specs, in order MVP → Config → Tags & Favorites → Article Polish → Advanced.

## Usage

### Subscriptions
`$XDG_CONFIG_HOME/markerss/urls` — newsboat format: `URL "Title" tag1 tag2`; `~` title prefix = custom display name.

### Config
`$XDG_CONFIG_HOME/markerss/config` — JSON / JSONC / TOML / YAML (by extension); keys `cache_ttl_days`, `export_dir`, refresh behavior.

### State & Cache
- DB: `$XDG_CACHE_HOME/markerss/markerss.db` (items + content + read flags)
- Export: `$XDG_DATA_HOME/markerss/<category>/<slug>.md` (uncategorized → root)

### Keys
`?` in-app help. Core: nav `j/k h/l enter`, article `n/p j/k ctrl+u ctrl+d enter`, `o` browser, `e` export, `u` read toggle, `A` mark-all-read, `r` refresh, `a/d/R` feed/category CRUD, `i/x` OPML.

## Build & Run (per branch)

```sh
# rust
cd ../markerss-rust && cargo build --release && cargo run

# go
cd ../markerss-go && go build && ./markerss

# cpp
cd ../markerss-cpp && make && ./build/markerss
```

## Status
MVP in progress — see PLAN.md.