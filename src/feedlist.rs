//! newsboat `urls` format parser + file I/O.
//! newsboat `urls` 格式解析器 + 文件读写。
//!
//! Line grammar: `URL "Title" tag1 tag2 ...`
//! 行语法：`URL "标题" 标签1 标签2 ...`
//! - `#` starts a comment line
//!   `#` 开头的行为注释行
//! - `~` prefix on quoted title = custom display name (overrides feed title)
//!   引号标题前的 `~` 前缀 = 自定义显示名（覆盖源自带的标题）
//! - tags = categories; first tag places feed in tree
//!   标签 = 分类；第一个标签决定源在目录树中的位置
//!
//! Rust concepts introduced here:
//! 本文件涉及的 Rust 概念：
//! - `Option<T>`: "a value or nothing" — like returning NULL from C, but the
//!   compiler FORCES you to handle the None case before you can touch the value.
//!   No null-pointer dereference crashes, ever.
//!   `Option<T>`："有值或没值" — 类似 C 里返回 NULL，但编译器强制你先处理
//!   None 才能取值。彻底杜绝空指针解引用崩溃。
//! - `Result<T, E>` + `?`: error propagation. `expr?` means "if this failed,
//!   return the error from the current function NOW" — like a checked call
//!   plus `goto fail`, written in one character.
//!   `Result<T, E>` 与 `?`：错误传播。`expr?` 表示「失败就立刻从当前函数
//!   返回错误」— 相当于 C 里检查返回值后 goto fail，一个字符搞定。
//! - `Vec<T>`: growable array, like a self-managing malloc/realloc/free buffer.
//!   `Vec<T>`：可增长数组，相当于自动管理 malloc/realloc/free 的缓冲区。
//! - iterator chains (`.iter().filter().map().collect()`): declarative loops,
//!   like writing `filter`/`map` over pointer ranges in C — but bounds-checked
//!   and allocation-safe.
//!   迭代器链（.iter().filter().map().collect()）：声明式循环，
//!   类似对 C 指针区间做 filter/map，但有越界检查且内存安全。
//! - `#[derive(Default)]`: compiler generates an all-zero-fields constructor.
//!   `#[derive(Default)]`：编译器生成全默认值的构造函数。

use std::fs;
use std::io;
use std::path::Path;


// Derives explained:
// - Debug: printable with {:?}
// - Clone: .clone() deep copy
// - PartialEq/Eq: allows `==` and assert_eq! comparisons in the tests below
// 派生说明：Debug 可用 {:?} 打印；Clone 支持 .clone() 深拷贝；
// PartialEq/Eq 让 == 和下方测试里的 assert_eq! 可用。
#[derive(Debug, Clone, PartialEq, Eq)]
/// One subscription line from the urls file — the app's source-of-truth for feeds.
/// urls 文件中的一条订阅记录 — 订阅信息的唯一权威来源。
pub struct Feed {
    pub url: String,
    /// Quoted title, `~` prefix stripped. `None` when no quoted title present.
    /// 引号中的标题，`~` 前缀已剥掉。没有引号标题时为 None。
    /// Option<String> ≈ "char* that may be NULL", but compiler-checked.
    /// Option<String> ≈ 可能为 NULL 的 char*，但由编译器检查。
    pub title: Option<String>,
    /// Title came with `~` prefix → custom display name, do not override from feed.
    /// 标题带 `~` 前缀 → 用户自定义名，不要被源 XML 里的标题覆盖。
    pub custom_name: bool,
    /// Title fetched from the feed XML (session; refreshed on each fetch).
    /// 从源 XML 抓到的标题（会话级；每次抓取时刷新）。不落盘到 urls 文件。
    pub feed_title: Option<String>,
    /// Categories (`#`-less words); first = tree placement.
    /// 分类（不带 # 的词）；第一个决定在目录树的位置。
    /// Vec<String>：自有所有权的字符串数组，像自动管理的 char** 数组。
    pub tags: Vec<String>,
    /// Feed tags (`#`-prefixed words).
    /// 源标签（带 # 前缀的词），纯标注，不影响树结构。
    pub feed_tags: Vec<String>,
    /// Favourite flag (`!favourite` marker).
    /// 收藏标记（行内写 !favourite）。
    pub favourite: bool,
    /// Lazy flag (`!lazy` marker) — skipped by auto refresh (startup/interval).
    /// 懒加载标记（行内写 !lazy）— 自动刷新（启动/定时）时跳过该源。
    pub lazy: bool,
}

