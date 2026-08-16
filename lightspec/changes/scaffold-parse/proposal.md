# Proposal: scaffold-parse

## Change

Scaffold Phase 1-2 of the C++ MVP TUI RSS reader: build system, XDG path
helpers, newsboat urls parser, ncurses three-pane skeleton. Skeleton only —
no fetching, no state, no persistence.

## Why

Validate the C++ branch stack: build tooling (no cmake → Makefile), TUI
library choice (ncurses vs FTXUI), newsboat urls parsing, and XDG path
handling — before committing to the full MVP. Also surfaces DESIGN gaps
early (pilot).

## What Changes

- Adds `Makefile` build (g++ C++20, ncursesw) — no cmake on machine
- Adds `src/xdg.{h,cpp}` XDG dir helpers (config/cache/state)
- Adds `src/feedlist.{h,cpp}` newsboat urls parser + typed `Feed` structs
- Adds `src/test_feedlist.cpp` assert-based parser tests
- Adds `src/main.cpp` ncursesw three-pane skeleton (nav/list/article)
- Nav tree built from parsed feeds (placeholder unread counts)

## Impact

- New files only; no existing code touched; no DESIGN/SPEC changes
- Unread counts are placeholders until Phase 3 state DB

## Requirements

### ADDED: Build system
- `make` (no cmake on machine) builds `build/markerss` TUI binary and
  `build/test_feedlist` parser test binary with g++ -std=c++20.
- `make test` runs parser tests; `make run` launches TUI.

### ADDED: XDG dir helpers
- `src/xdg.{h,cpp}` resolve config/cache/state homes per XDG spec
  (env var, else `~/.config`, `~/.cache`, `~/.local/state`), plus
  `markerss/`-suffixed dirs. No directory creation yet.

### ADDED: Feed list parser (module `feedlist`)
- `src/feedlist.{h,cpp}` parse newsboat `urls` format lines into typed
  `Feed` structs: `URL "Title" tag1 tag2`; quoted title optional;
  `~` title prefix → `custom_display` flag, tilde stripped; remaining
  tokens = tags; `#` comments and blank lines skipped; malformed lines
  skipped and counted.
- `src/test_feedlist.cpp`: plain assert-based tests (no framework) covering
  quoted title, `~` prefix, multiple tags, no title, comment lines,
  blank lines, unterminated quote, whole-file parse.

### ADDED: Three-pane TUI skeleton
- `src/main.cpp` ncursesw layout: nav (category tree: `All Unread` top,
  first-tag categories → feeds, uncategorized at root) | list (placeholder
  items) | article (header + content placeholders).
- Nav tree built from parsed feeds (placeholder unread counts, real state
  is Phase 3). urls source priority: argv > `$XDG_CONFIG_HOME/markerss/urls`
  > embedded sample.
- Keys: `q` quit, Tab pane focus switch only.

### TUI library decision
- ncursesw chosen over FTXUI: ncursesw already installed; FTXUI needs
  submodule + build → too heavy for a skeleton. Swap to FTXUI possible
  later; parser/xdg modules are lib-independent.

## Out of scope (later phases)
- Feed fetching, read-state DB, cache, refresh, export, OPML, category CRUD.
- h/l collapse, j/k/n/p navigation, enter actions.

## Design gaps surfaced (pilot note)
- DESIGN: "~ prefix … NOT hidden" — ambiguous whether tilde shown in UI.
  newsboat hides it; parser strips it and flags `custom_display`. Needs
  clarification in DESIGN.
- DESIGN lists no title case; parser treats unquoted tokens after URL as
  tags (newsboat-compatible). Confirmed OK.
- Unread counts and `All Unread (n)` need Phase 3 state — skeleton renders
  placeholder counts; nav tree shape is final.
