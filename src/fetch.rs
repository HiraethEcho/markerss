//! Network: feed refresh (reqwest + feed-rs) and full-article fetch.
//! 网络层：订阅源刷新（reqwest + feed-rs）与全文抓取。
//!
//! All blocking; callers run these in worker threads and receive results
//! over a channel.
//! 全部为阻塞式；调用方在 worker 线程中运行，通过 channel 收取结果。
//!
//! Rust concepts introduced here:
//! 本文件涉及的 Rust 概念：
//! - `Result<T, E>` and the `?` operator: fallible functions return Ok/Err;
//!   `?` means "if Err, return it from this fn now" — a type-checked early
//!   return. C analogue: checking every return value by hand; here one
//!   character does it.
//!   Result<T, E> 与 ? 运算符：可能失败的函数返回 Ok/Err；
//!   ? 表示“若是 Err 就立即从本函数返回该错误”。类型检查过的提前返回；
//!   C 里要逐个检查返回值，这里一个字符搞定。
//! - `.map_err(|e| ...)`: convert an error into our own error type (String)
//!   before `?` propagates it — a closure transforms the value inside Result.
//!   .map_err(|e| ...)：在 ? 传播前把错误转成本模块的错误类型 String。
//! - iterators as data pipeline: `.into_iter().map(|e| ...).collect()`
//!   converts feed entries into Items without a manual loop.
//!   迭代器即数据管道：into_iter().map(...).collect() 把条目流变成 Item 列表。
//! - `Option` combinator chain: `.map().and_then().or().filter()
//!   .unwrap_or_default()` — NULL-style plumbing, but enforced by types.
//!   Option 组合子链：map/and_then/or/filter/unwrap_or_default ——
//!   由类型系统强制的安全空值处理。
//! - `'outer:` loop labels: continue/break an OUTER loop directly —
//!   plain C has no syntax for this (you'd use goto or a flag).
//!   'outer: 循环标签：直接 continue 外层循环；C 无对应语法（得用 goto 或标志位）。

use std::time::Duration;

use crate::model::Item;

// const with &'static str: data baked into the binary at compile time.
// concat! is a compile-time macro; env!("CARGO_PKG_VERSION") reads the
// version from Cargo.toml AT COMPILE TIME — zero runtime cost.
// const + &'static str：编译期写死进二进制的数据。
// concat! 是编译期宏；env!("CARGO_PKG_VERSION") 在编译期读 Cargo.toml 的版本号，零运行时开销。
const USER_AGENT: &str = concat!("markerss/", env!("CARGO_PKG_VERSION"));

/// Build an HTTP client with the given timeout (seconds).
/// 构建带超时（秒）的 HTTP 客户端。
pub fn http(timeout_secs: u64) -> reqwest::blocking::Client {
    // Builder pattern: each method consumes self and returns a modified
    // builder (method chaining). .max(1) clamps so timeout >= 1s.
    // Builder 模式：每个方法消耗 self 并返回改好的 builder（链式调用）。
    // .max(1) 保证超时至少 1 秒。
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(timeout_secs.max(1)))
        .user_agent(USER_AGENT)
        .build()
        // build() returns Result — config errors are programmer bugs, so we
        // panic with expect() instead of propagating (like assert in C).
        // build() 返回 Result —— 配置错误属于程序 bug，用 expect() 直接 panic，不传播。
        .expect("http client")
}

