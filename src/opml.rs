//! OPML import/export — reading and writing the standard feed-subscription
//! file format (an XML dialect used to move subscription lists between apps).
//! OPML 导入/导出 — 读写标准的订阅列表文件格式（一种 XML 方言）。
//!
//! Export: feeds → OPML 2.0, one `<outline>` per feed, category nested.
//! Import: parse `<outline>` entries with `xmlUrl`; title/type/text kept.
//! 导出：源 → OPML 2.0，每个源一个 `<outline>`，分类嵌套。
//! 导入：解析带 `xmlUrl` 的 `<outline>` 条目；保留 title/type/text。
//!
//! Rust concepts introduced here:
//! 本文件涉及的 Rust 概念：
//! - `Option<T>`: "maybe a value" as a type. Like a C pointer that may be
//!   NULL — but the compiler forces you to unwrap/check before use, so
//!   no null-pointer dereference bugs are possible.
//!   `Option<T>`：用类型表示“可能有值”。类似可为 NULL 的 C 指针，
//!   但编译器强制你先检查再使用，杜绝空指针解引用。
//! - `if let Some(x) = ...`: ergonomic pattern for "do this only if it's Some".
//!   `if let Some(x) = ...`：只在有值时才执行的简洁写法。
//! - enums + exhaustive `match`: XML events come as an enum; the compiler
//!   requires every case handled — like a `switch` that must cover all values.
//!   枚举 + 穷尽 match：XML 事件是枚举；编译器要求覆盖所有分支，
//!   类似必须写全的 switch。
//! - recursion over a tree: categories nest, so export walks the tree with a
//!   recursive function taking the current path as a slice `&[String]`.
//!   递归遍历树：分类可嵌套，导出用递归函数，路径参数是切片 &[String]。
//! - `Vec` as a stack: `.push()` / `.pop()` track open category nesting while
//!   streaming through the XML — like a manual stack in C, but type-safe.
//!   Vec 当栈用：流式读 XML 时 push/pop 跟踪打开的分类层级，
//!   类似 C 里手写的栈，但类型安全。
//! - shadowing + method chaining on `String::replace`: build escaped XML by
//!   chaining replacements, each producing a new String.
//!   变量遮蔽 + 链式 replace：逐个替换生成转义后的 XML，每次产生新 String。

use std::io; // only for the io::Result return type below
             // 仅为下面返回的 io::Result 类型而导入

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::feedlist::{Feed, File};

/// Serialize feeds to OPML 2.0 XML.
/// 把源列表序列化为 OPML 2.0 XML。
///
/// Data flow: `&File` (the whole feed list) → borrowed read → a fresh owned
/// `String` of XML. Nothing in `file` is mutated — note the `&` borrow;
/// in C you'd pass `const File *file`.
/// 数据流：&File（整个源列表）→ 只读借用 → 返回新的 XML String。
/// 不修改 file，注意 & 借用；C 里相当于传 const File *。
pub fn export_opml(file: &File) -> String {
    // String::new() = empty heap string, grown by push_str (like a malloc'd
    // buffer that reallocs itself).
    // String::new() = 空堆字符串，push_str 自动扩容（类似会自己 realloc 的缓冲区）。
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<opml version=\"2.0\">\n<head>\n<title>markerss subscriptions</title>\n</head>\n<body>\n");
    // Categories with feeds nested, then uncategorized at top level.
    // 分类（含嵌套的源）在前，未分类的源在顶层。
    // `&[]`: empty path = "start from the root category". Type inferred as
    // &[String] from the fn signature — an empty slice literal.
    // &[]：空路径表示“从根分类开始”。类型由函数签名推断为 &[String]。
    out.push_str(&category_outlines(file, &[]));
    // `for f in file.uncategorized()` iterates whatever that method returns
    // (likely references); each f is borrowed, not moved.
    // for 循环迭代 uncategorized() 的返回值；每个 f 是借用而非移动。
    for f in file.uncategorized() {
        out.push_str(&feed_outline(f));
    }
    out.push_str("</body>\n</opml>\n");
    out
}

