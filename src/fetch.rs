//! Network: feed refresh (reqwest + feed-rs) and full-article fetch.
//!
//! All blocking; callers run these in worker threads and receive results
//! over a channel.

use std::time::Duration;

use crate::model::Item;

const USER_AGENT: &str = concat!("markerss/", env!("CARGO_PKG_VERSION"));

pub fn http(timeout_secs: u64) -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(timeout_secs.max(1)))
        .user_agent(USER_AGENT)
        .build()
        .expect("http client")
}

/// Refresh one feed; returns items sorted newest-first.
pub fn refresh_feed(url: &str, timeout_secs: u64) -> Result<(Option<String>, Vec<Item>), String> {
    let resp = http(timeout_secs)
        .get(url)
        .send()
        .map_err(|e| format!("GET {url}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("GET {url}: HTTP {}", resp.status()));
    }
    let body = resp
        .bytes()
        .map_err(|e| format!("read {url}: {e}"))?;
    let feed = feed_rs::parser::parse(&body[..]).map_err(|e| format!("parse {url}: {e}"))?;
    let feed_title = feed.title.map(|t| t.content).filter(|t| !t.is_empty());

    let mut items: Vec<Item> = feed
        .entries
        .into_iter()
        .map(|e| {
            let guid = {
                let id = e.id.clone();
                if !id.is_empty() {
                    id
                } else {
                    e.links.first().map(|l| l.href.clone()).unwrap_or_default()
                }
            };
            let title = e.title.map(|t| t.content).unwrap_or_default();
            let url = e.links.first().map(|l| l.href.clone()).unwrap_or_default();
            let summary = e.summary.map(|s| s.content).unwrap_or_default();
            // keep the feed-provided content (arrives with the feed, no extra
            // request); full-article fetch may later replace it
            let content = e
                .content
                .and_then(|c| c.body)
                .unwrap_or_default();
            let date = e
                .published
                .or(e.updated)
                .map(|d| d.to_rfc3339())
                .unwrap_or_default();
            Item {
                guid,
                title,
                url,
                summary,
                content,
                date,
                read_later: false,
                saved: false,
            }
        })
        .collect();
    // Newest first; items without date sink to the end.
    items.sort_by(|a, b| b.date.cmp(&a.date));
    Ok((feed_title, items))
}

/// Fetch full article, extract main content (Mozilla Readability via
/// dom_smoothie). Returns article HTML; falls back to the whole page.
pub fn fetch_article(url: &str, timeout_secs: u64) -> Result<String, String> {
    let html = fetch_html(url, timeout_secs)?;
    Ok(extract_main(url, &html))
}

fn fetch_html(url: &str, timeout_secs: u64) -> Result<String, String> {
    let resp = http(timeout_secs)
        .get(url)
        .send()
        .map_err(|e| format!("GET {url}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("GET {url}: HTTP {}", resp.status()));
    }
    resp.text().map_err(|e| format!("read {url}: {e}"))
}

/// Extract main article content; fallback: whole page.
pub fn extract_main(url: &str, html: &str) -> String {
    let owned = html.to_string();
    match dom_smoothie::Readability::new(owned, Some(url), None) {
        Ok(mut r) => match r.parse() {
            Ok(article) => article.content.to_string(),
            Err(_) => html.to_string(),
        },
        Err(_) => html.to_string(),
    }
}

/// HTML → readable markdown text (best effort), for export.
pub fn html_to_markdown(html: &str) -> String {
    // strip script/style (h2md may otherwise inline their text)
    let cleaned = strip_tag_blocks(html, "script");
    let cleaned = strip_tag_blocks(&cleaned, "style");
    let mut out = Vec::new();
    if h2md::convert(cleaned.as_bytes(), &mut out).is_err() {
        return String::new();
    }
    String::from_utf8(out).unwrap_or_default().trim().to_string()
}

/// Remove `<tag>…</tag>` blocks (case-insensitive).
fn strip_tag_blocks(html: &str, tag: &str) -> String {
    // byte-safe case-insensitive scan: tag names are ASCII, so lowering
    // A-Z never changes byte length (no to_lowercase offset divergence)
    let mut out = String::with_capacity(html.len());
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut rest = html;
    loop {
        match find_ci(rest, &open) {
            Some(i) => {
                out.push_str(&rest[..i]);
                let after = &rest[i..];
                let tag_end = after.find('>').map(|j| i + j + 1).unwrap_or(rest.len());
                let rest_after_tag = &rest[tag_end..];
                match find_ci(rest_after_tag, &close) {
                    Some(j) => {
                        let skip = tag_end + j + close.len();
                        rest = &rest[skip.min(rest.len())..];
                    }
                    None => {
                        // unterminated block — keep everything before the opener
                        return out;
                    }
                }
            }
            None => {
                out.push_str(rest);
                break;
            }
        }
    }
    out
}

/// Case-insensitive byte search (ASCII only — length-preserving).
fn find_ci(haystack: &str, needle: &str) -> Option<usize> {
    let h = haystack.as_bytes();
    let n = needle.as_bytes();
    if n.is_empty() || n.len() > h.len() {
        return None;
    }
    'outer: for i in 0..=h.len() - n.len() {
        for j in 0..n.len() {
            if !h[i + j].eq_ignore_ascii_case(&n[j]) {
                continue 'outer;
            }
        }
        return Some(i);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_to_md_basic() {
        let md = html_to_markdown("<html><body><h1>Title</h1><p>Hello <b>world</b>.</p><script>evil()</script></body></html>");
        assert!(md.contains("Title"));
        assert!(md.contains("Hello"));
        assert!(md.contains("world"));
        assert!(!md.contains("evil"));
        assert!(!md.contains("<script>"));
    }

    #[test]
    #[test]
    fn html_to_md_entities() {
        let md = html_to_markdown("<p>a &amp; b &lt; c</p>");
        assert!(md.contains("a & b"));
        assert!(md.contains("c"));
    }

    #[test]
    fn html_to_md_sub_sup() {
        // h2md strips sub/sup tags (renders as plain text — no markers)
        let md = html_to_markdown("<p>H<sub>2</sub>O x<sup>2</sup></p>");
        assert!(md.contains("H2O"), "got: {md}");
        assert!(md.contains("x2"), "got: {md}");
        assert!(!md.contains("<sub>"), "got: {md}");
        assert!(!md.contains("<sup>"), "got: {md}");
    }

    #[test]
    fn strip_blocks_cjk_content() {
        // regression: to_lowercase byte offsets used to panic on non-ASCII
        let html = "<p>中文内容<script>alert(1)</script>测试</p><style>body{}</style>";
        let md = html_to_markdown(html);
        assert!(!md.contains("alert"), "got: {md}");
        assert!(md.contains("中文内容"), "got: {md}");
    }

    #[test]
    fn html_to_md_no_residual_tags() {
        // the original pain point: no raw HTML tags survive conversion
        let html = r#"<div class="post"><h2>Head</h2><p>Text <span>span</span> <em>em</em></p><pre><code class="rust">fn main(){}</code></pre><ul><li>one</li><li>two</li></ul><blockquote>quote</blockquote><img src="https://x.com/i.png" alt="pic"></div>"#;
        let md = html_to_markdown(html);
        assert!(!md.contains('<'), "residual tag in: {md}");
        assert!(md.contains("## Head"), "got: {md}");
        assert!(md.contains("*em*") || md.contains("em"), "got: {md}");
        assert!(md.contains("fn main"), "got: {md}");
        assert!(md.contains("![pic](https://x.com/i.png)"), "got: {md}");
    }
}
