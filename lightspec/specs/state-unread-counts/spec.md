# state-unread-counts Specification

## Purpose
TBD - created by archiving change state-unread-counts. Update Purpose after archive.
## Requirements
### Requirement: Read-state store

The app SHALL persist read flags per (feed URL, item GUID) to a JSON file in the XDG state dir, loading on startup and saving on every change. Missing file SHALL yield empty state without error.

#### Scenario: Load state from XDG state dir
Given a JSON state file at `$XDG_STATE_HOME/markerss/state.json`, then read flags load on startup. Missing file → empty state, no error.

#### Scenario: Save on change
When a read flag is set or cleared, then the store persists immediately.

#### Scenario: Read flag keyed by feed URL + item GUID
Given feed `https://x/feed.xml` and item guid `abc`, then toggling read marks exactly that pair.

#### Scenario: Toggle both directions
When an item is read and toggled, then it becomes unread; toggled again → read.

### Requirement: Unread counting

The app SHALL compute unread counts per feed and an All Unread total, updating live on read-state changes.

#### Scenario: Per-feed unread count
Given a feed with 10 items of which 7 unread, then the feed's unread count is 7.

#### Scenario: All Unread total
Given feeds with 7 + 3 + 0 unread, then the All Unread node shows 10.

#### Scenario: Counts update live
When an item is marked read, then its feed count and the All Unread total decrement immediately.

### Requirement: Tree counts

The nav tree SHALL render unread counts per feed and the All Unread total.

#### Scenario: Nav shows counts
Given the nav tree, then each feed row renders `name (n)` where n is unread; All Unread renders its total.

