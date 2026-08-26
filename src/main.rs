//! markerss — TUI RSS reader.
//! 标记式 RSS 阅读器（TUI 终端界面）。
//!
//! This file is the application entry point and the heart of the app:
//! the `App` state machine, background-thread message handling, and the
//! terminal event loop (`main` → `run`). Key handling lives in keys.rs
//! as a second `impl App` block — Rust allows splitting a type's methods
//! across files within the same crate.
//! 本文件是程序入口和应用核心：App 状态机、后台线程消息处理、
//! 终端事件循环（main → run）。按键处理在 keys.rs 的另一个 impl App 块中 —
//! Rust 允许同一类型的方法分散在同 crate 的多个文件里。
//!
//! Rust concepts demonstrated here:
//! 本文件涉及的 Rust 概念：
//! - `fn main() -> io::Result<()>`: main can return a Result; `?` propagates
//!   errors up automatically (no errno checking, no goto fail).
//!   main 可返回 Result；`?` 自动向上传播错误（无需逐个检查错误码）。
//! - RAII guard pattern: `ratatui::init()` puts the terminal in raw mode and
//!   returns a guard; `restore()` undoes it — like free-on-return/atexit,
//!   but explicit and paired.
//!   RAII 守卫模式：init 进入 raw 模式，restore 恢复 — 类似退出时自动清理，
//!   成对显式调用。
//! - `std::sync::mpsc`: multi-producer/single-consumer channel for talking
//!   from worker threads back to the UI thread without mutexes.
//!   mpsc 通道：工作线程向 UI 线程回传消息，无需互斥锁。
//! - enums as state machines: `Scope`, `InputMode`, `PendingInput` each carry
//!   data per variant (tagged unions, like a C union + tag — but checked).
//!   枚举作状态机：每个变体可携带数据（类似 C 的 union+tag，但有编译期检查）。
//! - `&mut self`: exclusive mutable borrow — like passing `App *` to C, except
//!   the compiler proves at compile time that nothing else aliases it.
//!   &mut self：独占可变借用 — 相当于传 App*，但编译器证明无别名。
//! - `Vec` management: build → sort → clamp index patterns throughout.
//!   Vec 管理：构建 → 排序 → 索引钳制，贯穿全文件。
//!
//! Three panes: nav tree (categories → feeds) | item list | article.
//! Storage: SQLite (items + read state). Content: feed HTML → markdown →
//! styled Text (eilmeldung-style pipeline). Design authority: DESIGN.md.

mod clipboard;
mod config;
mod db;
mod feedlist;
mod fetch;
mod keys;
mod model;
mod opml;
mod ui;
mod util;
mod xdg;

use crate::ui::render;
use crate::util::{escape_yaml, slugify};
use ratatui::layout::Rect;

use std::io;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use model::Item;

use crate::config::{Config, ThemeColors};
use crate::db::Db;
use crate::feedlist::{Feed, File};

// ─── worker messages ────────────────────────────────────────────────────────

// Messages sent over the mpsc channel from background threads to the UI loop.
// Network fetches must NOT run on the UI thread (they would freeze drawing),
// so each fetch spawns a thread; results come back packaged as one of these.
// 后台线程经 mpsc 通道发给 UI 循环的消息。网络抓取不能在 UI 线程跑
// （会卡住绘制），所以每次抓取起一个线程，结果打包成以下变体传回。
enum Msg {
    // A feed worker finished. result carries (feed_title, items) or an error
    // string — errors are VALUES here, not exceptions/signal handlers.
    // 源刷新线程完成。result 携带 (源标题， 条目) 或错误字符串 —
    // 错误是普通值，不是异常或信号。
    FeedRefreshed {
        url: String,
        result: Result<(Option<String>, Vec<Item>), String>,
        full: bool,
    },
    ArticleFetched { url: String, guid: String, result: Result<String, String> },
    // timer thread's heartbeat: "time to auto-refresh"
    // 定时线程的心跳：该自动刷新了
    RefreshTick,
}

// Which text-input prompt is currently on screen. `Copy` means assignments
// duplicate it bitwise (it holds no heap data) — like copying a small C enum.
// 当前屏幕上的输入提示属于哪种模式。Copy 表示赋值时按位复制
// （不含堆数据）— 类似拷贝一个小 C 枚举。
#[derive(Debug, Clone, Copy, PartialEq)]
enum InputMode {
    AddUrl,
    AddTitle,
    AddCategory,
    AddTags,
    EditTags,
    RenameCategory,
    EditFeedTitle,
    EditTag,
    ExportFile,
    ExportSavedFile,
    ImportOpml,
    Search,
}

/// One line of user input being edited (prompt text + typed buffer).
/// Wrapped in `Option<App.input>`: None = no prompt on screen.
/// 一行正在编辑的用户输入（提示文本 + 已输入缓冲）。
/// 包在 Option 里：None = 屏幕上没有输入框。
struct InputPrompt {
    mode: InputMode,
    prompt: String,
    buf: String,
}

/// In-flight wizard state for multi-step input flows (add feed, edit tags,
/// export). One field instead of five parallel options.
enum PendingInput {
    AddFeed {
        url: String,
        title: Option<String>,
        category: Option<Vec<String>>,
    },
    EditTags { url: String },
    Export { feed_url: String, guid: String },
    ExportSaved,
}

// ─── app state ──────────────────────────────────────────────────────────────

/// What the middle list pane is currently showing. A tagged union:
/// simple views carry nothing; scoped views carry the key (String).
/// Like `struct { int kind; union { char *cat; char *feed; ... } }` in C,
/// but reading the wrong union member is a compile error, not UB.
/// 中间列表窗格当前显示什么。带标签的联合：简单视图不带数据，
/// 范围视图带键（String）。C 写法是 struct{kind + union}，
/// 但这里取错联合成员会编译报错，而非未定义行为。
#[derive(Debug, Clone, PartialEq)]
enum Scope {
    AllUnread,
    ReadLater,
    Saved,
    Favourite,
    Lazy,
    Category(String),
    Feed(String),
    Tag(String),
}

/// All mutable app state in ONE struct. There is exactly one `App` owner
/// (the `main` fn); the event loop borrows it as `&mut App` each iteration.
/// That single-owner + exclusive-borrow model is why no locks are needed:
/// worker threads never touch `App`, they only send `Msg`s.
/// 所有可变应用状态集中在这一个结构体。全局只有一个所有者（main），
/// 事件循环每轮以 &mut App 独占借用。单所有者 + 独占借用 = 不需要锁：
/// 工作线程从不碰 App，只发 Msg 消息。
struct App {
    cfg: Config,
    theme: ThemeColors,
    feeds: File,
    db: Db,

    // nav tree
    collapsed: std::collections::HashSet<String>,
    fav_expanded: bool,
    lazy_expanded: bool,
    preset_idx: usize,
    uncat_expanded: bool,
    tree_sel: usize,
    /// Viewport offset for the nav tree — sticky (same rule as list).
    /// 导航树视口偏移 — 粘性（与 list 同规则）。
    tree_offset: usize,
    tree_rows: Vec<TreeRow>,

    // list
    scope: Scope,
    list_sel: usize,
    /// Viewport offset for the item list — sticky: the window only moves when
    /// the selection crosses an edge (stays still when scrolling up from the
    /// bottom until the selection reaches the top of the window).
    list_offset: usize,
    scoped_items: Vec<(String, Item)>, // (feed_url, item)

    // in-memory unread/flag counts, refreshed via `recount` — avoids per-frame SQL
    unread: std::collections::HashMap<String, usize>,
    total_unread: usize,
    later_count: usize,
    saved_count: usize,

