//! TUI rendering: turns `App` state into terminal pixels every frame.
//! 界面渲染：每一帧把 App 状态画成终端像素。
//!
//! ratatui is an *immediate-mode* UI library: there is no widget tree that
//! persists between frames. Every redraw (~each keypress) rebuilds all
//! widgets from scratch from app state and paints them into a buffer; the
//! library then diffs that buffer against the last frame and sends only
//! changed cells to the terminal. Like a game loop, or like C code that
//! repaints a whole curses screen on every event.
//! ratatui 是"立即模式"UI 库：没有跨帧存活的控件树。
//! 每次重绘（约等于每次按键）都从应用状态重建所有控件并写入缓冲区，
//! 库再对比上一帧，只把变化的格子发给终端。
//! 类似游戏循环，或 C 里每次事件都重画整屏的 curses 代码。
//!
//! Rust concepts introduced here:
//! 本文件涉及的 Rust 概念：
//! - borrowing in practice: one function may hold two mutable borrows of two
//!   *different* values (`frame` and `app`) at once. Read-only draw helpers
//!   take `&App`; ones that update scroll state need `&mut App`.
//!   借用实战：一个函数可同时持有两个不同值的 &mut（frame 与 app）。
//!   只读绘制函数拿 &App；要更新滚动状态的拿 &mut App。
//! - builder pattern: widgets chain `.block(...).style(...)`, consuming and
//!   returning `self` (fluent chaining like C++ stream operators).
//!   构造器模式：控件链式调用 .block().style()，吃掉自身再返回 self。
//! - value-type widgets/styles: `Style`/`Line`/`Span`/`Rect` are small plain
//!   structs copied freely; widgets are throwaway, rebuilt every frame.
//!   Style/Line/Span/Rect 都是小值类型，随意拷贝；
//!   控件是一次性的，每帧重建。
//! - enums + `match`: `TreeRow` variants drive one exhaustive match — the
//!   compiler errors if you forget a case. C switch on ints, but checked.
//!   枚举 + match：TreeRow 的分支驱动穷尽匹配，漏写分支编译报错。

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use crate::model::Item;
use crate::util::fmt_date;
// Import types from the crate root (main.rs): App state, input-mode enums,
// nav-tree row type, scope filter. `crate::X` = path from crate root,
// like including "app.h" in C.
// 从 crate 根（main.rs）导入类型：App 状态、输入模式枚举、导航树行、作用域。
// crate::X 表示从 crate 根开始的路径，类似 C 的 #include "app.h"。
use crate::{App, InputMode, InputPrompt, Scope, TreeRow};

/// Concatenated favourite/lazy markers for a nav feed row.
/// 拼接某订阅源在导航栏中的收藏/惰性标记字符串。
///
/// Pure function: borrows `&App`, returns an owned `String`. Returns "" when
/// the feed isn't found or has no marks.
/// 纯函数：借用 &App，返回自有 String。找不到源或无标记时返回空串。
fn feed_marks(app: &App, url: &str) -> String {
    let mut s = String::new();
    // `if let Some(f) = ...find(...)`: find returns Option<&Feed> — Some(&feed)
    // or None. if-let unwraps only the success case, no NULL checks needed.
    // find 返回 Option<&Feed>，if let 只处理命中的情况，无需判空指针。
    if let Some(f) = app.feeds.feeds.iter().find(|f| f.url == url) {
        if f.favourite {
            s.push_str(&app.cfg.markers.favourite);
        }
        if f.lazy {
            s.push_str(&app.cfg.markers.lazy);
        }
    }
    s
}

/// Top-level render entry point: called once per frame by the main loop.
/// 顶层渲染入口：主循环每帧调用一次。
///
/// Takes `&mut App` because it writes back layout info (`article_area`) that
/// the event handler needs to translate mouse/scroll coordinates. Data flow:
/// App state → split areas → per-pane draw calls → terminal buffer.
/// 参数是 &mut App：要把 article_area 写回，供事件处理做坐标换算。
/// 数据流：App 状态 → 切分区域 → 各面板绘制 → 终端缓冲区。
pub(crate) fn render(frame: &mut Frame, app: &mut App) {
    // Layout::vertical splits the screen top-to-bottom per constraints:
    // Min(3) = "at least 3 rows, grow if possible"; Length(1) = exactly 1 row.
    // .areas() destructures the result into fixed-size array bindings.
    // Layout::vertical 按约束从上到下切分屏幕：
    // Min(3)=至少 3 行；Length(1)=恰好 1 行。areas() 把结果解构成数组绑定。
    let [main, status_bar] = Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).areas(frame.area());
    if app.fullscreen {
        // full-screen focus on the article pane
        // 全屏模式：只画文章面板 + 状态栏
        app.article_area = main;
        draw_article(frame, main, app);
        draw_status(frame, status_bar, app);
        if app.show_help {
            draw_help(frame, frame.area(), app);
        }
        if let Some(prompt) = &app.input {
            draw_input(frame, frame.area(), prompt);
        }
        return;
    }
    // Pane ratios come from config as f32 fractions; clamp negatives to 0,
    // then scale to integer percentages for the Percentage constraint.
    // 面板比例来自配置（f32），负数截为 0，再换算成整数百分比约束。
    let r = [
        app.cfg.pane_ratio[0].max(0.0),
        app.cfg.pane_ratio[1].max(0.0),
        app.cfg.pane_ratio[2].max(0.0),
    ];
    // Horizontal three-way split. Percentages don't have to sum to 100 —
    // Layout normalizes them across the available width.
    // 横向三等分。百分比不必凑满 100，Layout 会按可用宽度归一化。
    let [nav, list, article] = Layout::horizontal([
        Constraint::Percentage((r[0] * 100.0) as u16),
        Constraint::Percentage((r[1] * 100.0) as u16),
        Constraint::Percentage((r[2] * 100.0) as u16),
    ])
    .areas(main);
    // Remember where the article pane is so mouse events can hit-test it.
    // 记下文章区域位置，供鼠标事件命中判断。
    app.article_area = article;

    draw_nav(frame, nav, app);
    draw_list(frame, list, app);
    draw_article(frame, article, app);
    draw_status(frame, status_bar, app);

    // Overlays are drawn LAST so they paint on top of everything already in
    // the frame buffer (painter's algorithm: later = on top).
    // 覆盖层最后绘制，压在已有内容之上（画家算法：后画的在上）。
    if app.show_help {
        draw_help(frame, frame.area(), app);
    }
    if let Some(prompt) = &app.input {
        draw_input(frame, frame.area(), prompt);
    }
}

