# Skeleton scaffold — Phase 1-2

## ADDED Requirements

### Requirement: Build system
The project SHALL build with a plain Makefile using g++ `-std=c++20`
(no cmake — not installed on the machine). `make` SHALL produce
`build/markerss` (TUI) and `build/test_feedlist` (parser tests);
`make test` SHALL run the test binary; `make run` SHALL launch the TUI.

#### Scenario: make succeeds
Given a clean checkout with g++ and make
When `make` runs
Then both binaries exist under `build/` and exit 0

#### Scenario: parser tests pass
When `make test` runs
Then test binary exits 0 with zero failures

### Requirement: XDG path helpers
`src/xdg.{h,cpp}` SHALL resolve config/cache/state homes per XDG spec
(absolute env var, else `~/.config`, `~/.cache`, `~/.local/state`) and
SHALL expose `markerss/`-suffixed dirs. No directory creation.

#### Scenario: fallback when env unset
Given no XDG vars set
When `xdg_config_home()` is called
Then returns `$HOME/.config`

#### Scenario: env override
Given `XDG_CACHE_HOME=/tmp/x`
When `xdg_cache_home()` is called
Then returns `/tmp/x`

### Requirement: Feed list parser (module feedlist)
`src/feedlist.{h,cpp}` SHALL parse newsboat `urls` lines into typed `Feed`
structs: `URL "Title" tag1 tag2`; quoted title optional; `~` inside quotes
SHALL set `custom_display` and be stripped; trailing tokens SHALL be tags;
`#` comments and blank lines SHALL be skipped; malformed lines SHALL be
skipped and counted.

#### Scenario: quoted title
Given line `https://a/feed "T" c1`
Then Feed has url `https://a/feed`, title `T`, tags `[c1]`, no custom flag

#### Scenario: custom display name
Given line `https://a/feed "~My Name" c1`
Then title is `My Name` with `custom_display` true

#### Scenario: no title
Given line `https://a/feed c1`
Then title empty, tags `[c1]`

#### Scenario: comments and blanks skipped
Given content with `# c` and blank lines
Then they are skipped and counted, not parsed

#### Scenario: whole-file parse
Given multi-line urls content
Then returns ordered feeds + skipped count

### Requirement: Three-pane TUI skeleton
`src/main.cpp` SHALL render an ncursesw layout: nav (All Unread top,
first-tag categories → feeds, uncategorized at root) | list (placeholders)
| article (header + content placeholders). urls source SHALL be argv >
`$XDG_CONFIG_HOME/markerss/urls` > embedded sample. Keys SHALL be `q`
(quit) and Tab (pane focus switch) only.

#### Scenario: layout renders
Given terminal ≥ 30 cols
When binary runs
Then three bordered panes render with titles Nav/List/Article

#### Scenario: quit
When `q` pressed
Then program exits cleanly, terminal restored

#### Scenario: nav from urls file
Given urls file with tagged + untagged feeds
Then nav shows categories with feeds, uncategorized feeds at root

## MODIFIED Requirements

(none — new capability)

## Out of scope
Fetching, read-state DB, cache, refresh, export, OPML, CRUD, h/l, j/k/n/p,
enter actions — later phases.