    // article
    article_scroll: u16,
    fetching: bool,
    // rendered article body, keyed by item guid (avoid per-frame conversion)
    article_render: Option<((String, String, bool), ratatui::text::Text<'static>)>,

    focus: usize, // 0 nav, 1 list, 2 article
    fullscreen: bool,
    delete_armed: bool,
    help_scroll: u16,
    status: String,
    show_help: bool,
    pending_refreshes: usize,
    article_area: Rect,
    running: bool,
    pending_keys: Vec<KeyCode>,
    sort_stack: Vec<(String, bool)>,
    search_base: Option<Vec<(String, Item)>>,
    search_active: bool,
    search_query: String,
    keymap: std::collections::HashMap<Vec<KeyCode>, crate::keys::Action>,
    feed_errors: std::collections::HashMap<String, String>,
    input: Option<InputPrompt>,
    pending: Option<PendingInput>,
    rx: Receiver<Msg>,
    tx: Sender<Msg>,
}

// A row in the rendered nav tree. The tree is REBUILT from scratch whenever
// feeds/categories change (see rebuild_tree) — selection is an index into
// this flat Vec, so folding/expanding is just re-generating rows.
// 导航树的每一行。feeds/分类一变就整体重建树（见 rebuild_tree）—
// 选中项只是这个扁平 Vec 的下标，折叠/展开就是重新生成行。
#[derive(Debug, Clone)]
enum TreeRow {
    Section(String),
    AllUnread,
    ReadLater,
    Saved,
    Favourite,
    FavouriteFeed(String, String), // url, display name
    Lazy,
    LazyFeed(String, String), // url, display name
    Uncategorized,
    UncategorizedFeed(String, String), // url, display name
    Category(String),
    Feed(String, String, u8), // url, display name, indent
    Tag(String),
}

impl App {
    /// Constructor: load config-derived files, open the DB, create the
    /// mpsc channel, zero every counter. Returns a fully-owned `App` —
    /// no pointers, no partial init, unlike C where you malloc + check NULL
    /// + remember to init every field by hand (miss one = garbage).
    /// 构造函数：加载配置相关文件、打开 DB、建 mpsc 通道、计数清零。
    /// 返回完整拥有的 App — 不像 C 里 malloc + 判空 + 手动逐字段初始化，
    /// 漏一个就是脏数据；这里编译器强制初始化所有字段。
    fn new(cfg: Config) -> App {
        // Load-or-default pattern: missing file → empty feed list, not an error.
        // 缺文件时回退为空列表而不是报错。
        let feeds = File::load_or_default(&cfg.urls_path);
        // .expect() = unwrap with a message; panics (crashes) on None/Err.
        // Acceptable here: without a DB the app cannot run at all.
        // expect() = 带信息的 unwrap；None/Err 时 panic（崩溃）。
        // 这里可接受：没有 DB 应用根本跑不了。
        let db = Db::open(&cfg.db_path).expect("open sqlite db");
        // mpsc::channel() returns a (Sender, Receiver) pair. Cloning the Sender
        // gives each worker thread its own handle to the SAME queue — this is
        // the "multi-producer" part of multi-producer/single-consumer.
        // mpsc::channel() 返回 (发送端, 接收端)。克隆 Sender 给每个工作线程
        // 各自的句柄，指向同一队列 — 这就是多生产者的含义。
        let (tx, rx) = mpsc::channel();
        let theme = ThemeColors::load(cfg.theme_path.as_ref(), cfg.background);
        let sort_stack: Vec<(String, bool)> =
            cfg.sort.iter().map(|s| (s.clone(), false)).collect();
        let keymap = crate::keys::build_keymap(&cfg.keybindings);
        // Struct literal: EVERY field must be listed exactly once — the
        // compiler errors on missing or duplicated fields. Field order in the
        // literal need not match the declaration order.
        // 结构体字面量：每个字段必须恰好出现一次，缺了重了都编译报错。
        // 字段书写顺序不必与声明顺序一致。
        let mut app = App {
            cfg,
            theme,
            feeds,
            db,
            collapsed: Default::default(),
            fav_expanded: true,
            lazy_expanded: true,
            preset_idx: 0,
            uncat_expanded: true,
            tree_sel: 0,
            tree_offset: 0,
            tree_rows: Vec::new(),
            scope: Scope::AllUnread,
            list_sel: 0,
            list_offset: 0,
            scoped_items: Vec::new(),
            unread: std::collections::HashMap::new(),
            total_unread: 0,
            later_count: 0,
            saved_count: 0,
            article_scroll: 0,
            fetching: false,
            article_render: None,
            focus: 0,
            fullscreen: false,
            delete_armed: false,
            help_scroll: 0,
            status: String::new(),
            show_help: false,
            pending_refreshes: 0,
            article_area: Rect::default(),
            running: true,
            pending_keys: Vec::new(),
            sort_stack,
            search_base: None,
            search_active: false,
            search_query: String::new(),
            keymap,
            feed_errors: std::collections::HashMap::new(),
            input: None,
            pending: None,
            rx,
            tx,   // field-init shorthand: local `tx` moves into field `tx`
                  // 字段初始化简写：局部变量 tx 直接移入同名字段
        };
        // Three passes over freshly-loaded data before first draw.
        // 首帧绘制前的三次数据整理。
        app.rebuild_tree();
        app.rebuild_list();
        app.recount();
        app
    }

    /// Refresh in-memory unread/flag counts from the DB (one grouped query).
    fn recount(&mut self) {
        self.unread = self.db.unread_counts().unwrap_or_default();
        self.total_unread = self.db.total_unread().unwrap_or(0);
        self.later_count = self.db.flag_count("read_later").unwrap_or(0);
        self.saved_count = self.db.flag_count("saved").unwrap_or(0);
    }

    /// Cached per-feed unread count for the nav pane.
    /// 导航窗格用的每源未读数（带缓存）。
    fn unread(&self, url: &str) -> usize {
        // HashMap::get returns Option<&V>; .copied() turns Option<&usize>
        // into Option<usize> (usize is Copy — bitwise copy is fine);
        // unwrap_or(0) supplies the default for a missing key.
        // get 返回 Option<&V>；copied() 转成 Option<usize>（usize 是 Copy 型，
        // 按位复制即可）；unwrap_or(0) 给缺失的键提供默认值。
        self.unread.get(url).copied().unwrap_or(0)
    }

    /// In-place reflect a read/later change in the list snapshot (no rebuild).
    /// 原地更新列表快照中的已读/稍后读状态（不重建列表）。
    fn mark_scoped_read(&mut self, feed_url: &str, guid: &str, clear_later: bool) {
        for (u, i) in self.scoped_items.iter_mut() {
            if u == feed_url && i.guid == guid {
                i.read = true;
                if clear_later {
                    i.read_later = false;
                }
            }
        }
    }

    // ── tree ──────────────────────────────────────────────────────────────

    /// Regenerate the flat row list for the nav pane from cfg preset +
    /// feeds + fold state. Full rebuild each call — simple, and cheap at
    /// this scale (hundreds of rows). C analogy: rebuilding a linked list
    /// instead of surgically patching nodes.
    /// 根据预设 + 源列表 + 折叠状态，重新生成导航窗格的扁平行列表。
    /// 每次全量重建 — 简单且规模小（几百行）时开销可忽略。
    fn rebuild_tree(&mut self) {
        // Pick the active nav preset; fall back to the built-in default when
        // none is configured. .cloned() on Option<&Vec<String>> gives an owned Vec.
        // 选当前导航预设；未配置则回退内置默认。cloned() 把 Option<&Vec> 变成自有 Vec。
        let preset = self
            .cfg
            .nav_presets
            .get(self.preset_idx)
            .cloned()
            .unwrap_or_else(|| {
                crate::config::DEFAULT_NAV_PRESET
                    .iter()
                    .map(|s| s.to_string())
                    .collect()
            });
        let mut rows: Vec<TreeRow> = Vec::new();
        // Build rows section by section. Pattern used throughout: push a
        // header row, then (if not collapsed) push child rows. TreeRow variants
        // carry what render() needs (url, name, indent) so no lookups later.
        // 逐段构建行。通用模式：先推表头行，若未折叠再推子行。
        // TreeRow 变体自带渲染所需数据（url、名称、缩进），后续无需再查。
        for section in preset {
            // single-node sections (Unread / Read Later / Saved / Favourite)
            // render as the node itself; list sections get a foldable header
            let is_node_section = matches!(
                section.as_str(),
                "Unread" | "Read Later" | "Saved" | "Favourite" | "Lazy"
            );
            if !is_node_section {
                rows.push(TreeRow::Section(section.clone()));
            }
            let section_collapsed = self.collapsed.contains(&section);
            // No Category keeps its own fold state — visible even when the
            // Categories section is folded; sits after the category tree when
            // the section is expanded
            let uncat = !self.feeds.uncategorized().is_empty();
            if section == "Categories" && section_collapsed && uncat {
                rows.push(TreeRow::Uncategorized);
                if self.uncat_expanded {
                    for f in self.feeds.uncategorized() {
                        rows.push(TreeRow::UncategorizedFeed(
                            f.url.clone(),
                            f.display_name().to_string(),
                        ));
                    }
                }
            }
            if section_collapsed {
                continue;
            }
            match section.as_str() {
                // Each arm fills rows for one section type. The Categories arm
                // is the deep one; the rest follow the same push-header-then-
                // children pattern as above (not repeated per arm).
                // 每个 arm 填充一种段落。Categories 分支最深；其余分支与上面
                // 相同的“表头+子行”模式（不再逐行注释）。
                "Unread" => rows.push(TreeRow::AllUnread),
                "Read Later" => rows.push(TreeRow::ReadLater),
                "Saved" => rows.push(TreeRow::Saved),
                "Favourite" => {
                    rows.push(TreeRow::Favourite);
                    if self.fav_expanded {
                        for f in self.feeds.feeds.iter().filter(|f| f.favourite) {
                            rows.push(TreeRow::FavouriteFeed(
                                f.url.clone(),
                                f.display_name().to_string(),
                            ));
                        }
                    }
                }
                "Lazy" => {
                    rows.push(TreeRow::Lazy);
                    if self.lazy_expanded {
                        for f in self.feeds.feeds.iter().filter(|f| f.lazy) {
                            rows.push(TreeRow::LazyFeed(
                                f.url.clone(),
                                f.display_name().to_string(),
                            ));
                        }
                    }
                }
                "Categories" => {
                    for path in self.feeds.categories_tree() {
                        // path is e.g. ["tech", "rust"] — join for display,
                        // len() gives nesting depth.
                        // path 形如 ["tech", "rust"] — 拼接用于显示，len() 即嵌套深度。
                        let joined = path.join("/");
                        let depth = path.len();
                        // hidden when any ancestor is collapsed
                        let mut ancestor_folded = false;
                        for i in 1..depth {
                            if self.collapsed.contains(&path[..i].join("/")) {
                                ancestor_folded = true;
                                break;
                            }
                        }
                        if ancestor_folded {
                            continue;
                        }
                        rows.push(TreeRow::Category(joined.clone()));
                        if !self.collapsed.contains(&joined) {
                            for f in self.feeds.by_category_path(&path) {
                                rows.push(TreeRow::Feed(
                                    f.url.clone(),
                                    f.display_name().to_string(),
                                    (depth * 2 + 2) as u8,
                                ));
                            }
                        }
                    }
                    // expanded section: No Category trails the tree
                    if uncat {
                        rows.push(TreeRow::Uncategorized);
                        if self.uncat_expanded {
                            for f in self.feeds.uncategorized() {
                                rows.push(TreeRow::UncategorizedFeed(
                                    f.url.clone(),
                                    f.display_name().to_string(),
                                ));
                            }
                        }
                    }
                }
                "Tags" => {
                    for t in self.feeds.all_feed_tags() {
                        rows.push(TreeRow::Tag(t.clone()));
                        if !self.collapsed.contains(&format!("tag:{t}")) {
                            for f in self.feeds.feeds.iter().filter(|f| f.has_tag(&t)) {
                                rows.push(TreeRow::Feed(
                                    f.url.clone(),
                                    f.display_name().to_string(),
                                    4,
                                ));
                            }
                        }
                    }
                }
                "Feeds" => {
                    for f in self.feeds.feeds.iter() {
                        rows.push(TreeRow::Feed(f.url.clone(), f.display_name().to_string(), 2));
                    }
                }
                _ => {}
            }
        }
        self.tree_rows = rows;
        // Clamp selection into bounds after a rebuild — saturating_sub
        // returns 0 instead of panicking/underflowing when the Vec is empty
        // (in C, 0 - 1 on unsigned wraps around silently).
        // 重建后把选中下标钳回有效范围 — 空列表时 saturating_sub 返回 0，
        // 不像 C 的无符号下溢会静默回绕。
        if self.tree_sel >= self.tree_rows.len() {
            self.tree_sel = self.tree_rows.len().saturating_sub(1);
        }
    }

