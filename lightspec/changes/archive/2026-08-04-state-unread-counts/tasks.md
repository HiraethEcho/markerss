# Tasks — state-unread-counts

## 1. State store module (`src/state.rs`)
- [x] `State` struct: `HashSet<(String, String)>` read pairs (feed url, guid)
- [x] `State::load(path)` — JSON read, missing → empty; `State::save(path)` — atomic write (tmp + rename)
- [x] `is_read(url, guid)`, `toggle(url, guid)` returning new state
- [x] Unit tests: load missing / save+reload / toggle both directions / keying
- [x] Path from XDG helper: `$XDG_STATE_HOME/markerss/state.json`

## 2. Counting
- [x] `unread_count(feed, state)` — needs item list; wire sample items with guids
- [x] `total_unread(feeds, state)`
- [x] Unit tests: per-feed counts, All Unread total, live decrement

## 3. Tree integration
- [x] Nav renders `name (n)`, All Unread total
- [x] Sample data flows through state (all unread initially)
- [x] Manual smoke: `cargo run` shows counts; toggle path exercised via test

## 4. Verify
- [x] `cargo build` + `cargo test` green
- [x] `lightspec validate state-unread-counts --strict`
