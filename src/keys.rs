//! Key dispatch (on_key) + per-key actions.
//! 按键分发（on_key）+ 各按键对应的动作。
//!
//! Child module of main — impl App blocks may touch private fields.
//! main 的子模块 — 这里的 impl App 块可以直接访问私有字段。
//!
//! Rust concepts introduced here:
//! 本文件涉及的 Rust 概念：
//! - exhaustive `match` as a giant dispatch table: every arm is
//!   `pattern if guard => expr`; unlike C's `switch`, arms can destructure
//!   enum data, attach guards, and the whole match is an expression.
//!   穷尽 match 当巨型分发表：分支 = "模式 if 守卫 => 表达式"。
//!   与 C 的 switch 不同，分支能解构枚举数据、附加条件，且整个 match 是表达式。
//! - `Option<T>` pipelines: `.map()`, `.and_then()`, `unwrap_or*`, and
//!   `let Some(x) = ... else { return }` (let-else) replace C's
//!   NULL-check ladders.
//!   Option 流水线：map/and_then/unwrap_or 与 let-else 取代 C 的层层判空。
//! - `HashMap<Vec<KeyCode>, Action>`: hash map keyed by a heap Vec;
//!   used to implement vim-style multi-key sequences ("gg", "st").
//!   以 Vec 为键的 HashMap，用来实现 vim 式多键序列（"gg"、"st"）。
//! - iterators + closures: `.any()`, `.retain()`, `.sort_by()` with
//!   capturing closures (like qsort's cmp, but no void* context arg).
//!   迭代器与捕获闭包：any/retain/sort_by — 类似 qsort 的比较函数，但无需 void* 参数。
//! - early returns to flatten a state machine: each input stage
//!   (help overlay → input box → ctrl chords → key buffer) ends in
//!   `return` instead of nesting ifs.
//!   用提前 return 拍平状态机：帮助层 → 输入框 → Ctrl 键 → 按键缓冲，
//!   每阶段处理完即返回，避免嵌套 if。

use crossterm::event::{KeyCode, KeyModifiers};

use crate::clipboard::copy_to_clipboard;
use crate::{App, InputMode, TreeRow};

/// Keybinding actions — user-remappable single-key actions.
/// 按键动作 — 可由用户配置重新绑定的操作。
///
/// This enum is the vocabulary of the keymap: the config file names an
/// action (`from_str`), `build_keymap` maps key sequences to it,
/// `execute_action` performs it.
/// Rust enums are tagged unions (C's `union` + tag), but the compiler
/// proves you never read the wrong variant. Most variants carry no data;
/// `Sort { level, reverse }` carries a payload, destructured in patterns
/// like `Action::Sort { level, reverse } => ...`.
/// 该枚举是键位表的"词汇表"：配置写动作名（from_str 解析），
/// build_keymap 把按键序列映射到动作，execute_action 执行。
/// Rust 枚举 = 带 tag 的 union，但编译器保证不会读错分支。
/// 多数变体无数据；Sort { level, reverse } 携带数据，匹配时用模式解构。
#[derive(Debug, Clone, Copy, PartialEq)] // Copy: passed/assigned by bitwise copy like a C int — no ownership move
                                         // Copy：传参/赋值按位拷贝，类似 C 的 int，不转移所有权
pub(crate) enum Action {
    Open,
    Back,
    Quit,
    Refresh,
    RefreshAll,
    ToggleRead,
    MarkListRead,
    MarkAllRead,
    ToggleReadNext,
    Export,
    ExportSaved,
    Browser,
    Favourite,
    ReadLater,
    Saved,
    NewFeed,
    Delete,
    Rename,
    EditTags,
    Help,
    FocusNext,
    Search,
    JumpTop,
    JumpBottom,
    NextUnread,
    PrevUnread,
    ParentNext,
    ParentPrev,
    CopyItemUrl,
    CopyItemTitle,
    CopyFeedUrl,
    CopyItemSummary,
    CopyItemContent,
    Sort { level: &'static str, reverse: bool },
    FocusPrev,
    CyclePreset,
    ImportOpml,
    ExportOpml,
}

impl Action {
    /// Parse a config action name → Action.
    /// 把配置文件中的动作名解析成 Action。
    ///
    /// `Option` = "value or nothing" (C's NULL-returning function), but the
    /// compiler forces callers to handle the None case.
    /// Option 表示"有值或没有"（类似 C 返回 NULL 的函数），
    /// 但编译器强制调用方处理 None 的情况。
    pub(crate) fn from_str(s: &str) -> Option<Action> {
        // `Some(match ...)`: the whole match is an EXPRESSION yielding a
        // value, wrapped in Some here. The catch-all arm below uses
        // `return None` — an early return from inside the expression,
        // skipping the Some wrapper entirely.
        // 整个 match 是表达式，产出值后被包进 Some。最后的兜底分支用
        // return None 从表达式内部提前返回函数，完全绕开 Some 包装。
        Some(match s.trim() {
            "open" => Action::Open,
            "back" => Action::Back,
            "quit" => Action::Quit,
            "refresh" => Action::Refresh,
            "refresh_all" => Action::RefreshAll,
            "toggle_read" => Action::ToggleRead,
            "mark_list_read" => Action::MarkListRead,
            "mark_all_read" => Action::MarkAllRead,
            "toggle_read_next" => Action::ToggleReadNext,
            "export" => Action::Export,
            "export_saved" => Action::ExportSaved,
            "browser" => Action::Browser,
            "favourite" => Action::Favourite,
            "read_later" => Action::ReadLater,
            "saved" => Action::Saved,
            "new_feed" => Action::NewFeed,
            "delete" => Action::Delete,
            "rename" => Action::Rename,
            "edit_tags" => Action::EditTags,
            "help" => Action::Help,
            "focus_next" => Action::FocusNext,
            "search" => Action::Search,
            "jump_top" => Action::JumpTop,
            "jump_bottom" => Action::JumpBottom,
            "next_unread" => Action::NextUnread,
            "prev_unread" => Action::PrevUnread,
            "parent_next" => Action::ParentNext,
            "parent_prev" => Action::ParentPrev,
            "copy_item_url" => Action::CopyItemUrl,
            "copy_item_title" => Action::CopyItemTitle,
            "copy_feed_url" => Action::CopyFeedUrl,
            "copy_item_summary" => Action::CopyItemSummary,
            "copy_item_content" => Action::CopyItemContent,
            "sort_time" => Action::Sort { level: "time", reverse: false },
            "sort_title" => Action::Sort { level: "title", reverse: false },
            "sort_feed" => Action::Sort { level: "feed", reverse: false },
            "sort_unread" => Action::Sort { level: "unread", reverse: false },
            // reversed variants: same pattern as above, just reverse: true
            // 反转变体：与上面同模式，只是 reverse: true
            "sort_time_rev" => Action::Sort { level: "time", reverse: true },
            "sort_title_rev" => Action::Sort { level: "title", reverse: true },
            "sort_feed_rev" => Action::Sort { level: "feed", reverse: true },
            "sort_unread_rev" => Action::Sort { level: "unread", reverse: true },
            "focus_prev" => Action::FocusPrev,
            "cycle_preset" => Action::CyclePreset,
            "import_opml" => Action::ImportOpml,
            "export_opml" => Action::ExportOpml,
            _ => return None,
        })
    }