    /// Rotate to the next configured nav layout and rebuild.
    /// 轮换到下一个导航布局并重建树。
    fn cycle_preset(&mut self) {
        if self.cfg.nav_presets.len() > 1 {
            self.preset_idx = (self.preset_idx + 1) % self.cfg.nav_presets.len();
            self.rebuild_tree();
        }
    }

    /// Drop any active search when the scope changes (its base snapshot is stale).
    /// 切换范围时丢弃进行中的搜索（其基准快照已过期）。
    fn clear_search(&mut self) {
        self.search_base = None;
        self.search_active = false;
        self.search_query.clear();
        if let Some(p) = &self.input {
            if p.mode == InputMode::Search {
                self.input = None;
            }
        }
    }

    /// Enter the nav selection: set scope, reset list cursor, rebuild.
    /// 进入导航选中项：设置范围、重置列表光标、重建列表。
    fn select_scope(&mut self, row: &TreeRow) {
        // Section headers are not selectable — matches! is an enum-variant
        // pattern test (like `switch` on the tag, returning bool here).
        // 段落表头不可选。matches! 是枚举变体模式匹配（类似 switch 判 tag，返回 bool）。
        if matches!(row, TreeRow::Section(_)) {
            return;
        }
        // Map a TreeRow to its Scope. Match must be exhaustive — every variant
        // handled or the compiler errors (no forgotten `case` like C switch).
        // 把 TreeRow 映射到 Scope。match 必须穷尽所有变体，漏了编译不过 —
        // 不会出现 C switch 忘写 case 的 bug。
        self.scope = match row {
            TreeRow::Section(_) => return,
            TreeRow::AllUnread => Scope::AllUnread,
            TreeRow::ReadLater => Scope::ReadLater,
            TreeRow::Saved => Scope::Saved,
            TreeRow::Category(c) => Scope::Category(c.clone()),
            TreeRow::Feed(url, _, _) => Scope::Feed(url.clone()),
            TreeRow::FavouriteFeed(url, _) => Scope::Feed(url.clone()),
            TreeRow::LazyFeed(url, _) => Scope::Feed(url.clone()),
            TreeRow::UncategorizedFeed(url, _) => Scope::Feed(url.clone()),
            TreeRow::Tag(t) => Scope::Tag(t.clone()),
            TreeRow::Favourite => Scope::Favourite,
            TreeRow::Lazy => Scope::Lazy,
            TreeRow::Uncategorized => Scope::AllUnread,
        };
        self.list_sel = 0;
        self.list_offset = 0;
        self.clear_search();
        self.rebuild_list();
        // focus moves to the list pane after picking from nav
        // 从导航选择后焦点移到列表窗格
        self.focus = 1;
    }

    /// Follow nav selection into the list pane (preview) without moving focus.
    /// 预览模式：跟随导航选中项刷新列表，但不移动焦点。
    fn preview_scope(&mut self) {
        // .get() returns Option<&TreeRow> (None on out-of-range index instead
        // of UB like C's arr[i]); .cloned() copies the row out so we can
        // mutate self.scope while holding it — borrowing rules forbid holding
        // a & into self while also calling &mut self methods.
        // get 越界返回 None 而非 C 数组越界的未定义行为；cloned() 把行拷出来，
        // 避免借用 self 的同时再可变借用 self（借用规则禁止）。
        if let Some(row) = self.tree_rows.get(self.tree_sel).cloned() {
            if matches!(row, TreeRow::Section(_)) {
                return;
            }
            self.scope = match row {
                TreeRow::Section(_) => return,
                TreeRow::AllUnread => Scope::AllUnread,
                TreeRow::ReadLater => Scope::ReadLater,
                TreeRow::Saved => Scope::Saved,
                TreeRow::Category(c) => Scope::Category(c),
                TreeRow::Feed(url, _, _) => Scope::Feed(url),
                TreeRow::FavouriteFeed(url, _) => Scope::Feed(url),
                TreeRow::LazyFeed(url, _) => Scope::Feed(url),
                TreeRow::UncategorizedFeed(url, _) => Scope::Feed(url),
                TreeRow::Tag(t) => Scope::Tag(t),
                TreeRow::Favourite => Scope::Favourite,
                TreeRow::Lazy => Scope::Lazy,
                TreeRow::Uncategorized => Scope::AllUnread,
            };
            self.list_sel = 0;
            self.list_offset = 0;
            self.clear_search();
            self.rebuild_list();
        }
    }

    // ── list ──────────────────────────────────────────────────────────────

