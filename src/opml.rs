//! OPML import/export.
//!
//! Export: feeds → OPML 2.0, one `<outline>` per feed, category nested.
//! Import: parse `<outline>` entries with `xmlUrl`; title/type/text kept.

use std::io;

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::feedlist::{Feed, File};

/// Serialize feeds to OPML 2.0 XML.
pub fn export_opml(file: &File) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<opml version=\"2.0\">\n<head>\n<title>markerss subscriptions</title>\n</head>\n<body>\n");
    // Categories with feeds nested, then uncategorized at top level.
    out.push_str(&category_outlines(file, &[]));
    for f in file.uncategorized() {
        out.push_str(&feed_outline(f));
    }
    out.push_str("</body>\n</opml>\n");
    out
}

/// Recursively emit nested `<outline>` groups for a category subtree.
fn category_outlines(file: &File, path: &[String]) -> String {
    let mut out = String::new();
    for child in file.child_categories(path) {
        let mut child_path = path.to_vec();
        child_path.push(child.clone());
        out.push_str(&format!("<outline text=\"{child}\" title=\"{child}\">\n"));
        out.push_str(&category_outlines(file, &child_path));
        for f in file.by_category_path(&child_path) {
            out.push_str(&feed_outline(f));
        }
        out.push_str("</outline>\n");
    }
    out
}

fn feed_outline(f: &Feed) -> String {
    let title = f
        .title
        .clone()
        .unwrap_or_else(|| f.url.clone())
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('"', "&quot;");
    format!(
        "<outline type=\"rss\" text=\"{title}\" title=\"{title}\" xmlUrl=\"{}\"/>\n",
        f.url.replace('&', "&amp;")
    )
}

/// Parse OPML XML into feeds. Category groups become the feed's first tag.
pub fn import_opml(xml: &str) -> io::Result<Vec<Feed>> {
    let mut reader = Reader::from_str(xml);
    let mut feeds = Vec::new();
    let mut stack: Vec<String> = Vec::new(); // open category names
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "outline" {
                    let (url, title) = outline_attrs(&e);
                    if let Some(url) = url {
                        feeds.push(make_feed(url, title, &stack.join("/")));
                    } else if let Some(cat) = title {
                        stack.push(cat);
                    }
                }
            }
            Ok(Event::Empty(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "outline" {
                    let (url, title) = outline_attrs(&e);
                    if let Some(url) = url {
                        feeds.push(make_feed(url, title, &stack.join("/")));
                    }
                }
            }
            Ok(Event::End(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "outline" && !stack.is_empty() {
                    stack.pop();
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(feeds)
}

fn outline_attrs(e: &quick_xml::events::BytesStart) -> (Option<String>, Option<String>) {
    let mut url = None;
    let mut title = None;
    let mut text = None;
    for a in e.attributes().flatten() {
        let key = String::from_utf8_lossy(a.key.as_ref()).to_string();
        let val = unescape_entities(&String::from_utf8_lossy(&a.value));
        match key.as_str() {
            "xmlUrl" => url = Some(val),
            "title" => title = Some(val),
            "text" => text = Some(val),
            _ => {}
        }
    }
    (url, title.or(text))
}

fn make_feed(url: String, title: Option<String>, category: &str) -> Feed {
    let mut custom_name = false;
    let mut title = title;
    if let Some(t) = &title {
        if let Some(stripped) = t.strip_prefix('~') {
            custom_name = true;
            title = Some(stripped.to_string());
        }
    }
    Feed {
        url,
        title,
        custom_name,
        tags: if category.is_empty() {
            Vec::new()
        } else {
            vec![category.to_string()]
        },
        feed_tags: Vec::new(),
        favourite: false,
    }
}

/// Decode the common XML entities (quick-xml's unescape needs a Decoder).
fn unescape_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_file() -> File {
        let mut f = File::default();
        f.upsert(Feed { url: "https://a.com/f".into(), title: Some("A".into()), custom_name: false, tags: vec!["tech".into()], feed_tags: vec![], favourite: false });
        f.upsert(Feed { url: "https://b.com/f".into(), title: None, custom_name: false, tags: vec![], feed_tags: vec![], favourite: false });
        f
    }

    #[test]
    fn export_has_categories_and_urls() {
        let xml = export_opml(&sample_file());
        assert!(xml.contains("<opml version=\"2.0\">"));
        assert!(xml.contains("<outline text=\"tech\" title=\"tech\">"));
        assert!(xml.contains("xmlUrl=\"https://a.com/f\""));
        assert!(xml.contains("xmlUrl=\"https://b.com/f\""));
    }

    #[test]
    fn import_roundtrip() {
        let xml = export_opml(&sample_file());
        let feeds = import_opml(&xml).unwrap();
        assert_eq!(feeds.len(), 2);
        let a = feeds.iter().find(|f| f.url == "https://a.com/f").unwrap();
        assert_eq!(a.title.as_deref(), Some("A"));
        assert_eq!(a.tags, vec!["tech"]);
        let b = feeds.iter().find(|f| f.url == "https://b.com/f").unwrap();
        assert!(b.tags.is_empty());
    }

    #[test]
    fn import_unescapes_entities() {
        let xml = r#"<?xml version="1.0"?><opml version="2.0"><body><outline type="rss" title="A &amp; B" xmlUrl="https://x.com/f?q=1&amp;r=2"/></body></opml>"#;
        let feeds = import_opml(xml).unwrap();
        assert_eq!(feeds[0].title.as_deref(), Some("A & B"));
        assert_eq!(feeds[0].url, "https://x.com/f?q=1&r=2");
    }

    #[test]
    fn nested_import_export_roundtrip() {
        let mut f = File::default();
        f.upsert(Feed { url: "https://a.com/f".into(), title: Some("A".into()), custom_name: false, tags: vec!["tech/rust".into()], feed_tags: vec![], favourite: false });
        f.upsert(Feed { url: "https://b.com/f".into(), title: Some("B".into()), custom_name: false, tags: vec!["tech/go".into()], feed_tags: vec![], favourite: false });
        f.upsert(Feed { url: "https://c.com/f".into(), title: None, custom_name: false, tags: vec![], feed_tags: vec![], favourite: false });
        let xml = export_opml(&f);
        // nested outline groups
        assert!(xml.contains("<outline text=\"tech\" title=\"tech\">\n<outline text=\"rust\" title=\"rust\">\n"));
        assert!(xml.contains("<outline text=\"go\" title=\"go\">\n"));
        // uncategorized stays at top level
        assert!(xml.contains("xmlUrl=\"https://c.com/f\""));
        // roundtrip: nested path preserved as single category
        let feeds = import_opml(&xml).unwrap();
        assert_eq!(feeds.len(), 3);
        let a = feeds.iter().find(|x| x.url == "https://a.com/f").unwrap();
        assert_eq!(a.tags, vec!["tech/rust"]);
        let b = feeds.iter().find(|x| x.url == "https://b.com/f").unwrap();
        assert_eq!(b.tags, vec!["tech/go"]);
    }

    #[test]
    fn nested_import_flat_outlines() {
        // old-style flat OPML with slash categories stays put
        let xml = r#"<?xml version="1.0"?><opml version="2.0"><body><outline text="tech" title="tech"><outline type="rss" title="A" xmlUrl="https://a.com/f"/></outline></body></opml>"#;
        let feeds = import_opml(xml).unwrap();
        assert_eq!(feeds[0].tags, vec!["tech"]);
    }
}
