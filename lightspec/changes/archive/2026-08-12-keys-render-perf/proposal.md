# Change Proposal: keys-render-perf

Keybinding system (remappable combos), render pipeline upgrade, performance, cleanup.

## Why

Keys were hardcoded in a giant match (not remappable, combo state hand-rolled). HTML rendering used old converters with residual tags. The TUI repainted on a fixed 100ms tick regardless of input.

## ADDED Requirements

### Keybinding system

#### Scenario: Remappable actions
A `keybindings` map (config.toml `[keybindings]` or standalone `keybindings.toml`) maps action names to one key or a list of keys. Both formats support single keys, combos (`gg`, `st`), and special keys (`<enter>`).

#### Scenario: Combo prefix matching
Multi-key sequences accumulate in a buffer; the longest bound sequence wins; single keys still work when no combo matches.

#### Scenario: Default map
A built-in default map covers every action; user entries replace per-action defaults (others intact).

### Render pipeline

#### Scenario: h2md conversion
HTML → markdown via h2md (html5ever): no residual tags, aligned tables.

#### Scenario: tui-markdown rendering
Markdown → styled Text via tui-markdown (GFM tables, code blocks, StyleSheet mapped to theme colors).

### Performance

#### Scenario: Redraw on change only
The terminal redraws only on input or worker messages, not on an idle tick.

### Cleanup

- Removed: image subsystem (ratatui-image, image deps), proxy feature (config key, param chain, env fallback).
- Removed: keybindings limit to single keys (multi-key + combos added).

## CHANGED Requirements

- Default keys: `F` favourite (nav) / fullscreen (article), `D` delete, `n`/`p` parent navigation, `J`/`K` next/prev unread (list), `gg` jump top, `yp` copy feed url, `st/sn/sf/su` sorts with `sT/sN/sF/sU` reversed. Arrow keys restored.
- `yf` → `yp` for copy feed url.

## REMOVED Requirements

- Link jump (`f` hint mode) — user dropped; `f` freed.
- Kitty image display — unstable, default-off; removed entirely.
- Proxy — removed.