    /// Re-query the DB for everything in the current scope and replace the
    /// in-memory snapshot `scoped_items`. Each entry is (feed_url, Item) —
    /// the url travels with the item because the DB keys items by (url, guid).
    /// 按当前范围重新查库，整体替换内存快照 scoped_items。
    /// 每项是 (源url, 条目) — url 随条目一起走，因为 DB 以 (url,guid) 为键。
    fn rebuild_list(&mut self) {
        let mut items: Vec<(String, Item)> = Vec::new();
        // One match arm per scope. Pattern repeated in several arms:
        // for each feed in scope → query its items → push. Commented once here;
        // arms below follow the same shape.
        // 每个范围一个分支。多处分支重复同一模式：遍历范围内的源 → 查条目 → 推入。
        // 此处注释一次，下方同型分支不再重复。
        match &self.scope {
            Scope::AllUnread => {
                for f in &self.feeds.feeds {
                    if let Ok(list) = self.db.items_for_feed(&f.url) {
                        for i in list {
                            // startup/refresh view = unread only
                            if !i.read {
                                items.push((f.url.clone(), i));
                            }
                        }
                    }
                }
            }
            Scope::ReadLater => items = self.db.items_with_flag("read_later").unwrap_or_default(),
            Scope::Saved => items = self.db.items_with_flag("saved").unwrap_or_default(),
            Scope::Favourite => {
                for f in self.feeds.feeds.iter().filter(|f| f.favourite) {
                    if let Ok(list) = self.db.items_for_feed(&f.url) {
                        for i in list {
                            items.push((f.url.clone(), i));
                        }
                    }
                }
            }
            Scope::Lazy => {
                for f in self.feeds.feeds.iter().filter(|f| f.lazy) {
                    if let Ok(list) = self.db.items_for_feed(&f.url) {
                        for i in list {
                            items.push((f.url.clone(), i));
                        }
                    }
                }
            }
            Scope::Category(cat) => {
                for f in self.feeds.by_category(cat) {
                    if let Ok(list) = self.db.items_for_feed(&f.url) {
                        for i in list {
                            items.push((f.url.clone(), i));
                        }
                    }
                }
            }
            Scope::Feed(url) => {
                if let Ok(list) = self.db.items_for_feed(url) {
                    for i in list {
                        items.push((url.clone(), i));
                    }
                }
            }
            Scope::Tag(tag) => {
                for f in self.feeds.feeds.iter().filter(|f| f.has_tag(tag)) {
                    if let Ok(list) = self.db.items_for_feed(&f.url) {
                        for i in list {
                            items.push((f.url.clone(), i));
                        }
                    }
                }
            }
        }
        // stable order (date desc, as inserted by refresh) — never re-sort
        // on read toggles so the selection stays on the same item
        // sort_by takes a closure |a, b| — like qsort's comparator fn pointer,
        // but inline and able to capture environment. Tuple indexing: .1 is
        // the second element (the Item); cmp on strings is lexicographic.
        // sort_by 接收闭包 — 类似 qsort 比较函数指针，但可内联且能捕获环境。
        // 元组 .1 取第二个元素（Item）；字符串按字典序比较。
        items.sort_by(|a, b| b.1.date.cmp(&a.1.date));
        self.scoped_items = items;
        // explicit sort stack (st/sn/sf/su) re-orders the snapshot on demand
        self.apply_sort();
        self.reapply_search_filter();
        if self.list_sel >= self.scoped_items.len() {
            self.list_sel = self.scoped_items.len().saturating_sub(1);
        }
        if self.list_offset >= self.scoped_items.len() {
            self.list_offset = self.scoped_items.len().saturating_sub(1);
        }
    }

    /// Move the list cursor to idx if valid; reset article scroll.
    /// 合法时把列表光标移到 idx，并重置文章滚动位置。
    fn select_item(&mut self, idx: usize) {
        if idx >= self.scoped_items.len() {
            return;
        }
        self.list_sel = idx;
        self.article_scroll = 0;
    }

    /// Open the selected item: mark read in DB + snapshot, focus article.
    /// 打开选中项：DB 与快照中标记已读，焦点切到文章窗格。
    fn open_item(&mut self) {
        // let-else: destructure the Option or early-return. `let Some(x) = ...`
        // directly would be a compile error (non-exhaustive let); let-else is
        // the idiomatic "unwrap or bail" — like `if (p == NULL) return;` in C.
        // let-else：解构 Option，失败即提前返回 — 相当于 C 的判空返回。
        let Some((url, item)) = self.scoped_items.get(self.list_sel).cloned() else {
            return;
        };
        // `.ok()` on a Result discards the error ("best effort" write).
        // Use sparingly: fine for UI state, not for data you must not lose.
        // .ok() 丢弃错误 — 尽力而为的写入。UI 状态可用，重要数据别这么写。
        self.db.set_read(&url, &item.guid, true).ok();
        // reading clears read-later; do NOT rebuild the list — the item stays
        // visible in the current view until it's left or refreshed
        if item.read_later {
            self.db.set_flag(&url, &item.guid, "read_later", false).ok();
        }
        // reflect the read/flag change in the in-memory snapshot
        self.mark_scoped_read(&url, &item.guid, true);
        self.focus = 2;
        self.article_scroll = 0;
        // summary-only until opened — fetch full content only on explicit
        // <enter> in the article pane
        if item.content.trim().is_empty() {
            self.status = "summary only — press enter to fetch full article".into();
        }
    }

    /// The currently selected (feed_url, item), cloned out of the snapshot.
    /// Returns a copy so callers can freely mutate App afterwards.
    /// 当前选中的 (源url, 条目)，从快照克隆出来；调用方之后可随意改 App。
    fn current_item(&self) -> Option<(String, Item)> {
        self.scoped_items.get(self.list_sel).cloned()
    }

    /// Display body markdown: content only (summary lives in the header).
    /// Export body markdown: content, falling back to summary.
    /// 显示用正文：仅 content（摘要放页眉）。
    /// 导出用正文：content 优先，缺省回退 summary。
    fn article_markdown_export(&self, item: &Item) -> String {
        if !item.content.trim().is_empty() {
            fetch::html_to_markdown(&item.content)
        } else {
            item.summary.clone()
        }
    }

    /// Summary as markdown (HTML → markdown), empty when the feed has none.
    /// 摘要转 markdown（HTML → markdown）；源没提供则为空。
    fn article_markdown_summary(&self, item: &Item) -> String {
        if item.summary.trim().is_empty() {
            String::new()
        } else {
            fetch::html_to_markdown(&item.summary)
        }
    }

    /// Full article body as markdown: summary + content, in order — matches
    /// what the article pane shows. Empty when the feed has neither.
    /// 全文 markdown：摘要 + 正文按序拼接，与文章窗格一致；两者皆无则空串。
    fn article_markdown_body(&self, item: &Item) -> String {
        let summary_md = if !item.summary.trim().is_empty() {
            Some(fetch::html_to_markdown(&item.summary))
        } else {
            None
        };
        let content_md = if !item.content.trim().is_empty() {
            Some(fetch::html_to_markdown(&item.content))
        } else {
            None
        };
        match (summary_md, content_md) {
            (Some(s), Some(c)) => format!("{s}\n\n{c}"),
            (Some(s), None) => s,
            (None, Some(c)) => c,
            (None, None) => String::new(),
        }
    }

    /// Spawn a background thread to fetch the full article for the selected
    /// item; result arrives later as Msg::ArticleFetched.
    /// 起后台线程抓取选中项的全文；结果稍后以 Msg::ArticleFetched 返回。
    fn fetch_article(&mut self) {
        let Some((_, item)) = self.current_item() else { return };
        if item.url.is_empty() {
            self.status = "no url to fetch".into();
            return;
        }
        if self.fetching {
            return;
        }
        // always try to fetch full content — even if the feed provided some
        // Capture-by-move closure inputs: url/guid/tx are CLONED first because
        // the thread must own its data — a spawned thread may outlive the
        // current stack frame, so borrowing locals is rejected by the compiler
        // (in C you'd have to heap-allocate and pray about lifetimes).
        // 线程必须拥有自己的数据，所以先 clone 再 move 进闭包 —
        // 新线程可能比当前栈帧活得久，编译器禁止借用局部变量。
        self.fetching = true;
        self.status = format!("fetching {}", item.url);
        let timeout = self.cfg.fetch_timeout;
        let tx = self.tx.clone();
        let url = item.url.clone();
        let guid = item.guid.clone();
        // thread::spawn takes a `move` closure: all captured variables are
        // transferred INTO the thread (ownership moves). `.ok()` on send:
        // if the receiver is gone (app quitting) the message is dropped — fine.
        // thread::spawn 的 move 闭包：捕获的变量所有权移入线程。
        // send 用 .ok()：接收端没了（应用退出）就丢弃消息 — 无妨。
        thread::spawn(move || {
            let result = fetch::fetch_article(&url, timeout);
            tx.send(Msg::ArticleFetched { url, guid, result }).ok();
        });
    }

    // ── feed/category CRUD (rewrites urls file) ────────────────────────

    /// Persist the feed list to disk; failures surface in the status bar.
    /// 把源列表写回磁盘；失败信息显示在状态栏。
    fn save_urls(&mut self) {
        if let Err(e) = self.feeds.save(&self.cfg.urls_path) {
            self.status = format!("urls save failed: {e}");
        }
    }

    /// Open an input prompt for `mode`, prefilling the buffer where useful.
    /// 为 mode 打开输入框，必要时预填缓冲区。
    fn start_input(&mut self, mode: InputMode) {
        // One prompt string per input mode (match as expression, assigned).
        // 每种输入模式一条提示文案（match 作表达式赋值）。
        let prompt = match &mode {
            InputMode::AddUrl => "feed URL:".to_string(),
            InputMode::AddTitle => "display title (empty = none):".to_string(),
            InputMode::AddCategory => "category (space-separated, empty = none):".to_string(),
            InputMode::AddTags | InputMode::EditTags => {
                "tags (space-separated, empty = none):".to_string()
            }
            InputMode::RenameCategory => "new category name:".to_string(),
            InputMode::Search => "/ search:".to_string(),
            InputMode::EditFeedTitle => "display title (empty = default):".to_string(),
            InputMode::EditTag => "new tag name:".to_string(),
            InputMode::ExportFile => "export as (enter = default):".to_string(),
            InputMode::ExportSavedFile => {
                "append saved list as (enter = default):".to_string()
            }
            InputMode::ImportOpml => "OPML file path:".to_string(),
        };
        let mut buf = String::new();
        // prefill current tags when editing
        // Peek at pending wizard state without consuming it (`&` pattern in
        // match — borrow, don't move). See PendingInput below for the flow.
        // match 里用 & 模式只借用不移动 — 偷看向导状态但不消费。
        let pending_url = match &self.pending {
            Some(PendingInput::EditTags { url }) => Some(url.clone()),
            _ => None,
        };
        if let InputMode::EditTags = &mode {
            if let Some(url) = &pending_url {
                if let Some(f) = self.feeds.feeds.iter().find(|f| &f.url == url) {
                    buf = f
                        .feed_tags
                        .iter()
                        .map(|t| format!("#{t}"))
                        .collect::<Vec<_>>()
                        .join(" ");
                }
            }
        }
        // prefill the current custom title when editing
        if let InputMode::EditFeedTitle = &mode {
            if let Some(url) = &pending_url {
                if let Some(f) = self.feeds.feeds.iter().find(|f| &f.url == url) {
                    if f.custom_name {
                        buf = f.title.clone().unwrap_or_default();
                    }
                }
            }
        }
        self.input = Some(InputPrompt { mode, prompt, buf });
    }