/// Render the left navigation tree (sections, categories, feeds).
/// 渲染左侧导航树（分组、分类、订阅源）。
///
/// Builds a fresh `Vec<ListItem>` from `app.tree_rows` every frame —
/// immediate-mode style, no retained widget state. Takes `&mut App`: it
/// writes back the sticky scroll offset (`tree_offset`) each frame.
/// 每帧从 app.tree_rows 重建 Vec<ListItem> —— 立即模式风格，
/// 无持久控件状态。参数为 &mut App：每帧回写粘性滚动偏移 tree_offset。
fn draw_nav(frame: &mut Frame, area: Rect, app: &mut App) {
    let mut items: Vec<ListItem> = Vec::new();
    // Visible rows = pane height minus the border (top+bottom = 2 rows).
    // saturating_sub clamps at 0 instead of underflowing; .max(1) avoids an
    // empty window.
    // 可见行数 = 面板高度减去上下边框共 2 行。
    // saturating_sub 下溢时截为 0 而非 panic；max(1) 避免窗口为空。
    let visible = (area.height as usize).saturating_sub(2).max(1);
    // Sticky scroll window: same rule as the item list. `tree_offset` is
    // persisted in App between frames, so scrolling up from the bottom keeps
    // the window still until the cursor crosses the margin (config `offset`).
    // Callers clamp to len - visible so the window never slides past the end.
    // 粘性滚动窗口：与条目列表同规则。tree_offset 存于 App 跨帧保留，
    // 因此从底部向上滚动时窗口先不动，直到光标越过边距（config offset）。
    // 调用处限制为 len - visible，窗口不会滑过末尾。
    let max_offset = app.tree_rows.len().saturating_sub(visible);
    app.tree_offset = sticky_offset(app.tree_sel, app.tree_offset, visible, app.cfg.offset).min(max_offset);
    let offset = app.tree_offset;
    // Iterator pipeline: enumerate adds indices, skip/take slice the range,
    // collect materializes into Vec<(usize, &TreeRow)> — borrowed rows,
    // nothing copied. C equivalent: pointer arithmetic over an array.
    // 迭代器流水线：enumerate 加下标，skip/take 切片，collect 收集成 Vec。
    // 行是借用的 &TreeRow，不做拷贝。C 里等价于对数组做指针运算。
    let window: Vec<(usize, &TreeRow)> = app
        .tree_rows
        .iter()
        .enumerate()
        .skip(offset)
        .take(visible)
        .collect();
    for (i, row) in window {
        // Each TreeRow variant renders differently; the match yields a tuple
        // (display text, base style). All branches must produce the same
        // tuple type — that's what lets the match be used as one expression.
        // 每个 TreeRow 分支产出 (文本, 样式) 元组；所有分支类型必须一致，
        // 整个 match 才能作为一个表达式使用。
        let (text, style) = match row {
            TreeRow::Section(name) => {
                let prefix = if app.collapsed.contains(name) { "▸" } else { "▾" };
                (
                    format!("{prefix} {name}"),
                    Style::default()
                        .fg(app.theme.top)
                        .add_modifier(Modifier::BOLD),
                )
            }
            // The next few special categories share one shape:
            // bold label + live unread count in parentheses.
            // 下面几个特殊分类结构相同：粗体标签 + 实时未读数。
            TreeRow::AllUnread => {
                (format!("Unread ({})", app.total_unread), Style::default().add_modifier(Modifier::BOLD))
            }
            TreeRow::ReadLater => {
                (format!("Read Later ({})", app.later_count), Style::default().add_modifier(Modifier::BOLD))
            }
            TreeRow::Saved => {
                (format!("Saved ({})", app.saved_count), Style::default().add_modifier(Modifier::BOLD))
            }
            TreeRow::Favourite => {
                // Aggregate unread counts of favourite feeds:
                // filter keeps favourites, map turns each into its count,
                // sum folds them. Iterator chain instead of a manual loop.
                // 汇总收藏源的未读数：filter 筛选、map 转计数、sum 折叠求和。
                // 迭代器链式写法替代手写循环。
                let n: usize = app
                    .feeds
                    .feeds
                    .iter()
                    .filter(|f| f.favourite)
                    .map(|f| app.unread(&f.url))
                    .sum();
                let prefix = if app.fav_expanded { "▾" } else { "▸" };
                (format!("{prefix} Favourite ({n})"), Style::default().add_modifier(Modifier::BOLD))
            }
            TreeRow::Lazy => {
                // Same filter/map/sum pattern as Favourite, for lazy feeds.
                // 与 Favourite 相同的 filter/map/sum 模式，针对惰性源。
                let n: usize = app
                    .feeds
                    .feeds
                    .iter()
                    .filter(|f| f.lazy)
                    .map(|f| app.unread(&f.url))
                    .sum();
                let prefix = if app.lazy_expanded { "▾" } else { "▸" };
                (format!("{prefix} Lazy ({n})"), Style::default().add_modifier(Modifier::BOLD))
            }
            TreeRow::Uncategorized => {
                // Same aggregate pattern over uncategorized feeds.
                // 对未分类源套用同样的汇总模式。
                let n: usize = app
                    .feeds
                    .uncategorized()
                    .iter()
                    .map(|f| app.unread(&f.url))
                    .sum();
                let prefix = if app.uncat_expanded { "▾" } else { "▸" };
                (
                    format!("{prefix} No Category ({n})"),
                    Style::default().add_modifier(Modifier::BOLD),
                )
            }
            // `|` ORs several patterns into one branch — these three variants
            // share handling. Pattern binds `name`; the first element `_`
            // (the parent id) is deliberately ignored.
            // | 把多个模式合并到一个分支 —— 这三个变体共用逻辑。
            // name 绑定显示名；第一个字段 _（父 id）故意忽略。
            TreeRow::FavouriteFeed(_, name)
            | TreeRow::LazyFeed(_, name)
            | TreeRow::UncategorizedFeed(_, name) => {
                let f = app
                    .feeds
                    .feeds
                    .iter()
                    .find(|x| x.display_name() == name.as_str());
                // Option combinators replace NULL checks:
                // .map transforms when present, .unwrap_or gives a default.
                // Option 组合子替代判空：
                // .map 在有值时变换，.unwrap_or 给默认值。
                let n = f.map(|x| app.unread(&x.url)).unwrap_or(0);
                let mark = f
                    .filter(|x| app.feed_errors.contains_key(&x.url))
                    .map(|_| " !")
                    .unwrap_or("");
                let marks = f.map(|x| feed_marks(app, &x.url)).unwrap_or_default();
                (
                    format!("  {name} ({n}){marks}{mark}"),
                    Style::default(),
                )
            }
            TreeRow::Category(cat) => {
                let n: usize = app
                    .feeds
                    .by_category(cat)
                    .iter()
                    .map(|f| app.unread(&f.url))
                    .sum();
                let prefix = if app.collapsed.contains(cat) { "▸" } else { "▾" };
                // Nested categories indent by 2 spaces per '/' depth level:
                // matches('/').count() = slash count = nesting depth.
                // 按 '/' 层数缩进：每个斜杠 2 格。
                // matches().count() 即嵌套深度。
                let indent = cat.matches('/').count() * 2 + 2;
                (format!("{}{prefix} {cat} ({n})", " ".repeat(indent)), Style::default())
            }

            TreeRow::Feed(url, name, indent) => {
                let n = app.unread(url);
                let mark = if app.feed_errors.contains_key(url.as_str()) { " !" } else { "" };
                (
                    format!(
                        "{}{} ({n}){}{mark}",
                        " ".repeat(*indent as usize),
                        name,
                        feed_marks(app, url)
                    ),
                    Style::default(),
                )
            }
            TreeRow::Tag(t) => {
                // Aggregate unread across feeds carrying this tag —
                // same filter/map/sum pattern again.
                // 对带此标签的源汇总未读数 —— 同样的 filter/map/sum 模式。
                let n: usize = app
                    .feeds
                    .feeds
                    .iter()
                    .filter(|f| f.has_tag(t))
                    .map(|f| app.unread(&f.url))
                    .sum();
                let prefix = if app.collapsed.contains(&format!("tag:{t}")) {
                    "▸"
                } else {
                    "▾"
                };
                (format!("  {prefix} {t} ({n})"), Style::default())
            }
        };
        let item = ListItem::new(text);
        // top-level entries get a highlight fg (yellow) on top of their base style
        // 顶层条目在基础样式之上再加高亮前景色（黄色）
        // matches! macro: pattern-match a value, return bool — terser than a
        // full match expression.
        // matches! 宏：对值做模式匹配返回布尔值，比完整 match 更简洁。
        let is_top = matches!(
            row,
            TreeRow::Section(_)
                | TreeRow::AllUnread
                | TreeRow::ReadLater
                | TreeRow::Saved
                | TreeRow::Favourite
                | TreeRow::Lazy
                | TreeRow::Uncategorized
        );
        // Style::patch overlays another style's set fields onto this one —
        // unset fields keep their previous value (like CSS cascading).
        // patch 用另一样式覆盖已设置的字段，未设置字段保留原值（类似 CSS 级联）。
        let base = if is_top {
            style.patch(Style::default().fg(app.theme.top))
        } else {
            style
        };
        // patch selection into the row style so base styles (bold etc.) survive
        // 把选中态叠加进行样式，保留原有的粗体等基础样式
        let row_style = if i == app.tree_sel {
            if app.focus == 0 {
                Style::default().bg(app.theme.selected).fg(Color::White)
            } else {
                Style::default().fg(app.theme.top)
            }
        } else {
            Style::default()
        };
        // Final style = base patched with selection highlight.
        // 最终样式 = 基础样式叠加选中高亮。
        items.push(item.style(base.patch(row_style)));
    }
    // render_widget consumes the widget (moves it into the call) and paints
    // it into the frame buffer at `area`. Widgets are throwaway value types —
    // rebuilt each frame, like immediate-mode GUI draws.
    // render_widget 按值消费控件并画进缓冲区。
    // 控件是一次性值类型，每帧重建，如同立即模式 GUI。
    frame.render_widget(
        List::new(items)
            .block(pane_block("Nav", app.focus == 0, &app.theme))
            .style(Style::default().bg(app.theme.bg)),
        area,
    );
}