    /// Parse a key string → KeyCode ("l", "enter", "esc", "tab", "?", …).
    /// 把按键名解析成 KeyCode（"l"、"enter"、"esc"、"tab"、"?" 等）。
    pub(crate) fn parse_key(s: &str) -> Option<KeyCode> {
        // Shadowing as a pipeline: each `let s` replaces the previous
        // binding, so transformations chain without inventing temp names.
        // 变量遮蔽当流水线：每个 let s 覆盖前一个绑定，
        // 连续变换无需起临时变量名。
        let s = s.trim().to_ascii_lowercase();
        // Option chaining: strip '<', then '>' off THAT result; any step
        // failing yields None, and unwrap_or falls back to the untouched
        // string. Flat and null-safe — no nested `if (p = strchr(...))`.
        // Option 链：先剥 '<'，再对结果剥 '>'；任一步失败得 None，
        // unwrap_or 回退为原串。扁平且无空指针风险。
        let s = s.strip_prefix('<').and_then(|x| x.strip_suffix('>')).unwrap_or(&s);
        Some(match s {
            "enter" => KeyCode::Enter,
            "esc" => KeyCode::Esc,
            "tab" => KeyCode::Tab,
            "backtab" => KeyCode::BackTab,
            "up" => KeyCode::Up,
            "down" => KeyCode::Down,
            "left" => KeyCode::Left,
            "right" => KeyCode::Right,
            "space" => KeyCode::Char(' '),
            // match GUARD: an extra `if` condition attached to an arm.
            // Any single-char string binds as `other`; since the guard
            // proved length == 1, `.next()` cannot fail, so unwrap() is
            // safe (it would only panic on a logic bug).
            // match 守卫：分支上附加 if 条件。任意单字符字符串绑定到 other；
            // 守卫已证明长度为 1，next() 必然成功，unwrap() 安全
            // （只有在逻辑 bug 时才会 panic）。
            other if other.chars().count() == 1 => {
                KeyCode::Char(other.chars().next().unwrap())
            }
            _ => return None,
        })
    }

    /// Parse a key sequence string → Vec<KeyCode>: `"gg"` → [g, g],
    /// `"<enter>"` → [Enter], `"l"` → [l]. Sequences are at most 2 keys.
    /// 解析按键序列字符串 → Vec<KeyCode>："gg" → [g,g]，"<enter>" → [Enter]。
    /// 序列最多 2 个键。
    pub(crate) fn parse_seq(s: &str) -> Option<Vec<KeyCode>> {
        let t = s.trim();
        // angle-bracket names (<enter>) are one special key — delegate
        // 尖括号形式（<enter>）是单个特殊键 — 委托给 parse_key
        if t.starts_with('<') {
            return Action::parse_key(t).map(|k| vec![k]);
        }
        // collect into a Vec first so length can be checked cheaply
        // 先收集成 Vec 才能方便地检查长度
        let chars: Vec<char> = t.chars().collect();
        if chars.is_empty() || chars.len() > 2 {
            return None;
        }
        // with_capacity pre-sizes the Vec so pushes never reallocate
        // （同 base64_encode 注释）with_capacity 预分配容量，push 不再扩容
        let mut out = Vec::with_capacity(chars.len());
        for c in chars {
            out.push(KeyCode::Char(c));
        }
        Some(out)
    }
}

/// Build the user keymap: key → action (invalid entries skipped).
/// 构建用户键位表：按键序列 → 动作（无效条目直接跳过）。
///
/// `HashMap<K, V>` = C++ `unordered_map` / a C hash table; keys and values
/// are OWNED by the map. Lookups take `&K`, so querying costs no clone.
/// HashMap 类似 C++ 的 unordered_map / C 的哈希表，键值由表持有；
/// 查询只需引用，不用拷贝。
pub(crate) fn build_keymap(
    raw: &std::collections::HashMap<String, Vec<String>>,
) -> std::collections::HashMap<Vec<KeyCode>, Action> {
    let mut map = std::collections::HashMap::new();
    for (action, keys) in raw {
        // let-else: unwrap the Option, or `continue` to the next entry.
        // Replaces a whole match-and-continue ladder from C.
        // let-else：解包 Option，失败则 continue 跳过该条目。
        // 比 C 里 match 加 continue 更简洁。
        let Some(a) = Action::from_str(action) else { continue };
        for key in keys {
            if let Some(seq) = Action::parse_seq(key) {
                map.insert(seq, a);
            }
        }
    }
    map
}

impl App {
    /// Reset prefix-key and delete-armed state (help/input/ctrl paths return early).
    /// 清空前缀键缓冲与删除待确认状态（帮助/输入/Ctrl 路径会提前 return）。
    fn clear_pending(&mut self) {
        self.pending_keys.clear();
        self.delete_armed = false;
    }