    /// Dispatch a submitted prompt to its handler. Multi-step flows (add
    /// feed) chain through `pending`: each step consumes it with .take()
    /// (leaving None behind) and re-stores updated state, then opens the next
    /// prompt. take() = "swap out the value, leave None" — no use-after-move.
    /// 分发已提交的输入。多步流程（加源）经 pending 串联：每步用 take()
    /// 取走并留下 None，再存入更新后的状态，然后打开下一个输入框。
    /// take() = 取走换入 None，避免“移动后使用”错误。
    fn submit_input(&mut self) {
        let Some(prompt) = self.input.take() else { return };
        let val = prompt.buf.trim().to_string();
        match prompt.mode {
            InputMode::Search => {} // Enter is intercepted before submit
            InputMode::AddUrl => {
                if val.is_empty() {
                    self.status = "add feed cancelled (empty url)".into();
                    return;
                }
                // Step 1 of the add-feed wizard: remember the url, ask title.
                // 加源向导第一步：记下 url，接着问标题。
                self.pending = Some(PendingInput::AddFeed { url: val, title: None, category: None });
                self.start_input(InputMode::AddTitle);
            }
            InputMode::AddTitle => {
                let Some(PendingInput::AddFeed { url, .. }) = self.pending.take() else {
                    return;
                };
                self.pending = Some(PendingInput::AddFeed {
                    url,
                    title: if val.is_empty() { None } else { Some(val) },
                    category: None,
                });
                self.start_input(InputMode::AddCategory);
            }
            InputMode::AddCategory => {
                let Some(PendingInput::AddFeed { url, title, .. }) = self.pending.take() else {
                    return;
                };
                let tags: Vec<String> = val.split_whitespace().map(str::to_string).collect();
                self.pending = Some(PendingInput::AddFeed { url, title, category: Some(tags) });
                self.start_input(InputMode::AddTags);
            }
            InputMode::AddTags => {
                // Final wizard step: assemble the Feed struct and commit.
                // unwrap_or_default() on Option<Vec> = the Vec or empty —
                // like `p ? p : NULL-safe-default` in C, but total.
                // 向导最后一步：组装 Feed 并提交。unwrap_or_default() 取值或空默认。
                let Some(PendingInput::AddFeed { url, title, category }) = self.pending.take() else {
                    return;
                };
                let tags = category.unwrap_or_default();
                let feed_tags: Vec<String> = val
                    .split_whitespace()
                    .map(|w| w.trim_start_matches('#').to_string())
                    .filter(|t| !t.is_empty())
                    .collect();
                let feed = Feed {
                    url: url.clone(),
                    title,
                    custom_name: false,
                    feed_title: None,
                    tags,
                    feed_tags,
                    favourite: false,
                    lazy: false,
                };
                self.feeds.upsert(feed);
                self.save_urls();
                self.rebuild_tree();
                self.status = format!("added {url}");
                // new feed: fetch its items right away (append-only refresh)
                // 新源：立即抓取条目（只增刷新）。
                self.refresh_feed_thread(url, false);
            }
            InputMode::EditTags => {
                let Some(PendingInput::EditTags { url }) = self.pending.take() else {
                    return;
                };
                let feed_tags: Vec<String> = val
                    .split_whitespace()
                    .map(|w| w.trim_start_matches('#').to_string())
                    .filter(|t| !t.is_empty())
                    .collect();
                if let Some(f) = self.feeds.feeds.iter_mut().find(|f| f.url == url) {
                    f.feed_tags = feed_tags;
                }
                self.save_urls();
                self.rebuild_tree();
                self.status = format!("tags updated for {url}");
            }
            InputMode::RenameCategory => {
                if val.is_empty() {
                    self.status = "rename cancelled".into();
                    return;
                }
                if let Some(TreeRow::Category(old)) = self.tree_rows.get(self.tree_sel) {
                    let old = old.clone();
                    self.feeds.rename_category(&old, &val);
                    self.save_urls();
                    self.rebuild_tree();
                    self.status = format!("category {old} → {val}");
                }
            }
            InputMode::EditFeedTitle => {
                let Some(PendingInput::EditTags { url }) = self.pending.take() else {
                    return;
                };
                if let Some(f) = self.feeds.feeds.iter_mut().find(|f| f.url == url) {
                    if val.is_empty() {
                        // clear the custom title — fall back to feed-provided name
                        f.title = None;
                        f.custom_name = false;
                    } else {
                        f.title = Some(val.clone());
                        f.custom_name = true;
                    }
                }
                self.save_urls();
                self.rebuild_tree();
                self.status = format!("title updated for {url}");
            }
            InputMode::EditTag => {
                if val.is_empty() {
                    self.status = "rename cancelled".into();
                    return;
                }
                if let Some(TreeRow::Tag(old)) = self.tree_rows.get(self.tree_sel) {
                    let old = old.clone();
                    for f in self.feeds.feeds.iter_mut() {
                        if let Some(slot) = f.feed_tags.iter_mut().find(|t| **t == old) {
                            *slot = val.clone();
                        }
                    }
                    self.save_urls();
                    self.rebuild_tree();
                    self.status = format!("tag {old} → {val}");
                }
            }
            InputMode::ExportFile => {
                let path = if val.is_empty() {
                    // fall back to the prefilled default
                    prompt.buf.trim().to_string()
                } else {
                    val
                };
                self.finish_export(std::path::PathBuf::from(path));
            }
            InputMode::ExportSavedFile => {
                let path = if val.is_empty() {
                    prompt.buf.trim().to_string()
                } else {
                    val
                };
                self.finish_saved_export(std::path::PathBuf::from(path));
            }
            InputMode::ImportOpml => {
                if val.is_empty() {
                    self.status = "import cancelled".into();
                    return;
                }
                match std::fs::read_to_string(&val) {
                    Ok(xml) => match opml::import_opml(&xml) {
                        Ok(feeds) => {
                            let n = feeds.len();
                            for f in feeds {
                                self.feeds.upsert(f);
                            }
                            self.save_urls();
                            self.rebuild_tree();
                            self.status = format!("imported {n} feeds from {val}");
                        }
                        Err(e) => self.status = format!("OPML parse failed: {e}"),
                    },
                    Err(e) => self.status = format!("read failed: {e}"),
                }
            }
        }
    }

    /// Remove the selected feed from urls file + DB + current snapshot.
    /// 从 urls 文件、DB、当前快照三处删除选中的源。
    fn delete_selected_feed(&mut self) {
        // and_then chains another fallible step onto the Option (like nested
        // ifs / goto-fail in C): get row → if it's any *Feed variant, extract
        // (url, name), else None. The `|` pattern matches several variants at once.
        // and_then 在 Option 上串联下一步可失败操作。`|` 模式一次匹配多个变体。
        let Some((url, name)) = self.tree_rows.get(self.tree_sel).and_then(|r| match r {
            TreeRow::Feed(u, n, _) | TreeRow::FavouriteFeed(u, n) | TreeRow::LazyFeed(u, n) | TreeRow::UncategorizedFeed(u, n) => {
                Some((u.clone(), n.clone()))
            }
            _ => None,
        }) else {
            return;
        };
        // Inner block `{ ... }` is just for visual scoping of this removal
        // sequence; it has no ownership effect here.
        // 内层块只为视觉上圈住删除流程，此处无所有权含义。
        {
            self.feeds.remove(&url);
            self.save_urls();
            self.rebuild_tree();
            self.status = format!("removed {name}");
            self.db.remove_feed_items(&url).ok();
            // Vec::retain keeps only elements where the closure is true —
            // in-place filtered removal, no manual index juggling like C.
            // retain 只保留闭包为真的元素 — 原地过滤删除，无需像 C 手动挪下标。
            self.scoped_items.retain(|(u, _)| u != &url);
            if self.scope == Scope::Feed(url) {
                self.scope = Scope::AllUnread;
                self.rebuild_list();
            }
        }
    }