/// Refresh one feed; returns items sorted newest-first.
/// 刷新一个订阅源；返回按最新在前排序的条目列表。
///
/// Errors are flattened to `String` for easy transport across the worker
/// thread's channel back to the UI.
/// 错误统一压平成 String，便于经 channel 从 worker 线程传回 UI。
pub fn refresh_feed(url: &str, timeout_secs: u64) -> Result<(Option<String>, Vec<Item>), String> {
    // Chain of fallible steps, each ending in `?`:
    // send() fails → map_err formats it as String → ? returns it immediately.
    // Equivalent to nested if-error checks in C, one line per step.
    // 一串可能失败的步骤，每步以 ? 结尾：失败 → map_err 格式化 → 立即返回。
    // 相当于 C 里层层 if 检查错误，每步一行。
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
    // &body[..] slices the byte buffer to &[u8] — full view, no copy.
    // &body[..] 把字节缓冲切成 &[u8] 视图，不拷贝。
    let feed = feed_rs::parser::parse(&body[..]).map_err(|e| format!("parse {url}: {e}"))?;
    // Option plumbing: take feed.title's inner content, then drop empty ones.
    // .map() runs only when Some; .filter() turns Some("") into None.
    // Option 组合子：有 title 才取内容；filter 把空串变回 None。
    let feed_title = feed.title.map(|t| t.content).filter(|t| !t.is_empty());

    // Iterator pipeline instead of a for-loop with pushes:
    // into_iter() MOVES entries out of the feed (we own them now),
    // map() converts each entry into an Item,
    // collect() gathers results into Vec<Item> — target type inferred from
    // the declared binding type below.
    // 迭代器管线代替手写循环：into_iter() 把 entries 从 feed 中移动出来，
    // map() 逐个转换成 Item，collect() 收集成 Vec<Item>（类型由声明推断）。
    let mut items: Vec<Item> = feed
        .entries
        .into_iter()
        .map(|e| {
            // Inner block just to scope `id` cleanly; guid falls back:
            // entry id first, else first link href, else empty string.
            // 内层块限定 id 作用域；guid 回退链：entry id → 首个链接 → 空串。
            let guid = {
                // Clone the id out of `e` — we need an owned String because
                // e will be consumed by this closure.
                // 克隆 id 出来 —— e 会被本闭包消费，需要自有 String。
                let id = e.id.clone();
                if !id.is_empty() {
                    id // move the already-owned String straight out
                       // 已是自有 String，直接移动出去
                } else {
                    // first() on the links gives Option<&Link>; map clones
                    // the href; unwrap_or_default() = empty String if absent
                    // (String's Default is "").
                    // first() 给 Option<&Link>；map 克隆 href；
                    // unwrap_or_default() 缺失时给空串（String 默认值）。
                    e.links.first().map(|l| l.href.clone()).unwrap_or_default()
                }
            };
            // Each field: Option from the parser → map to inner value →
            // unwrap_or_default() substitutes the type's default when absent.
            // 每个字段：解析器给 Option → map 取内部值 → 缺失时 unwrap_or_default() 补默认值。
            let title = e.title.map(|t| t.content).unwrap_or_default();
            let url = e.links.first().map(|l| l.href.clone()).unwrap_or_default();
            let summary = e.summary.map(|s| s.content).unwrap_or_default();
            // keep the feed-provided content (arrives with the feed, no extra
            // request); full-article fetch may later replace it
            // 保留源自带的全文（随源一起到达，无需额外请求）；之后可用全文抓取替换。
            // and_then(): like map but the closure itself returns an Option —
            // flattens Option<Option<T>> into Option<T>.
            // and_then()：类似 map 但闭包本身返回 Option，自动压平两层嵌套。
            let content = e
                .content
                .and_then(|c| c.body)
                .unwrap_or_default();
            // published OR updated: `.or()` = "if published is None, try updated".
            // Then format as RFC3339 (“2025-01-02T…”).
            // published 或 updated：or() 表示前者为 None 时用后者；再格式化为 RFC3339。
            let date = e
                .published
                .or(e.updated)
                .map(|d| d.to_rfc3339())
                .unwrap_or_default();
            // authors: borrow each (&a), keep non-empty names, CLONE each name
            // into its own String, collect into a Vec<String>, join with commas.
            // collect::<Vec<_>>() — turbofish pins the target collection type.
            // authors：逐个借用，过滤空名，clone 成自有 String，收进 Vec 再逗号连接。
            // collect::<Vec<_>>() 用 turbofish 语法钉住目标集合类型。
            let author = e
                .authors
                .iter()
                .filter(|a| !a.name.is_empty()).map(|a| a.name.clone())
                .collect::<Vec<_>>()
                .join(", ");
            // Struct literal again — every Item field exactly once; the local
            // variables above move in via field-shorthand.
            // 结构体字面量：每个字段恰好一次；上面的局部变量经简写直接移入。
            Item {
                guid,
                title,
                url,
                summary,
                content,
                date,
                author,
                read: false,
                read_later: false,
                saved: false,
            }
        })
        .collect();
    // Newest first; items without date sink to the end.
    // 最新的在前；无日期的沉到末尾。
    // sort_by takes a comparator closure. Empty string "" sorts below any
    // date, so undated items naturally land last. The closure receives
    // &Item borrows; b-before-a flips the order (descending).
    // sort_by 接受比较闭包。空串小于任何日期，无日期项自然排最后。
    // 闭包参数是 &Item 借用；b 在前 a 在后即降序。
    items.sort_by(|a, b| b.date.cmp(&a.date));
    Ok((feed_title, items))
}