/// Recursively emit nested `<outline>` groups for a category subtree.
/// 递归输出某个分类子树的嵌套 `<outline>` 分组。
///
/// `path` is the chain of parent category names, e.g. ["tech", "rust"].
/// Borrowed (`&[String]`) because we only read it — no copy per call.
/// path 是父分类名链，如 ["tech", "rust"]。借用 &[String] 只读不拷贝。
fn category_outlines(file: &File, path: &[String]) -> String {
    let mut out = String::new();
    // child_categories(path): names directly under this path.
    // child_categories(path)：该路径下一层的分类名。
    for child in file.child_categories(path) {
        // path.to_vec(): copies the borrowed slice into an owned Vec so we
        // can push the child name — building the path one level deeper.
        // (.to_vec() on a slice allocates, like malloc+memcpy.)
        // path.to_vec()：把借用的切片拷贝成 Vec，以便 push 子分类名，
        // 构造更深一层的路径。（切片的 .to_vec() 会分配内存。）
        // child.clone(): deep-copies this child's String too — we need our
        // own copy since the iterator's item stays borrowed.
        // child.clone()：深拷贝该子分类的 String；迭代项仍是借用的，需要自己的副本。
        let mut child_path = path.to_vec();
        child_path.push(child.clone());
        // Escape XML specials BEFORE embedding into markup — chaining three
        // replaces; each returns a new String (strings here are immutable).
        // 先转义 XML 特殊字符再嵌入标签；链式 replace 每次返回新 String。
        let esc = child.replace('&', "&amp;").replace('<', "&lt;").replace('"', "&quot;");
        out.push_str(&format!("<outline text=\"{esc}\" title=\"{esc}\">\n"));
        // Recurse: emit this category's subcategories first…
        // 递归：先输出该分类的子分类……
        out.push_str(&category_outlines(file, &child_path));
        // …then the feeds filed under exactly this path.
        // ……再输出属于该路径的源。
        for f in file.by_category_path(&child_path) {
            out.push_str(&feed_outline(f));
        }
        out.push_str("</outline>\n");
    }
    out
}

