## ADDED Requirements

### Requirement: Three-Pane Layout

The system SHALL present a three-pane TUI: nav tree (left), item list
(middle), article reader (right). Pane borders SHALL align exactly with the
terminal viewport — total rendered height equals terminal height, top border
always visible.

#### Scenario: Exact layout

- **WHEN** the app renders at any terminal size
- **THEN** the three panes plus footer occupy exactly the terminal rows and
  the top border of every pane is visible

### Requirement: Feed Source (newsboat urls)

The system SHALL read subscriptions live from
`$XDG_CONFIG_HOME/markerss/urls` in newsboat format: `URL "Title" tag1 tag2`.
Tags act as categories (first tag = tree placement); `~` title prefix marks a
custom display name. TUI add/delete/rename SHALL rewrite this file.

#### Scenario: Add feed

- **WHEN** the user presses `a` and enters URL, title, tags
- **THEN** the feed is appended to the urls file and appears in the nav tree

### Requirement: State Persistence

The system SHALL store items in SQLite at
`$XDG_CACHE_HOME/markerss/markerss.db`, keyed (feed_url, guid). Read flags and
fetched content SHALL survive refresh. Unread counts and All Unread
aggregation SHALL be derived from stored state.

#### Scenario: Read flag survives refresh

- **WHEN** an item is marked read and a refresh re-ingests its feed
- **THEN** the item remains read and its fetched content is preserved

### Requirement: Reading Flow

The list SHALL NOT mark items read on cursor movement. Enter opens the
article and marks it read. Full content SHALL be fetched only on an explicit
second Enter in the article pane (auto-fetch off). n/p move between items
marking read; j/k scroll; ctrl+u/ctrl+d (and pgup/pgdown) half-page scroll.
h/q/esc back-nav: article → list → nav → fold category → quit.

#### Scenario: No read change on preview

- **WHEN** the user moves the cursor in the nav or list pane
- **THEN** the article pane live-previews title, meta and summary, and no
  item is marked read

### Requirement: Refresh

The system SHALL fetch feeds on startup (background, non-blocking) and on
`r`. Fetches SHALL run in parallel with bounded concurrency. Cached items
SHALL render before the first fetch completes. Refresh stores summaries only.

#### Scenario: Startup shows cache

- **WHEN** the app starts with previously stored items
- **THEN** the UI renders them immediately and refreshes in the background

### Requirement: Full-Article Fetch

On explicit Enter in the article pane, the system SHALL fetch the article
page, extract main content (readability), and store it. Already-fetched
content SHALL be reused.

#### Scenario: Fetch on demand

- **WHEN** the user presses Enter in the article pane on an item without content
- **THEN** the article is fetched and rendered; a second Enter shows
  "already fetched" status

### Requirement: Rendering

Content HTML SHALL convert to markdown for display. Links render as
underlined alt text (URL hidden); images as `[img]`; sub/sup as `~x~`/`^x^`.
Long lines SHALL truncate without breaking pane borders.

#### Scenario: Link rendering

- **WHEN** article content contains a link
- **THEN** the terminal shows only the underlined link text, never the URL

### Requirement: Export

`e` SHALL write markdown with YAML frontmatter (title, link, date, feed) to
`$XDG_DATA_HOME/markerss/<category>/<slug>.md` (uncategorized → root).
Markdown SHALL be generated only at export time.

#### Scenario: Export writes file

- **WHEN** the user presses `e` on an article titled "Hello World" in category "tech"
- **THEN** a file `tech/hello-world.md` exists with frontmatter and content

### Requirement: OPML Interop

`i` SHALL import an OPML file, merging new feeds into the urls file with
categories from outline nesting. `x` SHALL export subscriptions to
`$XDG_DATA_HOME/markerss/subscriptions.opml`, newsboat-compatible.

#### Scenario: Round trip

- **WHEN** feeds with categories are exported then re-imported
- **THEN** feed URLs, custom titles and first-tag categories are preserved

### Requirement: Keys — hjkl + arrows

All hjkl bindings SHALL have arrow-key equivalents: j/↓, k/↑, h/←, l/→.

#### Scenario: Arrow navigation

- **WHEN** the user presses arrow keys in any pane
- **THEN** behavior matches the corresponding hjkl binding
