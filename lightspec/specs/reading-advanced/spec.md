# reading-advanced Specification

## Purpose
TBD - created by archiving change reading-advanced. Update Purpose after archive.
## Requirements
### Requirement: Reading width

The article body SHALL render at a maximum width of 80 columns, left-aligned, centered horizontally in the pane, when the pane is wider.

#### Scenario: Wide terminal
Given a terminal wider than 84 columns, then the article body occupies the middle 80 columns.

### Requirement: Header grows with summary

The article header SHALL size dynamically so the full summary is visible (up to 8 summary lines); the body scrolls below a separator.

#### Scenario: Long summary
Given an item with a 20-line summary, then the header shows the first 8 wrapped lines and the body area shrinks accordingly.

### Requirement: Scrollbar

The article pane SHALL show a scrollbar on its right edge when content exceeds the pane; scrolling SHALL clamp to the content length.

#### Scenario: Content taller than pane
Given content taller than the body area, then a scrollbar bar appears and `G` stops at the last line.

### Requirement: Theme file

A `theme` config path SHALL load a TOML file of named colors (`h1`, `h2`, `h3`, `code`, `quote`, `link`, `accent`, `dim`) overriding the markdown theme and pane accent/dim colors. Missing or invalid files SHALL fall back to defaults.

#### Scenario: Theme overrides
Given `theme = "~/x/theme.toml"` with `accent = "green"`, then the article title and nav highlights render green.

### Requirement: Jump keys

`gg` SHALL jump to the top and `G` to the bottom in nav, list, and article panes (article clamped).

#### Scenario: gg in list
Given focus in the list, pressing `gg` selects the first item.

### Requirement: Page scroll

Ctrl+f / Ctrl+b SHALL move a full page in the list (selection) and article (scroll).

#### Scenario: Ctrl+f in article
Given focus in the article, Ctrl+f scrolls down one full page.

### Requirement: Modal search

`/` in the list SHALL open a live filter over title+summary (case-insensitive). Enter keeps the filter; esc restores the pre-search list.

#### Scenario: Search filter
Given a query "rust", the list shows only items whose title or summary contains "rust".

### Requirement: Sort stack

`st`/`sn`/`sf`/`su` SHALL push time/title/feed/unread sort levels (last pressed = highest priority; keep 3); uppercase variants push the reversed level. Sorting SHALL apply to the visible snapshot only and SHALL NOT modify the config file; the `sort` config array seeds the stack.

#### Scenario: Push order
Given `st` then `sf`, the list sorts by feed, then time.

#### Scenario: Reversed level
Given `sT`, time sorts ascending (oldest first).

### Requirement: Copy keys

`yy` SHALL copy the item url, `yn` the item title, `yf` the feed url to the clipboard via OSC52.

#### Scenario: Copy url
Given focus in the list, pressing `yy` emits an OSC52 clipboard sequence with the item url.

### Requirement: Combo help

Pressing `s` or `y` SHALL show the available combo keys in the status bar.

#### Scenario: s help
Given focus in the list, pressing `s` shows "sort: st/sn/sf/su …" in the status bar.

### Requirement: foldlevel config

`foldlevel = n` SHALL fold category paths deeper than n at startup (0 = top-level rows only). No fold keybindings exist.

#### Scenario: foldlevel 0
Given `foldlevel = 0`, then only top-level category rows are visible at startup.