    /// Execute a remapped action (user keybindings). Guards per pane.
    /// 执行重映射后的动作（用户键位）。用守卫按焦点面板限制生效范围。
    ///
    /// A big exhaustive match ≈ a supercharged switch. Each arm is
    /// `pattern if guard => expr`, and the compiler demands every case be
    /// covered — the trailing `_ => {}` no-op is that safety net.
    /// Guards like `if self.focus == 1` gate an arm to one pane; if the
    /// guard fails, matching simply continues downward.
    /// focus: 0 = nav tree, 1 = article list, 2 = article view.
    /// 大型穷尽 match ≈ 强化版 switch。分支 = "模式 if 守卫 => 表达式"，
    /// 编译器要求覆盖所有情况 — 末尾的 _ => {} 空操作就是兜底。
    /// focus：0 = 导航树，1 = 文章列表，2 = 文章视图。
    fn execute_action(&mut self, action: Action) {
        match action {
            Action::Open => self.go_right(),
            Action::Back => self.go_left(),
            Action::Quit => self.running = false,
            Action::Refresh => self.refresh_all(false, false),
            Action::RefreshAll => self.refresh_all(true, false),
            Action::ToggleRead => self.toggle_read(),
            Action::MarkListRead => self.mark_all_read(false),
            Action::MarkAllRead => self.mark_all_read(true),
            // match guard: fires only when focus == 1 (list pane);
            // otherwise matching continues downward. Same pattern below.
            // match 守卫：仅当 focus == 1（列表面板）才命中，否则继续向下匹配。下同。
            Action::ToggleReadNext if self.focus == 1 => self.toggle_read_and_next(),
            Action::Export => self.start_export(),
            Action::ExportSaved if self.focus == 0 => self.start_saved_export(),
            Action::Browser => self.open_browser(),
            Action::Favourite => match self.focus {
                0 => self.toggle_favourite_feed(),
                2 => self.fullscreen = !self.fullscreen,
                _ => {}
            },
            Action::ReadLater => match self.focus {
                0 => self.toggle_lazy_feed(),
                _ if self.focus >= 1 => self.toggle_item_flag("read_later"),
                _ => {}
            },
            Action::Saved if self.focus >= 1 => self.toggle_item_flag("saved"),
            Action::NewFeed if self.focus == 0 => self.start_input(InputMode::AddUrl),
            // two-press confirmation: first D only arms deletion and shows
            // a hint; second D actually deletes. `delete_armed` is the
            // single state bit of this tiny state machine.
            // 两段式确认：第一次 D 只进入待删状态并提示；第二次 D 才真删。
            // delete_armed 就是这个微型状态机的唯一状态位。
            Action::Delete if self.focus == 0 => {
                if self.delete_armed {
                    self.delete_armed = false;
                    self.delete_selected_feed();
                } else {
                    self.delete_armed = true;
                    // Option pipeline: .get may return None (out of
                    // bounds); .map extracts the feed name via pattern
                    // match on the row enum; unwrap_or_default yields ""
                    // instead of panicking.
                    // Option 流水线：get 越界得 None；map 对行枚举做模式匹配取源名；
                    // unwrap_or_default 失败时给空串而非 panic。
                    let name = self
                        .tree_rows
                        .get(self.tree_sel)
                        .map(|r| match r {
                            TreeRow::Feed(_, n, _)
                            | TreeRow::FavouriteFeed(_, n)
                            | TreeRow::LazyFeed(_, n)
                            | TreeRow::UncategorizedFeed(_, n) => n.clone(),
                            _ => String::new(),
                        })
                        .unwrap_or_default();
                    self.status = format!("press D again to delete {name}");
                }
            }
            Action::Rename if self.focus == 0 => {
                // dispatch on WHICH row type is selected: each TreeRow
                // variant carries different data, destructured right in
                // the patterns below.
                // 按选中行的枚举变体分发：各变体携带不同数据，在下方模式中直接解构。
                match self.tree_rows.get(self.tree_sel) {
                    Some(TreeRow::Category(_)) => self.start_input(InputMode::RenameCategory),
                    Some(TreeRow::Tag(_)) => self.start_input(InputMode::EditTag),
                    Some(TreeRow::Feed(url, _, _))
                    | Some(TreeRow::FavouriteFeed(url, _))
                    | Some(TreeRow::LazyFeed(url, _))
                    | Some(TreeRow::UncategorizedFeed(url, _)) => {
                        self.pending = Some(crate::PendingInput::EditTags { url: url.clone() });
                        self.start_input(InputMode::EditFeedTitle);
                    }
                    _ => {}
                }
            }
            Action::EditTags if self.focus == 0 => {
                // let-else + and_then again: "maybe fetch row" chained with
                // "maybe extract url"; any failure aborts via `return`.
                // 再次 let-else + and_then：把"取行"与"提取 url"两个可能失败的步骤串联，
                // 任一失败即 return 退出。
                let Some(url) = self.tree_rows.get(self.tree_sel).and_then(|r| match r {
                    TreeRow::Feed(u, _, _) | TreeRow::FavouriteFeed(u, _) | TreeRow::LazyFeed(u, _) | TreeRow::UncategorizedFeed(u, _) => Some(u.clone()),
                    _ => None,
                }) else {
                    return;
                };
                {
                    self.pending = Some(crate::PendingInput::EditTags { url });
                    self.start_input(InputMode::EditTags);
                }
            }
            Action::Help => self.show_help = true,
            // `% 3` wraps focus cyclically over the three panes (0→1→2→0).
            // FocusPrev adds 2 instead of subtracting 1, because usize
            // cannot go negative (0 - 1 would panic/underflow).
            // % 3 让 focus 在三个面板间循环（0→1→2→0）。
            // FocusPrev 用 +2 代替 −1，因为 usize 不能为负。
            Action::FocusNext => self.focus = (self.focus + 1) % 3,
            Action::Search if self.focus == 1 => {
                self.search_base = Some(self.scoped_items.clone());
                self.search_active = true;
                self.search_query.clear();
                self.start_input(InputMode::Search);
            }
            Action::JumpTop => match self.focus {
                0 => self.tree_sel = 0,
                1 => {
                    self.list_sel = 0;
                    self.list_offset = 0;
                    self.article_scroll = 0;
                }
                2 => self.article_scroll = 0,
                _ => {}
            },
            // u16::MAX is the renderer's "scroll to very bottom" sentinel;
            // saturating_sub prevents usize 0 - 1 underflow (panics in debug).
            // u16::MAX 是渲染层的"滚到底"哨兵值；saturating_sub 防止 usize 下溢（调试模式下会 panic）。
            Action::JumpBottom => match self.focus {
                0 => self.tree_sel = self.tree_rows.len().saturating_sub(1),
                1 => {
                    self.list_sel = self.scoped_items.len().saturating_sub(1);
                    self.article_scroll = 0;
                }
                2 => self.article_scroll = u16::MAX,
                _ => {}
            },
            Action::NextUnread if self.focus == 1 => self.mark_read_and_jump(1),
            Action::PrevUnread if self.focus == 1 => self.mark_read_and_jump(-1),
            // parent navigation: article → list cursor, list → nav cursor
            Action::ParentNext => match self.focus {
                2 => {
                    self.move_list_sel(1);
                    self.mark_current_read();
                }
                1 => self.move_nav_sel(1),
                _ => {}
            },
            Action::ParentPrev => match self.focus {
                2 => {
                    self.move_list_sel(-1);
                    self.mark_current_read();
                }
                1 => self.move_nav_sel(-1),
                _ => {}
            },
            // The copy_* arms share one skeleton (same pattern below):
            // current_item() → Option; `if let Some` runs the body only
            // when an item is selected. `if let` = a match with exactly
            // one pattern of interest.
            // copy_* 系列共用同一骨架（下同）：current_item() 返回 Option，
            // 有选中条目才执行主体。if let = 只关心一个模式的轻量 match。
            Action::CopyItemUrl if self.focus >= 1 => {
                if let Some((_, item)) = self.current_item() {
                    copy_to_clipboard(&item.url);
                    self.status = "copied item url".into();
                }
            }
            Action::CopyItemTitle if self.focus >= 1 => {
                if let Some((_, item)) = self.current_item() {
                    copy_to_clipboard(item.display_title());
                    self.status = "copied item title".into();
                }
            }
            Action::CopyItemSummary if self.focus >= 1 => {
                if let Some((_, item)) = self.current_item() {
                    let md = self.article_markdown_summary(&item);
                    if md.is_empty() {
                        self.status = "no summary to copy".into();
                    } else {
                        copy_to_clipboard(&md);
                        self.status = "copied summary".into();
                    }
                }
            }
            Action::CopyItemContent if self.focus >= 1 => {
                if let Some((_, item)) = self.current_item() {
                    let md = self.article_markdown_body(&item);
                    if md.is_empty() {
                        self.status = "no content to copy".into();
                    } else {
                        copy_to_clipboard(&md);
                        self.status = "copied full content".into();
                    }
                }
            }
            Action::CopyFeedUrl => {
                // inner match as expression: yields an Option<String> for
                // the feed url depending on which pane we are in
                // 内层 match 直接产出 Option<String>（按所在面板选择取 url 的方式）
                // — 再次体现 match 是表达式。
                let url = match self.focus {
                    0 => self.tree_rows.get(self.tree_sel).and_then(|r| match r {
                        TreeRow::Feed(u, _, _) | TreeRow::FavouriteFeed(u, _) | TreeRow::LazyFeed(u, _) | TreeRow::UncategorizedFeed(u, _) => Some(u.clone()),
                        _ => None,
                    }),
                    _ => self.current_item().map(|(u, _)| u),
                };
                if let Some(u) = url {
                    copy_to_clipboard(&u);
                    self.status = "copied feed url".into();
                }
            }
            Action::Sort { level, reverse } if self.focus == 1 => {
                self.push_sort(level, reverse)
            }
            Action::FocusPrev => self.focus = (self.focus + 2) % 3,
            Action::CyclePreset if self.focus == 0 => self.cycle_preset(),
            Action::ImportOpml => self.start_input(InputMode::ImportOpml),
            Action::ExportOpml => self.export_opml(),
            _ => {}
        }
    }