impl Feed {
    /// Best display name: user title > feed XML title > URL.
    /// 显示名优先级：用户标题 > 源 XML 标题 > URL。
    pub fn display_name(&self) -> &str {
        // `match &self.title`: borrowing the Option, not consuming it —
        // the enum's two variants become patterns Some(t) / None.
        // In C you'd write `if (title != NULL)`; here the compiler proves
        // every variant is handled (exhaustiveness checking).
        // match &self.title：借用 Option 而非夺走所有权 —
        // 枚举的两个变体对应模式 Some(t) / None。C 里写 if (title != NULL)；
        // 这里编译器强制穷尽所有变体（穷尽性检查）。
        match &self.title {
            // t: &String — auto-coerced to &str on return (deref coercion)
            // t 是 &String，返回时自动强转为 &str（解引用强制转换）
            Some(t) => t,
            // as_deref(): Option<String> -> Option<&str> without moving out;
            // unwrap_or(fallback): take the value or use fallback if None
            // as_deref()：Option<String> -> Option<&str>，不移动内部值；
            // unwrap_or(fallback)：有值取值，None 则用后备值
            None => self.feed_title.as_deref().unwrap_or(&self.url),
        }
    }

    /// First tag = category (may be a slash path `cat/sub`); `None` = uncategorized.
    /// 第一个标签 = 分类（可能是 `cat/sub` 形式的路径）；None = 未分类。
    pub fn category(&self) -> Option<&str> {
        // .first() returns Option<&String> (empty Vec -> None).
        // .map(String::as_str) converts the inner value only when present:
        // Option is a functor — map applies to the Some case, passes None through.
        // C analogue: check for NULL then convert; here it's one chained call.
        // .first() 返回 Option<&String>（空 Vec 则为 None）。
        // .map(String::as_str) 只在有值时转换：Option 是函子，
        // map 作用于 Some 分支，None 原样透传。C 里需先判 NULL 再转换，这里一行链式搞定。
        self.tags.first().map(String::as_str)
    }

    /// Category path segments (`cat/sub` → `["cat", "sub"]`); empty = uncategorized.
    /// 分类路径切段（`cat/sub` → `["cat", "sub"]`）；空 = 未分类。
    pub fn category_segments(&self) -> Vec<&str> {
        // Iterator chain, evaluated lazily:
        // category()          -> Option<&str>       (may be None)
        // .map(|c| ...)       -> Option<Vec<&str>>  (split only if Some)
        // .unwrap_or_default()-> Vec<&str>          (None becomes empty Vec)
        // This "map inside Option" shape replaces nested NULL checks in C.
        // 迭代器链惰性求值：category() 可能是 None；.map 只在 Some 时切分；
        // .unwrap_or_default() 把 None 变成空 Vec。
        // 这种「Option 内 map」写法替代了 C 里的嵌套 NULL 判断。
        self.category()
            .map(|c| c.split('/').filter(|s| !s.is_empty()).collect())
            .unwrap_or_default()
    }

    /// Serialize back to urls-file line form.
    /// 序列化回 urls 文件的一行文本。
    pub fn to_line(&self) -> String {
        // .clone(): we own `url` as a String and are about to append to it —
        // a fresh owned copy is required because building the line mutates it.
        // .clone()：要把 url 变成可追加的 String 底座，需要一份自有拷贝。
        let mut line = self.url.clone();
        // Classic Option pattern: `if let Some(x) = opt` unwraps only when Some,
        // skips the block entirely on None. Like `if (t) {...}` in C but checked.
        // 经典 Option 用法：if let Some(x) = opt 有值才进块，None 直接跳过。
        // 类似 C 的 if (t)，但由编译器检查。
        if let Some(t) = &self.title {
            let prefix = if self.custom_name { "~" } else { "" };
            line.push_str(&format!(" \"{prefix}{t}\""));
        }
        // `for tag in &self.tags`: iterating by reference (&Vec<T> yields &T),
        // so tags are borrowed, not moved — the Feed stays intact afterwards.
        // for tag in &self.tags：按引用迭代（产出 &T），只借用不转移所有权，
        // 之后 Feed 仍完好可用。
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
        if self.lazy {
            line.push_str(" !lazy");
        }
        line
    }

