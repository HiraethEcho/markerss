# Change Proposal: scaffold-parse

Pilot skeleton for the Rust branch: Phase 1 (cargo project, three-pane TUI
layout, XDG dirs) + Phase 2 (newsboat `urls` parser wired into nav tree
placeholder). Display only — no fetch, no state DB, no cache, no persistence.

## ADDED Requirements

### TUI skeleton

#### Scenario: Launch shows three panes
Given `cargo run`, then a nav/list/article three-pane layout renders with
static placeholder content.

#### Scenario: Quit and focus switch
When `q` is pressed, app exits. When `tab` is pressed, pane focus cycles
nav → list → article.

### urls parser

#### Scenario: Quoted title
Given line `https://x/feed.xml "My Feed" tech`, then feed parses with url,
title "My Feed", and tag tech.

#### Scenario: Tilde prefix = custom display name
Given title `"~My Name"`, then `custom_name` is true and stored title is
"My Name".

#### Scenario: Multiple tags
Given `URL "T" tech rust web`, then tags are kept in order; first tag
places feed in the tree.

#### Scenario: No title
Given `URL tech`, then title is `None`; display name falls back to URL.

#### Scenario: Comments and blank lines skipped
Given `#` comment lines and blank lines, then no feeds are produced from
them.

### XDG dirs helper

#### Scenario: Resolve config/cache/state homes
Given XDG vars or unset, then helper returns
`$XDG_CONFIG_HOME|$XDG_CACHE_HOME|$XDG_STATE_HOME` joined with `markerss`,
falling back to `~/.config`, `~/.cache`, `~/.local/state` per XDG spec.

### Nav tree placeholder

#### Scenario: Parsed feeds appear in tree
Given parsed feeds, then nav tree shows All Unread node on top, categories
(first tag) with feeds indented, uncategorized feeds at root.

## MODIFIED Requirements

None.