    /// Mark the current item read and clear its read-later (article view).
    /// 标记当前条目已读并清除其稍后读标记（文章视图下）。
    fn mark_current_read(&mut self) {
        // let-else: no selected item → nothing to do.
        // let-else：没有选中条目则直接返回。
        let Some((url, item)) = self.current_item() else { return };
        self.db.set_read(&url, &item.guid, true).ok();
        if item.read_later {
            self.db.set_flag(&url, &item.guid, "read_later", false).ok();
        }
        self.mark_scoped_read(&url, &item.guid, true);
    }

    /// Move the list selection one step (article preview follows).
    /// 列表选中移动一步（右侧文章预览跟随刷新）。
    ///
    /// `isize` = signed size type. Indices are usize (cannot be negative),
    /// so arithmetic that may dip below zero runs in isize and is
    /// range-checked BEFORE casting back — no C-style wraparound bugs.
    /// isize 是有符号长度类型。下标是 usize（不能为负），
    /// 可能变负的计算先在 isize 中进行，检查范围后再转回，杜绝回绕 bug。
    pub(crate) fn move_list_sel(&mut self, dir: isize) {
        let n = self.scoped_items.len() as isize;
        if n == 0 {
            return;
        }
        let idx = self.list_sel as isize + dir;
        if idx >= 0 && idx < n {
            self.list_sel = idx as usize;
            self.article_scroll = 0;
        }
    }

    /// Move the nav selection one step (list preview follows).
    /// 导航树选中移动一步（列表预览跟随刷新）。同样的 isize 安全移动套路。
    fn move_nav_sel(&mut self, dir: isize) {
        let n = self.tree_rows.len() as isize;
        if n == 0 {
            return;
        }
        let idx = self.tree_sel as isize + dir;
        if idx >= 0 && idx < n {
            self.tree_sel = idx as usize;
            self.preview_scope();
        }
    }

    /// Central key dispatcher — the app's input state machine.
    /// 中央按键分发器 — 应用的输入状态机。
    ///
    /// Stages in priority order, each usually ending in an early `return`
    /// (flat exits instead of nested ifs — compare C's deep else-chains):
    /// 1. help overlay open → help-only keys
    /// 2. text-input prompt open → editing keys
    /// 3. Ctrl chord → built-in scroll shortcuts (not remappable)
    /// 4. key buffer → longest-match multi-key sequence ("gg")
    /// 5. unbound prefix → wait for more keys / show hint
    /// 6. single key → user binding via keymap
    /// 7. leftover key → pane-local default handler (j/k/scroll)
    /// 按优先级逐级处理，各阶段多以提前 return 收尾（扁平退出代替嵌套 if）：
    /// 帮助浮层 → 输入框 → Ctrl 组合键 → 按键缓冲序列 → 未完序列等待 →
    /// 单键用户绑定 → 面板内默认按键。
    pub(crate) fn on_key(&mut self, key: KeyCode, mods: KeyModifiers) {
        // Stage 1: while the help overlay is visible it swallows all keys.
        // 阶段 1：帮助浮层可见时吞掉所有按键。
        if self.show_help {
            // `|` ORs several patterns into one arm; `_ => {}` ignores the rest.
            // 一个分支可用 | 合并多个模式；_ => {} 忽略其余按键。
            match key {
                KeyCode::Char('?') | KeyCode::Char('q') | KeyCode::Esc => self.show_help = false,
                KeyCode::Char('j') | KeyCode::Down => self.help_scroll += 1,
                KeyCode::Char('k') | KeyCode::Up => {
                    // saturating_sub(1): stops at 0 instead of underflowing
                    // (unsigned underflow panics in debug builds)
                    // saturating_sub(1)：到 0 为止不再减，防止无符号下溢（调试模式下溢会 panic）
                    self.help_scroll = self.help_scroll.saturating_sub(1)
                }
                _ => {}
            }
            self.clear_pending();
            return;
        }
        // Stage 2: a text-input prompt is open. `.take()` removes the value
        // out of the Option (leaving None), giving us OWNERSHIP of the prompt
        // while editing — a borrow-checker-friendly move; arms below put it
        // back with `self.input = Some(prompt)` when done.
        // 阶段 2：输入框打开中。take() 把值从 Option 中取出（留 None），
        // 编辑期间取得其所有权 — 对借用检查器友好；各分支处理完再放回去。
        if let Some(mut prompt) = self.input.take() {
            let search_mode = prompt.mode == InputMode::Search;
            match key {
                KeyCode::Esc => {
                    if search_mode {
                        self.cancel_search();
                    }
                }
                KeyCode::Enter if search_mode => {
                    // keep the filter active (left stops it); keep the base
                    // snapshot so left can restore the full list
                }
                KeyCode::Enter => {
                    self.input = Some(prompt);
                    self.submit_input();
                    return;
                }
                KeyCode::Backspace => {
                    // String::pop() removes the last CHAR (UTF-8 aware),
                    // never a stray half of a multibyte character.
                    // pop() 删除最后一个字符（按 UTF-8 字符计），不会切坏多字节字符。
                    prompt.buf.pop();
                    if search_mode {
                        self.apply_search_filter(&prompt.buf);
                    }
                    self.input = Some(prompt);
                }
                KeyCode::Char(c) => {
                    prompt.buf.push(c);
                    if search_mode {
                        self.apply_search_filter(&prompt.buf);
                    }
                    self.input = Some(prompt);
                }
                _ => {
                    self.input = Some(prompt);
                }
            }
            self.clear_pending();
            return;
        }
        // ctrl chords are never buffered/rebindable
        // Ctrl 组合键不进按键缓冲、不可重绑定
        if mods.contains(KeyModifiers::CONTROL) {
            match key {
                // vi-style paging: Ctrl+f forward, Ctrl+b back (list/article
                // panes only). Any other ctrl key falls to article scrolling.
                // vi 风格翻页：Ctrl+f 下翻、Ctrl+b 上翻（仅列表/文章面板）；
                // 其余 Ctrl 键交给文章滚动逻辑。
                KeyCode::Char('f') | KeyCode::Char('b') if self.focus >= 1 => {
                    let dir = if key == KeyCode::Char('f') { 1 } else { -1 };
                    self.page_scroll(dir);
                }
                _ if self.focus == 2 => self.article_scroll_ctrl(key),
                _ => {}
            }
            self.clear_pending();
            return;
        }
        // combo/key buffer: accumulate key presses and match the longest
        // bound sequence (single keys and combos like gg / st / yy live in
        // the same map). Ctrl chords are handled above and never buffered.
        // 按键缓冲：累积按键并对键位表做最长匹配（单键与 gg/st/yy 等组合在同一张表里）。
        // Ctrl 组合键在上面已处理，不会进入缓冲。
        self.pending_keys.push(key);
        // keep at most 2 pending keys (= longest sequence length);
        // remove(0) drops the OLDEST press, like a ring buffer shift
        // 缓冲最多保留 2 个键（= 最长序列长度）；remove(0) 移除最早的按键，类似环形缓冲前移。
        if self.pending_keys.len() > 2 {
            self.pending_keys.remove(0);
        }
        // exact-sequence lookup: the whole pending buffer as the map key.
        // `if let Some(&action)` copies the Action out of the map (it's Copy).
        // 以整段缓冲为键做精确查找；if let Some(&action) 把动作拷出（Action 实现了 Copy）。
        if let Some(&action) = self.keymap.get(&self.pending_keys) {
            self.clear_pending();
            self.execute_action(action);
            return;
        }
        // a longer bound sequence starts with what we have — keep waiting
        // 键位表中存在以当前缓冲为前缀的更长序列 — 继续等待后续按键
        // `.any()` short-circuits on the first true; the closure checks
        // both "longer than ours" and "starts with ours".
        // any() 遇到第一个 true 即短路；闭包同时检查"更长"与"以此为前缀"。
        let is_prefix = self
            .keymap
            .keys()
            .any(|k| k.len() > self.pending_keys.len() && k.starts_with(&self.pending_keys));
        if is_prefix {
            // slice PATTERN matching: [x] matches a one-element buffer,
            // just like pattern matching an array in other languages.
            // 切片模式匹配：[x] 匹配单元素缓冲，与其他语言的数组模式匹配同理。
            match self.pending_keys.as_slice() {
                [KeyCode::Char('y')] => {
                    self.status = "y: yy url · yn title · yp feed · ys summary · yc content".into();
                }
                [KeyCode::Char('s')] => {
                    self.status = "s: st/sn/sf/su forward · sT/sN/sF/sU reversed".into();
                }
                _ => {}
            }
            return;
        }
        // no combo — treat the last key alone (after clearing the buffer)
        // 不是任何组合的前缀 — 清空缓冲后按单键重试
        self.clear_pending();
        if let Some(&action) = self.keymap.get(&vec![key]) {
            self.execute_action(action);
            return;
        }
        // pane-local keys (j/k movement, scroll, etc.)
        // 面板内默认按键（j/k 移动、滚动等）— 按当前焦点面板分发
        match self.focus {
            0 => self.nav_key(key),
            1 => self.list_key(key),
            2 => self.article_key(key),
            _ => {}
        }
    }
    /// Toggle favourite flag on the nav-selected feed.
    /// 切换导航树中所选源的收藏标记。
    fn toggle_favourite_feed(&mut self) {
        // let-else + and_then: row → Option<url>; non-feed rows abort here.
        // let-else + and_then：从行取 url，非源行直接返回。
        let Some(url) = self.tree_rows.get(self.tree_sel).and_then(|r| match r {
            TreeRow::Feed(u, _, _) | TreeRow::FavouriteFeed(u, _) | TreeRow::LazyFeed(u, _) | TreeRow::UncategorizedFeed(u, _) => Some(u.clone()),
            _ => None,
        }) else {
            return;
        };
        // The bare `{ ... }` block SCOPES the &mut borrow of self.feeds:
        // after it ends we call save_urls()/rebuild_tree(), which also need
        // &mut self. Rust allows sequential borrows, never simultaneous —
        // the block is how you express "borrow ends here".
        // 裸块限定 self.feeds 可变借用的作用域：块结束后再调 save_urls/
        // rebuild_tree（同样需要 &mut self）。Rust 允许借用串行存在，
        // 裸块就是"借用到此为止"的写法。
        let new_state = {
            let Some(f) = self.feeds.feeds.iter_mut().find(|f| f.url == url) else {
                return;
            };
            f.favourite = !f.favourite;
            f.favourite
        };
        self.save_urls();
        self.rebuild_tree();
        self.status = if new_state { "favourited".into() } else { "unfavourited".into() };
    }