/// Sticky scroll offset for nav/list: the window only moves when the
/// selection crosses a margin edge. `margin` rows stay visible above/below
/// the selection (vim scrolloff); margin is capped at half the window.
/// Callers clamp the result to `len - visible`.
/// nav/list 的粘性滚动偏移：只有光标越过边距时窗口才移动。
/// margin 行保持在光标上方/下方可见（vim scrolloff）；边距上限为窗口一半。
/// 调用处把结果限制为 len - visible。
///
/// Pure function — same idea as ncurses scrolling, but written explicitly.
/// 纯函数 — 思路同 ncurses 滚动，只是显式写出。
fn sticky_offset(sel: usize, offset: usize, visible: usize, margin: usize) -> usize {
    let m = margin.min(visible / 2);
    if sel < offset + m {
        sel.saturating_sub(m)
    } else if sel >= offset + visible - m {
        sel.saturating_sub(visible - m - 1)
    } else {
        offset
    }
}

/// Render the middle article-list pane for the active scope.
/// 渲染中间的文章列表面板（当前作用域内）。
///
/// Takes `&mut App` — it writes back the sticky scroll offset each frame.
/// 参数为 &mut App —— 每帧回写粘性滚动偏移。
fn draw_list(frame: &mut Frame, area: Rect, app: &mut App) {
    let mut items: Vec<ListItem> = Vec::new();
    if app.scoped_items.is_empty() {
        items.push(ListItem::new("no items — r to refresh"));
    }
    // window around the selection so long lists scroll with the cursor.
    // Sticky scroll: viewport only moves when the selection crosses a margin
    // edge — config `offset` rows stay visible above/below (vim scrolloff).
    // 视口围绕选中项。粘性滚动：只有光标越过边距视口才移动；
    // config offset 行保持在光标上下方可见。
    let visible = (area.height as usize).saturating_sub(2).max(1);
    // Compute new offset and persist it in app state — this is why this fn
    // needs &mut App. State lives in App between frames; widgets keep none.
    // The .min() clamp keeps the window from sliding past the list end.
    // 计算新偏移并存回应用状态 — 所以本函数要 &mut App。
    // 状态存在 App 里跨帧保存；控件本身不保存任何状态。
    // .min() 限制窗口不会滑过列表末尾。
    app.list_offset = sticky_offset(app.list_sel, app.list_offset, visible, app.cfg.offset)
        .min(app.scoped_items.len().saturating_sub(visible));
    let offset = app.list_offset;
    let window: Vec<(usize, &(String, Item))> = app
        .scoped_items
        .iter()
        .enumerate()
        .skip(offset)
        .take(visible)
        .collect();
    // Pattern destructure: each scoped item is (key-string, Item); `_`
    // ignores the key since display needs only the Item itself.
    // 解构元组：(键字符串, Item)；_ 忽略键，显示只需要 Item。
    for (i, (_, item)) in window {
        let read = item.read;
        let marker = if read { " " } else { "•" };
        // Build the flag suffix (saved / read-later markers) conditionally.
        // 按需拼出标记后缀（收藏 / 稍后读）。
        let mut flags = String::new();
        if item.saved {
            flags.push_str(&app.cfg.markers.saved);
        }
        if item.read_later {
            flags.push_str(&app.cfg.markers.later);
        }
        // Shadowing again: `flags` becomes the space-prefixed version.
        // 再次遮蔽：flags 变成带前导空格的版本。
        let flags = if flags.is_empty() {
            String::new()
        } else {
            format!(" {flags}")
        };
        let text = format!("{marker}{flags} {}", item.display_title());
        let mut li = ListItem::new(text);
        // Selected row gets the focus-dependent highlight; already-read rows
        // get dimmed. Unselected+unread keeps the default style.
        // 选中行按焦点高亮；已读行变暗。未选中且未读保持默认样式。
        if i == app.list_sel {
            let style = if app.focus == 1 {
                Style::default().bg(app.theme.selected).fg(Color::White)
            } else {
                Style::default().fg(app.theme.top)
            };
            li = li.style(style);
        } else if read {
            li = li.style(Style::default().fg(app.theme.dim));
        }
        items.push(li);
    }
    // Pane title reflects the current scope; Feed scope shows the feed's
    // display name, falling back to its URL when the record is gone.
    // 面板标题反映当前作用域；Feed 作用域显示源名，查不到时退回 URL。
    let title = match &app.scope {
        Scope::AllUnread => "All Unread".to_string(),
        Scope::Favourite => "Favourite".to_string(),
        Scope::Lazy => "Lazy".to_string(),
        Scope::ReadLater => "Read Later".to_string(),
        Scope::Saved => "Saved".to_string(),
        Scope::Category(c) => c.clone(),
        Scope::Feed(u) => app
            .feeds
            .feeds
            .iter()
            .find(|f| &f.url == u)
            .map(|f| f.display_name().to_string())
            .unwrap_or_else(|| u.clone()),
        Scope::Tag(t) => format!("#{t}"),
    };
    frame.render_widget(
        List::new(items)
            .block(pane_block(&title, app.focus == 1, &app.theme))
            .style(Style::default().bg(app.theme.bg)),
        area,
    );
}


