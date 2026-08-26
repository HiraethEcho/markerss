//! Shared data model — the plain data types every other module passes around.
//! 共享数据模型 — 其他所有模块都会用到的纯数据类型。
//!
//! Rust concepts introduced here:
//! 本文件涉及的 Rust 概念：
//! - `struct`: a bag of named fields, like a C `struct`, but no typedef needed.
//!   `struct`：命名字段的集合，类似 C 的 struct，但不需要 typedef。
//! - `String`: an owned, heap-allocated, growable UTF-8 string.
//!   Think `char*` from `malloc()` — but freed automatically when it goes
//!   out of scope (no `free()`, no leaks, no use-after-free).
//!   `String`：拥有所有权、堆分配、可增长的 UTF-8 字符串。
//!   相当于 malloc 出来的 char*，但离开作用域时自动释放（无需 free）。
//! - `&str`: a borrowed string slice — just a pointer + length into
//!   someone else's string. Like `const char*` that you may not keep alive
//!   past the owner. Cheaper than `String` because nothing is copied.
//!   `&str`：借用的字符串切片 — 指向别人字符串的指针+长度。
//!   类似 const char*，但不能比原字符串活得久。不拷贝，所以便宜。
//! - `#[derive(...)]`: the compiler writes boilerplate trait impls for you.
//!   `#[derive(...)]`：让编译器自动生成常用的 trait 实现（模板代码）。
//! - `impl` block: where methods live; there is no separate header/impl split
//!   like in C.
//!   `impl` 块：方法的定义处；Rust 不像 C 那样分头文件和实现文件。

/// One feed item (article).
/// 一条订阅源条目（一篇文章）。
///
/// All fields are owned (`String`) — an `Item` is self-contained and can be
/// passed between functions, stored in a `Vec`, etc., without lifetime bookkeeping.
/// 所有字段都是拥有所有权的 String — Item 是自包含的，
/// 可以随意在函数间传递、存入 Vec，不需要操心生命周期。
#[derive(Debug, Clone)] // Debug: printable with {:?}; Clone: .clone() makes a deep copy
                        // Debug：可用 {:?} 打印；Clone：调用 .clone() 得到深拷贝
pub struct Item {
    /// Globally unique id from the feed — used as DB key.
    /// 来自源的全局唯一 id — 用作数据库主键。
    pub guid: String,
    pub title: String,
    pub url: String,
    /// Short summary shown in the list view.
    /// 列表视图中显示的摘要。
    pub summary: String,
    /// Full content from the feed, if the feed provides it.
    /// 文章全文（如果源提供了的话）。
    pub content: String,
    /// ISO-8601 published date; empty if unknown.
    /// ISO-8601 格式的发布日期；未知则为空串。
    pub date: String,
    /// Author(s) from the feed, comma-joined; empty if unknown.
    /// 作者（多个用逗号连接）；未知则为空串。
    pub author: String,
    /// Read flag (item-level, from DB).
    /// 已读标记（条目级，来自数据库）。
    pub read: bool,
    /// Read-later flag (item-level).
    /// 稍后读标记（条目级）。
    pub read_later: bool,
    /// Saved flag (item-level, exempt from TTL).
    /// 收藏标记（条目级，不受缓存 TTL 清理影响）。
    pub saved: bool,
}

impl Item {
    /// Returns the title to display, substituting "untitled" when blank.
    /// 返回用于显示的标题，空标题时回退为 "untitled"。
    ///
    /// Rust concepts:
    /// - `&self`: method takes an immutable borrow of the instance — like a
    ///   C function taking `const Item *self`, but the compiler *proves*
    ///   nobody mutates the item while we hold the borrow.
    ///   `&self`：方法不可变借用实例自身 — 类似 C 的 `const Item *self`，
    ///   但编译器能证明借用期间没人修改它。
    /// - Return type `&str` is tied to `&self`'s lifetime automatically:
    ///   we hand back a view into our own title, never a copy.
    ///   返回值 `&str` 的生命周期自动绑定到 `&self`：
    ///   返回的是自身 title 的视图，不做任何拷贝。
    ///
    /// Note both branches yield `&str` but different kinds: one is a
    /// `'static` string literal, one borrows `self.title`. The compiler
    /// unifies them to the shorter (self's) lifetime. In C you'd worry about
    /// returning a pointer to a local; here ownership rules make it safe.
    /// 注意两个分支都是 &str 但来源不同：一个是 'static 字面量，
    /// 一个借用 self.title。编译器统一成较短的生命周期。
    /// 在 C 里返回局部变量指针是未定义行为；这里所有权规则保证安全。
    pub fn display_title(&self) -> &str {
        if self.title.is_empty() {
            "untitled" // &'static str literal — baked into the binary
                       // 'static 字符串字面量 — 编译期写死在二进制里
        } else {
            &self.title // auto-coerce: &String -> &str (deref coercion)
                        // 自动转换：&String -> &str（解引用强制转换）
        }
    }
}