    /// Toggle lazy flag on the nav-selected feed (`L` in nav pane; urls-file `!lazy`).
    /// 切换所选源的 lazy 标记（导航面板按 L；urls 文件里写 !lazy）。
    /// Lazy 源不随 refresh_all 自动刷新。结构与 toggle_favourite_feed 相同。
    fn toggle_lazy_feed(&mut self) {
        let Some(url) = self.tree_rows.get(self.tree_sel).and_then(|r| match r {
            TreeRow::Feed(u, _, _)
            | TreeRow::FavouriteFeed(u, _)
            | TreeRow::LazyFeed(u, _)
            | TreeRow::UncategorizedFeed(u, _) => Some(u.clone()),
            _ => None,
        }) else {
            return;
        };
        let new_state = {
            let Some(f) = self.feeds.feeds.iter_mut().find(|f| f.url == url) else {
                return;
            };
            f.lazy = !f.lazy;
            f.lazy
        };
        self.save_urls();
        self.rebuild_tree();
        self.status = if new_state { "marked lazy".into() } else { "unmarked lazy".into() };
    }

    /// Toggle read_later / saved flag on the current item.
    /// In-place snapshot update — the list keeps its current order/selection;
    /// flagged items leave the view only after refresh or scope change.
    /// 切换当前条目的稍后读/收藏标记。
    /// 就地更新快照 — 列表保持现有顺序与选中位置；
    /// 被标记的条目要等刷新或切换范围后才离开视图。
    fn toggle_item_flag(&mut self, flag: &str) {
        let Some((url, item)) = self.current_item() else { return };
        let on = self.db.toggle_flag(&url, &item.guid, flag).unwrap_or(false);
        if flag == "read_later" && on {
            // marking read-later also marks unread
            self.db.set_read(&url, &item.guid, false).ok();
        }
        // patch the in-memory snapshot so the UI updates without refetch
        // 就地修改内存快照，UI 立即生效而无需重新查询
        for (u, i) in self.scoped_items.iter_mut() {
            if u == &url && i.guid == item.guid {
                if flag == "read_later" {
                    i.read_later = on;
                    if on {
                        i.read = false;
                    }
                } else if flag == "saved" {
                    i.saved = on;
                }
            }
        }
    }

    /// Left: article→list→nav→parent in file tree.
    /// 左方向键：文章 →列表 → 导航树 → 上级目录。
    fn go_left(&mut self) {
        // an active search is stopped by left; list stays until next left
        // 第一次左键先关闭搜索过滤；列表保持不动，再次左键才离开
        if self.focus == 1 && self.search_active {
            self.cancel_search();
            return;
        }
        match self.focus {
            2 => {
                if self.fullscreen {
                    self.fullscreen = false;
                }
                self.focus = 1;
            }
            1 => self.focus = 0,
            _ => self.nav_left(),
        }
    }

    /// Right: expand tree→list→article→fetch full.
    /// 右方向键：展开树节点 → 打开文章 → 抓取全文。
    fn go_right(&mut self) {
        match self.focus {
            0 => self.nav_right(),
            1 => self.open_item(),
            _ => self.fetch_article(),
        }
    }