    /// Does this feed carry feed-tag `tag`? (`#tech` etc.)
    /// 该源是否带标签 tag（如 #tech）？
    pub fn has_tag(&self, tag: &str) -> bool {
        // .iter().any(closure): true if ANY element satisfies the predicate —
        // short-circuits like a `for` loop with `return true` inside.
        // .iter().any(闭包)：任一元素满足即 true，短路求值，
        // 相当于循环内提前 return true。
        self.feed_tags.iter().any(|t| t == tag)
    }
}

/// Parsed urls file.
/// 解析后的 urls 文件整体。
#[derive(Debug, Default)] // Default: File::default() gives { feeds: Vec::new() }
                          // Default：File::default() 得到空 feeds 的实例
pub struct File {
    pub feeds: Vec<Feed>,
}

impl File {
    /// Read + parse a urls file from disk.
    /// 从磁盘读取并解析 urls 文件。
    pub fn load(path: &Path) -> io::Result<File> {
        // fs::read_to_string returns io::Result<String>: Ok(contents) or Err(io error).
        // The trailing `?` propagates any Err upward immediately — this whole
        // function is "do the work, bail out on first failure".
        // fs::read_to_string 返回 io::Result<String>。末尾的 ? 在出错时立刻
        // 向上传播错误 — 整个函数就是「干活，遇错即退」。
        let data = fs::read_to_string(path)?;
        // Struct-init shorthand: field name == variable name, so `feeds: parse(&data)`.
        // Ok(...) wraps the value into Result's Ok variant.
        // 结构体初始化简写；Ok(...) 把值包成 Result 的 Ok 变体。
        Ok(File { feeds: parse(&data) })
    }

    /// Load the file, falling back to an empty list if missing/corrupt —
    /// used at startup so a fresh install works with no urls file yet.
    /// 读取文件；缺失或损坏时回退到空列表 — 首次安装没有文件也能启动。
    pub fn load_or_default(path: &Path) -> File {
        // unwrap_or_default(): Result<File,_> -> File (default if Err).
        // One-liner replacing `if err return default` boilerplate.
        // unwrap_or_default()：Err 时给默认值，一行替代错误判断样板代码。
        File::load(path).unwrap_or_default()
    }

    /// Write all feeds back to disk, one line per feed, header comment first.
    /// 全量写回磁盘：每源一行，先写头部注释。
    pub fn save(&self, path: &Path) -> io::Result<()> {
        // Ensure parent dirs exist before writing (e.g. ~/.local/state/...).
        // path.parent() is Option<&Path> — None means bare filename, skip.
        // 写之前确保父目录存在（如 ~/.local/state/...）。
        // path.parent() 是 Option<&Path> — 纯文件名时为 None，直接跳过。
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let mut out = String::from("# markerss subscriptions (newsboat urls format)\n");
        for f in &self.feeds {
            // &f.to_line(): format!-style deref — pass a &String where &str expected.
            out.push_str(&f.to_line());
            out.push('\n');
        }
        // Single atomic-ish write of the fully built string.
        // 整串一次性写入。
        fs::write(path, out)
    }

    /// Add or replace a feed with the same URL.
    /// 按 URL 新增或替换订阅。
    pub fn upsert(&mut self, feed: Feed) {
        // `&mut self`: we mutate our own Vec, so we need a mutable borrow.
        // `feed: Feed` by value: ownership moves INTO this fn — we store it.
        // &mut self：要修改自己的 Vec，需要可变借用。
        // feed: Feed 按值传入：所有权移入本函数，随后被存起来。
        // iter_mut() yields &mut Feed; find(closure) stops at first URL match.
        // iter_mut() 产出 &mut Feed；find(闭包) 在首个匹配处停止。
        if let Some(existing) = self.feeds.iter_mut().find(|f| f.url == feed.url) {
            // Deref-assign through the &mut: replaces the Vec slot in place.
            // 通过 &mut 赋值：原地替换 Vec 槽位。
            *existing = feed;
        } else {
            self.feeds.push(feed);
        }
    }

