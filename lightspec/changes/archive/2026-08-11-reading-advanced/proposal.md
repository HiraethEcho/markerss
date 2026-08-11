# Change Proposal: reading-advanced

Reading comfort (Article Polish) + efficiency keys (Advanced), per SPEC.md
`## Article Polish` and `## Advanced`.

## Why

Article pane was a fixed 6-line header (long summaries truncated) with no
scrollbar and no reading-width cap; no fast navigation, search, sort, or copy
keys. This change adds comfortable long-form reading and vim-style efficiency
keys.

## ADDED Requirements

### Reading polish

#### Scenario: Reading width
Given a wide terminal, then the article body is capped at 80 columns,
left-aligned, centered in the pane.

#### Scenario: Header grows with summary
Given a long summary, then the article header height grows (up to 8 summary
lines) so the whole summary is visible; the body scrolls below.

#### Scenario: Scrollbar
Given content longer than the pane, then a scrollbar shows on the article
pane's right edge; scrolling past the end is clamped.

#### Scenario: Theme file
Given a `theme` path in config, then a TOML file (h1/h2/h3/code/quote/link/
accent/dim named colors) overrides the markdown theme and pane accent/dim
colors. Missing/invalid file → defaults.

### Efficiency keys

#### Scenario: gg / G
In nav, list, or article: `gg` jumps to the top, `G` to the bottom
(article scroll clamps to content length).

#### Scenario: Ctrl+f / Ctrl+b
Full-page move: list selection in list, scroll in article.

#### Scenario: / search
`/` in the list opens a live filter box (title+summary, case-insensitive).
Enter keeps the filter; esc restores the pre-search list.

#### Scenario: sort stack
`st`/`sn`/`sf`/`su` push time/title/feed/unread levels (last pressed =
highest priority, keep 3); uppercase `sT`/`sN`/`sF`/`sU` pushes the
reversed level. Applied to the visible snapshot only; `sort` config seeds
the stack. Keypresses never modify the config file.

#### Scenario: copy keys
`yy` copies the item url, `yn` the item title, `yf` the feed url — via
OSC52 (no dependency). `s` and `y` show their combo help in the status bar.

#### Scenario: foldlevel
`foldlevel = n` in config folds category paths deeper than n at startup
(0 = top-level rows only). No fold keybindings.

## DELETED Requirements

- zr/zm/zR/zM fold keys (user: not needed)

## CHANGED Requirements

- Sort reverse: per-level uppercase prefix (sT…) instead of a global `ss` toggle.
