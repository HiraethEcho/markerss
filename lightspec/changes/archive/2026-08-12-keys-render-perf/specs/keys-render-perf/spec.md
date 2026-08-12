# keys-render-perf Spec

## ADDED Requirements

### Requirement: Remappable keybindings

A `keybindings` map SHALL map action names to one key string or a list of key strings, configured via `[keybindings]` in config.toml or a standalone `keybindings.toml` (which replaces the config map when present). Key strings SHALL support single keys (`"l"`), combos (`"gg"`), and angle-bracket specials (`"<enter>"`). A built-in default map SHALL cover every action; user entries SHALL replace per-action defaults without affecting others.

#### Scenario: Multi-key binding
Given `keybindings = { open = ["l", "<enter>"] }`, both l and Enter open.

#### Scenario: Combo binding
Given `jump_top = ["gg"]`, pressing g then g jumps to the top.

#### Scenario: Standalone file
Given `~/.config/markerss/keybindings.toml` with a keybindings map, it replaces the config.toml map.

### Requirement: Combo prefix matching

Key presses SHALL accumulate in a buffer (max 2); the longest bound sequence SHALL match; when the buffer is only a prefix of a longer bound sequence the app SHALL wait for the next key; otherwise the last key SHALL be treated as a single-key binding.

#### Scenario: Prefix wait
Given `jump_top = ["gg"]`, pressing g alone does nothing; g then g jumps.

### Requirement: h2md conversion

HTML SHALL convert to markdown via h2md (html5ever) with no residual HTML tags in the output.

#### Scenario: No residual tags
Given HTML with `<div>`/`<span>` nesting, the markdown output contains no `<` tag characters.

### Requirement: tui-markdown rendering

Markdown SHALL render to styled Text via tui-markdown with GFM tables, code blocks, and a StyleSheet mapped from the theme colors.

#### Scenario: Aligned table
Given a markdown table, the rendered text uses box-drawing borders and column alignment.

### Requirement: Redraw on change

The terminal SHALL redraw only when input arrives or a worker message is processed, not on an idle timer.

#### Scenario: Idle CPU
Given no input and no messages, no draw calls occur.

## CHANGED Requirements

- Default keys: F favourite (nav)/fullscreen (article), D delete, n/p parent navigation (article→list, list→nav), J/K next/prev unread (list only), gg jump top, yp copy feed url, sT/sN/sF/sU reversed sorts, arrow keys.

## REMOVED Requirements

- Link jump (f hint mode).
- Kitty image display.
- Proxy.
