# markerss — Rust 学习版

TUI RSS reader — browse feeds in the terminal, store blog posts as markdown
on command. This branch (`rust-comment`) is a **Rust learning fork**: the
working reader plus dense bilingual (English + 中文) teaching comments on
every file, written for someone coming from C.

# markerss — TUI RSS 阅读器

本分支是 **Rust 学习版**：完整可用的阅读器 + 每个文件的双语（英文+中文）
教学注释，面向有少量 C 基础的读者。

> Start with [GUIDE.md](GUIDE.md) — project overview + Rust tutorial tied to
> this codebase. / 从 GUIDE.md 开始：项目概览 + 结合本代码库的 Rust 教程。

## Build & Run / 构建与运行

```sh
cargo run            # debug build + launch / 调试构建并启动
cargo build --release   # optimized binary in target/release/markerss
```

Config lives in `~/.config/markerss/` (`config.default.toml` and
`theme.default.toml` are templates). Cache/data in `~/.cache/markerss/` and
`~/.local/share/markerss/`. Supports OPML import/export.

## How to read the code / 阅读顺序

Files are commented so concepts build on each other. Read in this order:

文件注释按概念递进编写，建议按此顺序阅读：

| # | File | Lines* | What it teaches / 学习内容 |
|---|------|--------|---------------------------|
| 1 | `src/model.rs` | 33 | structs, `String` vs `&str`, derive |
| 2 | `src/util.rs` | 36 | iterators, closures, shadowing |
| 3 | `src/xdg.rs` | 21 | `Option<T>`, lazy defaults |
| 4 | `src/clipboard.rs` | 49 | traits, byte strings, bit ops |
| 5 | `src/opml.rs` | 223 | enums + match, error handling |
| 6 | `src/fetch.rs` | 233 | external crates, HTTP, iterators |
| 7 | `src/feedlist.rs` | 462 | Vec state management |
| 8 | `src/db.rs` | 597 | rusqlite, prepared statements, `?` |
| 9 | `src/config.rs` | 791 | serde derive, Default impl |
| 10 | `src/ui.rs` | 701 | ratatui rendering, borrowing |
| 11 | `src/keys.rs` | 1021 | exhaustive match, state machines |
| 12 | `src/main.rs` | 1442 | RAII guards, threads, event loop |

\* code lines before comments were added

Each file starts with a `//!` header listing the Rust concepts it shows;
every function has a doc comment; tricky lines explain ownership/borrowing
inline with C analogies. 中文注释紧跟在每行英文之下。

## Comment style / 注释约定

- `//!` module header — purpose + concept list
- `///` on every struct/fn — what + data flow
- `//` inline at teaching moments — ownership, borrows, `Result`/`?`,
  traits, lifetimes…
- Code is never modified for comment's sake; behavior is untouched.

## Repo layout / 仓库结构

- `main` branch — design docs (SPEC/PLAN/DESIGN), no code
- this branch — implementation + teaching comments
- Upstream: `rust` branch (clean implementation, no learning comments)