    /// Remove the feed with this URL (no-op if absent).
    /// 按 URL 删除订阅（不存在则无事发生）。
    pub fn remove(&mut self, url: &str) {
        // retain(predicate): keep only elements where the closure is true —
        // in-place compaction, C would need a second array + memcpy dance.
        // retain(谓词)：只保留闭包为 true 的元素，原地压缩；
        // C 里需要第二个数组加拷贝才能做到。
        self.feeds.retain(|f| f.url != url);
    }

    /// Distinct categories in tree order, feeds of `None` = uncategorized.
    /// 分类等于 cat 的所有源；category 为 None 的属未分类，不在此列。
    pub fn by_category(&self, cat: &str) -> Vec<&Feed> {
        // Borrowing pipeline: iterate &Feed refs, filter, collect into Vec<&Feed> —
        // zero copies of Feed data, just pointers. Caller sees an immutable view.
        // 借用管线：迭代 &Feed 引用、过滤、收集成 Vec<&Feed> —
        // 不拷贝数据只存指针。调用方拿到的是不可变视图。
        self.feeds
            .iter()
            .filter(|f| f.category() == Some(cat))
            .collect()
    }

    /// All category tree nodes as paths — every distinct prefix of every
    /// feed's category path, in first-seen order (parents before children).
    /// 全部分类树节点（路径形式）— 每个分类路径的所有去重前缀，
    /// 按首次出现顺序排列（父节点在子节点之前）。
    pub fn categories_tree(&self) -> Vec<Vec<String>> {
        let mut out: Vec<Vec<String>> = Vec::new();
        for f in &self.feeds {
            // Convert borrowed &str segments into owned Strings so the result
            // doesn't borrow from `self` (caller can keep it while mutating feeds).
            // 把借用的 &str 段转成自有 String，结果不再借用 self，
            // 调用方可持有它继续修改 feeds。
            let segs: Vec<String> =
                f.category_segments().iter().map(|s| s.to_string()).collect();
            if segs.is_empty() {
                continue;
            }
            // Inclusive range 1..=len: emit [a], [a,b], [a,b,c] — every prefix.
            // segs[..i] slices the Vec without copying.
            // 闭区间 1..=len：依次产出 [a]、[a,b]、[a,b,c] 等全部前缀。
            // segs[..i] 切片不拷贝。
            for i in 1..=segs.len() {
                if !out.contains(&segs[..i].to_vec()) {
                    out.push(segs[..i].to_vec());
                }
            }
        }
        out
    }

    /// Direct child category names of `path` (next segment only, deduped).
    /// path 的直接子分类名（只取下一段），去重。
    pub fn child_categories(&self, path: &[String]) -> Vec<String> {
        // Root call (empty path): empty prefix; otherwise "cat/" style prefix.
        // 根调用（空路径）：空前缀；否则形如 "cat/" 的前缀。
        let prefix = if path.is_empty() {
            String::new()
        } else {
            format!("{}/", path.join("/"))
        };
        let mut out: Vec<String> = Vec::new();
        for f in &self.feeds {
            // Nested if-let ladder: descend into the Option/slice step by step;
            // each level only runs when the previous unwrap succeeded.
            // 嵌套 if-let 阶梯：逐层深入，上一级取到值才进入下一级判断。
            if let Some(c) = f.category() {
                // strip_prefix: like strncmp against a prefix, returns the
                // remainder as Option<&str> (None when it doesn't match).
                // strip_prefix：前缀比较，匹配时返回剩余部分 Option<&str>，不配则 None。
                if let Some(rest) = c.strip_prefix(&prefix) {
                    // next(): first whitespace-split word — the child segment.
                    // next()：取第一段 — 即子分类名。
                    if let Some(next) = rest.split('/').next() {
                        if !out.iter().any(|x| x == next) {
                            // to_string() copies out of the borrowed slice before pushing.
                            // push 前先 to_string() 把借用内容拷贝出来。
                            out.push(next.to_string());
                        }
                    }
                }
            }
        }
        out
    }

    /// Feeds whose category is exactly `path` (joined) — direct children only.
    /// 分类恰好等于 path（拼接后）的源 — 只取直接子节点。
    pub fn by_category_path(&self, path: &[String]) -> Vec<&Feed> {
        // join: ["tech","rust"] -> "tech/rust"; as_str() borrows it for the filter.
        // join 把段拼成路径；as_str() 借用它供 filter 比较。
        let joined = path.join("/");
        self.feeds
            .iter()
            .filter(|f| f.category() == Some(joined.as_str()))
            .collect()
    }

