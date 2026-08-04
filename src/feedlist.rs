//! newsboat `urls` format parser + file I/O.
//!
//! Line grammar: `URL "Title" tag1 tag2 ...`
//! - `#` starts a comment line
//! - `~` prefix on quoted title = custom display name (overrides feed title)
//! - tags = categories; first tag places feed in tree

use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Feed {
    pub url: String,
    /// Quoted title, `~` prefix stripped. `None` when no quoted title present.
    pub title: Option<String>,
    /// Title came with `~` prefix → custom display name, do not override from feed.
    pub custom_name: bool,
    pub tags: Vec<String>,
}

impl Feed {
    pub fn display_name(&self) -> &str {
        match &self.title {
            Some(t) => t,
            None => &self.url,
        }
    }

    /// First tag = category; `None` = uncategorized.
    pub fn category(&self) -> Option<&str> {
        self.tags.first().map(String::as_str)
    }

    /// Serialize back to urls-file line form.
    pub fn to_line(&self) -> String {
        let mut line = self.url.clone();
        if let Some(t) = &self.title {
            let prefix = if self.custom_name { "~" } else { "" };
            line.push_str(&format!(" \"{prefix}{t}\""));
        }
        for tag in &self.tags {
            line.push(' ');
            line.push_str(tag);
        }
        line
    }
}

/// Parsed urls file.
#[derive(Debug, Default)]
pub struct File {
    pub feeds: Vec<Feed>,
}

impl File {
    pub fn load(path: &Path) -> io::Result<File> {
        let data = fs::read_to_string(path)?;
        Ok(File { feeds: parse(&data) })
    }

    pub fn load_or_default(path: &Path) -> File {
        File::load(path).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let mut out = String::from("# markerss subscriptions (newsboat urls format)\n");
        for f in &self.feeds {
            out.push_str(&f.to_line());
            out.push('\n');
        }
        fs::write(path, out)
    }

    /// Add or replace a feed with the same URL.
    pub fn upsert(&mut self, feed: Feed) {
        if let Some(existing) = self.feeds.iter_mut().find(|f| f.url == feed.url) {
            *existing = feed;
        } else {
            self.feeds.push(feed);
        }
    }

    pub fn remove(&mut self, url: &str) {
        self.feeds.retain(|f| f.url != url);
    }

    /// Distinct categories in tree order, feeds of `None` = uncategorized.
    pub fn categories(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for f in &self.feeds {
            if let Some(c) = f.category() {
                if !out.iter().any(|x| x == c) {
                    out.push(c.to_string());
                }
            }
        }
        out
    }

    pub fn by_category(&self, cat: &str) -> Vec<&Feed> {
        self.feeds
            .iter()
            .filter(|f| f.category() == Some(cat))
            .collect()
    }

    pub fn uncategorized(&self) -> Vec<&Feed> {
        self.feeds.iter().filter(|f| f.category().is_none()).collect()
    }
}

pub fn parse(input: &str) -> Vec<Feed> {
    input.lines().filter_map(parse_line).collect()
}

/// Parse one non-empty, non-comment line. Malformed lines → `None`.
pub fn parse_line(line: &str) -> Option<Feed> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let url = line.split_whitespace().next()?;
    let mut rest = line[url.len()..].trim();

    let mut title = None;
    let mut custom_name = false;
    if let Some(r) = rest.strip_prefix('"') {
        let end = r.find('"')?;
        let t = &r[..end];
        if let Some(stripped) = t.strip_prefix('~') {
            custom_name = true;
            title = Some(stripped.to_string());
        } else {
            title = Some(t.to_string());
        }
        rest = r[end + 1..].trim();
    }

    let tags = rest.split_whitespace().map(str::to_string).collect();
    Some(Feed {
        url: url.to_string(),
        title,
        custom_name,
        tags,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoted_title() {
        let f = parse_line(r#"https://x.com/feed.xml "My Feed" tech"#).unwrap();
        assert_eq!(f.url, "https://x.com/feed.xml");
        assert_eq!(f.title.as_deref(), Some("My Feed"));
        assert!(!f.custom_name);
    }

    #[test]
    fn tilde_prefix_custom_name() {
        let f = parse_line(r#"https://x.com/feed.xml "~My Name""#).unwrap();
        assert_eq!(f.title.as_deref(), Some("My Name"));
        assert!(f.custom_name);
    }

    #[test]
    fn multiple_tags() {
        let f = parse_line(r#"https://x.com/feed.xml "T" tech rust web"#).unwrap();
        assert_eq!(f.tags, vec!["tech", "rust", "web"]);
    }

    #[test]
    fn no_title() {
        let f = parse_line("https://x.com/feed.xml tech").unwrap();
        assert_eq!(f.title, None);
        assert!(!f.custom_name);
        assert_eq!(f.tags, vec!["tech"]);
    }

    #[test]
    fn comment_lines_skipped() {
        let feeds = parse("# a comment\nhttps://x.com/feed.xml \"T\"\n# another");
        assert_eq!(feeds.len(), 1);
    }

    #[test]
    fn blank_lines_skipped() {
        let feeds = parse("\n   \nhttps://x.com/feed.xml \"T\"\n\n");
        assert_eq!(feeds.len(), 1);
    }

    #[test]
    fn roundtrip_line() {
        let f = parse_line(r#"https://x.com/feed.xml "~My Name" tech rust"#).unwrap();
        assert_eq!(f.to_line(), r#"https://x.com/feed.xml "~My Name" tech rust"#);
        let f2 = parse_line(r#"https://x.com/plain.xml"#).unwrap();
        assert_eq!(f2.to_line(), "https://x.com/plain.xml");
    }

    #[test]
    fn file_upsert_remove() {
        let mut file = File::default();
        file.upsert(parse_line(r#"https://a.com/f "A" tech"#).unwrap());
        file.upsert(parse_line(r#"https://b.com/f"#).unwrap());
        assert_eq!(file.feeds.len(), 2);
        file.upsert(parse_line(r#"https://a.com/f "A2" tech"#).unwrap());
        assert_eq!(file.feeds.len(), 2);
        assert_eq!(file.feeds[0].title.as_deref(), Some("A2"));
        file.remove("https://a.com/f");
        assert_eq!(file.feeds.len(), 1);
    }

    #[test]
    fn categories_and_grouping() {
        let mut file = File::default();
        file.upsert(parse_line(r#"https://a.com/f "A" tech"#).unwrap());
        file.upsert(parse_line(r#"https://b.com/f "B" tech"#).unwrap());
        file.upsert(parse_line(r#"https://c.com/f "C" blog"#).unwrap());
        file.upsert(parse_line(r#"https://d.com/f"#).unwrap());
        assert_eq!(file.categories(), vec!["tech", "blog"]);
        assert_eq!(file.by_category("tech").len(), 2);
        assert_eq!(file.uncategorized().len(), 1);
    }
}