/// Fetch full article, extract main content (Mozilla Readability via
/// dom_smoothie). Returns article HTML; falls back to the whole page.
/// Fetch full article, extract main content (Mozilla Readability via
/// dom_smoothie). Returns article HTML; falls back to the whole page.
/// 抓取全文并提取正文（dom_smoothie 实现 Mozilla Readability 算法）。
/// 返回正文 HTML；提取失败则回退为整个页面。
pub fn fetch_article(url: &str, timeout_secs: u64) -> Result<String, String> {
    // `?` propagates network errors up; extraction below never fails
    // (it has built-in fallbacks).
    // ? 向上传播网络错误；下面的提取不会失败（自带回退）。
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
    // Last expression IS the return value (no `return` keyword needed):
    // text() is Result<String>, mapped to our error type — ready to return.
    // 最后一个表达式就是返回值（无需 return 关键字）：
    // text() 的 Result 经 map_err 转换后直接作为函数结果。
    resp.text().map_err(|e| format!("read {url}: {e}"))
}

/// Extract main article content; fallback: whole page.
/// 提取正文主体；失败则回退为整个页面。
pub fn extract_main(url: &str, html: &str) -> String {
    // The readability lib wants an OWNED String (it may consume/modify it),
    // so we copy the borrowed &str once with to_string().
    // readability 库要求自有 String（可能会消费/修改），先把借用的 &str 拷贝一次。
    let owned = html.to_string();
    // Nested match on two Results: constructor failure or parse failure both
    // degrade to returning the raw page. match ≈ switch, but on enums with data.
    // 两层 match 分别处理构造失败和解析失败，均回退返回原始页面。
    // match 类似 switch，但能匹配带数据的枚举。
    match dom_smoothie::Readability::new(owned, Some(url), None) {
        Ok(mut r) => match r.parse() {
            Ok(article) => article.content.to_string(),
            Err(_) => html.to_string(),
        },
        Err(_) => html.to_string(),
    }
}

/// HTML → readable markdown text (best effort), for export.
/// HTML 转可读的 markdown 文本（尽力而为），用于导出。
pub fn html_to_markdown(html: &str) -> String {
    // strip script/style (h2md may otherwise inline their text)
    // 先剥掉 script/style 块（否则 h2md 可能把其文本内联进来）。
    // Shadowing `cleaned` twice keeps one logical "current value" without
    // inventing cleaned2/cleaned3 names.
    // 两次遮蔽 cleaned，保持单一“当前值”，不必起 cleaned2/cleaned3 这类名字。
    let cleaned = strip_tag_blocks(html, "script");
    let cleaned = strip_tag_blocks(&cleaned, "style");
    // Write into a growable byte buffer (Vec<u8>) — like a malloc'd buffer
    // that reallocs itself.
    // 写入可增长的字节缓冲 Vec<u8> —— 类似会自己 realloc 的缓冲区。
    let mut out = Vec::new();
    // h2md writes bytes into the writer (`&mut out`); we ignore its specific
    // error and just return "" on failure (best-effort export).
    // h2md 往 writer（&mut out）写字节；出错就返回空串（导出尽力而为）。
    if h2md::convert(cleaned.as_bytes(), &mut out).is_err() {
        return String::new();
    }
    // Turn bytes back into a String; invalid UTF-8 → default (empty).
    // trim() removes surrounding whitespace; to_string() makes it owned.
    // 字节转 String；非法 UTF-8 则给默认值（空串）。trim 去首尾空白，to_string 转自有。
    String::from_utf8(out).unwrap_or_default().trim().to_string()
}