/// Build the styled article body Text (HTML → markdown → tui-markdown).
/// Images render as `[img] desc (url)` fallback text so the draw pass can
/// locate rows and fetch/overlay real pictures.
/// `[alt](url)` → `alt` for display (export keeps the url). Images `![…]`
/// are left alone.
/// 构建带样式的正文 Text（HTML → markdown → tui-markdown）。
/// 图片渲染成 `[img] 描述 (url)` 占位文本，便于绘制阶段定位行并抓取覆盖真图。
/// 显示时 `[alt](url)` → `alt`（导出仍保留 url）。图片 `![…]` 保持原样。
///
/// Uses a regex. Note `Regex::new(...).unwrap()`: regex compilation can fail
/// (bad pattern), so it returns Result; here the pattern is a compile-time
/// constant known valid, so unwrap cannot panic.
/// 这里用了正则。Regex::new 返回 Result（模式可能非法）；
/// 此处模式是写死的合法常量，unwrap 不会 panic。
fn strip_link_urls(md: &str) -> String {
    // `[alt](url)` → alt, skipping images (`![…](…)`): check the char before
    // each match start instead of a lookbehind (rust regex lacks lookbehind)
    // `[alt](url)` → alt，跳过图片（`![…](…)`）：检查匹配起点的前一字符，
    // 替代后行断言（rust regex 不支持 lookbehind）。
    let re = regex::Regex::new(r"\[([^\]]*)\]\([^)]*\)").unwrap();
    // replace_all with a closure: each match passes Captures; group 1 is the
    // alt text. Returning the whole match keeps images untouched.
    // replace_all 配闭包：每个匹配传入 Captures；第 1 组即 alt 文本。
    // 图片时整体原样返回。
    re.replace_all(md, |caps: &regex::Captures| {
        let start = caps.get(0).unwrap().start();
        // next_back(): last char before the match start — '!' means image.
        // next_back()：匹配起点前的最后一个字符 — '!' 即图片。
        let prev = md[..start].chars().next_back();
        if prev == Some('!') {
            caps.get(0).unwrap().as_str().to_string() // image — keep
                                                      // 图片 — 保留
        } else {
            caps.get(1).unwrap().as_str().to_string()
        }
    })
    // Cow<str> → owned String: replace_all returns a copy-on-write smart
    // pointer; .to_string() forces the owned copy.
    // replace_all 返回 Cow<str>（写时复制智能指针），to_string 转成自有 String。
    .to_string()
}

