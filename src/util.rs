//! Small pure helpers: slugify, date formatting, display width, YAML escaping.
//! 纯函数小工具：slug 生成、日期格式化、显示宽度、YAML 转义。
//!
//! Rust concepts introduced here:
//! 本文件涉及的 Rust 概念：
//! - `pub(crate)`: visible everywhere in *this crate*, but not to outsiders.
//!   Like C's `static` functions (internal linkage), but per-crate not per-file.
//!   `pub(crate)`：仅在本 crate 内可见。类似 C 的 static 函数（内部链接），
//!   但范围是整个 crate 而不是单个文件。
//! - `&str` input / `String` output: the idiomatic signature for "read text,
//!   return new text". Borrow the input (no copy), allocate a fresh owned
//!   result. C equivalent: `char *f(const char *in)` where you must free
//!   the result — here memory is reclaimed automatically.
//!   入参 &str / 返回 String：惯用签名，表示"读文本、产新文本"。
//!   借用输入（不拷贝），返回新的堆上结果。
//! - iterators + closures: `for c in ...`, `.chars()`, `|x| ...` lambdas.
//!   迭代器与闭包：for 循环、.chars()、|x| ... 即 lambda。

/// Escape a string so it is safe inside a double-quoted TOML value.
/// 转义字符串，使其可安全放进双引号包裹的 TOML 值中。
///
/// `.replace()` returns a NEW String each time (strings are immutable here);
/// we chain two passes: backslash first, then quotes — order matters,
/// otherwise the backslashes we add would get doubled again.
/// 每次 .replace() 都会返回新 String；先处理反斜杠再处理引号，
/// 顺序很重要，否则新加的反斜杠会被再次翻倍。
pub(crate) fn escape_yaml(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Turn an arbitrary feed title into a filesystem-safe slug:
/// lowercase alphanumerics, everything else collapses to `-`.
/// 把任意源标题转成文件系统安全的 slug：
/// 小写字母数字，其他字符折叠成 '-'。
pub(crate) fn slugify(title: &str) -> String {
    // String::new() = empty heap string we will grow. Like a malloc'd buffer,
    // but it grows itself and frees itself.
    // String::new() = 空的可增长堆字符串。类似 malloc 的缓冲区，
    // 但会自己扩容、自己释放。
    let mut out = String::new();

    // Rust strings are UTF-8, so indexing by bytes can split a multibyte char.
    // `.chars()` iterates real Unicode scalar values instead — safe.
    // Rust 字符串是 UTF-8，按字节下标可能切断多字节字符，
    // .chars() 按完整 Unicode 字符迭代 — 安全。
    for c in title.chars() {
        if c.is_alphanumeric() {
            out.push(c.to_ascii_lowercase()); // A-Z -> a-z only; 中文 etc. kept as-is
        } else if c.is_whitespace() || c == '-' || c == '_' {
            out.push('-');
        }
        // anything else (punctuation, symbols) is dropped
        // 其他字符（标点、符号）直接丢弃
    }

    // Collapse runs of dashes: "a---b" -> "a-b".
    // Loop because one pass could create new "--" at chunk boundaries.
    // 折叠连续的 '-'："a---b" -> "a-b"。用循环是因为一轮替换
    // 可能在拼接处产生新的 "--"。
    while out.contains("--") {
        out = out.replace("--", "-");
    }

    // Shadowing: this `out` reuses the name but is a different (&str) binding.
    // Legal and idiomatic — the old String still exists, we just borrow it.
    // 变量遮蔽：同名新绑定，类型从 String 变成 &str。
    // 旧 String 还活着，我们只是借用它。
    let out = out.trim_matches('-');

    if out.is_empty() {
        // `.to_string()` copies the borrowed slice into a fresh owned String.
        // .to_string() 把借用的切片拷贝成新的自有 String。
        "untitled".to_string()
    } else {
        out.to_string()
    }
}

/// Format an ISO date ("2025-01-02T...") as just the first 10 chars ("2025-01-02").
/// 把 ISO 日期（"2025-01-02T..."）截取前 10 个字符（"2025-01-02"）。
///
/// Chained iterator adapters instead of a manual loop — closer to how you'd
/// write it with pointer arithmetic in C, but bounds-checked.
/// 链式迭代器适配器代替手写循环 — 类似 C 里指针运算，但有越界检查。
pub(crate) fn fmt_date(iso: &str) -> String {
    iso.chars().take(10).collect()
}

// Unit tests: compiled only with `cargo test` (`#[cfg(test)]`).
// 单元测试：仅在 cargo test 时编译（#[cfg(test)]）。
#[cfg(test)]
mod html_tests {
    use super::*; // import everything from the parent module (this file)
                  // 导入父模块（本文件）的全部内容

}
