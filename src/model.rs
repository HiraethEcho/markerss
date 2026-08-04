//! Shared data model.

use serde::{Deserialize, Serialize};

/// One feed item (article).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub guid: String,
    pub title: String,
    pub url: String,
    pub summary: String,
    /// Full content from the feed, if the feed provides it.
    pub content: String,
    /// ISO-8601 published date; empty if unknown.
    pub date: String,
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