/// Render markdown → styled Text (link URLs stripped for display).
/// 渲染 markdown → 带样式的 Text（链接 URL 已剥离用于显示）。
///
/// Returns `Text<'static>`: after conversion every span owns its string data,
/// so the returned text outlives any borrow — callers need no lifetime
/// annotations.
/// 返回 Text<'static'>：转换后每个 span 自有字符串数据，
/// 文本不依赖任何借用，调用方无须标注生命周期。
fn render_markdown(app: &App, md: &str) -> Text<'static> {
    if md.trim().is_empty() {
        return Text::from("");
    }
    let md = strip_link_urls(md);
    // Theme-driven options: styles cloned from config; images become
    // "[img] alt (url)" placeholders.
    // 主题驱动的选项：样式从配置克隆；图片转成占位文本。
    let options = tui_markdown::Options::new(app.theme.styles.clone())
        .image_fallback(tui_markdown::ImageFallback::AltTextAndUrl);
    let text = tui_markdown::from_str_with_options(&md, &options);
    // Convert borrowed spans into owned ('static) ones: into_owned() copies
    // each string out of `md`, letting the returned Text live independently.
    // Struct literal below rebuilds Line keeping the unchanged fields.
    // 把借用型 span 转为自有（'static）：into_owned() 复制字符串，
    // 让返回的 Text 独立存活。下面的结构体字面量重建 Line 并保留其余字段。
    let lines: Vec<Line<'static>> = text
        .lines
        .into_iter()
        .map(|l| {
            let spans = l
                .spans
                .into_iter()
                .map(|s| Span::styled(s.content.into_owned(), s.style))
                .collect();
            Line { spans, style: l.style, alignment: l.alignment }
        })
        .collect();
    Text::from(lines)
}

/// Header text: title / meta (feed · date · read · flags) / url / summary.
/// 文章头部文本：标题 / 元信息（源 · 日期 · 已读 · 标记）/ URL / 摘要。
///
/// Lifetime `'a` ties the output to both `item` and `feed_name` — the header
/// borrows slices of those inputs instead of copying. (Only here do we spell
/// a lifetime out; elsewhere the compiler infers it.)
/// 生命周期 'a 把输出绑定到 item 和 feed_name — 头部借用输入切片而不拷贝。
/// （仅此处显式写出生命周期，其他地方编译器自动推断。）
fn article_header<'a>(app: &App, item: &'a Item, feed_name: &'a str) -> Text<'a> {
    let title_style = Style::default().fg(app.theme.accent).add_modifier(Modifier::BOLD);
    let meta_style = Style::default().fg(app.theme.dim);
    let dim_style = Style::default().fg(app.theme.dim);
    // Plain if/else expression assigned to a binding — Rust has no ?:
    // ternary; any block whose last expression isn't followed by `;`
    // evaluates to its value.
    // 普通 if/else 表达式赋给变量 — Rust 无 ?: 三元运算符；
    // 块中最后不带分号的表达式即为块的值。
    let read_mark = if item.read {
        "read"
    } else {
        "unread"
    };
    let mut flags = String::new();
    if item.saved {
        flags.push_str(&app.cfg.markers.saved);
    }
    if item.read_later {
        flags.push_str(&app.cfg.markers.later);
    }
    let flags_mark = if flags.is_empty() {
        String::new()
    } else {
        format!(" {flags}")
    };
    let author_part = if item.author.is_empty() {
        String::new()
    } else {
        format!(" · {}", item.author)
    };
    // Text = Vec<Line>; Line = Vec<Span>; Span = (string, style).
    // Three styled lines stacked vertically: title, meta, URL.
    // Text = Vec<Line>；Line = Vec<Span>；Span = (字符串, 样式)。
    // 三行纵向堆叠：标题、元信息、URL。
    Text::from(vec![
        Line::from(Span::styled(item.display_title().to_string(), title_style)),
        Line::from(Span::styled(
            format!(
                "{feed_name}{author_part} · {} · {read_mark}{flags_mark}",
                fmt_date(&item.date)
            ),
            meta_style,
        )),
        Line::from(Span::styled(item.url.clone(), dim_style)),
    ])
}

