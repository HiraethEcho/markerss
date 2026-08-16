## ADDED Requirements

### Requirement: Nav Pane Structure

The nav pane SHALL render preset-driven sections. Top entries: Unread,
Read Later, Favourite, Categories, Tags, Saved (per config `nav_presets`;
Feeds and No Category available). Unread / Read Later / Saved are leaf
nodes; Favourite, Categories, Tags, Feeds, No Category are foldable
containers. Categories SHALL support nested categories (cat/subcat).

#### Scenario: Default nav render

- **WHEN** the app starts with the default preset
- **THEN** the nav pane shows Unread, Read Later, Favourite, Categories,
  Tags, Saved in order, top entries highlighted

### Requirement: Category and Tag Rows

Each feed SHALL have exactly one category (tree placement, first non-`#`
token in urls) and 0..n `#tags`. The Categories section SHALL nest
categories (cat/subcat → indented tree); the Tags section SHALL list tags,
each foldable to the feeds carrying it; feeds without a category appear at
the end of Categories under a `No Category` row.

#### Scenario: Nested category

- **WHEN** feeds carry categories `tech` and `tech/go`
- **THEN** `go` renders indented under `tech`

#### Scenario: Tag filter

- **WHEN** a tag is selected
- **THEN** the list pane shows items from feeds carrying that tag