    /// Nav left: expanded node → fold; folded node → fold its parent;
    /// feed → fold its containing container; top-level folded → stay.
    /// 导航左键：已展开节点 → 折叠；已折叠节点 → 折叠其父级；
    /// 源条目 → 折叠其所在容器；顶层已折叠 → 不动。
    fn nav_left(&mut self) {
        // `.cloned()` upgrades Option<&TreeRow> to Option<TreeRow> so we
        // own a copy — avoids fighting the borrow checker while mutating
        // self.collapsed / tree_rows below.
        // cloned() 把 Option<&TreeRow> 变成自有拷贝，避免下方修改
        // collapsed/tree_rows 时与借用冲突。
        let Some(row) = self.tree_rows.get(self.tree_sel).cloned() else {
            return;
        };
        match row {
            TreeRow::Section(name) => {
                // top-level: expanded → fold; folded → stay
                // 顶层节点：已展开 → 折叠；已折叠 → 不动
                if !self.collapsed.contains(&name) {
                    self.collapsed.insert(name);
                    self.rebuild_tree();
                }
            }
            TreeRow::Favourite => {
                if self.fav_expanded {
                    self.fav_expanded = false;
                    self.rebuild_tree();
                }
            }
            TreeRow::Lazy => {
                if self.lazy_expanded {
                    self.lazy_expanded = false;
                    self.rebuild_tree();
                }
            }
            TreeRow::Uncategorized => {
                if self.uncat_expanded {
                    self.uncat_expanded = false;
                    self.rebuild_tree();
                }
            }
            TreeRow::Category(cat) => {
                if !self.collapsed.contains(&cat) {
                    self.collapsed.insert(cat.clone());
                    self.rebuild_tree();
                // rfind('/') finds the LAST '/' — cat[..i] is the parent
                // path ("a/b/c" → "a/b"). Byte-index slicing is safe here:
                // '/' is ASCII, so the split point never lands inside a
                // multibyte UTF-8 character.
                // rfind('/') 找最后一个 '/'，cat[..i] 即父路径（"a/b/c" → "a/b"）。
                // 按字节下标切片在此安全：'/' 是 ASCII，切分点不会落入多字节字符中间。
                } else if let Some(parent) = cat.rfind('/').map(|i| cat[..i].to_string()) {
                    // already folded — fold the parent category instead
                    // 已折叠 — 改为折叠其父分类
                    self.collapsed.insert(parent.clone());
                    self.rebuild_tree();
                    if let Some(idx) = self
                        .tree_rows
                        .iter()
                        .position(|r| matches!(r, TreeRow::Category(c) if c == &parent))
                    {
                        self.tree_sel = idx;
                    }
                } else {
                    // top-level folded category — fold the Categories section
                    // 顶级分类已折叠 — 折叠 Categories 分节
                    self.collapsed.insert("Categories".to_string());
                    self.rebuild_tree();
                    if let Some(idx) = self
                        .tree_rows
                        .iter()
                        .position(|r| matches!(r, TreeRow::Section(n) if n == "Categories"))
                    {
                        self.tree_sel = idx;
                    }
                }
            }
            TreeRow::Tag(t) => {
                let key = format!("tag:{t}");
                if !self.collapsed.contains(&key) {
                    self.collapsed.insert(key);
                    self.rebuild_tree();
                } else {
                    // fold the Tags section (parent)
                    // 折叠 Tags 分节（父容器）
                    self.collapsed.insert("Tags".to_string());
                    self.rebuild_tree();
                    if let Some(idx) = self
                        .tree_rows
                        .iter()
                        .position(|r| matches!(r, TreeRow::Section(n) if n == "Tags"))
                    {
                        self.tree_sel = idx;
                    }
                }
            }
            TreeRow::Feed(_, _, _)
            | TreeRow::FavouriteFeed(_, _)
            | TreeRow::LazyFeed(_, _)
            | TreeRow::UncategorizedFeed(_, _) => {
                // jump to and fold the nearest container above this row
                // walk UPWARD from the selected feed; the first container
                // row found gets folded and selected. Each arm folds one
                // container kind — same pattern repeated throughout.
                // 从选中的源向上回溯，遇到的第一个容器行被折叠并选中。
                // 各分支折叠一种容器类型 — 同模式重复（不再逐条注释）。
                for j in (0..self.tree_sel).rev() {
                    match &self.tree_rows[j] {
                        TreeRow::Category(c) => {
                            self.collapsed.insert(c.clone());
                            self.rebuild_tree();
                            self.tree_sel = j;
                            return;
                        }
                        TreeRow::Tag(t) => {
                            self.collapsed.insert(format!("tag:{t}"));
                            self.rebuild_tree();
                            self.tree_sel = j;
                            return;
                        }
                        TreeRow::Favourite => {
                            self.fav_expanded = false;
                            self.rebuild_tree();
                            self.tree_sel = j;
                            return;
                        }
                        TreeRow::Lazy => {
                            self.lazy_expanded = false;
                            self.rebuild_tree();
                            self.tree_sel = j;
                            return;
                        }
                        TreeRow::Uncategorized => {
                            self.uncat_expanded = false;
                            self.rebuild_tree();
                            self.tree_sel = j;
                            return;
                        }
                        TreeRow::Section(n) => {
                            self.collapsed.insert(n.clone());
                            self.rebuild_tree();
                            self.tree_sel = j;
                            return;
                        }
                        _ => {}
                    }
                }
            }
            // Unread / Read Later / Saved: no fold, stay
            // Unread/Read Later/Saved 行无可折叠内容 — 原地不动
            _ => {}
        }
    }

    /// Nav right: collapsed section/category/favourite → expand; else descend.
    /// 导航右键：折叠的分节/分类/收藏 → 展开；否则进入对应列表。
    ///
    /// Note the guard ordering: `Some(X) if condition` arms (expand-if-
    /// folded) are checked BEFORE the plain `Some(X)` arms (already
    /// expanded). Match arms run top to bottom — first match wins.
    /// 注意守卫顺序：带 if 守卫的分支（折叠则展开）排在无条件分支
    /// （已展开）之前。match 分支自上而下，先命中先赢。
    fn nav_right(&mut self) {
        match self.tree_rows.get(self.tree_sel).cloned() {
            Some(TreeRow::Section(name)) if self.collapsed.contains(&name) => {
                self.collapsed.remove(&name);
                self.rebuild_tree();
            }
            Some(TreeRow::Section(_)) => {}
            Some(TreeRow::Category(cat)) if self.collapsed.contains(&cat) => {
                self.collapsed.remove(&cat);
                self.rebuild_tree();
            }
            Some(TreeRow::Favourite) if !self.fav_expanded => {
                self.fav_expanded = true;
                self.rebuild_tree();
            }
            Some(TreeRow::Favourite) => {
                // already expanded — descend into the aggregate list
                // 已展开 — 进入该聚合列表
                self.select_scope(&TreeRow::Favourite.clone());
            }
            Some(TreeRow::Lazy) if !self.lazy_expanded => {
                self.lazy_expanded = true;
                self.rebuild_tree();
            }
            Some(TreeRow::Lazy) => {
                // already expanded — descend into the aggregate list
                // 已展开 — 进入该聚合列表
                self.select_scope(&TreeRow::Lazy.clone());
            }
            Some(TreeRow::Uncategorized) if !self.uncat_expanded => {
                self.uncat_expanded = true;
                self.rebuild_tree();
            }
            Some(TreeRow::Tag(t)) if self.collapsed.contains(&format!("tag:{t}")) => {
                self.collapsed.remove(&format!("tag:{t}"));
                self.rebuild_tree();
            }
            Some(row) => self.select_scope(&row),
            None => {}
        }
    }