/// Render the right article pane: header + scrollable markdown body.
/// 渲染右侧文章面板：头部 + 可滚动 markdown 正文。
///
/// Needs `&mut App`: caches the rendered body (`article_render`) and clamps
/// `article_scroll` against real content height.
/// 需要 &mut App：缓存渲染结果（article_render），
/// 并把滚动位置按实际内容高度收敛。
fn draw_article(frame: &mut Frame, area: Rect, app: &mut App) {
    // let-else: destructure on success; on None run the diverging block
    // (must not fall through). Here: nothing selected → placeholder + bail.
    // let-else：成功则解构继续；None 则进入必须发散的块。
    // 此处是无选中文章 → 占位提示并提前返回。
    let Some((url, item)) = app.current_item() else {
        frame.render_widget(
            Paragraph::new("select an article")
                .block(pane_block("Article", app.focus == 2, &app.theme)),
            area,
        );
        return;
    };
    // Find the owning feed's display name; default to "" if missing.
    // 找所属源的显示名；缺失时默认空串。
    let feed_name = app
        .feeds
        .feeds
        .iter()
        .find(|f| f.url.as_str() == url.as_str())
        .map(|f| f.display_name().to_string())
        .unwrap_or_default();

    let in_article = app.focus == 2;

    // Fixed header (title/meta/summary), content scrolls below a separator.
    // 固定头部（标题/元信息/摘要），其下正文滚动。
    let block = pane_block("Article", app.focus == 2, &app.theme);
    // block.inner(): the rectangle inside the borders — inner drawing must
    // not overwrite the block's own frame.
    // block.inner()：边框内部的矩形 — 内部绘制不能覆盖边框本身。
    let inner = block.inner(area);
    // fixed header: title/meta/url + 2 summary lines; the rest of a long
    // summary flows into the body area
    // 固定头部占 3 行（标题/元信息/URL）；过长的摘要溢出到正文区
    let [head, body] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
    ])
    .areas(inner);

    let header_text = article_header(app, &item, feed_name.as_str());
    // Wrap { trim: true }: reflow long lines, trimming leading whitespace.
    // Wrap { trim: true }：自动折行并去掉行首空白。
    frame.render_widget(
        Paragraph::new(header_text)
            .style(Style::default().bg(app.theme.bg))
            .wrap(Wrap { trim: true }),
        head,
    );

    // Body: feed/fetched HTML → markdown → styled ratatui Text
    // (h2md → tui-markdown pipeline).
    // Links render as underlined alt text (no URL); images as [img].
    // body markdown: preview = full summary; article = summary + content
    // (both, in order, no truncation, no duplication)
    // The HTML→markdown conversion is expensive, so it runs only on a cache
    // miss (content change / mode switch) — never per frame.
    // 正文：HTML → markdown → ratatui 样式 Text（h2md → tui-markdown 流水线）。
    // 链接显示为下划线 alt 文本（不含 URL）；图片显示为 [img]。
    // 正文内容：预览 = 全部摘要；文章页 = 摘要 + 全文（顺序拼接，不截断不重复）。
    // HTML→markdown 开销大，只在缓存未命中时执行（内容变化/模式切换）— 不逐帧重算。
    //
    // Cache key: (url, guid, mode) — any change invalidates the cached Text.
    // Tuple equality makes the lookup a one-liner.
    // 缓存键：(url, guid, 模式) — 任一变化即失效。元组比较一行完成查找。
    let key = (url.clone(), item.guid.clone(), in_article);
    let body_text = if matches!(&app.article_render, Some((k, _)) if k == &key) {
        // Cache hit: clone the stored Text (cheap-ish; spans are small).
        // 命中缓存：克隆存的 Text（开销小，span 都很小）。
        app.article_render.as_ref().map(|(_, t)| t.clone()).unwrap_or_default()
    } else {
        // Cache miss: convert HTML pieces lazily — skip empty ones.
        // 缓存未命中：按需转换非空的 HTML 片段。
        let summary_md = if !item.summary.trim().is_empty() {
            Some(crate::fetch::html_to_markdown(&item.summary))
        } else {
            None
        };
        let content_md = if !item.content.trim().is_empty() {
            Some(crate::fetch::html_to_markdown(&item.content))
        } else {
            None
        };
        // match on a tuple of Options enumerates all four combinations.
        // 对 Option 元组做 match，穷举四种组合。
        let md = if in_article {
            match (summary_md, content_md) {
                (Some(s), Some(c)) => format!("{s}\n\n{c}"),
                (Some(s), None) => s,
                (None, Some(c)) => c,
                (None, None) => String::new(),
            }
        } else {
            summary_md.unwrap_or_default()
        };
        if md.trim().is_empty() {
            // cheap hint states — never cached (fetching flips frequently)
            // 廉价提示状态 — 不缓存（fetching 翻转频繁）
            if app.fetching {
                Text::from("fetching…")
            } else if in_article {
                Text::from("") // feed has neither summary nor content — enter fetches
                               // 源既无摘要也无全文 — 回车触发抓取
            } else {
                Text::from("l/enter to read")
            }
        } else {
            let t = render_markdown(app, &md);
            // Store in the cache AND hand a clone to this frame's painter.
            // 存入缓存，并把克隆交给本帧绘制。
            app.article_render = Some((key, t.clone()));
            t
        }
    };
    // reading width: cap (and center) only when configured (>0 = unlimited)
    // 阅读宽度：仅在配置了上限时收窄并居中（>0 = 不限制）
    let content_w = if app.cfg.reading_width > 0 {
        body.width.min(app.cfg.reading_width as u16)
    } else {
        body.width
    };
    let x_off = (body.width - content_w) / 2;

    // Draw the bordered block itself. Order within one frame doesn't matter
    // — everything lands in a buffer flushed once at frame end.
    // 绘制带边框的面板块。同帧内的绘制顺序无关紧要 —
    // 全部先进缓冲区，帧末一次性输出。
    frame.render_widget(block, area);

    // clamp scroll to the exact wrapped content height. Paragraph::line_count
    // uses the same WordWrapper as rendering, so the bottom is always
    // reachable — the old width-based estimate could cut content short
    // (e.g. after fetching full text for a summary-only item).
    // 滚动上限取精确折行后的总高度。Paragraph::line_count 与渲染共用同一个
    // WordWrapper，保证能滚到底 — 旧的按宽度估算会截断内容
    // （例如纯摘要条目抓到全文之后）。
    let para = Paragraph::new(body_text).wrap(Wrap { trim: true });
    let total_lines = para.line_count(content_w.max(1)) as u16;
    let max_scroll = total_lines.saturating_sub(body.height);
    let scroll = app.article_scroll.min(max_scroll);
    // write back only in article mode — in list mode the body is a one-line
    // hint and clamping would zero the saved article position
    // 仅文章模式回写 — 列表模式下正文只是一行提示，
    // 收敛会把已保存的文章阅读位置清零
    if in_article {
        app.article_scroll = scroll;
    }
    // .scroll((y, x)): skip `scroll` lines before painting — manual scrolling
    // implemented as an offset into the wrapped text.
    // .scroll((y, x))：绘制前跳过 scroll 行 — 以偏移实现手动滚动。
    let para = para.scroll((scroll, 0));
    // Manually build a possibly narrower, x-shifted rect for centered reading.
    // 手动构造更窄且水平偏移的矩形，实现居中阅读。
    let content_area = Rect {
        x: body.x + x_off,
        y: body.y,
        width: content_w,
        height: body.height,
    };
    frame.render_widget(
        para.style(Style::default().bg(app.theme.bg)),
        content_area,
    );

    // scrollbar on the pane's right edge
    // 面板右缘的滚动条
    let total = total_lines;
    if total > body.height && scroll > 0 {
        // Bar length ∝ viewport/content ratio (min 1 row); bar position ∝
        // scroll fraction of the scrollable range. Float math first, cast last.
        // 条长 ∝ 视口/内容比（最少 1 行）；位置 ∝ 可滚范围内的滚动比例。
        // 先浮点运算，最后整数化。
        let bar_h = ((body.height as f32 * body.height as f32 / total as f32).max(1.0)) as u16;
        let bar_y = (scroll as f32 * (body.height - bar_h) as f32 / max_scroll as f32) as u16;
        // A 1-column rect painted blank with background color — that's the
        // whole scrollbar. Rendering IS just filling terminal cells.
        // 一列宽矩形填背景色空白即是滚动条。渲染本质就是填终端格子。
        let sb_area = Rect {
            x: body.x + body.width - 1,
            y: body.y + bar_y,
            width: 1,
            height: bar_h,
        };
        frame.render_widget(
            Paragraph::new(" ").style(Style::default().bg(app.theme.dim)),
            sb_area,
        );
    }

}