    /// Export all feeds/categories as OPML into the config dir.
    /// 把全部源/分类导出为 OPML 写入配置目录。
    fn export_opml(&mut self) {
        let path = self.cfg.config_dir.join("feeds.opml");
        match std::fs::write(&path, opml::export_opml(&self.feeds)) {
            Ok(_) => self.status = format!("OPML exported to {}", path.display()),
            Err(e) => self.status = format!("OPML export failed: {e}"),
        }
    }

    // ── refresh ───────────────────────────────────────────────────────────

    /// `full=true` (manual `r`): replace items + rebuild the list.
    /// `full=false` (auto fetch): upsert new items only, append new unread
    /// to the current list — never removes read articles.
    /// Spawn one worker thread per feed refresh; results arrive as
    /// Msg::FeedRefreshed. `pending_refreshes` counts in-flight workers.
    /// 每个源的刷新各起一个工作线程；结果以 Msg::FeedRefreshed 返回。
    /// pending_refreshes 统计在途线程数。
    fn refresh_feed_thread(&mut self, url: String, full: bool) {
        // Increment before spawning so a fast worker can't race us into
        // thinking nothing is pending.
        // 先自增再起线程，避免快速完成的工作线程误报“无在途任务”。
        self.pending_refreshes += 1;
        let timeout = self.cfg.fetch_timeout;
        let tx = self.tx.clone();
        thread::spawn(move || {
            // Blocking network work happens HERE, off the UI thread. The
            // result (owned data) is shipped back through the channel.
            // 阻塞的网络操作在这里执行（UI 线程之外），结果经通道传回。
            let result = fetch::refresh_feed(&url, timeout);
            tx.send(Msg::FeedRefreshed { url, result, full }).ok();
        });
    }

    /// Feed urls belonging to the current scope (for partial refresh).
    /// Feed urls belonging to the current scope (for partial refresh).
    /// 当前范围内的所有源 url（用于部分刷新）。
    fn scope_feeds(&self) -> Vec<String> {
        match &self.scope {
            Scope::AllUnread | Scope::ReadLater | Scope::Saved => {
                self.feeds.feeds.iter().map(|f| f.url.clone()).collect()
            }
            Scope::Favourite => self
                .feeds
                .feeds
                .iter()
                .filter(|f| f.favourite)
                .map(|f| f.url.clone())
                .collect(),
            Scope::Lazy => self
                .feeds
                .feeds
                .iter()
                .filter(|f| f.lazy)
                .map(|f| f.url.clone())
                .collect(),
            Scope::Category(c) => self
                .feeds
                .by_category(c)
                .iter()
                .map(|f| f.url.clone())
                .collect(),
            Scope::Feed(u) => vec![u.clone()],
            Scope::Tag(t) => self
                .feeds
                .feeds
                .iter()
                .filter(|f| f.has_tag(t))
                .map(|f| f.url.clone())
                .collect(),
        }
    }

    /// Kick off a refresh wave. Guards against overlap: if any worker is
    /// still in flight, do nothing (avoids duplicate DB writes).
    /// 发起一轮刷新。防重入：仍有在途线程时直接跳过（避免重复写库）。
    fn refresh_all(&mut self, full: bool, auto: bool) {
        if self.pending_refreshes > 0 {
            return;
        }
        // partial refresh targets only the feeds in the current list;
        // full refresh targets every feed. Auto refresh (startup/interval)
        // skips lazy feeds — they only refresh on manual r/R.
        let mut urls: Vec<String> = if full {
            self.feeds.feeds.iter().map(|f| f.url.clone()).collect()
        } else {
            self.scope_feeds()
        };
        if auto {
            urls.retain(|u| !self.feeds.feeds.iter().any(|f| f.url == *u && f.lazy));
        }
        if urls.is_empty() {
            self.status = if auto {
                "no non-lazy feeds to auto-refresh".into()
            } else {
                "no feeds — add subscriptions to the urls file".into()
            };
            return;
        }
        self.status = if full {
            format!("refreshing {} feeds…", urls.len())
        } else {
            format!("fetching new items from {} feeds…", urls.len())
        };
        for url in urls {
            self.refresh_feed_thread(url, full);
        }
    }

    /// Apply one finished FeedRefreshed message to state + DB.
    /// 把一条已完成的 FeedRefreshed 消息落到状态与 DB。
    fn handle_feed_refreshed(
        &mut self,
        url: String,
        result: Result<(Option<String>, Vec<Item>), String>,
        full: bool,
    ) {
        self.pending_refreshes = self.pending_refreshes.saturating_sub(1);
        match result {
            Ok((feed_title, mut items)) => {
                self.feed_errors.remove(&url);
                // show the real feed title in nav when no custom name, and
                // persist it to the urls file (quoted title)
                if let Some(ft) = feed_title {
                    if let Some(f) = self.feeds.feeds.iter_mut().find(|f| f.url == url) {
                        if f.title.is_none() {
                            f.title = Some(ft.clone());
                        }
                        f.feed_title = Some(ft);
                    }
                    self.save_urls();
                    self.rebuild_tree();
                }
                if let Some(cap) = self.cfg.max_items_per_feed {
                    items.truncate(cap);
                    // truncate() shortens the Vec in place, dropping the tail —
                    // no realloc/copy dance like manual C array shrinking.
                    // truncate() 原地截断丢弃尾部，无需像 C 手工缩数组。
                }
                if full {
                    self.db.replace_feed_items_preserving_read(&url, &items).ok();
                    self.status = format!("refreshed {url} ({} items)", items.len());
                } else {
                    let added = self.db.upsert_fetch(&url, &items).unwrap_or_default();
                    self.status =
                        format!("fetched {url} ({} new)", added.len());
                    self.append_new_unread(&url, &added);
                    // in-place refresh of snapshot entries (content etc.) —
                    // no rebuild, so read items stay in place
                    self.refresh_snapshot_content(&url);
                }
            }
            Err(e) => {
                self.status = e.clone();
                self.feed_errors.insert(url.clone(), e);
            }
        }
        if self.pending_refreshes == 0 {
            // last worker done — append "done" to the status line
            // 最后一个线程完成 — 状态栏追加 “— done”
            self.status.push_str(" — done");
        }
        if full {
            self.rebuild_list();
        }
    }

    /// Update the in-memory list snapshot for one feed from the DB
    /// (content/summary/title) without rebuilding or reordering.
    /// 从 DB 刷新某源在内存快照中的内容字段，不重建不重排。
    fn refresh_snapshot_content(&mut self, feed_url: &str) {
        let Ok(list) = self.db.items_for_feed(feed_url) else {
            return;
        };
        let by_guid: std::collections::HashMap<String, Item> =
            list.into_iter().map(|i| (i.guid.clone(), i)).collect();
        // Iterator pipeline: into_iter() consumes the Vec → map each item to
        // a (guid, item) pair → collect() builds the HashMap. This is the
        // idiomatic replacement for a C build-a-hash loop.
        // 迭代器流水线：into_iter() 消耗 Vec → map 变 (guid, item) → collect 建 HashMap。
        // 惯用写法，替代 C 的手工建哈希循环。
        for (u, i) in self.scoped_items.iter_mut() {
            if u == feed_url {
                if let Some(fresh) = by_guid.get(&i.guid) {
                    i.content = fresh.content.clone();
                    i.summary = fresh.summary.clone();
                    i.title = fresh.title.clone();
                }
            }
        }
        // the article body cache may reference the old content
        self.article_render = None;
    }

    /// Append newly-fetched unread items to the current list (no reorder of
    /// existing entries).
    /// 把新抓到的未读条目追加进当前列表（不重排已有条目）。
    fn append_new_unread(&mut self, feed_url: &str, added: &[String]) {
        if added.is_empty() {
            return;
        }
        let in_scope = match &self.scope {
            Scope::AllUnread => true,
            Scope::Feed(u) => u == feed_url,
            Scope::Category(c) => self.feeds.by_category(c).iter().any(|f| f.url == feed_url),
            Scope::Tag(t) => self
                .feeds
                .feeds
                .iter()
                .any(|f| f.url == feed_url && f.has_tag(t)),
            Scope::Favourite => self
                .feeds
                .feeds
                .iter()
                .any(|f| f.url == feed_url && f.favourite),
            Scope::Lazy => self
                .feeds
                .feeds
                .iter()
                .any(|f| f.url == feed_url && f.lazy),
            _ => false,
        };
        if !in_scope {
            return;
        }
        let existing: std::collections::HashSet<String> = self
            .scoped_items
            .iter()
            .map(|(_, i)| i.guid.clone())
            .collect();
        // Dedup set: O(1) membership tests instead of scanning the list per
        // item. collect() infers HashSet from the binding's type annotation.
        // 去重集合：O(1) 查存在性，免得逐项扫列表。类型由标注推断出 HashSet。
        let mut fresh: Vec<(String, Item)> = Vec::new();
        if let Ok(list) = self.db.items_for_feed(feed_url) {
            for i in list {
                if added.contains(&i.guid)
                    && !existing.contains(&i.guid)
                    && !i.read
                {
                    fresh.push((feed_url.to_string(), i));
                }
            }
        }
        if !fresh.is_empty() {
            // splice(0..0, fresh) inserts at the FRONT of the Vec — newest on
            // top, matching date-desc order, without re-sorting.
            // splice(0..0, fresh) 头部插入 — 最新在上，契合日期倒序且无需重排。
            self.scoped_items.splice(0..0, fresh);
        }
        self.reapply_search_filter();
    }