    /// Rename a category path `old` → `new`; renames the whole subtree
    /// (feeds under `old/sub` become `new/sub`).
    /// 重命名分类 old → new；整个子树一起改（old/sub 变成 new/sub）。
    pub fn rename_category(&mut self, old: &str, new: &str) {
        for f in self.feeds.iter_mut() {
            if let Some(c) = f.category() {
                // c borrows from f.tags[0], so writes below go through f.tags[0]
                // after the borrow ends — NLL makes this sequence legal.
                // c 借用了 f.tags[0]，下面的写入发生在借用结束后 —
                // NLL（非词法作用域生命周期）使这种写法合法。
                if c == old {
                    f.tags[0] = new.to_string();
                } else if let Some(rest) = c.strip_prefix(&format!("{old}/")) {
                    // Subtree member: rewrite just the leading part.
                    // 子树成员：只重写前缀部分。
                    f.tags[0] = format!("{new}/{rest}");
                }
            }
        }
    }

    /// Feeds with no category at all.
    /// 完全没有分类的源。
    pub fn uncategorized(&self) -> Vec<&Feed> {
        // .is_none(): Option's "is it NULL?" — returns bool without unwrapping.
        // .is_none()：判断 Option 是否为 None，直接返回 bool，不解包。
        self.feeds.iter().filter(|f| f.category().is_none()).collect()
    }

    /// All distinct feed tags, in tree order.
    /// 所有去重后的源标签，按首次出现顺序。
    pub fn all_feed_tags(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for f in &self.feeds {
            for t in &f.feed_tags {
                if !out.contains(t) {
                    // clone() needed: t is a &String borrowed from the feed,
                    // the out Vec must own its own copy.
                    // 需要 clone()：t 是借用自源的 &String，
                    // out 必须持有自己的拷贝。
                    out.push(t.clone());
                }
            }
        }
        out
    }
}

/// Parse a whole urls file: one `Feed` per valid line, bad lines silently dropped.
/// 解析整个 urls 文件：每个合法行产出一个 Feed，坏行静默丢弃。
pub fn parse(input: &str) -> Vec<Feed> {
    // .lines(): iterator over newline-separated &str slices (no allocation per line).
    // filter_map(f): like map but KEEP only Some results — None entries vanish.
    // Perfect fit for "parse each line, skip failures".
    // .lines()：按换行迭代 &str 切片（每行不分配内存）。
    // filter_map(f)：类似 map 但只保留 Some 结果，None 被丢弃 —
    // 天然适合「逐行解析、失败即跳过」的场景。
    input.lines().filter_map(parse_line).collect()
}

/// Parse one non-empty, non-comment line. Malformed lines → `None`.
pub fn parse_line(line: &str) -> Option<Feed> {
    // Shadowing: rebind `line` to its trimmed view. Same name, new value.
    // 变量遮蔽：line 重新绑定为去掉首尾空白的视图。
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None; // comment or blank — not an error, just skipped
                     // 注释或空行 — 不是错误，仅跳过
    }
    // `?` on Option: take the value out of Some, or return None from THIS fn.
    // Like `if (tok == NULL) return NULL;` in C, in one character.
    // Option 上的 ?：Some 则取出值，None 则立刻从本函数返回 None。
    // 相当于 C 的判空加提前返回，一个字符搞定。
    let url = line.split_whitespace().next()?;
    // Slicing by byte offset is safe here: url came from split_whitespace,
    // so url.len() lands on a char boundary (UTF-8 safe).
    // 按字节偏移切片安全：url 来自 split_whitespace，
    // url.len() 落在字符边界上（不会切断 UTF-8 字符）。
    let mut rest = line[url.len()..].trim();

    let mut title = None;
    let mut custom_name = false;
    // Optional quoted-title section: present only when the line has a leading '"'.
    // 可选的引号标题段：只有行内出现引号时才处理。
    if let Some(r) = rest.strip_prefix('"') {
        // find returns Option — the `?` bails out (None) if no closing quote:
        // an unterminated quote makes the whole line invalid.
        // find 返回 Option，`?` 在没有右引号时直接返回 None：
        // 引号不闭合则整行无效。
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

    // map(str::to_string): convert each borrowed &str word to an owned String,
    // collect() gathers them into a Vec. C: loop + strdup + realloc.
    // map(str::to_string)：把每个借用的 &str 单词转成自有 String，
    // collect() 收集成 Vec。C 里要手写循环 + strdup + realloc。
    let words: Vec<String> = rest.split_whitespace().map(str::to_string).collect();
    let mut categories = Vec::new();
    let mut feed_tags = Vec::new();
    let mut favourite = false;
    let mut lazy = false;
    for w in words {
        // Dispatch on prefix: '#'-words are feed tags, '!' words are flags,
        // anything else is a category. `w` was moved into the loop, so pushing
        // it into a Vec is free (no copy needed).
        // 按前缀分派：带 # 的是源标签，带 ! 的是标记，其余是分类。
        // w 的所有权已移入循环，push 进 Vec 无需拷贝。
        if let Some(t) = w.strip_prefix('#') {
            if !t.is_empty() {
                feed_tags.push(t.to_string());
            }
        } else if w == "!favourite" {
            favourite = true;
        } else if w == "!lazy" {
            lazy = true;
        } else {
            categories.push(w);
        }
    }
    Some(Feed {
        url: url.to_string(),
        // Field-init shorthand: `title,` means `title: title`. The local vars
        // are MOVED into the struct — after this they're owned by the Feed.
        // 字段初始化简写：title, 等价于 title: title。局部变量所有权
        // 移入结构体，之后归 Feed 所有。
        title,
        custom_name,
        feed_title: None,
        tags: categories,
        feed_tags,
        favourite,
        lazy,
    })
}

