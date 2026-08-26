# GUIDE — Learn Rust with markerss

This document is your roadmap: what this project is, how it works, and how
each part of it teaches you Rust — assuming you know a little C and almost
no Rust. Pair it with the reading order in [README.md](README.md).

# 指南 — 用 markerss 学 Rust

本文是你的路线图：这个项目是什么、怎么工作、每个部分教你什么 Rust 知识。
假设你会一点 C、几乎不会 Rust。配合 README.md 的阅读顺序使用。

---

## 1. What this project is

markerss is a terminal RSS reader (TUI). It fetches RSS/Atom feeds over
HTTP, parses them, stores articles in SQLite, and renders a three-pane
interface: feed tree | article list | article reader. It also does OPML
import/export, read-later/saved flags, and exports articles as markdown.

## 1. 这个项目是什么

markerss 是一个终端 RSS 阅读器（TUI）。它通过 HTTP 抓取 RSS/Atom 源，
解析后存入 SQLite，渲染三栏界面：订阅源树 | 文章列表 | 阅读器。
还支持 OPML 导入导出、稍后读/收藏标记、文章导出为 markdown。

## 2. Architecture in one picture

```
 opml.rs ──导入/导出──┐
                      ▼
 fetch.rs ──HTTP──► main.rs (App state) ──► ui.rs (render)
   │                  │    │
   │parse (feed-rs)   │    └── keys.rs (key dispatch)
   ▼                  ▼
 model.rs (Item)    db.rs (SQLite)
                    config.rs (TOML settings)
                    util.rs / xdg.rs / clipboard.rs (helpers)
```

Data flow: startup loads config + DB → builds feed list → user browses;
refresh spawns background threads that fetch feeds, parse into `Item`
structs, and send them back to the UI thread through a channel.
数据流：启动时加载配置和数据库 → 构建源列表 → 用户浏览；
刷新时后台线程抓取并解析为 `Item`，再通过通道发回 UI 线程。

In C you would pass `Item*` pointers around and pray nothing dangles or
leaks. In Rust the compiler proves every pointer valid at compile time —
that is the single biggest idea of the language, and everything below
serves it.
C 里你到处传 Item* 指针，祈祷没有悬垂或泄漏。Rust 在编译期证明每个指针
有效 — 这是该语言最核心的思想，下面的内容都围绕它展开。

## 3. Toolchain / 工具链

```sh
cargo new hello      # create a project (Cargo = build tool + pkg manager,
                     # ≈ make + autotools + pip in one)
cargo check          # type-check fast, no binary — run this constantly
cargo run            # build + run
cargo build --release# optimized build
cargo test           # run unit tests
rustup doc --std     # offline stdlib docs
```

Dependencies live in `Cargo.toml` (≈ Makefile deps + version pins). One
`cargo add ratatui` pulls and versions a library — no header hunting.

依赖写在 Cargo.toml 里（相当于 Makefile 依赖 + 版本锁定）。
一条 cargo add 即可引入库，不用找头文件。

## 4. Core concepts, each tied to real code in src/

Read each section, then open the named file and find the pattern live.

每节对应 src/ 里的真实代码。先读概念，再到文件里找实例。

### 4.1 Variables, mutability, shadowing — `src/util.rs`

```rust
let out = String::new();     // immutable by default! (like const)
let mut out2 = String::new();// mut = can change
out2.push('x');
let out = out.trim();        // SHADOWING: rebind same name, even new type.
                             // Old value still exists until scope ends.
```

C has one namespace per scope and `const`; Rust makes immutability the
default and lets you reuse names freely (`let` again shadows).
C 每个作用域一套命名 + const；Rust 默认不可变，且允许同名重新绑定（遮蔽）。

### 4.2 `String` vs `&str` — everywhere; see `src/model.rs`

```rust
pub struct Item { pub title: String }        // owned: heap buffer, auto-freed
pub fn display_title(&self) -> &str { ... }  // borrowed: pointer+len view
```

