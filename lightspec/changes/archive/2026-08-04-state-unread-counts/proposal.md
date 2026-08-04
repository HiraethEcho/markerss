# Change Proposal: state-unread-counts

Phase 3 of the Rust branch: read-state persistence + unread counting +
All Unread aggregation. No fetch, no cache, no list-pane interaction yet.

## Why

DESIGN.md reading flow depends on read state (`u` toggle, `A` mark-all,
auto-advance). Counts must exist before the tree can be triage-useful.

## ADDED Requirements

### Read-state store

#### Scenario: Load state from XDG state dir
Given a JSON state file at `$XDG_STATE_HOME/markerss/state.json`, then
read flags load on startup. Missing file → empty state, no error.

#### Scenario: Save on change
When a read flag is set or cleared, then the store persists immediately.

#### Scenario: Read flag keyed by feed URL + item GUID
Given feed `https://x/feed.xml` and item guid `abc`, then toggling read
marks exactly that pair.

#### Scenario: Toggle both directions
When an item is read and `u` pressed, then it becomes unread; pressed
again → read.

### Unread counting

#### Scenario: Per-feed unread count
Given a feed with 10 items of which 7 unread, then the feed's unread
count is 7.

#### Scenario: All Unread total
Given feeds with 7 + 3 + 0 unread, then the All Unread node shows 10.

#### Scenario: Counts update live
When an item is marked read, then its feed count and the All Unread
total decrement immediately.

### Tree integration

#### Scenario: Nav shows counts
Given the nav tree, then each feed row renders `name (n)` where n is
unread; All Unread renders its total; counts respect collapsed state
(no change, display only).

## MODIFIED Requirements

### TUI skeleton (from scaffold-parse)

#### Scenario: Sample data wired to state
The in-memory sample feeds now flow through the state store; read flags
start unread.

## Out of Scope

- List pane item rendering (Phase 5)
- Keybindings `u` / `A` (Phase 5)
- Fetch, cache, OPML, category CRUD
