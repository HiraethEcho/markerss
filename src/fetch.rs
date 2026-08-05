//! Network: feed refresh (reqwest + feed-rs) and full-article fetch.
//!
//! All blocking; callers run these in worker threads and receive results
//! over a channel.

use std::time::Duration;

use crate::model::Item;

const USER_AGENT: &str = concat!("markerss/", env!("CARGO_PKG_VERSION"));

pub fn http(timeout_secs: u64) -> reqwest::blocking::Client {
    let mut b = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(timeout_secs.max(1)))
        .user_agent(USER_AGENT);
    if let Ok(proxy) = std::env::var("MARKERSS_PROXY") {
        if let Ok(p) = reqwest::Proxy::all(&proxy) {
            b = b.proxy(p);
        }
    }
    b.build().expect("http client")
}

/// Refresh one feed; returns items sorted newest-first.
pub fn refresh_feed(url: &str, timeout_secs: u64) -> Result<Vec<Item>, String> {
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
            // summary only on refresh — full content fetched on demand
            let content = String::new();
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
    Ok(items)
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
    let cleaned = strip_tag_blocks(html, "script");
    let cleaned = strip_tag_blocks(&cleaned, "style");
    // sub/sup → markdown markers (html2md passes them through raw; the
    // markdown renderer understands ~x~ / ^x^)
    let sub_re = regex::Regex::new(r"(?i)</?sub>").unwrap();
    let cleaned = sub_re.replace_all(&cleaned, "~").to_string();
    let sup_re = regex::Regex::new(r"(?i)</?sup>").unwrap();
    let cleaned = sup_re.replace_all(&cleaned, "^").to_string();
    let mut md = html2md::parse_html(&cleaned);
    // html2md escapes `~` as `\~` — unescape so the renderer sees subscript
    md = md.replace("\\~", "~");
    // Collapse excessive blank lines.
    while md.contains("\n\n\n") {
        md = md.replace("\n\n\n", "\n\n");
    }
    md.trim().to_string()
}

/// Remove `<tag>…</tag>` blocks (case-insensitive).
fn strip_tag_blocks(html: &str, tag: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let lower = html.to_lowercase();
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut rest = html;
    let mut lo = lower.as_str();
    loop {
        match lo.find(&open) {
            Some(i) => {
                out.push_str(&rest[..i]);
                // skip past the opening tag's '>'
                let after_open = &lo[i..];
                let tag_end = after_open.find('>').map(|j| i + j + 1).unwrap_or(rest.len());
                let close_off = lo[tag_end..].find(&close);
                match close_off {
                    Some(j) => {
                        let skip = tag_end + j + close.len();
                        rest = &rest[skip.min(rest.len())..];
                        lo = &lo[skip.min(lo.len())..];
                    }
                    None => break,
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
        let md = html_to_markdown("<p>H<sub>2</sub>O x<sup>2</sup></p>");
        assert!(md.contains("H~2~O"));
        assert!(md.contains("x^2^"));
        assert!(!md.contains("<sub>"));
        assert!(!md.contains("<sup>"));
    }
}