// Unit tests — run with `cargo test`. Each test builds a Feed from a sample
// line and asserts the parse result. `.unwrap()` on Option/Result in tests:
// "this must be Some/Ok, panic with a clear message if not" — acceptable
// because a failure here IS what we want to see.
// 单元测试 — cargo test 运行。每个测试用样例行构造 Feed 并断言解析结果。
// 测试里的 .unwrap()：「这里必须是 Some/Ok，否则带信息崩溃」—
// 测试失败正是我们想看到的，所以可以这么写。
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoted_title() {
        // r#"..."# = raw string literal: backslashes/quotes need no escaping.
        // r#"..."# 原始字符串：内部的引号和反斜杠无需转义。
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
        let f = parse_line(r#"https://x.com/feed.xml "T" blog #tech #rust !favourite !lazy"#).unwrap();
        assert_eq!(f.tags, vec!["blog"]);
        assert_eq!(f.feed_tags, vec!["tech", "rust"]);
        assert!(f.favourite);
        assert!(f.lazy);
        assert!(f.has_tag("tech"));
        assert!(!f.has_tag("blog"));
    }

    #[test]
    fn roundtrip_tags_favourite() {
        let f = parse_line(r#"https://x.com/f "~N" cat #t1 !favourite !lazy"#).unwrap();
        assert_eq!(f.to_line(), r#"https://x.com/f "~N" cat #t1 !favourite !lazy"#);
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
        assert_eq!(
            file.categories_tree(),
            vec![vec!["tech".to_string()], vec!["blog".to_string()]]
        );
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
            file.categories_tree(),
            vec![
                vec!["systems".to_string()],
                vec!["systems".to_string(), "lang".to_string()],
                vec!["dev".to_string()],
                vec!["dev".to_string(), "go".to_string()],
                vec!["blog".to_string()],
            ]
        );
    }
}

#[cfg(test)]
// Second test module for display-name precedence (user name > feed title > URL).
// 第二个测试模块：验证显示名优先级（用户名 > 源标题 > URL）。
mod feed_title_tests {
    use super::*;

    #[test]
    fn display_name_prefers_feed_title_over_url() {
        let f = Feed {
            url: "https://x.com/f".into(),
            title: None,
            custom_name: false,
            feed_title: Some("X Blog".into()),
            tags: vec![],
            feed_tags: vec![],
            favourite: false,
            lazy: false,
        };
        assert_eq!(f.display_name(), "X Blog");
    }

    #[test]
    fn custom_title_beats_feed_title() {
        let f = Feed {
            url: "https://x.com/f".into(),
            title: Some("My Name".into()),
            custom_name: true,
            feed_title: Some("X Blog".into()),
            tags: vec![],
            feed_tags: vec![],
            favourite: false,
            lazy: false,
        };
        assert_eq!(f.display_name(), "My Name");
    }
}