- `String` ≈ `char*` from `malloc`, but grows itself and frees itself at
  end of scope. No `free()`.
- `&str` ≈ `const char*` view into someone else's string. Cannot outlive
  the owner — compiler enforces it ("lifetimes").
- Rule of thumb: struct fields take `String` (must own); fn params take
  `&str` (just need to read).
- String ≈ malloc 出来的 char*，自动扩容自动释放；&str ≈ 借用视图，
  不能比原字符串活得久（编译器强制）。经验：结构体字段用 String，
  函数参数用 &str。

### 4.3 Ownership & borrowing — `src/main.rs`, `src/ui.rs`

Three rules:
1. Every value has exactly one owner; dropped (freed) when owner leaves scope.
2. Move: assigning/passing by value transfers ownership — old name unusable.
3. Borrows: `&x` (shared, many allowed) or `&mut x` (exclusive, only one).

```rust
fn render(app: &mut App) { ... }  // exclusive borrow: compiler PROVES no
                                  // other code touches app during the call
```

This replaces entire bug classes: use-after-free, double-free, data races —
all become compile errors instead of 3am segfaults.
三条规则：唯一所有者、赋值即转移所有权、借用分共享/独占。
直接消灭整类 bug：use-after-free、double-free、数据竞争全变编译错误。

### 4.4 `Option<T>` — no more NULL — `src/xdg.rs`

```rust
dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
```

`Option<T>` is `Some(T)` or `None` — a nullable pointer, but the type system
forces you to handle `None` before using the value. NULL dereferences are
impossible. Compare: `if (p == NULL)` discipline in C, now enforced by types.
Option 是"可能为空的值"，但类型系统强制处理 None 才能取值，
空指针解引用从语言层面不可能发生。

### 4.5 `Result<T, E>` and `?` — error handling — `src/db.rs`, `src/main.rs`

```rust
fn main() -> io::Result<()> {
    let conn = Connection::open(path)?;   // ? = "if error, return it now"
}
```

No errno checking after every call, no `goto fail`. `?` propagates errors
upward; `match` handles them where you want. Errors are values in the return
type — the signature tells you what can fail.
`?` 表示"出错就立即返回"，错误是返回类型的一部分，
函数签名直接告诉你哪些调用可能失败。

### 4.6 Enums + match — state machines — `src/keys.rs`, `src/main.rs`

```rust
enum InputMode { Normal, Insert, Prompt }
match mode {
    InputMode::Normal => {...}
    InputMode::Insert => {...}
}   // compiler ERROR if you forget an arm — exhaustiveness checked
```

Like `switch` on a tagged union, but the compiler refuses missing cases.
Variants can carry data (`Some(u32)` above). The whole key-handling layer is
one giant exhaustive match — read it as a map from keystroke to action.
枚举类似带 tag 的 union，match 类似 switch 但强制穷尽所有分支。
keys.rs 整个按键层就是一个大 match：按键 → 动作。

### 4.7 Traits — interfaces without headers — `src/clipboard.rs`, `src/ui.rs`

```rust
use std::io::Write;              // import the trait...
stdout().write_all(bytes)?       // ...to get its methods on stdout()
```

A trait is a set of methods a type can implement (`Debug`, `Clone`,
`Serialize`). `#[derive(Debug, Clone)]` auto-generates common ones. Generic
functions accept any `T: SomeTrait` — like C++ concepts/templates but
checked once, at definition.
trait ≈ 接口：一组方法。derive 自动生成常用实现；
泛型函数用 `T: Trait` 约束能力，定义处检查一次。

### 4.8 Iterators & closures — replace loops — `src/util.rs`, `src/fetch.rs`

```rust
iso.chars().take(10).collect::<String>()   // first 10 chars, chained adapters
feeds.sort_by(|a, b| a.title.cmp(&b.title)) // closure as comparator
```