/// Remove `<tag>…</tag>` blocks (case-insensitive).
fn strip_tag_blocks(html: &str, tag: &str) -> String {
    // byte-safe case-insensitive scan: tag names are ASCII, so lowering
    // A-Z never changes byte length (no to_lowercase offset divergence)
    // 按字节做大小写不敏感扫描：标签名是 ASCII，转小写不改字节数，
    // 因此不会出现 to_lowercase 导致的偏移错位问题。
    let mut out = String::with_capacity(html.len());
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    // `rest` is a moving &str window into html — reassigning a slice is cheap
    // (just pointer+len, nothing copied). Classic C substring-walking, but
    // bounds-checked.
    // rest 是指向 html 内部的滑动窗口；重新赋值切片很便宜（只改指针+长度，不拷贝）。
    // 经典的 C 式子串游走，但有越界保护。
    let mut rest = html;
    loop {
        // find_ci returns Option<usize>: byte offset of next opener, if any.
        // find_ci 返回 Option<usize>：下一个开标签的字节偏移。
        match find_ci(rest, &open) {
            Some(i) => {
                // rest[..i]: slice syntax = pointer arithmetic with bounds check.
                // Keep everything BEFORE the tag.
                // rest[..i] 切片语法 = 带越界检查的指针运算；保留标签之前的内容。
                out.push_str(&rest[..i]);
                let after = &rest[i..];
                // Skip past the '>' closing the opening tag. find('>') gives
                // offset within `after`; i+j+1 converts back to rest's coords.
                // unwrap_or: no '>' at all → treat everything as inside the tag.
                // 跳过开标签的 '>'；i+j+1 换算回 rest 的坐标。
                // 找不到 '>' 时把剩余全部当作标签内部。
                let tag_end = after.find('>').map(|j| i + j + 1).unwrap_or(rest.len());
                let rest_after_tag = &rest[tag_end..];
                match find_ci(rest_after_tag, &close) {
                    Some(j) => {
                        // Jump rest past the closing </tag>. min() guards a
                        // miscalculated index against slicing panic.
                        // 让 rest 越过闭合标签；min() 防止下标越界导致 panic。
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
                // No more openers: flush the remainder verbatim and stop.
                // 没有更多开标签：原样输出剩余部分并结束。
                out.push_str(rest);
                break;
            }
        }
    }
    out
}

/// Case-insensitive byte search (ASCII only — length-preserving).
fn find_ci(haystack: &str, needle: &str) -> Option<usize> {
    // as_bytes(): raw byte views — memcmp-style scanning, no UTF-8 decoding.
    // Safe here because ASCII case-folding never changes byte count.
    // as_bytes()：原始字节视图 —— memcmp 式扫描，不做 UTF-8 解码。
    // 安全前提：ASCII 大小写折叠不改变字节数。
    let h = haystack.as_bytes();
    let n = needle.as_bytes();
    if n.is_empty() || n.len() > h.len() {
        return None;
    }
    // 'outer labels THIS loop so `continue 'outer` can jump to its next
    // iteration directly from the INNER loop — C needs goto for this.
    // 'outer 标记外层循环，内层循环里可直接 continue 'outer 跳到外层下一轮 ——
    // C 里得靠 goto 实现。
    'outer: for i in 0..=h.len() - n.len() {
        // Compare needle against h[i..] byte by byte; bail on first mismatch.
        // 逐字节比较；首个不匹配立即放弃本轮起点。
        for j in 0..n.len() {
            if !h[i + j].eq_ignore_ascii_case(&n[j]) {
                continue 'outer;
            }
        }
        return Some(i); // all bytes matched / 全部匹配
    }
    None // not found / 未找到
}

// Unit tests — run with `cargo test`. The `!` in assert!/assert_eq! marks a
// macro (compile-time code expansion), not a function call.
// 单元测试 — cargo test 运行。assert!/assert_eq! 的 ! 表示宏（编译期展开），非普通函数。
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
        // 当初的核心痛点：转换后不允许残留任何原始 HTML 标签。
        // r#"..."# = RAW string literal: quotes/backslashes need no escaping
        // (C has no equivalent — you'd escape every \").
        // r#"..."# 原始字符串：引号、反斜杠无需转义；C 没有对应语法。
        let html = r#"<div class="post"><h2>Head</h2><p>Text <span>span</span> <em>em</em></p><pre><code class="rust">fn main(){}</code></pre><ul><li>one</li><li>two</li></ul><blockquote>quote</blockquote><img src="https://x.com/i.png" alt="pic"></div>"#;
        let md = html_to_markdown(html);
        assert!(!md.contains('<'), "residual tag in: {md}");
        assert!(md.contains("## Head"), "got: {md}");
        assert!(md.contains("*em*") || md.contains("em"), "got: {md}");
        assert!(md.contains("fn main"), "got: {md}");
        assert!(md.contains("![pic](https://x.com/i.png)"), "got: {md}");
    }
}