/// Render the one-line status bar (status message + key hints).
/// 渲染单行状态栏（状态消息 + 按键提示）。
fn draw_status(frame: &mut Frame, area: Rect, app: &App) {
    let line = format!(
        "{}  |  ? help  Q quit  F favourite  L lazy  J/K unread  n/p parent  l/enter open  r fetch  R refresh",
        app.status
    );
    // Status text color contrasts with the light/dark background setting.
    // 状态文字颜色与明暗背景设置形成对比。
    let status_fg = match app.cfg.background {
        crate::config::LightDark::Light => Color::Black,
        crate::config::LightDark::Dark => Color::White,
    };
    frame.render_widget(
        Paragraph::new(line).style(Style::default().fg(status_fg).bg(app.theme.bg)),
        area,
    );
}

/// Render the floating help overlay (centered, opaque, scrollable).
/// 渲染悬浮帮助层（居中、不透明、可滚动）。
fn draw_help(frame: &mut Frame, area: Rect, app: &App) {
    // `\` at end of line continues the string literal, eating the newline
    // and leading indentation — builds one multi-line string at compile time.
    // 行尾反斜杠续接字符串字面量并吞掉换行和缩进 — 编译期拼接多行字符串。
    let text = Text::from(
        "Keys\n\
         ─────\n\
         nav:   j/k move · h/l expand+descend · N new feed · D delete · M rename · F favourite · L lazy\n\
         list:  j/k move · l/enter open · / search (enter keep, left stop) · J/K next/prev unread\n\
         article: j/k scroll · n/p parent (move list) · ctrl+u/d half page · ctrl+f/b full page\n\
         left:  h/q/esc — article→list→nav→parent\n\
         right: l/enter — expand→list→article→fetch\n\
         jump:  gg/G top/bottom (nav+list+article)\n\
         sort:  st/sn/sf/su forward · sT/sN/sF/sU reversed — time/title/feed/unread\n\
         copy:  yy url · yn title · yp feed url · ys summary · yc full content\n\
         global: o browser · e export · E saved-list · a list read · A all feeds read · u toggle read · L/S flags · r/R refresh\n\
         i/x OPML · t preset · tab focus · Q quit · ? help\n\n\
         export → $XDG_DATA_HOME/markerss/<category>/<slug>.md · saved list → saved.md",
    );
    // floating opaque window, default colors, scrollable with j/k
    // 悬浮不透明窗口，默认配色，j/k 可滚动
    // Overlay sized at 3/4 of the screen with sane minimum dimensions.
    // 覆盖层取屏幕 3/4 大小，并设最小尺寸。
    let w = (area.width * 3 / 4).max(40);
    let h = (area.height * 3 / 4).max(10);
    let rect = Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    };
    let block = Block::default().borders(Borders::ALL).title("Help");
    // Clear wipes the cells under the overlay first — otherwise help text
    // would blend with stale characters from panes drawn beneath it.
    // Clear 先清空覆盖区域的旧内容 — 否则帮助文字会和底下残留字符混在一起。
    frame.render_widget(ratatui::widgets::Clear, rect);
    // .scroll((help_scroll, 0)): j/k scrolling is purely state-driven.
    // .scroll((help_scroll, 0))：j/k 滚动完全由状态驱动。
    frame.render_widget(
        Paragraph::new(text).block(block).scroll((app.help_scroll, 0)),
        rect,
    );
}

/// Render the floating input prompt (search / rename / etc.).
/// 渲染悬浮输入框（搜索 / 重命名等）。
fn draw_input(frame: &mut Frame, area: Rect, prompt: &InputPrompt) {
    // Prompt text + current edit buffer rendered as one paragraph.
    // 提示语 + 当前输入缓冲渲染成一个段落。
    let text = Text::from(format!("{} {}", prompt.prompt, prompt.buf));
    let block = Block::default()
        .borders(Borders::ALL)
        .title("input (esc cancel)");
    // Reserve room for borders: max height inside the parent area, max
    // inner width leaving margins. saturating_sub keeps these >= 0.
    // 为边框留空间：高度不超过父区域，宽度留出左右边距。
    // saturating_sub 保证不为负。
    let max_h = area.height.saturating_sub(2).max(3) as usize;
    let inner_w = area.width.saturating_sub(8).saturating_sub(2) as usize;
    // Auto-size the box: measure how many lines the text wraps to, clamp by
    // available height, then add the 2 border rows back.
    // 自适应尺寸：按折行数测量所需行数，受限于可用高度，再加回上下边框 2 行。
    let content = wrapped_lines(&text, inner_w).min(max_h.saturating_sub(2)).max(1);
    let h = (content + 2).max(3) as u16;
    let box_rect = Rect {
        x: area.x + 4,
        y: if prompt.mode == InputMode::Search {
            area.y + 1
        } else {
            area.y + area.height.saturating_sub(h) / 2
        },
        width: area.width.saturating_sub(8),
        height: h,
    };
    frame.render_widget(ratatui::widgets::Clear, box_rect);
    // Search prompts get a more descriptive title than generic input.
    // 搜索提示用更具体的标题，区别于普通输入。
    let title = if prompt.mode == InputMode::Search {
        "search (enter keep · esc restore)"
    } else {
        "input (esc cancel)"
    };
    // trim: false preserves leading spaces while wrapping — matters for
    // typed input where whitespace can be meaningful.
    // trim: false 折行时保留行首空白 — 输入内容的空白可能有意义。
    frame.render_widget(
        Paragraph::new(text)
            .wrap(ratatui::widgets::Wrap { trim: false })
            .block(block.title(title)),
        box_rect,
    );
}