Lazy chains of transformations instead of index juggling. `|a, b| ...` is a
closure (lambda) capturing its environment. Zero-cost: compiles to the same
loop you'd have written.
链式惰性变换代替下标操作；`|a, b|` 是闭包（lambda），可捕获环境变量。
零开销抽象：编译结果与你手写的循环相同。

### 4.9 Collections — `Vec`, `HashMap` — `src/feedlist.rs`, `src/config.rs`

```rust
let mut items: Vec<Item> = Vec::new();  // growable array, owns its elements
counts.entry(key).or_insert(0);         // HashMap upsert idiom
```

Bounds-checked indexing (panic, not UB, on overflow). No manual realloc.
越界是受控 panic 而不是未定义行为；无需手动 realloc。

### 4.10 Modules & crates — `mod` / `use` — `src/main.rs`

```rust
mod db;            // declare: db.rs becomes a module
use crate::db::Db; // import for use here
```

One file = one module; `main.rs` is the crate root. Visibility: private by
default, `pub`, `pub(crate)` (= visible within this program, like C `static`
but crate-wide). No headers — the compiler reads all files at once.

一个文件一个模块；默认私有，pub 对外，pub(crate) 相当于 crate 级 static。
没有头文件，编译器一次读全部源码。

## 5. Suggested path / 学习路径

1. Read `model.rs` (33 lines) — structs + String/&str.
2. Read `util.rs`, `xdg.rs`, `clipboard.rs` — small wins.
3. Do exercises below on these files before moving on.
4. Continue in the README order; skim `DESIGN` intent only if curious —
   design docs live on the `main` branch.
5. When a comment mentions a concept you don't know, search it in
   `rustup doc --std` or https://doc.rust-lang.org/book/.

1. 先读 model.rs（33 行）：结构体 + String/&str。
2. 再读 util/xdg/clipboard：小文件快速建立信心。
3. 做完下面的练习再继续。
4. 之后按 README 顺序推进。
5. 注释里遇到陌生概念就查 rustup doc 或 The Book。

## 6. Exercises (modify locally, `cargo check` to verify)

1. `model.rs`: add field `lang: String` to `Item`; fix every compile error
   it creates — feel how the compiler tracks data flow for you.
2. `util.rs`: write `slugify` variant keeping digits-only words; add a test
   next to the existing ones.
3. `xdg.rs`: change fallbacks so cache uses `/tmp/markerss-cache`;
   observe `unwrap_or_else` closures.
4. `clipboard.rs`: extend base64 tests with your own RFC 4648 vectors
   (compute expected output by hand — good bit-twiddling practice).
5. `db.rs`: find one query; change `SELECT *` to explicit columns and fix
   fallout. (Harder — do after step 8.)
6. Capstone: write a tiny CLI (`main`-less new bin in `src/bin/`) that uses
   `crate::util::slugify` on argv inputs.

练习（本地修改，用 cargo check 验证）：
1) 给 Item 加字段，跟着编译器修错；2) 改 slugify 并加测试；
3) 改缓存目录回退逻辑；4) 手算 base64 测试向量；5) 改写一条 SQL 查询；
6) 毕业练习：在 src/bin/ 写个小 CLI 调用 util::slugify。

## 7. Glossary C → Rust

| C | Rust | Note |
|---|------|------|
| `malloc/free` | `String::new()` / automatic drop | RAII |
| `char*` | `&str` | borrowed slice |
| `NULL` + if-check | `Option<T>` + match/? | type-enforced |
| `errno` checks | `Result<T,E>` + `?` | errors in signatures |
| `switch` | `match` | exhaustive, patterns |
| header + .c | one `.rs` module | no forward decls |
| `static` fn | `pub(crate)` or private | visibility |
| `const T*` / `T*` | `&T` / `&mut T` | aliasing proven safe |
| make + pkg-config | Cargo + Cargo.toml | one tool |

对照表：C 与 Rust 概念一一映射，见上表。
