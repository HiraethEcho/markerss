//! Small pure helpers: slugify, date formatting, display width, YAML escaping.


pub(crate) fn escape_yaml(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

pub(crate) fn slugify(title: &str) -> String {
    let mut out = String::new();
    for c in title.chars() {
        if c.is_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if c.is_whitespace() || c == '-' || c == '_' {
            out.push('-');
        }
    }
    while out.contains("--") {
        out = out.replace("--", "-");
    }
    let out = out.trim_matches('-');
    if out.is_empty() {
        "untitled".to_string()
    } else {
        out.to_string()
    }
}

pub(crate) fn fmt_date(iso: &str) -> String {
    iso.chars().take(10).collect()
}

/// Remove HTML tags from a summary for the plain-text header.
pub(crate) fn strip_html_tags(s: &str) -> String {
    let re = regex::Regex::new(r"<[^>]*>").unwrap();
    re.replace_all(s, "").trim().to_string()
}

/// Approximate terminal display width (CJK wide chars count 2).
pub(crate) fn display_width(s: &str) -> usize {
    s.chars()
        .map(|c| if (c as u32) >= 0x2E80 { 2 } else { 1 })
        .sum()
}

#[cfg(test)]
mod html_tests {
    use super::*;

    #[test]
    fn strip_html_tags_removes_tags() {
        assert_eq!(strip_html_tags("<p>Hello</p> world"), "Hello world");
        assert_eq!(strip_html_tags("<b>bold</b> &amp; <i>it</i>"), "bold &amp; it");
        assert_eq!(strip_html_tags("no tags"), "no tags");
    }
}