/// Number of lines `text` occupies when wrapped at `width` columns.
/// 文本在 width 列宽下折行后占多少行。
///
/// Ceiling division per logical line (`(w + width - 1) / width` is the classic
/// C ceil-div idiom), then summed. Estimate only — good enough for sizing
/// the input box.
/// 每行做向上取整除法（(w+width-1)/width 即经典 C 写法）后求和。
/// 仅是估算 — 用于给输入框定尺寸足够了。
fn wrapped_lines(text: &Text, width: usize) -> usize {
    if width == 0 {
        return 1;
    }
    let n: usize = text
        .lines
        .iter()
        .map(|l| (l.width() + width - 1) / width)
        .sum();
    n.max(1)
}

/// Build a bordered pane block; border color signals keyboard focus.
/// 构建带边框的面板块；边框颜色指示键盘焦点。
///
/// `Block<'a>` borrows the title str — lifetime `'a` says the returned Block
/// must not outlive the borrowed title (no copy made).
/// Block<'a> 借用标题字符串 — 生命周期 'a 保证返回的 Block 不比标题活得久（无拷贝）。
fn pane_block<'a>(title: &'a str, focused: bool, theme: &crate::config::ThemeColors) -> Block<'a> {
    let color = if focused { theme.focused } else { theme.dim };
    Block::bordered()
        .title(title)
        .border_style(Style::default().fg(color))
        .style(Style::default().bg(theme.bg))
        // (duplicated .style call is harmless — later bg overwrites earlier)
        // （重复的 .style 调用无害 — 后面的 bg 覆盖前面的）
        .style(Style::default().bg(theme.bg))
}

// Unit tests — compiled/run only under `cargo test`.
// 单元测试 — 仅在 cargo test 时编译运行。
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_link_urls_keeps_text_drops_url() {
        assert_eq!(strip_link_urls("[text](https://x.com)"), "text");
        assert_eq!(strip_link_urls("a [**bold**](https://x.com) b"), "a **bold** b");
        // images untouched
        // 图片保持原样
        assert_eq!(strip_link_urls("![alt](https://x.com/i.png)"), "![alt](https://x.com/i.png)");
        assert_eq!(strip_link_urls("no links here"), "no links here");
    }

    #[test]
    fn markdown_roundtrip() {
        let html = "<h2>Title</h2><p>Hello <b>bold</b> <a href=\"https://e.com\">link</a></p>";
        let md = crate::fetch::html_to_markdown(html);
        assert!(md.contains("Title"), "got: {md}");
        assert!(md.contains("**bold**"), "got: {md}");
        assert!(md.contains("[link](https://e.com)"), "got: {md}");
    }

    #[test]
    fn sticky_offset_scrolls_down_pins_bottom() {
        // scrolling down: selection pinned to the bottom edge once the list
        // is longer than the window (margin 0 = edge-pinning)
        // 向下滚动：列表超过窗口后，选中项贴底边（margin 0 = 贴边）
        let mut off = 0;
        for sel in 0..=25 {
            off = sticky_offset(sel, off, 20, 0);
        }
        assert_eq!(off, 6); // 25 - 20 + 1
        assert_eq!(sticky_offset(26, off, 20, 0), 7);
    }

    #[test]
    fn sticky_offset_scroll_up_keeps_window_still() {
        // at the bottom (sel 99, offset 80, window 20) scrolling up keeps the
        // window still until the selection reaches the top edge (80)
        // 在底部（sel 99，offset 80，窗口 20）向上滚动时窗口不动，
        // 直到选中项到达顶边（80）
        let mut off = 80;
        for sel in (80..=99).rev() {
            off = sticky_offset(sel, off, 20, 0);
            assert_eq!(off, 80, "window must stay still at sel {sel}");
        }
        // crossing the top edge scrolls the window up
        // 越过顶边后窗口上移
        assert_eq!(sticky_offset(79, off, 20, 0), 79);
    }

    #[test]
    fn sticky_offset_short_list_stays_at_top() {
        // list shorter than the window: offset stays 0
        // 列表短于窗口：offset 保持为 0
        let mut off = 0;
        for sel in 0..=5 {
            off = sticky_offset(sel, off, 20, 0);
            assert_eq!(off, 0);
        }
    }

    #[test]
    fn sticky_offset_margin_keeps_rows_above_below() {
        // margin 3: scrolling down stops with selection 3 rows above the
        // bottom edge (row 16 of a 20-row window), not pinned to the edge
        // margin 3：向下滚动时选中项停在距底边 3 行处（20 行窗口第 16 行），
        // 而不是钉在底边
        let mut off = 0;
        for sel in 0..=25 {
            off = sticky_offset(sel, off, 20, 3);
        }
        assert_eq!(off, 9); // 25 - (20 - 3 - 1)
        // scrolling up from there keeps the window still until the margin
        // hits the top edge, then scrolls
        // 从该位置向上滚：窗口保持不动，直到边距触及顶边才滚动
        let mut off = 9;
        for sel in (12..=25).rev() {
            off = sticky_offset(sel, off, 20, 3);
            assert_eq!(off, 9, "window must stay still at sel {sel}");
        }
        assert_eq!(sticky_offset(11, off, 20, 3), 8); // 11 - 3
    }
}
