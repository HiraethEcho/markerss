//! Shared data model.


/// One feed item (article).
#[derive(Debug, Clone)]
pub struct Item {
    pub guid: String,
    pub title: String,
    pub url: String,
    pub summary: String,
    /// Full content from the feed, if the feed provides it.
    pub content: String,
    /// ISO-8601 published date; empty if unknown.
    pub date: String,
    /// Read-later flag (item-level).
    pub read_later: bool,
    /// Saved flag (item-level, exempt from TTL).
    pub saved: bool,
}

impl Item {
    pub fn display_title(&self) -> &str {
        if self.title.is_empty() {
            "untitled"
        } else {
            &self.title
        }
    }
}