    /// Apply a finished article fetch: persist content, patch snapshot,
    /// invalidate the render cache.
    /// 落地抓到的全文：写库、原地更新快照、失效渲染缓存。
    fn handle_article_fetched(&mut self, url: String, guid: String, result: Result<String, String>) {
        self.fetching = false;
        match result {
            Ok(html) => {
                // locate the row by guid so a mid-fetch selection change
                // can't write to the wrong feed
                let Some((feed_url, _)) = self
                    .scoped_items
                    .iter()
                    .find(|(_, i)| i.guid == guid)
                    .cloned()
                    .or_else(|| self.current_item())
                else {
                    self.status = format!("fetched {} (stale view)", url);
                    return;
                };
                self.db.update_item_content(&feed_url, &guid, &html).ok();
                // content changed — invalidate the rendered-body cache
                self.article_render = None;
                // update the scoped item in place — a rebuild would drop this
                // just-opened (read) item from the AllUnread view
                for (u, i) in self.scoped_items.iter_mut() {
                    if u == &feed_url && i.guid == guid {
                        i.content = html.clone();
                        break;
                    }
                }
                self.status = format!("fetched {} ({} chars)", url, html.len());
            }
            Err(e) => {
                self.status = format!("fetch failed: {e}");
            }
        }
    }

    // ── actions ───────────────────────────────────────────────────────────

    fn mark_all_read(&mut self, full: bool) {
        if full {
            // A = mark every item in every feed read
            let urls: Vec<String> = self.feeds.feeds.iter().map(|f| f.url.clone()).collect();
            for u in urls {
                self.db.mark_all_read(&u).ok();
            }
            // in-place — the snapshot keeps read items visible until refresh
            for (_, i) in self.scoped_items.iter_mut() {
                i.read = true;
            }
            self.status = "marked all feeds read".into();
            return;
        }
        // a = mark every item currently in the list read. Using scoped_items
        // (rather than whole feeds) keeps flag-scoped/search views precise.
        let items = self.scoped_items.clone();
        for (url, item) in items {
            self.db.set_read(&url, &item.guid, true).ok();
            // matching open/read behaviour: reading clears read-later
            if item.read_later {
                self.db.set_flag(&url, &item.guid, "read_later", false).ok();
            }
        }
        for (_, i) in self.scoped_items.iter_mut() {
            i.read = true;
            i.read_later = false;
        }
        self.status = "marked current list read".into();
    }

    fn toggle_read(&mut self) {
        let Some((url, item)) = self.current_item() else { return };
        let on = self.db.toggle_read(&url, &item.guid).unwrap_or(item.read);
        // in-place snapshot update — no rebuild, keeps the list stable
        for (u, i) in self.scoped_items.iter_mut() {
            if u == &url && i.guid == item.guid {
                i.read = on;
            }
        }
    }

    /// `<space>`: toggle the current item's read state, then move down one.
    /// `<space>`：翻转当前项已读状态，然后下移一行。
    fn toggle_read_and_next(&mut self) {
        self.toggle_read();
        self.move_list_sel(1);
    }

    /// Open the selected item's link in the browser.
    /// 用浏览器打开选中项链接。
    fn open_browser(&mut self) {
        let Some((_, item)) = self.current_item() else { return };
        if item.url.is_empty() {
            self.status = "no url".into();
            return;
        }
        self.open_url(&item.url);
    }

    /// Open an arbitrary URL in the configured browser (fallback xdg-open).
    /// 用配置的浏览器打开任意 URL（缺省回退 xdg-open）。
    fn open_url(&mut self, url: &str) {
        // std::process::Command = fork/exec equivalent. spawn() returns
        // immediately without waiting for the child (no waitpid here).
        // Command 相当于 fork/exec。spawn() 立即返回，不等子进程（不 waitpid）。
        let cmd = self.cfg.browser.clone().unwrap_or_else(|| "xdg-open".to_string());
        match std::process::Command::new(&cmd).arg(url).spawn() {
            Ok(_) => self.status = format!("opened {url}"),
            Err(e) => self.status = format!("{cmd} failed: {e}"),
        }
    }

    /// Start the saved-list export flow: prompt with the default path prefilled.
    /// 启动收藏列表导出流程：提示框预填默认路径。
    fn start_saved_export(&mut self) {
        let default_path = self.cfg.export_saved_path.clone();
        self.pending = Some(PendingInput::ExportSaved);
        self.input = Some(InputPrompt {
            mode: InputMode::ExportSavedFile,
            prompt: "append saved list as (enter = default):".to_string(),
            buf: default_path.to_string_lossy().to_string(),
        });
    }

    /// Start the export flow: prompt with the default filename as placeholder.
    /// 启动文章导出流程：提示框预填默认文件名。
    fn start_export(&mut self) {
        let Some((feed_url, item)) = self.current_item() else {
            self.status = "no item selected".into();
            return;
        };
        let feed = self.feeds.feeds.iter().find(|f| f.url == feed_url);
        let category = feed.and_then(|f| f.category()).unwrap_or("");
        let slug = slugify(&item.title);
        // Build the suggested path: <export_dir>/<category>/<slug>.md.
        // PathBuf::join is portable (adds separators as needed).
        // 构造建议路径：<导出目录>/<分类>/<slug>.md。join 跨平台自动补分隔符。
        let dir = if category.is_empty() {
            self.cfg.export_dir.clone()
        } else {
            self.cfg.export_dir.join(category)
        };
        let default_path = dir.join(format!("{slug}.md"));
        self.pending = Some(PendingInput::Export { feed_url, guid: item.guid });
        self.input = Some(InputPrompt {
            mode: InputMode::ExportFile,
            prompt: "export as (enter = default):".to_string(),
            buf: default_path.to_string_lossy().to_string(),
        });
    }

    /// Finish the export after the rename prompt (or default path).
    /// 重命名提示结束后落地导出（或用默认路径）。
    fn finish_export(&mut self, path: std::path::PathBuf) {
        let Some(PendingInput::Export { feed_url, guid }) = self.pending.take() else {
            return;
        };
        // Chained fallible lookup: query DB → find item by guid. `.ok()`
        // converts Result→Option so and_then can chain; let-else bails if absent.
        // 链式可失败查询：查库 → 按 guid 找条目。ok() 把 Result 变 Option 以便串联，
        // 找不到则 let-else 提前返回。
        let Some(item) = self
            .db
            .items_for_feed(&feed_url)
            .ok()
            .and_then(|list| list.into_iter().find(|i| i.guid == guid))
        else {
            self.status = "export failed: item not found".into();
            return;
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).ok();
        }
        match self.write_export(&path, &feed_url, &item) {
            Ok(_) => self.status = format!("exported {}", path.display()),
            Err(e) => self.status = format!("export failed: {e}"),
        }
    }

    /// Write one article as a markdown file with YAML front matter.
    /// 把一篇文章写成带 YAML front matter 的 markdown 文件。
    ///
    /// Returns io::Result<()> — errors propagate via `?` to finish_export's
    /// match. No global errno; the error value IS the return value.
    /// 返回 io::Result<()> — 错误经 ? 上抛给 finish_export 处理；
    /// 不靠全局 errno，错误本身就是返回值。
    fn write_export(&self, path: &std::path::Path, feed_url: &str, item: &Item) -> io::Result<()> {
        let feed = self.feeds.feeds.iter().find(|f| f.url == feed_url);
        let body = self.article_markdown_export(item);
        let mut md = String::new();
        md.push_str(&format!("---\ntitle: \"{}\"\n", escape_yaml(&item.title)));
        md.push_str(&format!("link: {}\n", item.url));
        if !item.date.is_empty() {
            md.push_str(&format!("date: {}\n", item.date));
        }
        if let Some(f) = feed {
            md.push_str(&format!("feed: \"{}\"\n", escape_yaml(f.display_name())));
        }
        md.push_str("---\n\n");
        md.push_str(&body);
        md.push('\n');
        std::fs::write(path, md)
    }

