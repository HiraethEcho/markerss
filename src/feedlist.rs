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
    /// Categories (`#`-less words); first = tree placement.
    pub tags: Vec<String>,
    /// Feed tags (`#`-prefixed words).
    pub feed_tags: Vec<String>,
    /// Favourite flag (`!favourite` marker).
    pub favourite: bool,
}

impl Feed {
    pub fn display_name(&self) -> &str {
        match &self.title {
            Some(t) => t,
            None => &self.url,
        }
    }

    /// First tag = category (may be a slash path `cat/sub`); `None` = uncategorized.
    pub fn category(&self) -> Option<&str> {
        self.tags.first().map(String::as_str)
    }

    /// Category path segments (`cat/sub` → `["cat", "sub"]`); empty = uncategorized.
    pub fn category_segments(&self) -> Vec<&str> {
        self.category()
            .map(|c| c.split('/').filter(|s| !s.is_empty()).collect())
            .unwrap_or_default()
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
        for tag in &self.feed_tags {
            line.push(' ');
            line.push('#');
            line.push_str(tag);
        }
        if self.favourite {
            line.push_str(" !favourite");
        }
        line
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.feed_tags.iter().any(|t| t == tag)
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

    /// All category tree nodes as paths — every distinct prefix of every
    /// feed's category path, in first-seen order (parents before children).
    pub fn categories_tree(&self) -> Vec<Vec<String>> {
        let mut out: Vec<Vec<String>> = Vec::new();
        for f in &self.feeds {
            let segs: Vec<String> =
                f.category_segments().iter().map(|s| s.to_string()).collect();
            if segs.is_empty() {
                continue;
            }
            for i in 1..=segs.len() {
                if !out.contains(&segs[..i].to_vec()) {
                    out.push(segs[..i].to_vec());
                }
            }
        }
        out
    }

    /// Direct child category names of `path` (next segment only, deduped).
    pub fn child_categories(&self, path: &[String]) -> Vec<String> {
        let prefix = if path.is_empty() {
            String::new()
        } else {
            format!("{}/", path.join("/"))
        };
        let mut out: Vec<String> = Vec::new();
        for f in &self.feeds {
            if let Some(c) = f.category() {
                if let Some(rest) = c.strip_prefix(&prefix) {
                    if let Some(next) = rest.split('/').next() {
                        if !out.iter().any(|x| x == next) {
                            out.push(next.to_string());
                        }
                    }
                }
            }
        }
        out
    }

    /// Feeds whose category is exactly `path` (joined) — direct children only.
    pub fn by_category_path(&self, path: &[String]) -> Vec<&Feed> {
        let joined = path.join("/");
        self.feeds
            .iter()
            .filter(|f| f.category() == Some(joined.as_str()))
            .collect()
    }

    /// Rename a category path `old` → `new`; renames the whole subtree
    /// (feeds under `old/sub` become `new/sub`).
    pub fn rename_category(&mut self, old: &str, new: &str) {
        for f in self.feeds.iter_mut() {
            if let Some(c) = f.category() {
                if c == old {
                    f.tags[0] = new.to_string();
                } else if let Some(rest) = c.strip_prefix(&format!("{old}/")) {
                    f.tags[0] = format!("{new}/{rest}");
                }
            }
        }
    }

    pub fn uncategorized(&self) -> Vec<&Feed> {
        self.feeds.iter().filter(|f| f.category().is_none()).collect()
    }

    /// All distinct feed tags, in tree order.
    pub fn all_feed_tags(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for f in &self.feeds {
            for t in &f.feed_tags {
                if !out.contains(t) {
                    out.push(t.clone());
                }
            }
        }
        out
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

    let words: Vec<String> = rest.split_whitespace().map(str::to_string).collect();
    let mut categories = Vec::new();
    let mut feed_tags = Vec::new();
    let mut favourite = false;
    for w in words {
        if let Some(t) = w.strip_prefix('#') {
            if !t.is_empty() {
                feed_tags.push(t.to_string());
            }
        } else if w == "!favourite" {
            favourite = true;
        } else {
            categories.push(w);
        }
    }
    Some(Feed {
        url: url.to_string(),
        title,
        custom_name,
        tags: categories,
        feed_tags,
        favourite,
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
    fn feed_tags_and_favourite() {
        let f = parse_line(r#"https://x.com/feed.xml "T" blog #tech #rust !favourite"#).unwrap();
        assert_eq!(f.tags, vec!["blog"]);
        assert_eq!(f.feed_tags, vec!["tech", "rust"]);
        assert!(f.favourite);
        assert!(f.has_tag("tech"));
        assert!(!f.has_tag("blog"));
    }

    #[test]
    fn roundtrip_tags_favourite() {
        let f = parse_line(r#"https://x.com/f "~N" cat #t1 !favourite"#).unwrap();
        assert_eq!(f.to_line(), r#"https://x.com/f "~N" cat #t1 !favourite"#);
    }

    #[test]
    fn no_tags_ok() {
        let f = parse_line("https://x.com/plain.xml").unwrap();
        assert!(f.feed_tags.is_empty());
        assert!(!f.favourite);
        assert!(f.tags.is_empty());
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

    #[test]
    fn nested_categories_tree() {
        let mut file = File::default();
        file.upsert(parse_line(r#"https://a.com/f "A" tech/rust/lang"#).unwrap());
        file.upsert(parse_line(r#"https://b.com/f "B" tech/rust"#).unwrap());
        file.upsert(parse_line(r#"https://c.com/f "C" tech/go"#).unwrap());
        file.upsert(parse_line(r#"https://d.com/f "D" blog"#).unwrap());
        file.upsert(parse_line(r#"https://e.com/f"#).unwrap());
        // tree: parents before children, deduped
        assert_eq!(
            file.categories_tree(),
            vec![
                vec!["tech".to_string()],
                vec!["tech".to_string(), "rust".to_string()],
                vec!["tech".to_string(), "rust".to_string(), "lang".to_string()],
                vec!["tech".to_string(), "go".to_string()],
                vec!["blog".to_string()],
            ]
        );
        // child categories of "tech"
        assert_eq!(
            file.child_categories(&["tech".to_string()]),
            vec!["rust".to_string(), "go".to_string()]
        );
        // exact (direct) feeds only
        assert_eq!(file.by_category_path(&["tech".to_string()]).len(), 0);
        assert_eq!(file.by_category_path(&["tech".to_string(), "rust".to_string()]).len(), 1);
        assert_eq!(file.by_category("tech/rust").len(), 1);
    }

    #[test]
    fn rename_category_renames_subtree() {
        let mut file = File::default();
        file.upsert(parse_line(r#"https://a.com/f "A" tech/rust"#).unwrap());
        file.upsert(parse_line(r#"https://b.com/f "B" tech/rust/lang"#).unwrap());
        file.upsert(parse_line(r#"https://c.com/f "C" tech/go"#).unwrap());
        file.upsert(parse_line(r#"https://d.com/f "D" blog"#).unwrap());
        file.rename_category("tech", "dev");
        assert_eq!(file.by_category("dev/rust").len(), 1);
        assert_eq!(file.by_category("dev/rust/lang").len(), 1);
        assert_eq!(file.by_category("dev/go").len(), 1);
        assert_eq!(file.by_category("tech").len(), 0);
        assert_eq!(file.by_category("blog").len(), 1);
        // rename a mid-level node only moves its subtree
        file.rename_category("dev/rust", "systems");
        assert_eq!(file.by_category("systems").len(), 1);
        assert_eq!(file.by_category("systems/lang").len(), 1);
        assert_eq!(file.by_category("dev/go").len(), 1);
        assert_eq!(
            file.categories(),
            vec!["systems", "systems/lang", "dev/go", "blog"]
        );
    }
}