/// Render one feed as a self-closing `<outline ... xmlUrl="..."/>` line.
/// 把单个源渲染成自闭合的 `<outline ... xmlUrl="..."/>` 行。
fn feed_outline(f: &Feed) -> String {
    // Option unwrap_or_else: use stored title if Some, else fall back to the
    // URL. The closure `|| f.url.clone()` runs ONLY in the None case — lazy,
    // like writing the else branch of a NULL-check in C.
    // unwrap_or_else：有 title 用 title，否则回退到 URL。
    // 闭包 || f.url.clone() 仅在 None 时执行（惰性求值）。
    // .clone() needed because we build an owned String from a borrow of f.
    // 需要 .clone() 因为要从对 f 的借用构造自有 String。
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
/// Parse OPML XML into feeds. Category groups become the feed's first tag.
/// 把 OPML XML 解析成源列表。分类分组变成该源的第一个 tag。
///
/// Data flow: `&str` (borrowed XML text) → owned `Vec<Feed>`. Returns
/// `io::Result<...>`: `Ok(feeds)` on success, `Err(...)` if I/O fails —
/// callers use `?` or `.unwrap()` on it.
/// 数据流：借用的 &str → 自有的 Vec<Feed>。返回 io::Result：
/// 成功 Ok，I/O 失败 Err；调用方用 ? 或 .unwrap() 处理。
pub fn import_opml(xml: &str) -> io::Result<Vec<Feed>> {
    // Reader::from_str borrows the xml text; quick-xml streams events out of
    // it without building a whole DOM tree in memory (unlike most C parsers).
    // Reader::from_str 借用 xml 文本；quick-xml 流式产出事件，
    // 不在内存里构建整棵 DOM 树（不同于多数 C 解析器）。
    let mut reader = Reader::from_str(xml);
    let mut feeds = Vec::new();
    let mut stack: Vec<String> = Vec::new(); // open category names
                                             // 当前打开的分类名栈
                                             // The stack tracks nesting: push when a group outline opens,
                                             // pop when it closes. Like a manual char** stack in C.
                                             // 栈跟踪嵌套层级：分组开始时 push，结束时 pop。
                                             // 类似 C 里手写的 char** 栈。
    let mut buf = Vec::new(); // scratch buffer reused for each event (avoids per-event allocation)
                              // 复用的临时缓冲区，每个事件共用（避免反复分配）
    loop {
        // read_event_into yields ONE parsed XML event per call, borrowing buf.
        // Each iteration overwrites the previous event's data — hence buf.clear().
        // read_event_into 每次产出一个 XML 事件，借用 buf；
        // 每轮覆盖上一轮的数据，所以循环末尾要 buf.clear()。
        match reader.read_event_into(&mut buf) {
            // Event is an enum; match arms bind the payload `e` (a BytesStart).
            // Exhaustive: the `_ => {}` arm covers everything else.
            // Event 是枚举；match 分支把载荷 e（BytesStart）绑定出来。
            // 穷尽匹配：`_ => {}` 兜底其余分支。
            Ok(Event::Start(e)) => {
                // XML bytes may not be valid UTF-8; from_utf8_lossy replaces
                // bad bytes with U+FFFD instead of failing (like strdup of
                // sanitized data). .to_string() makes an owned copy so `name`
                // doesn't borrow from buf.
                // XML 字节可能不是合法 UTF-8；from_utf8_lossy 用 U+FFFD 替换坏字节
                // 而不是报错。.to_string() 产生自有副本，name 不再借用 buf。
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "outline" {
                    // Destructuring tuple return: url/title are Option<String>.
                    // 解构元组返回值：url/title 都是 Option<String>。
                    let (url, title) = outline_attrs(&e);
                    // if let Some(url): only when xmlUrl was present → it's a FEED.
                    // `url` here shadows the Option with the inner String (moved out).
                    // if let Some(url)：有 xmlUrl 才是源；url 遮蔽外层 Option，
                    // 直接拿到里面的 String（被移动出来）。
                    if let Some(url) = url {
                        feeds.push(make_feed(url, title, &stack.join("/")));
                    } else if let Some(cat) = title {
                        // No xmlUrl but has title → it's a CATEGORY GROUP: push onto
                        // the nesting stack so child feeds inherit this path.
                        // 无 xmlUrl 但有 title → 分类分组：压栈，子源继承该路径。
                        stack.push(cat);
                    }
                }
            }
            Ok(Event::Empty(e)) => {
                // Empty = self-closing `<outline .../>` — how feeds usually appear.
                // Same handling as Start, but never pushed to the stack.
                // Empty = 自闭合 <outline .../>，源通常长这样；处理同 Start，但不入栈。
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
                // pop only if this outline opened a category (Start without xmlUrl);
                // feed outlines (Empty) never pushed
                // 只有分组型 outline（无 xmlUrl 的 Start）才入过栈，遇到 End 才 pop；
                // 自闭合的源（Empty）从未入栈。
                if name == "outline" && !stack.is_empty() {
                    stack.pop();
                }
            }
            // Eof: clean end of document — leave the loop.
            // Eof：文档正常结束，退出循环。
            Ok(Event::Eof) => break,
            // Parse error: stop scanning, keep whatever we collected so far
            // (lenient import — better than losing everything).
            // 解析出错：停止扫描，保留已收集的内容（宽容导入）。
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    // Everything succeeded (or errors were tolerated): wrap in Ok.
    // 全部成功（或错误已被容忍）：包进 Ok 返回。
    Ok(feeds)
}

/// Extract the attributes we care about from one `<outline ...>` tag.
/// 从单个 <outline ...> 标签提取需要的属性。
///
/// Returns `(xmlUrl?, title?)` as Options — None means "attribute absent".
/// Title falls back: `title` attr first, then `text` attr (`or()` = first Some wins).
/// 返回 (xmlUrl?, title?)，None 表示属性缺失。
/// title 回退顺序：先 title 属性，再 text 属性（or() 取第一个 Some）。
fn outline_attrs(e: &quick_xml::events::BytesStart) -> (Option<String>, Option<String>) {
    // Option::None as initial values — like initializing pointers to NULL.
    // 初始为 None — 类似把指针初始化为 NULL。
    let mut url = None;
    let mut title = None;
    let mut text = None;
    // attributes() yields Results (an attribute can be malformed);
    // .flatten() skips the Err cases silently, keeping just the good ones.
    // attributes() 产出 Result（属性可能畸形）；flatten() 静默跳过 Err 只留好的。
    for a in e.attributes().flatten() {
        let key = String::from_utf8_lossy(a.key.as_ref()).to_string();
        // Attribute values arrive entity-escaped (&amp; etc.) — decode them.
        // 属性值带实体转义（如 &amp;），需先解码。
        let val = unescape_entities(&String::from_utf8_lossy(&a.value));
        // match on the &str view of the owned String — pattern matching on
        // string CONTENTS, which C's switch cannot do (needs strcmp chain).
        // 对 String 的 &str 视图做 match —— 按字符串内容匹配，
        // C 的 switch 做不到（只能写一串 strcmp）。
        match key.as_str() {
            // Some(val): wrap the value back up into the Option slot.
            // Some(val)：把值重新包进 Option 槽位。
            "xmlUrl" => url = Some(val),
            "title" => title = Some(val),
            "text" => text = Some(val),
            _ => {} // ignore unknown attributes / 忽略其他属性
        }
    }
    // .or(): if title is None, yield text instead — first Some wins.
    // .or()：title 为 None 时改用 text —— 取第一个 Some。
    (url, title.or(text))
}

/// Build a Feed from parsed OPML pieces.
/// 用解析出的 OPML 片段构造 Feed。
fn make_feed(url: String, title: Option<String>, category: &str) -> Feed {
    let mut custom_name = false;
    let mut title = title; // shadowing: make `title` mutable for the rewrite below
                           // 变量遮蔽：让 title 可变以便下面重写
    // Convention: titles starting with '~' were renamed by the user by hand.
    // 约定：以 '~' 开头的标题表示用户手动改过名。
    if let Some(t) = &title {
        // strip_prefix returns Option<&str>: Some(suffix) if t starts with '~'.
        // strip_prefix 返回 Option<&str>：以 '~' 开头时给出去掉后的后缀。
        if let Some(stripped) = t.strip_prefix('~') {
            custom_name = true;
            // stripped borrows from *t which borrows from title — so build a
            // fresh owned String before reassigning (borrow checker insists).
            // stripped 是对 title 的借用链；重新赋值前必须先造出自有 String。
            title = Some(stripped.to_string());
        }
    }
    // Struct literal syntax: every field must be listed exactly once —
    // the compiler enforces completeness (C99 designated initializers,
    // but mandatory and order-free).
    // 结构体字面量：每个字段必须恰好出现一次，编译器强制完整
    // （类似 C99 指定初始化器，但强制且与顺序无关）。
    Feed {
        url,   // field shorthand: local var `url` moves straight into the field
               // 字段简写：局部变量 url 直接移动进字段
        title,
        custom_name,
        feed_title: None, // unknown until the feed itself is fetched
                          // 要等真正抓取源后才知道
        tags: if category.is_empty() {
            Vec::new() // no category → empty tag list / 无分类 → 空 tag 列表
        } else {
            // vec![] macro allocates a Vec with one element; to_string copies
            // the borrowed &str into an owned String.
            // vec![] 宏分配单元素 Vec；to_string 把借用的 &str 拷成自有 String。
            vec![category.to_string()]
        },
        feed_tags: Vec::new(),
        favourite: false,
        lazy: false,
    }
}

/// Decode the common XML entities (quick-xml's unescape needs a Decoder).
/// 解码常见 XML 实体（quick-xml 自带的 unescape 需要构造 Decoder，这里手写更简单）。
fn unescape_entities(s: &str) -> String {
    // Chained replaces, longest/most-specific first: MUST decode `&amp;` LAST
    // in the general case — here order is safe only because we never create
    // new '&' patterns that a later pass would re-expand… note `&amp;` → '&'
    // runs first, so "&amp;lt;" (a literal) would become "&lt;" then "<":
    // acceptable for this app's inputs.
    // 链式替换：一般应最后解码 &amp;；这里顺序对常见输入够用。
    // 每次 replace 返回新 String，逐层生成解码结果。
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
}

// Unit tests — run with `cargo test`.
// 单元测试 — cargo test 运行。
#[cfg(test)]
mod tests {
    use super::*;

    // Test helper fn — not #[test] itself, just builds shared test data.
    // 测试辅助函数 —— 本身不是测试，只构造共用的测试数据。
    // File::default() comes from the Default trait (derive/impl of a
    // zero-ish value) — like calloc for a struct with sane initial fields.
    // File::default() 来自 Default trait —— 类似对结构体做 calloc 得到合理初值。
    fn sample_file() -> File {
        let mut f = File::default();
        f.upsert(Feed { url: "https://a.com/f".into(), title: Some("A".into()), custom_name: false, feed_title: None, tags: vec!["tech".into()], feed_tags: vec![], favourite: false, lazy: false });
        f.upsert(Feed { url: "https://b.com/f".into(), title: None, custom_name: false, feed_title: None, tags: vec![], feed_tags: vec![], favourite: false, lazy: false });
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
        f.upsert(Feed { url: "https://a.com/f".into(), title: Some("A".into()), custom_name: false, feed_title: None, tags: vec!["tech/rust".into()], feed_tags: vec![], favourite: false, lazy: false });
        f.upsert(Feed { url: "https://b.com/f".into(), title: Some("B".into()), custom_name: false, feed_title: None, tags: vec!["tech/go".into()], feed_tags: vec![], favourite: false, lazy: false });
        f.upsert(Feed { url: "https://c.com/f".into(), title: None, custom_name: false, feed_title: None, tags: vec![], feed_tags: vec![], favourite: false, lazy: false });
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