    /// After expanding a fold, move the cursor to its first child row.
    /// （原文注释如此；实际实现为 j/k 移动导航选中行，preview_scope 刷新预览。）
    /// 导航面板默认按键：j/k（等价方向键）上下移动。
    fn nav_key(&mut self, key: KeyCode) {
        match key {
            // j (down): explicit bounds check — usize can't go negative,
            // but it CAN overflow past the end, so check before adding.
            // j（下移）：显式越界检查 — usize 不会变负，但可能越过末尾，先检查再加。
            KeyCode::Char('j') | KeyCode::Down => {
                if self.tree_sel + 1 < self.tree_rows.len() {
                    self.tree_sel += 1;
                    self.preview_scope();
                }
            }
            // k (up): saturating_sub stops at 0 — can't scroll above top
            // k（上移）：saturating_sub 到 0 为止，不会滚出顶部
            KeyCode::Char('k') | KeyCode::Up => {
                self.tree_sel = self.tree_sel.saturating_sub(1);
                self.preview_scope();
            }
            _ => {}
        }
    }

    /// Pane-local list keys: j/k move selection; article scroll resets.
    /// 列表面板默认按键：j/k 移动选中行，同时文章滚动归零。
    /// 与 nav_key 同构 — 只是操作的字段不同。
    fn list_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('j') | KeyCode::Down => {
                if self.list_sel + 1 < self.scoped_items.len() {
                    self.list_sel += 1;
                    self.article_scroll = 0;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.list_sel = self.list_sel.saturating_sub(1);
                self.article_scroll = 0;
            }
            _ => {}
        }
    }

    /// Mark the current item read, then select the next/previous unread
    /// item in the current list (no reorder).
    /// 标记当前条目已读，然后跳到当前列表中下/上一个未读条目（不重排）。
    fn mark_read_and_jump(&mut self, dir: isize) {
        if self.scoped_items.is_empty() {
            return;
        }
        let (url, item) = self.scoped_items[self.list_sel].clone();
        self.db.set_read(&url, &item.guid, true).ok();
        if item.read_later {
            self.db.set_flag(&url, &item.guid, "read_later", false).ok();
        }
        self.mark_scoped_read(&url, &item.guid, true);
        // linear scan in `dir`; stops at either end or on the first
        // unread item; if none found we stay put. i is isize so a
        // negative dir can be added safely.
        // 沿 dir 方向线性扫描，到头或遇到未读即停；找不到则原地不动。
        // i 用 isize 才能安全加负方向。
        let n = self.scoped_items.len() as isize;
        let mut i = self.list_sel as isize;
        loop {
            i += dir;
            if i < 0 || i >= n {
                // no unread in that direction — stay on current
                break;
            }
            let (_, it) = &self.scoped_items[i as usize];
            if !it.read {
                self.list_sel = i as usize;
                self.article_scroll = 0;
                break;
            }
        }
    }