    /// Append `title url summary` lines for all saved items to `path`.
    /// 把所有收藏条目以 “title url summary” 追加写入 path。
    fn finish_saved_export(&mut self, path: std::path::PathBuf) {
        let Some(PendingInput::ExportSaved) = self.pending.take() else {
            return;
        };
        let items = self.db.items_with_flag("saved").unwrap_or_default();
        if items.is_empty() {
            self.status = "no saved items to export".into();
            return;
        }
        let mut out = String::new();
        let n = items.len();
        for (_, item) in &items {
            let summary = fetch::html_to_markdown(&item.summary)
                .replace(['\n', '\r'], " ")
                .trim()
                .to_string();
            let title = item.title.replace(['\n', '\r'], " ");
            out.push_str(&format!("{title} {} {summary}\n", item.url));
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).ok();
        }
        // OpenOptions builder pattern: chain flags then open — the Rust
        // equivalent of open(path, O_CREAT | O_APPEND).
        // OpenOptions 构建器模式：链式设标志再打开 — 相当于 open 的 O_CREAT|O_APPEND。
        match std::fs::OpenOptions::new().create(true).append(true).open(&path) {
            Ok(mut f) => match std::io::Write::write_all(&mut f, out.as_bytes()) {
                Ok(_) => self.status = format!(
                    "appended {} saved items to {}",
                    n,
                    path.display()
                ),
                Err(e) => self.status = format!("export failed: {e}"),
            },
            Err(e) => self.status = format!("export failed: {e}"),
        }
    }

    /// Move selection by delta (clamped), auto-marking read in article pane.
    /// 光标按 delta 移动（钳制边界）；文章窗格内移动即标记已读。
    fn next_prev_item(&mut self, delta: isize) {
        if self.scoped_items.is_empty() {
            return;
        }
        // isize = signed integer: lets idx go negative during computation
        // before clamping, unlike usize which would underflow. Cast back to
        // usize only after clamping proves it's non-negative.
        // isize 有符号，计算中可为负再钳制；usize 会下溢。钳制后才转回 usize。
        let n = self.scoped_items.len() as isize;
        let mut idx = self.list_sel as isize + delta;
        if idx < 0 {
            idx = 0;
        }
        if idx >= n {
            idx = n - 1;
        }
        self.select_item(idx as usize);
        // in the article pane, showing content marks it read
        if self.focus == 2 {
            if let Some((url, item)) = self.current_item() {
                self.db.set_read(&url, &item.guid, true).ok();
                if item.read_later {
                    self.db.set_flag(&url, &item.guid, "read_later", false).ok();
                }
                self.mark_scoped_read(&url, &item.guid, true);
                // no rebuild — keeps the current view stable
            }
        }
    }
}

// ── main ────────────────────────────────────────────────────────────────

/// Entry point. Returns io::Result<()> so `?` inside can bubble errors up:
/// on Err, Rust prints the error and exits nonzero — no printf + exit(1) dance.
/// 入口。返回 io::Result<()>，内部用 ? 上抛错误：出错时 Rust 自动打印并
/// 以非零码退出，无需手写 printf + exit(1)。
fn main() -> io::Result<()> {
    let cfg = Config::load();
    // The ONE owned App instance for the whole process. Everything else
    // borrows it: run(&mut app), worker threads send Msgs instead of touching it.
    // 全进程唯一的 App 实例。其余全是借用：run(&mut app)，
    // 工作线程只发消息不碰它。
    let mut app = App::new(cfg);
    // foldlevel: initial nav fold depth (0 = only top rows visible)
    if let Some(level) = app.cfg.foldlevel {
        for path in app.feeds.categories_tree() {
            if path.len() > level {
                app.collapsed.insert(path.join("/"));
            }
        }
        // level 0 = fold every foldable section to a single header row
        if level == 0 {
            for s in ["Categories", "Tags", "Feeds"] {
                app.collapsed.insert(s.to_string());
            }
            app.fav_expanded = false;
            app.lazy_expanded = false;
            app.uncat_expanded = false;
        }
        app.rebuild_tree();
    }
    // Best-effort cleanup of expired cached content (TTL); ignore failures.
    // 尽力清理过期缓存内容（TTL）；失败忽略。
    app.db.cleanup_content(app.cfg.cache_ttl_days).ok();
    // startup: fetch new items (append-only) — never a full refresh.
    // Lazy feeds are skipped (manual r/R only).
    if app.cfg.refresh_on_startup {
        app.refresh_all(false, true);
    }
    // interval auto-refresh
    if let Some(interval_min) = app.cfg.refresh_interval_minutes {
        if interval_min > 0 {
            // Timer thread: sleeps, then pings the channel every interval.
            // The loop owns a Sender clone; the UI side drains with try_recv
            // (non-blocking) — classic producer/consumer without locks.
            // 定时线程：睡眠后按周期发心跳。循环持有 Sender 克隆；
            // UI 侧用 try_recv 非阻塞取 — 经典生产者/消费者，无锁。
            let tx = app.tx.clone();
            thread::spawn(move || loop {
                thread::sleep(Duration::from_secs(interval_min * 60));
                tx.send(Msg::RefreshTick).ok();
            });
        }
    }

    // ratatui::init() = enter alternate screen + raw mode (no line buffering,
    // keys arrive immediately, no echo). It returns a DefaultTerminal guard.
    // init 进入备用屏 + raw 模式（无行缓冲、按键即时到达、不回显），
    // 返回 DefaultTerminal 守卫对象。
    let mut terminal = ratatui::init();
    let result = run(&mut terminal, &mut app);
    // restore() leaves raw mode / repaints the normal shell. C analogy: this is
    // the free()/atexit pairing done manually — forget it and the user's shell
    // stays broken. (The ?-propagated early exits above happen BEFORE init,
    // so nothing can skip the restore except a panic.)
    // restore 恢复终端、重绘 shell。相当于手动配对的 free/atexit —
    // 忘了调用用户的终端就废了。（上面的 ？ 提前退出都发生在 init 之前，
    // 除 panic 外不会跳过 restore。）
    ratatui::restore();
    result
}

/// The event loop: draw on demand, poll input, drain worker messages.
/// Takes BOTH pieces of state as &mut — exclusive borrows, proven non-aliased
/// at compile time (in C you'd just pass two pointers and hope).
/// 事件循环：按需绘制、轮询输入、清空工作线程消息。两份状态都以 &mut
/// 独占借用传入，编译期证明无别名（C 里只传指针然后靠祈祷）。
fn run(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> io::Result<()> {
    // redraw flag: only repaint when something actually changed.
    // redraw 标志：只在真正有变化时才重绘。
    let mut redraw = true;
    while app.running {
        // draw only when something changed (key, message, or first frame) —
        // avoids the per-tick full repaint that made scrolling lag
        if redraw {
            // terminal.draw takes a closure receiving the frame; render()
            // reads App through it (&App borrow inside, &mut App outside —
            // sequential, so no conflict). `?` bubbles IO errors to main.
            // draw 接收一个拿到帧的闭包；render 借用 App 绘制。
            // ? 把 IO 错误上抛 main。
            terminal.draw(|f| render(f, app))?;
            redraw = false;
        }
        // poll(timeout) waits up to 100ms for an event → bool "event ready".
        // This caps CPU use while keeping the loop responsive to messages.
        // poll(超时) 最多等 100ms，返回是否有事件。既限制 CPU 占用又保持响应。
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(k) => {
                    // Press + Repeat (held keys auto-repeat), not Release
                    // recount() refreshes cached unread counters after every
                    // keypress that might change read state — cheap (one query)
                    // and keeps the nav badge honest without finer tracking.
                    // 每次可能改变已读状态的按键后重算缓存未读数 — 一次查询，
                    // 简单可靠，无需更细粒度的追踪。
                    if matches!(k.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
                        app.on_key(k.code, k.modifiers);
                        app.recount();
                    }
                    redraw = true;
                }
                Event::Resize(_w, _h) => {
                    // terminal.draw() re-queries size each frame — just repaint
                    redraw = true;
                }
                _ => redraw = true,
            }
            // swallow resize/other events, still redraw
            // Drain any events queued up while we were handling the first one —
            // coalesces a burst (e.g. held key + resize) into one frame.
            // 清空积压事件 — 把突发（按住键+resize）合并成一帧处理。
            while event::poll(Duration::from_millis(0))? {
                match event::read()? {
                    Event::Key(k) if matches!(k.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
                        app.on_key(k.code, k.modifiers);
                        app.recount();
                        redraw = true;
                    }
                    Event::Resize(_w, _h) => {
                        // terminal.draw() re-queries size each frame — just repaint
                        redraw = true;
                    }
                    _ => redraw = true, // other events → repaint
                }
            }
        }
        // Drain worker messages without blocking: try_recv returns Ok(msg),
        // or Err when the queue is empty (loop ends). This is where the
        // spawned threads' results re-enter the single-threaded UI world.
        // 非阻塞清空工作线程消息：try_recv 有消息返回 Ok，队列空返回 Err（循环结束）。
        // 后台线程的结果由此回到单线程 UI 世界。
        while let Ok(msg) = app.rx.try_recv() {
            redraw = true;
            match msg {
                Msg::FeedRefreshed { url, result, full } => {
                    app.handle_feed_refreshed(url, result, full);
                    app.recount();
                }
                Msg::ArticleFetched { url, guid, result } => {
                    app.handle_article_fetched(url, guid, result)
                }
                Msg::RefreshTick => app.refresh_all(false, true),
            }
        }
    }
    Ok(())
}