    /// Pane-local article keys: j/k scroll lines, n/p jump items.
    /// 文章面板默认按键：j/k 逐行滚动，n/p 跳上/下一篇。
    fn article_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('j') | KeyCode::Down => self.article_scroll = self.article_scroll.saturating_add(1),
            KeyCode::Char('k') | KeyCode::Up => self.article_scroll = self.article_scroll.saturating_sub(1),
            KeyCode::Char('n') | KeyCode::PageDown => self.next_prev_item(1),
            KeyCode::Char('p') | KeyCode::PageUp => self.next_prev_item(-1),
            _ => {}
        }
    }

    /// Ctrl+u / Ctrl+d: half-page scroll (vi convention).
    /// Ctrl+u / Ctrl+d：半页滚动（vi 惯例）。
    fn article_scroll_ctrl(&mut self, key: KeyCode) {
        let half = (self.article_area.height.saturating_sub(4) / 2).max(1);
        match key {
            KeyCode::Char('u') => self.article_scroll = self.article_scroll.saturating_sub(half),
            KeyCode::Char('d') => self.article_scroll = self.article_scroll.saturating_add(half),
            _ => {}
        }
    }

    /// Ctrl+f / Ctrl+b: full-page move — list selection or article scroll.
    /// Ctrl+f / Ctrl+b：整页移动 — 列表选中或文章滚动。
    fn page_scroll(&mut self, dir: isize) {
        let page = (self.article_area.height.saturating_sub(4)).max(1) as isize;
        match self.focus {
            1 => {
                let n = self.scoped_items.len() as isize;
                if n > 0 {
                    // clamp pins the result into the legal range:
                    // selection stays within 0..=n-1; scroll below uses max(0)
                    // clamp 把结果夹回合法区间：选中下标限 0..=n-1；下方滚动用 max(0) 保底。
                    self.list_sel =
                        (self.list_sel as isize + dir * page).clamp(0, n - 1) as usize;
                    self.article_scroll = 0;
                }
            }
            2 => {
                self.article_scroll =
                    (self.article_scroll as isize + dir * page).max(0) as u16;
            }
            _ => {}
        }
    }

    /// Apply the current search query to the snapshot taken when `/` opened.
    /// 把当前搜索词应用到按下 / 时的快照上进行过滤。
    ///
    /// `search_base` holds the pre-search list; filtering always rebuilds
    /// from it, so edits to the query never compound.
    /// search_base 保存搜索前列表；每次过滤都从它重建，查询词变化不会叠加。
    pub(crate) fn apply_search_filter(&mut self, q: &str) {
        self.search_query = q.to_string();
        let Some(base) = &self.search_base else { return };
        let q = q.trim().to_lowercase();
        if q.is_empty() {
            self.scoped_items = base.clone();
        } else {
            // case-insensitive filter over title OR summary; matching rows
            // are cloned into a fresh Vec (base stays intact for restore)
            // 对标题或摘要做不区分大小写的过滤；命中的克隆进新 Vec（base 保持原样以便恢复）。
            self.scoped_items = base
                .iter()
                .filter(|(_, i)| {
                    i.title.to_lowercase().contains(&q)
                        || i.summary.to_lowercase().contains(&q)
                })
                .cloned()
                .collect();
        }
        self.list_sel = 0;
        self.list_offset = 0;
        self.article_scroll = 0;
    }

    /// Re-run the active search filter after list mutations (append/rebuild).
    /// 列表变动（追加/重建）后重跑当前过滤。
    pub(crate) fn reapply_search_filter(&mut self) {
        if self.search_active {
            let q = self.search_query.clone();
            self.apply_search_filter(&q);
        }
    }

    /// Esc from the search box — restore the pre-search list.
    /// 在搜索框按 Esc — 恢复搜索前的列表。
    fn cancel_search(&mut self) {
        self.search_active = false;
        self.search_query.clear();
        // take() removes & returns the snapshot; restoring it as the view
        // take() 取走快照并作为视图恢复
        if let Some(base) = self.search_base.take() {
            self.scoped_items = base;
        } else {
            // snapshot already dropped (enter kept the filter) — rebuild the
            // full scope list from scratch (active is false, so no re-filter)
            // 快照已被 take 掉（Enter 保留了过滤结果）— 从头重建完整列表
            // （search_active 已为 false，不会再套用过滤器）
            self.rebuild_list();
        }
        self.list_sel = 0;
        self.list_offset = 0;
    }

    /// Push a sort level (last pressed = highest priority); keep last 3.
    /// `reverse` inverts that level's direction (sT = time ascending).
    /// 压入一个排序层级（后按的优先级高）；最多保留若干层级。
    /// reverse 反转该层级方向（sT = 时间升序）。详见 apply_sort 的消费方式。
    fn push_sort(&mut self, level: &str, reverse: bool) {
        // retain keeps elements whose closure returns true — here it drops
        // any older entry for the same level, so re-pressing moves it to front.
        // retain 保留闭包为 true 的元素 — 此处删除同层级旧条目，重复按压相当于提到最前。
        self.sort_stack.retain(|(l, _)| l != level);
        self.sort_stack.insert(0, (level.to_string(), reverse));
        self.sort_stack.truncate(2);
        self.rebuild_list();
        let shown: Vec<String> = self
            .sort_stack
            .iter()
            .map(|(l, r)| format!("{}{}", if *r { "-" } else { "" }, l))
            .collect();
        self.status = format!("sort: {}", shown.join(" > "));
    }

    /// Sort the current list snapshot by the sort stack (no DB changes).
    /// 按排序栈对当前列表快照排序（不改数据库）。
    pub(crate) fn apply_sort(&mut self) {
        if self.sort_stack.is_empty() {
            return;
        }
        // precompute read flags once (comparator must not hit the DB)
        // 预先收集已读标记集合一次 — 比较器闭包内不应访问数据库
        let read_set: std::collections::HashSet<(String, String)> = self
            .scoped_items
            .iter()
            .filter(|(_, i)| i.read)
            .map(|(u, i)| (u.clone(), i.guid.clone()))
            .collect();
        // sort_by takes a comparator closure returning Ordering
        // (Less/Equal/Greater) — like C's qsort cmp function, but the
        // closure CAPTURES read_set instead of needing a void* ctx arg.
        // Walks the sort stack; the first non-equal level decides.
        // sort_by 接收返回 Ordering 的比较闭包 — 类似 qsort 的 cmp 函数，
        // 但闭包直接捕获 read_set，无需 void* 上下文参数。依次查排序栈，第一个分出胜负的层级生效。
        self.scoped_items.sort_by(|(ua, a), (ub, b)| {
            let mut ord = std::cmp::Ordering::Equal;
            for (level, reverse) in &self.sort_stack {
                let mut o = match level.as_str() {
                    "time" => b.date.cmp(&a.date), // newest first
                    "title" => a.title.cmp(&b.title),
                    "feed" => ua.cmp(ub),
                    "unread" => {
                        let ra = read_set.contains(&(ua.clone(), a.guid.clone()));
                        let rb = read_set.contains(&(ub.clone(), b.guid.clone()));
                        ra.cmp(&rb)
                    }
                    _ => ord,
                };
                if *reverse {
                    o = o.reverse(); // flips Less↔Greater, Equal unchanged
                }                 // 反转大小于，Equal 不变
                if o != std::cmp::Ordering::Equal {
                    ord = o;
                    break;
                }
            }
            ord
        });
    }
}
// Unit tests — compiled only with `cargo test`. These cover the config
// parsers and keymap builder (pure functions, no TUI needed).
// 单元测试 — 仅 cargo test 时编译。覆盖配置解析与键位表构建（纯函数，无需 TUI）。
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_actions() {
        assert_eq!(Action::from_str("open"), Some(Action::Open));
        assert_eq!(Action::from_str("refresh_all"), Some(Action::RefreshAll));
        assert_eq!(Action::from_str("mark_list_read"), Some(Action::MarkListRead));
        assert_eq!(Action::from_str("export_saved"), Some(Action::ExportSaved));
        assert_eq!(Action::from_str("mark_all_read"), Some(Action::MarkAllRead));
        assert_eq!(Action::from_str("copy_item_summary"), Some(Action::CopyItemSummary));
        assert_eq!(Action::from_str("copy_item_content"), Some(Action::CopyItemContent));
        assert_eq!(Action::from_str("bogus"), None);
    }

    #[test]
    fn parse_keys() {
        assert_eq!(Action::parse_key("l"), Some(KeyCode::Char('l')));
        assert_eq!(Action::parse_key("L"), Some(KeyCode::Char('l')));
        assert_eq!(Action::parse_key("enter"), Some(KeyCode::Enter));
        assert_eq!(Action::parse_key("esc"), Some(KeyCode::Esc));
        assert_eq!(Action::parse_key("??"), None);
    }

    #[test]
    fn keymap_skips_invalid() {
        let mut raw = std::collections::HashMap::new();
        raw.insert("open".to_string(), vec!["o".to_string(), "<enter>".to_string()]);
        raw.insert("bogus".to_string(), vec!["x".to_string()]);
        raw.insert("help".to_string(), vec!["xyz".to_string()]);
        let m = build_keymap(&raw);
        assert_eq!(m.len(), 2); // o + enter, both → open
        assert_eq!(m.get(&vec![KeyCode::Char('o')]), Some(&Action::Open));
        assert_eq!(m.get(&vec![KeyCode::Enter]), Some(&Action::Open));
    }

    #[test]
    fn default_keymap_binds_mark_read() {
        let m = build_keymap(&crate::config::default_keybindings());
        assert_eq!(m.get(&vec![KeyCode::Char('a')]), Some(&Action::MarkListRead));
        assert_eq!(m.get(&vec![KeyCode::Char('A')]), Some(&Action::MarkAllRead));
        assert_eq!(m.get(&vec![KeyCode::Char('u')]), Some(&Action::ToggleRead));
        assert_eq!(m.get(&vec![KeyCode::Char('E')]), Some(&Action::ExportSaved));
    }

    #[test]
    fn keymap_combos() {
        let mut raw = std::collections::HashMap::new();
        raw.insert("jump_top".to_string(), vec!["gg".to_string()]);
        raw.insert("sort_time".to_string(), vec!["st".to_string()]);
        let m = build_keymap(&raw);
        assert_eq!(m.get(&vec![KeyCode::Char('g'), KeyCode::Char('g')]), Some(&Action::JumpTop));
        assert_eq!(
            m.get(&vec![KeyCode::Char('s'), KeyCode::Char('t')]),
            Some(&Action::Sort { level: "time", reverse: false })
        );
        // case-sensitive: sT ≠ st
        assert!(m.get(&vec![KeyCode::Char('s'), KeyCode::Char('T')]).is_none());
        assert!(m.get(&vec![KeyCode::Char('g')]).is_none()); // prefix only
    }

    #[test]
    fn parse_angle_keys() {
        assert_eq!(Action::parse_key("<enter>"), Some(KeyCode::Enter));
        assert_eq!(Action::parse_key("<esc>"), Some(KeyCode::Esc));
        assert_eq!(Action::parse_key("<TAB>"), Some(KeyCode::Tab));
        assert_eq!(Action::parse_key("<space>"), Some(KeyCode::Char(' ')));
    }
}
