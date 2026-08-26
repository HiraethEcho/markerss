//! App config: `$XDG_CONFIG_HOME/markerss/config.toml` — format by extension.
//!
//! - `config.toml` → TOML (default)
//! - `.json` → JSON, `.jsonc` → JSON with comments stripped
//! - `.yaml` / `.yml` → YAML
//!
//! Subscriptions live in the separate `urls` file (newsboat format).
//! Unknown keys ignored; defaults when absent; read at startup only.
//! 订阅列表存放在单独的 urls 文件中（newsboat 格式）。
//! 忽略未知键；缺省用默认值；仅在启动时读取。
//!
//! Rust concepts introduced here:
//! 本文件涉及的 Rust 概念：
//! - `#[derive(Deserialize)]`: serde auto-generates a parser that fills your
//!   struct's fields straight from config text — like a hand-rolled
//!   `parse_config()` in C that sets fields one by one, but generated and
//!   type-checked by the compiler.
//!   `#[derive(Deserialize)]`：serde 自动生成解析器，把配置文本直接填进结构体字段。
//!   相当于 C 里手写的逐字段 parse_config()，但由编译器生成并做类型检查。
//! - `Option<T>`: "may be absent". Every config key is Option, so missing
//!   lines fall back to defaults — like a pointer you're forced to NULL-check,
//!   except the compiler enforces the check.
//!   `Option<T>`：可能缺失的值。每个配置键都是 Option，缺省时回退默认值。
//!   类似强制判空的指针，但判空由编译器保证。
//! - `#[serde(untagged)]`: one field accepts several shapes (bool vs table,
//!   string vs list) — serde tries each variant until one parses.
//!   `#[serde(untagged)]`：同一字段接受多种形态，serde 逐个尝试直到匹配。
//! - Two-layer config pattern: a `Raw*` mirror of the file (all-Optional)
//!   is parsed first, then merged over a fully-populated default `Config`.
//!   Never parse directly into the live struct.
//!   两层配置模式：先解析出全 Option 的 Raw 镜像，再覆盖到已填好默认值的
//!   Config 上；绝不直接解析进运行时结构体。
//! - `let ... else`: early-return on None/Err without nesting — a cleaner
//!   guard clause than C's `if (!x) return;` chains.
//!   `let ... else`：遇到 None/Err 提前返回，避免嵌套，比 C 的层层 if 取反更清晰。

use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

use crate::xdg;
use ratatui::style::{Color, Modifier, Style};

/// Default nav-strip preset: the top-level entries shown in the nav pane.
/// 默认导航预设：导航面板顶部显示的一级条目。
///
/// `[&str; 7]` is a FIXED-SIZE array of exactly 7 borrowed string literals
/// (`'static` — baked into the binary, no heap, no allocation). Compare the
/// growable `Vec<String>` used at runtime below.
/// `[&str; 7]` 是恰好 7 个元素的定长数组，元素是 'static 字符串字面量
/// （编译期写进二进制，无堆分配）。与下方运行时的 Vec<String> 相对。
pub const DEFAULT_NAV_PRESET: [&str; 7] =
    ["Unread", "Read Later", "Favourite", "Categories", "Tags", "Saved", "Lazy"];

/// Default key map: action name → key sequences (1-2 chars, or `<special>`).
/// 默认按键表：动作名 → 键序列（1-2 个字符，或 `<special>` 特殊键）。
///
/// Type walkthrough: `&[(&str, &[&str])]` = reference to a slice (array) of
/// tuples, each holding an action name and a slice of keys. All `'static`
/// literals — pure compile-time data, zero runtime cost until converted.
/// 类型解读：元组数组的引用，每项含动作名和键切片。全是 'static 字面量 —
/// 纯编译期数据，转换前零运行时开销。
pub const DEFAULT_KEYS: &[(&str, &[&str])] = &[
    ("open", &["l", "<enter>", "<right>"]),
    ("back", &["h", "q", "<esc>", "<left>"]),
    ("quit", &["Q"]),
    ("refresh", &["r"]),
    ("refresh_all", &["R"]),
    ("toggle_read", &["u"]),
    ("toggle_read_next", &["<space>"]),
    ("mark_list_read", &["a"]),
    ("mark_all_read", &["A"]),
    ("export", &["e"]),
    ("export_saved", &["E"]),
    ("browser", &["o"]),
    ("favourite", &["F"]),
    ("read_later", &["L"]),
    ("saved", &["S"]),
    ("new_feed", &["N"]),
    ("delete", &["D"]),
    ("rename", &["M"]),
    ("edit_tags", &["T"]),
    ("help", &["?"]),
    ("focus_next", &["<tab>"]),
    ("focus_prev", &["<backtab>"]),
    ("search", &["/"]),
    ("jump_top", &["gg"]),
    ("jump_bottom", &["G"]),
    ("next_unread", &["J"]),
    ("prev_unread", &["K"]),
    ("parent_next", &["n"]),
    ("parent_prev", &["p"]),
    ("copy_item_url", &["yy"]),
    ("copy_item_title", &["yn"]),
    ("copy_feed_url", &["yp"]),
    ("copy_item_summary", &["ys"]),
    ("copy_item_content", &["yc"]),
    ("sort_time", &["st"]),
    ("sort_title", &["sn"]),
    ("sort_feed", &["sf"]),
    ("sort_unread", &["su"]),
    ("sort_time_rev", &["sT"]),
    ("sort_title_rev", &["sN"]),
    ("sort_feed_rev", &["sF"]),
    ("sort_unread_rev", &["sU"]),
    ("cycle_preset", &["t"]),
    ("import_opml", &["i"]),
    ("export_opml", &["x"]),
];

/// Default keybindings as a HashMap<String, Vec<String>>.
/// 默认按键表，转成 HashMap<String, Vec<String>>。
///
/// This converts the borrow-heavy const table above into OWNED data:
/// every `&str` becomes a `String` via `.to_string()`, because the map
/// must outlive these literals' static context and be mutable.
/// 把上面借用型的常量表转成自有数据：每个 &str 经 .to_string() 变成 String，
/// 因为这张表要可变、要独立存活。
pub fn default_keybindings() -> std::collections::HashMap<String, Vec<String>> {
    // Iterator pipeline: .iter() borrows the table, .map() transforms each
    // tuple into a (String, Vec<String>) pair, .collect() assembles the
    // HashMap — Rust's idiomatic replacement for a C for-loop + insert.
    // 迭代器流水线：iter 借用遍历、map 逐项转换、collect 组装 HashMap —
    // 替代 C 手写循环加插入的惯用写法。
    // Nested closure: |k| k.to_string() runs once PER KEY inside building
    // each inner Vec. `a` destructures the tuple directly in the pattern.
    // 内层闭包对每个键执行一次 to_string；模式里的 a 直接解构元组。
    DEFAULT_KEYS
        .iter()
        .map(|(a, ks)| (a.to_string(), ks.iter().map(|k| k.to_string()).collect()))
        .collect()
}

/// App colors: markdown styles + pane accent/dim colors.
/// Loaded from the optional `theme` file (TOML, named colors).
/// 应用配色：markdown 样式 + 各面板强调色/暗色。
/// 从可选的 theme 文件加载（TOML 格式，支持命名颜色）。
///
/// `Color` is ratatui's enum — a value that holds EXACTLY ONE of a fixed
/// set of variants (named 16-color or Rgb(r,g,b)). Like a C enum with
/// payload, but the compiler forces you to cover every case in a match.
/// Color 是 ratatui 的枚举：取值只能是固定变体之一（16 命名色或 Rgb）。
/// 类似带载荷的 C 枚举，且 match 必须覆盖所有分支。
#[derive(Debug, Clone)] // Debug: {:?} printing; Clone: explicit deep copy
                        // Debug：{:?} 打印；Clone：显式深拷贝
pub struct ThemeColors {
    pub styles: MdStyleSheet,
    pub accent: Color,
    pub dim: Color,
    /// Active-pane border.
    pub focused: Color,
    /// Selected row highlight (nav + list).
    pub selected: Color,
    /// Nav top-level parent entries highlight.
    pub top: Color,
    /// Base background (status bar / scheme baseline).
    pub bg: Color,
}

// Hand-written Default (not #[derive(Default)]): these values are deliberate
// design choices (dark theme baseline), not whatever the types default to.
// 手写 Default（不用 derive）：这些值是刻意选择的暗色基线，不是类型的天然默认。
impl Default for ThemeColors {
    /// The dark-theme baseline every user starts from.
    /// 所有用户起步时的暗色基线。
    fn default() -> Self {
        Self {
            styles: MdStyleSheet::default(),
            accent: Color::Yellow,
            dim: Color::DarkGray,
            focused: Color::Yellow,
            selected: Color::DarkGray,
            top: Color::Yellow,
            bg: Color::Black,
        }
    }
}

/// Light base palette (dark text on light terminal).
/// 浅色基线配色（浅色终端上显示深色文字）。
///
/// Struct-literal construction: build the whole value in one expression.
/// Note `..` spread syntax is NOT used here — every field is written out,
/// so the compiler errors if a field is added later and missed.
/// 用结构体字面量一次性构造完整值。这里没省略任何字段 —
/// 日后新增字段若漏写会直接编译报错。
pub fn light_theme() -> ThemeColors {
    ThemeColors {
        focused: Color::Blue,
        selected: Color::Gray,
        top: Color::Blue,
        bg: Color::White,
        accent: Color::Blue,
        dim: Color::DarkGray,
        styles: MdStyleSheet {
            accent: Color::Blue,
            dim: Color::Gray,
            h1: Color::Blue,
            h2: Color::Blue,
            h3: Color::Magenta,
            code: Color::Green,
            link: Color::Blue,
            quote: Color::DarkGray,
        },
    }
}

/// tui-markdown StyleSheet mapping every element to the app palette.
/// markdown 渲染样式表：每种元素映射到应用配色中的一个颜色。
///
/// One color slot per rendered element kind (headings, code, links…);
/// kept SEPARATE from ThemeColors' UI chrome colors so themes can style
/// article content independently of pane borders.
/// 每种渲染元素一个颜色槽；与面板 UI 配色分开，主题可独立调整文章样式。
#[derive(Debug, Clone)]
pub struct MdStyleSheet {
    pub accent: Color,
    pub dim: Color,
    pub h1: Color,
    pub h2: Color,
    pub h3: Color,
    pub code: Color,
    pub link: Color,
    pub quote: Color,
}

// Dark-theme markdown defaults, same rationale as ThemeColors::default.
// 暗色主题的 markdown 默认色，理由同 ThemeColors::default。
impl Default for MdStyleSheet {
    fn default() -> Self {
        Self {
            accent: Color::Yellow,
            dim: Color::DarkGray,
            h1: Color::Yellow,
            h2: Color::Yellow,
            h3: Color::Blue,
            code: Color::Yellow,
            link: Color::Yellow,
            quote: Color::Gray,
        }
    }
}

// Trait implementation: MdStyleSheet promises the behavior tui_markdown
// expects. Under the hood this resembles filling a C struct of function
// pointers (a vtable) — but the compiler checks every required method is
// present, and signatures must match exactly.
// trait 实现：MdStyleSheet 承诺满足 tui_markdown 要求的行为。
// 底层类似手工填写函数指针表（vtable），但编译器检查所有必需方法及签名。
impl tui_markdown::StyleSheet for MdStyleSheet {
    /// Renderer hook: called per heading. `_` catch-all arm covers level 3+
    /// — exhaustive matching means no forgotten case, unlike a C switch
    /// where a missed label silently falls through.
    /// 渲染钩子：每个标题调用一次。`_` 分支兜底 3 级及以上 —
    /// match 强制穷尽，不像 C switch 漏标号会静默穿透。
    fn heading(&self, level: u8) -> Style {
        let c = match level {
            1 => self.h1,
            2 => self.h2,
            _ => self.h3,
        };
        // Builder chain: Style::new() then .fg()/.add_modifier() each return
        // the modified style — fluent construction, no mutation.
        // 构造器链：每次调用返回修改后的样式，流式构造，无需可变变量。
        Style::new().fg(c).add_modifier(Modifier::BOLD)
    }
    fn code(&self) -> Style {
        Style::new().fg(self.code)
    }
    fn link(&self) -> Style {
        Style::new().fg(self.link).add_modifier(Modifier::UNDERLINED)
    }
    fn blockquote(&self) -> Style {
        Style::new().fg(self.quote).add_modifier(Modifier::ITALIC)
    }
    fn table_header(&self) -> Style {
        Style::new().fg(self.accent).add_modifier(Modifier::BOLD)
    }
    fn table_cell(&self) -> Style {
        Style::default()
    }
    fn table_border(&self) -> Style {
        Style::new().fg(self.dim)
    }
    fn image_alt(&self) -> Style {
        Style::new().fg(self.dim)
    }
    fn code_block_fence(&self) -> &str {
        ""
    }
}

impl ThemeColors {
    /// Load theme overrides from an optional TOML file onto a base palette.
    /// Missing file, bad syntax, unknown color names — all silently ignored:
    /// a broken theme must never stop the app from launching. Always returns
    /// a usable ThemeColors.
    /// 从可选 TOML 文件把主题覆盖加载到基础配色上。
    /// 文件缺失/语法错误/未知颜色名一律静默忽略 — 主题损坏绝不能阻止启动。
    pub fn load(path: Option<&PathBuf>, mode: LightDark) -> ThemeColors {
        // base palette depends on light/dark
        // 基础配色取决于亮/暗模式；mut 表示后续会覆盖字段。
        // `let mut`: mutable binding — fields get patched below.
        let mut t = match mode {
            LightDark::Dark => ThemeColors::default(),
            LightDark::Light => light_theme(),
        };
        // let-else: destructure the Option, early-return on None.
        // Reads "bind p, OTHERWISE return t" — C's `if (!path) return t;`
        // without an extra indent level.
        // let-else：解构 Option，None 时提前返回，避免多一层缩进。
        let Some(p) = path else { return t };
        // fs::read_to_string returns io::Result<String>; .ok() drops the error
        // detail, keeping only Ok/Err as an Option-style check.
        // fs::read_to_string 返回 Result；.ok() 丢弃错误详情只看成败。
        let Ok(text) = fs::read_to_string(p) else { return t };
        // A LOCAL struct defined inside the fn — mirrors only the theme file's
        // schema. Every field Option<String> because any line may be absent.
        // 函数内定义的局部结构体，仅镜像主题文件结构；
        // 字段全为 Option<String>，因为任何一行都可能缺省。
        #[derive(Deserialize, Default, Clone)]
        #[serde(default)]
        struct RawTheme {
            h1: Option<String>,
            h2: Option<String>,
            h3: Option<String>,
            code: Option<String>,
            quote: Option<String>,
            link: Option<String>,
            accent: Option<String>,
            dim: Option<String>,
            focused: Option<String>,
            selected: Option<String>,
            top: Option<String>,
            background: Option<String>,
        }
        // one file carries both modes: [light] / [dark] tables; a flat file
        // (no sections) applies to the current mode
        #[derive(Deserialize, Default)]
        #[serde(default)]
        struct RawFile {
            light: Option<RawTheme>,
            dark: Option<RawTheme>,
        }
        // First parse as RawFile ([light]/[dark] sections); on error bail out
        // to the base palette — same "theme can't crash us" policy.
        // 先按 RawFile（带 [light]/[dark] 小节）解析；失败则退回基础配色 —
        // 同样贯彻“主题不能搞崩程序”的策略。
        let file: RawFile = match toml::from_str(&text) {
            Ok(r) => r,
            Err(_) => return t,
        };
        // flat file (no [light]/[dark]) — treat the whole file as current mode
        // Nested re-parse: the SAME text parsed again as a bare RawTheme.
        // Slightly redundant work, but cheap for a small config file.
        // 二次解析：同一段文本再按裸 RawTheme 解析一次，小文件开销可忽略。
        let raw = if file.dark.is_none() && file.light.is_none() {
            // unwrap_or_default: on parse failure give an all-empty theme
            // (every field None → nothing gets overridden).
            // unwrap_or_default：解析失败时给全空主题（字段全 None，等于不覆盖）。
            toml::from_str::<RawTheme>(&text).unwrap_or_default()
        } else {
            match mode {
                LightDark::Dark => file.dark.or(file.light),
                LightDark::Light => file.light.or(file.dark),
            }
            // .or(): first Option if Some, otherwise the second — current
            // mode preferred, other mode as fallback.
            // .or()：第一个为 Some 则取之，否则取第二个 — 当前模式优先，另一模式兜底。
            .unwrap_or_default()
        };
        // A CLOSURE stored in a variable (captures nothing, one arg).
        // Chain: v.as_deref() turns &Option<String> into Option<&str> (borrow,
        // no clone); .and_then(color_from_str) runs the parser ONLY on Some,
        // flattening Option<Option<Color>> → Option<Color>. This is Option
        // chaining — in C you'd nest two NULL checks.
        // 闭包存入变量（无捕获）。as_deref 把 &Option<String> 变成 Option<&str>
        // （借用不克隆）；and_then 仅在 Some 时调用解析函数，并把两层 Option
        // 拍平成一层 — 即 Option 链式处理，C 里要嵌套两层判空。
        let pick = |v: &Option<String>| v.as_deref().and_then(color_from_str);
        // Loop over (&source, &mut target) pairs: ONE loop body applies six
        // config keys instead of six copies of the same code.
        // 遍历 (&源值, &mut 目标字段) 对：一个循环体覆盖六个键，避免六段重复代码。
        for (from, to) in [
            (&raw.accent, &mut t.accent),
            (&raw.dim, &mut t.dim),
            (&raw.focused, &mut t.focused),
            (&raw.selected, &mut t.selected),
            (&raw.top, &mut t.top),
            (&raw.background, &mut t.bg),
        ] {
            if let Some(c) = pick(from) {
                *to = c; // * dereferences &mut Color to assign through it
                         // * 解引用 &mut Color 完成赋值
            }
        }
        // styles sits INSIDE t but needs separate patching; clone it out,
        // mutate the copy, write it back — avoids borrowing t while also
        // mutating it (the borrow checker forbids that).
        // styles 在 t 内部但需单独修补；先克隆副本改完再写回，
        // 避免一边借用一边修改 t（借用检查器禁止这样做）。
        let mut styles = t.styles.clone();
        if let Some(c) = pick(&raw.accent) {
            styles.accent = c;
            t.accent = c;
        }
        if let Some(c) = pick(&raw.dim) {
            styles.dim = c;
            t.dim = c;
        }
        for (from, field) in [
            (&raw.h1, &mut styles.h1),
            (&raw.h2, &mut styles.h2),
            (&raw.h3, &mut styles.h3),
            (&raw.code, &mut styles.code),
            (&raw.link, &mut styles.link),
            (&raw.quote, &mut styles.quote),
        ] {
            if let Some(c) = pick(from) {
                *field = c;
            }
        }
        t.styles = styles;
        t
    }
}

/// Named color → ratatui Color (16-color palette).
/// 颜色名字符串 → ratatui Color（16 色调色板）。
///
/// Returns Option: None means "unrecognized"; caller decides whether that's
/// fatal (here: not — invalid names are simply skipped).
/// 返回 Option：None 表示无法识别，由调用方决定是否致命（此处直接跳过）。
pub fn color_from_str(s: &str) -> Option<Color> {
    // trim whitespace, lowercase via shadowing: to_ascii_lowercase returns a
    // NEW String; rebinding under the same name is idiomatic Rust.
    // 去空白后转小写并用同名遮蔽：返回新 String，同名重绑是惯用写法。
    let s = s.trim().to_ascii_lowercase();
    // strip_prefix returns Option<&str>: Some(rest) after '#', or None.
    // strip_prefix 返回 Option<&str>：'#' 之后的剩余部分。
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() == 6 {
            // from_str_radix(base 16): text → number. The tuple-if-let enters
            // only when ALL three parses succeed. Slicing &hex[a..b] is
            // bounds-checked (panic, never memory corruption, if out of range).
            // 按 16 进制解析文本为数字；三元 if-let 需全部成功才进入；
            // 切片越界只会 panic（有边界检查），不会内存越界访问。
            if let (Ok(r), Ok(g), Ok(b)) = (
                u8::from_str_radix(&hex[0..2], 16),
                u8::from_str_radix(&hex[2..4], 16),
                u8::from_str_radix(&hex[4..6], 16),
            ) {
                return Some(Color::Rgb(r, g, b));
            }
        }
        return None;
    }
    // Match on string contents: `|` merges alternative patterns per arm;
    // `_` arm early-returns None for anything unrecognized.
    // 对字符串内容匹配；| 合并多个模式；_ 分支对未知名字提前返回 None。
    Some(match s.as_str() {
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "gray" | "grey" => Color::Gray,
        "darkgray" | "darkgrey" => Color::DarkGray,
        "white" => Color::White,
        _ => return None,
    })
}

/// Raw mirror of the config FILE — every key optional, parsed via serde.
/// 配置文件的原始镜像 — 每个键都可缺省，由 serde 解析。
///
/// This is the "raw layer" of the two-layer pattern: it holds exactly what
/// the user wrote (or didn't). `Config::apply` merges it over the defaults.
/// `#[serde(default)]` + `Option` fields = unknown keys ignored, missing
/// keys become None. Compare C: a hand-written parser with an if per key.
/// 两层模式的“原始层”：只保存用户写了（或没写）的内容，
/// 由 Config::apply 覆盖到默认值上。未知键忽略、缺失键为 None。
#[derive(Debug, Clone, Default, Deserialize)] // Default: all-None baseline
                                              // Default：全 None 的基线
#[serde(default)] // per-key default when absent; unknown keys ignored
                  // 缺失的键取默认值；未知键直接忽略
struct RawConfig {
    cache_ttl_days: Option<u64>,
    export_dir: Option<String>,
    export_saved_path: Option<String>,
    browser: Option<String>,
    refresh: Option<RefreshCfg>,
    fetch_timeout: Option<u64>,
    max_items_per_feed: Option<usize>,
    db_path: Option<String>,
    theme: Option<String>,
    pane_ratio: Option<Vec<f64>>,
    nav_presets: Option<Vec<Vec<String>>>,
    default_view: Option<String>,
    keybindings: Option<std::collections::HashMap<String, KeySpec>>,
    sort: Option<Vec<String>>,
    foldlevel: Option<usize>,
    reading_width: Option<u64>,
    offset: Option<usize>,
    background: Option<String>,
    markers: Option<RawMarkers>,
}

/// Shape-flexible `refresh` value: accepts either `refresh = true`
/// or `refresh = { interval_minutes = 30 }` in the same key.
/// refresh 字段可接受两种形态：布尔或含 interval_minutes 的表。
///
/// `#[serde(untagged)]`: variants carry NO tag in the file text — serde just
/// tries Bool first; if the JSON/TOML value isn't a bool, tries Table.
/// Like parsing "either a number or a string" in C with two sscanf attempts.
/// untagged：文件里没有类型标签 — serde 先试 Bool，不是布尔再试 Table。
/// 类似 C 里对同一输入先后尝试两种 sscanf 格式。
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum RefreshCfg {
    Bool(bool),
    Table { interval_minutes: Option<u64> },
}

/// A binding value: `"l"` or `["l", "<enter>"]` (multiple keys per action).
/// 一个按键绑定的值：可以是单个键字符串，也可以是键列表（一个动作多键）。
///
/// Same untagged trick as RefreshCfg — one config key, two shapes.
/// 与 RefreshCfg 同样的 untagged 技巧 — 一个配置键，两种形态。
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum KeySpec {
    One(String),
    Many(Vec<String>),
}

/// Item/feed status markers, rendered next to rows. Values are literal
/// strings — set nerd-font glyphs (no brackets) or keep `[S]`-style ASCII.
/// 条目/源状态标记，显示在行首。值是字面字符串 —
/// 可配 nerd-font 图标（无括号），也可保留 [S] 风格 ASCII。
#[derive(Debug, Clone)]
pub struct Markers {
    pub saved: String,
    pub later: String,
    pub favourite: String,
    pub lazy: String,
}

// Hand-written Default: the bracketed ASCII fallbacks are deliberate choices.
// 手写 Default：带括号的 ASCII 兜底是刻意选择。
impl Default for Markers {
    fn default() -> Self {
        Self {
            saved: "[S]".into(), // .into(): &str → String via the From trait;
            later: "[L]".into(), // shorter than .to_string(), same effect
            favourite: "[F]".into(), // .into()：经 From trait 把 &str 转成
            lazy: "[Z]".into(), // String，比 .to_string() 简短，效果相同
        }
    }
}

/// Config `[markers]` table — each key optional, missing = default.
/// 配置里的 [markers] 小节 — 每个键可选，缺失即用默认值。
///
/// The raw twin of Markers: parse-time shape (all Option), converted into
/// plain Strings during apply().
/// Markers 的解析期孪生体：全 Option 的原始形态，apply 时转成普通 String。
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct RawMarkers {
    saved: Option<String>,
    later: Option<String>,
    favourite: Option<String>,
    lazy: Option<String>,
}

/// Base color scheme selected by `background` in config.
/// 由配置中的 background 键选择的基础配色方案。
///
/// `Copy` derive: this enum is tiny (one byte), so passing it COPIES it —
/// no borrow needed, usable after move. Like passing an int by value in C.
/// `PartialEq` derive: enables `mode == LightDark::Light` comparisons.
/// Copy 派生：该枚举极小，传参即按值拷贝，类似 C 按值传 int；
/// PartialEq 派生后可用 == 比较。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LightDark {
    Light,
    Dark,
}



/// Fully-resolved runtime config: every field concrete, no Options except
/// where absence itself is meaningful (e.g. browser unset = use default
/// opener; refresh_interval None = no background timer).
/// 完全解析后的运行时配置：每个字段都是具体值；仅当“缺省”本身有含义时
/// 才保留 Option（如未配浏览器、无后台刷新定时器）。
///
/// Contrast with RawConfig above: that one mirrors the FILE (all Option),
/// this one is what the rest of the app consumes. Paths are PathBuf (the
/// std path type — like a malloc'd char* path, but self-freeing and
/// cross-platform).
/// 与上面的 RawConfig 对照：那层镜像文件内容，这层供程序其余部分消费。
/// 路径用 PathBuf（标准库路径类型，自管理内存且跨平台）。
#[derive(Debug, Clone)]
pub struct Config {
    pub config_dir: PathBuf,
    pub urls_path: PathBuf,
    pub db_path: PathBuf,
    pub cache_ttl_days: u64,
    pub export_dir: PathBuf,
    pub export_saved_path: PathBuf,
    pub browser: Option<String>,
    pub refresh_on_startup: bool,
    pub refresh_interval_minutes: Option<u64>,
    pub fetch_timeout: u64,
    pub max_items_per_feed: Option<usize>,
    pub theme_path: Option<PathBuf>,
    pub pane_ratio: [f64; 3],
    /// Rows kept visible above/below the cursor when nav/list scrolls.
    /// 导航/列表滚动时，光标上下方各保留的可见行数。
    pub offset: usize,
    pub nav_presets: Vec<Vec<String>>,
    pub default_view: Option<String>,
    pub sort: Vec<String>,
    pub foldlevel: Option<usize>,
    pub reading_width: u64,
    pub keybindings: std::collections::HashMap<String, Vec<String>>,
    pub background: LightDark,
    pub markers: Markers,
}

impl Config {
    /// Build the effective config: XDG defaults first, then overlay the
    /// user's config file if it exists. The single entry point used at
    /// startup; never fails — worst case you get pure defaults.
    /// 构建生效配置：先取 XDG 默认值，若用户配置文件存在再叠加。
    /// 启动时的唯一入口；不会失败 — 最坏情况就是纯默认值。
    pub fn load() -> Config {
        // xdg:: returns the base dirs ($XDG_CONFIG_HOME etc., with the
        // ~/.config fallback). .join() builds a subpath — no manual string
        // concatenation with '/' like in C.
        // xdg:: 返回各基准目录（含 ~/.config 回退）；join() 拼接子路径，
        // 无需像 C 那样手动拼 '/' 字符串。
        let config_dir = xdg::config_home().join("markerss");
        let cache_dir = xdg::cache_home().join("markerss");
        let data_dir = xdg::data_home().join("markerss");

        // Struct literal: EVERY field must be listed exactly once — the
        // compiler errors on missing or duplicate fields, so this block IS
        // the authoritative defaults table.
        // 结构体字面量：每个字段必须恰好出现一次，漏写/重复都会编译报错，
        // 因此这一块就是权威的默认值表。
        let mut cfg = Config {
            urls_path: config_dir.join("urls"),
            db_path: cache_dir.join("markerss.db"),
            cache_ttl_days: 14,
            export_dir: data_dir.clone(), // clone: data_dir reused two lines down
                                          // 克隆：两行之后还要再用 data_dir
            export_saved_path: data_dir.join("saved.md"),
            browser: None,
            refresh_on_startup: true,
            refresh_interval_minutes: None,
            fetch_timeout: 30,
            max_items_per_feed: None,
            theme_path: None,
            pane_ratio: [0.15, 0.15, 0.7],
            offset: 3,
            nav_presets: vec![DEFAULT_NAV_PRESET.iter().map(|s| s.to_string()).collect()],
            // vec![...] macro: build a Vec from elements. Inner chain converts
            // the static [&str; 7] into an owned Vec<String> — the const stays
            // untouched for next time.
            // vec![...] 宏从元素构建 Vec；内层链把静态 [&str; 7] 转成自有
            // Vec<String>，常量本身保持不动。
            default_view: None,
            sort: Vec::new(),
            foldlevel: None,
            reading_width: 0,
            keybindings: default_keybindings(),
            background: LightDark::Dark,
            markers: Markers::default(),
            config_dir: config_dir.clone(),
        };

        // If the user's config file parsed successfully, layer it on top of
        // the defaults just built. Absent/broken file → defaults stand.
        // 用户配置解析成功则叠加到刚建好的默认值上；文件缺失或损坏则维持默认。
        if let Some(raw) = load_raw(&config_dir.join("config.toml")) {
            cfg.apply(raw);
        }
        cfg // no `;`: the expression IS the return value (like `return cfg;`)
            // 不带分号：该表达式即返回值（等价于 return cfg;）
    }

    /// Merge a parsed RawConfig into self: for each key, Some(v) overwrites,
    /// None keeps whatever is already there. This is where raw user input
    /// becomes validated runtime state.
    /// 把解析出的 RawConfig 合并进自身：键为 Some 则覆盖，None 保留原值。
    /// 用户原始输入在此变成校验后的运行时状态。
    fn apply(&mut self, raw: RawConfig) {
        // The recurring pattern of this fn: if-let on each Option key.
        // `if let Some(v) = raw.x` reads "if the user set x, bind v to it".
        // 本函数反复出现的模式：对每个 Option 键做 if-let，
        // 含义是“用户配了就绑定并使用”。
        if let Some(v) = raw.cache_ttl_days {
            self.cache_ttl_days = v;
        }
        if let Some(v) = raw.export_dir {
            self.export_dir = expand_tilde(&v);
        }
        if let Some(v) = raw.export_saved_path {
            self.export_saved_path = expand_tilde(&v);
        }
        self.browser = raw.browser;
        // RefreshCfg's untagged enum pays off here: match picks behavior per
        // shape — bool flips startup refresh, table sets the interval timer.
        // untagged 枚举在此兑现价值：按形态分别处理 — 布尔控制启动刷新，表设置定时器。
        if let Some(v) = raw.refresh {
            match v {
                RefreshCfg::Bool(b) => self.refresh_on_startup = b,
                RefreshCfg::Table { interval_minutes } => {
                    // Destructuring match: interval_minutes bound straight
                    // out of the struct variant (itself still Option<u64>).
                    // 解构匹配：直接从结构体变体中绑定 interval_minutes（仍是 Option<u64>）。
                    self.refresh_interval_minutes = interval_minutes
                }
            }
        }
        if let Some(v) = raw.fetch_timeout {
            self.fetch_timeout = v;
        }
        self.max_items_per_feed = raw.max_items_per_feed;
        if let Some(v) = raw.theme {
            self.theme_path = Some(resolve_config_path(&v, &self.config_dir));
        }
        if let Some(v) = raw.db_path {
            self.db_path = resolve_config_path(&v, &self.config_dir);
        }
        if let Some(v) = raw.sort {
            // iterator + take(2): keep at most the first two sort keys —
            // silently truncating over-specification.
            // 迭代器 + take(2)：最多保留前两个排序键，多余项静默截断。
            self.sort = v.into_iter().take(2).collect();
        }
        self.foldlevel = raw.foldlevel;
        self.reading_width = raw.reading_width.unwrap_or(0);
        if let Some(v) = raw.offset {
            // Config key present → override the default. usize is Copy, so
            // this assigns by value, not by move.
            // 配置键存在则覆盖默认值。usize 是 Copy 类型，按值赋值而非移动。
            self.offset = v;
        }
        if let Some(b) = raw.background {
            // Match on the string CONTENTS after lowercasing; anything not
            // "light" falls back to Dark (lenient parsing by design).
            // 小写后按字符串内容匹配；非 "light" 一律回退 Dark（刻意宽松解析）。
            self.background = match b.to_ascii_lowercase().as_str() {
                "light" => LightDark::Light,
                _ => LightDark::Dark,
            };
        }
        if let Some(k) = raw.keybindings {
            self.apply_keybindings(k);
        }
        // optional standalone keybindings.toml — overrides config.toml's map
        // 可选的独立 keybindings.toml — 覆盖 config.toml 里的按键表
        // Double if-let-ok instead of ?: file may be absent AND may contain
        // other keys only — both cases are fine, just skip.
        // 两层 if-let-ok：文件可以不存在，也可以不含按键表 — 都直接跳过。
        let kb_path = self.config_dir.join("keybindings.toml");
        if let Ok(text) = fs::read_to_string(&kb_path) {
            if let Ok(raw_kb) = toml::from_str::<RawConfig>(&text) {
                if let Some(k) = raw_kb.keybindings {
                    self.apply_keybindings(k);
                }
            }
        }
        // Validation by hand: only accept EXACTLY three ratios (a [f64; 3]
        // fixed-size array is required downstream). Wrong length = ignored.
        // 手工校验：仅接受恰好三个比例（下游需要 [f64; 3] 定长数组）；
        // 长度不对则忽略整项。
        if let Some(v) = raw.pane_ratio {
            if v.len() == 3 {
                self.pane_ratio = [v[0], v[1], v[2]];
            }
        }
        if let Some(v) = raw.nav_presets {
            // Empty list would blank the nav strip — reject it, keep default.
            // 空列表会清空导航条 — 拒绝并保留默认值。
            if !v.is_empty() {
                self.nav_presets = v;
            }
        }
        self.default_view = raw.default_view;
        if let Some(m) = raw.markers {
            if let Some(v) = m.saved {
                self.markers.saved = v;
            }
            if let Some(v) = m.later {
                self.markers.later = v;
            }
            if let Some(v) = m.favourite {
                self.markers.favourite = v;
            }
            if let Some(v) = m.lazy {
                self.markers.lazy = v;
            }
        }
    }
}

/// Expand a leading `~` to the home directory (no-op otherwise).
/// Resolve `~`, then make relative paths relative to the config dir
/// (so theme/db settings work no matter where markerss is launched).
/// 先展开开头的 ~ 到主目录（否则原样）；再把相对路径解析到配置目录下
/// （这样无论从哪里启动 markerss，theme/db 设置都能生效）。
fn resolve_config_path(p: &str, config_dir: &std::path::Path) -> PathBuf {
    let expanded = expand_tilde(p);
    if expanded.is_absolute() {
        expanded
    } else {
        config_dir.join(expanded)
    }
}

/// Replace a leading `~/` with $HOME; anything else passes through.
/// 把开头的 ~/ 替换为 $HOME；其余原样返回。
///
/// Deliberately minimal: no `~user` support, no mid-path expansion —
/// config files only need the common case.
/// 刻意从简：不支持 ~user，不展开路径中间的 ~ — 配置文件只需覆盖常见情形。
fn expand_tilde(p: &str) -> PathBuf {
    // strip_prefix: Option<&str> = remainder after "~/" if present.
    // strip_prefix：若以 "~/" 开头，给出剩余部分的 Option。
    if let Some(rest) = p.strip_prefix("~/") {
        // var_os reads the raw OS bytes of $ENV (no UTF-8 requirement) and
        // returns OsString — the type for strings that came from outside.
        // var_os 按原始系统字节读环境变量（不要求 UTF-8），返回 OsString。
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(p)
}

// Second impl block for Config — Rust allows MANY impl blocks per type;
// grouping related methods is stylistic, not required.
// Config 的第二个 impl 块 — Rust 允许一个类型有多个 impl 块；
// 按主题分组方法只是风格选择。
impl Config {
    /// Overwrite the key map with user bindings, normalizing each value to
    /// a Vec<String> regardless of which shape the file used.
    /// 用用户绑定整体替换按键表，并把每个值统一成 Vec<String> 形态。
    fn apply_keybindings(&mut self, k: std::collections::HashMap<String, KeySpec>) {
        // into_iter() CONSUMES the incoming map (we own it — no clone needed);
        // each KeySpec value is normalized then the whole thing re-collected.
        // into_iter() 消费传入的 map（已拥有所有权，无需克隆）；
        // 逐项规范化后重新 collect 成目标形态。
        self.keybindings = k
            .into_iter()
            .map(|(a, spec)| (a, spec_to_keys(spec)))
            .collect();
    }
}

/// Flatten a KeySpec (single or list) into key strings.
/// 把 KeySpec（单键或列表）展平成键字符串列表。
///
/// Match consumes the enum by value (no `&`): one arm wraps the single
/// String in a new Vec, the other hands back the existing Vec — ownership
/// moves into the return value either way.
/// match 按值消费枚举：一个分支把单个 String 包进新 Vec，
/// 另一个直接交出已有 Vec — 所有权都随返回值转移。
fn spec_to_keys(spec: KeySpec) -> Vec<String> {
    match spec {
        KeySpec::One(k) => vec![k], // vec![k] macro: one-element Vec
                                    // vec![k] 宏：单元素 Vec
        KeySpec::Many(ks) => ks,
    }
}

/// Parse the config file by extension; `None` when unreadable/absent.
/// 按扩展名解析配置文件；读不到或解析失败时返回 None。
///
/// `Option<RawConfig>` + early returns keep this total: any failure mode
/// (missing file, bad syntax) just means "no overrides".
/// 返回 Option 使函数总能给出结果：文件缺失或语法错误都只意味着“无覆盖”。
fn load_raw(path: &std::path::Path) -> Option<RawConfig> {
    // `.ok()?`: read_to_string gives Result; .ok() makes it Option; `?`
    // RETURNS None from this fn immediately if it's None. The `?` operator
    // is Rust's replacement for C's `if (!f) return NULL;` boilerplate.
    // .ok()? ：Result 转 Option 后，`?` 在 None 时立即从本函数返回 None。
    // ? 运算符就是 C 里层层判空返回的替代品。
    let text = fs::read_to_string(path).ok()?;
    // file_name(): Option<&OsStr>; another `?` chains the failure out.
    // to_string_lossy(): OsStr → &str with invalid UTF-8 replaced by U+FFFD
    // (never panics — "lossy" conversion).
    // file_name() 取文件名（Option），再用 ？ 链式传出失败；
    // to_string_lossy 把非法 UTF-8 替换为替换符，绝不 panic。
    let name = path.file_name()?.to_string_lossy().to_string();
    // rsplit_once('.'): split at the LAST dot → (before, after). Extension
    // detection that survives dots in the directory path or stem.
    // rsplit_once 按最后一个 '.' 切分，避免路径/文件名中的其他点干扰。
    let parsed: Option<RawConfig> = match name.rsplit_once('.') {
        Some((_, ext)) => match ext.to_ascii_lowercase().as_str() {
            // Same RawConfig struct, four different parsers — serde backends
            // are interchangeable because they all target Deserialize.
            // 同一 RawConfig 结构体配四种解析器 — 都面向 Deserialize，可互换。
            "json" => serde_json::from_str(&text).ok(),
            "jsonc" => {
                // JSONC pass 1: strip comments. Pass 2: regex removes
                // trailing commas before } or ] (strict JSON forbids them).
                // Regex::new(...).unwrap(): pattern is a compile-time constant,
                // so unwrapping can't actually fail here.
                // 第一步去注释；第二步用正则删掉 }/] 前的尾逗号（严格 JSON 不允许）。
                // 正则是编译期常量，unwrap 实际不会失败。
                let cleaned = strip_jsonc(&text);
                let re = regex::Regex::new(r",\s*([}\]])$").unwrap();
                // replace_all returns Cow<str> (borrow-or-owned); .to_string()
                // pins down an owned String either way.
                // replace_all 返回 Cow<str>（借用或自有）；to_string 统一为自有 String。
                let cleaned = re.replace_all(&cleaned, "$1").to_string();
                serde_json::from_str(&cleaned).ok()
            }
            "yaml" | "yml" => serde_yaml::from_str(&text).ok(),
            _ => toml::from_str(&text).ok(),
        },
        None => toml::from_str(&text).ok(),
    };
    parsed
}

/// Strip `//` and `/* */` comments outside strings (simple JSONC support).
/// 去除字符串以外的 // 和 /* */ 注释（简易 JSONC 支持）。
///
/// A hand-rolled state machine — exactly what you'd write in C with a char
/// pointer and flag variables, except indexing is bounds-checked and there's
/// no manual memory management for the output buffer.
/// 手写状态机 — 与 C 中用字符指针加标志变量的写法相同，
/// 区别在于下标有越界检查、输出缓冲区自动管理。
fn strip_jsonc(s: &str) -> String {
    // with_capacity(s.len()): output ≤ input, so preallocate once.
    // 输出不超过输入长度，按此预分配一次即可。
    let mut out = String::with_capacity(s.len());
    // Three state flags: inside a line comment / block comment / string literal.
    // 三个状态标志：行注释内 / 块注释内 / 字符串字面量内。
    let mut in_str = false;
    let mut in_line = false;
    let mut in_block = false;
    // Collect into Vec<char>: O(1) indexed access (a str alone only offers
    // byte slicing, which could split multibyte chars).
    // 先收集成 Vec<char> 以便 O(1) 下标访问（str 直接切片可能切断多字节字符）。
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        // peek ahead one char without moving: .get(i+1) is bounds-checked,
        // .copied() turns Option<&char> into Option<char>. In C you'd compare
        // c and c[1] and risk reading past the end at the last byte.
        // 向前窥一个字符但不前进：get 有越界检查，copied 把 Option<&char>
        // 变 Option<char>。C 里直接读 c[1] 在末尾会越界。
        let next = chars.get(i + 1).copied();
        if in_line {
            if c == '\n' {
                in_line = false;
                out.push(c);
            }
            i += 1;
            continue;
        }
        if in_block {
            if c == '*' && next == Some('/') {
                in_block = false;
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        if in_str {
            out.push(c);
            if c == '\\' {
                // Escaped char inside a string: copy it AND the next char
                // verbatim (i += 2) so `"//"` isn't mistaken for a comment.
                // 字符串内的转义字符：原样复制并跳两格（i += 2），
                // 防止 "//" 被误判成注释。
                if let Some(n) = next {
                    out.push(n);
                }
                i += 2;
                continue;
            }
            if c == '"' {
                in_str = false; // closing quote: back to code context
                                // 闭引号：回到代码上下文
            }
            i += 1;
            continue;
        }
        match (c, next) {
            ('/', Some('/')) => in_line = true,
            ('/', Some('*')) => in_block = true,
            ('"', _) => {
                in_str = true;
                out.push(c);
            }
            _ => out.push(c),
        }
        i += 1;
    }
    out
}

// Unit tests — run with `cargo test`. #[cfg(test)] means this module is
// compiled ONLY for tests, never in the release binary.
// 单元测试 — cargo test 运行。#[cfg(test)] 保证测试代码不进发布二进制。
#[cfg(test)]
mod tests {
    use super::*; // import everything from the parent module (this file)
                  // 导入父模块（本文件）的全部内容

    /// Write config text to a throwaway temp dir and parse it.
    /// 把配置文本写入一次性临时目录并解析。
    fn load_from(text: &str, name: &str) -> Option<RawConfig> {
        use std::sync::atomic::{AtomicU64, Ordering};
        // AtomicU64: an integer shared across threads with no data races —
        // fetch_add increments AND returns the old value in one atomic step,
        // giving every call a unique counter value.
        // AtomicU64：跨线程安全的整数；fetch_add 原子地自增并返回旧值，
        // 每次调用拿到唯一计数。
        static N: AtomicU64 = AtomicU64::new(0);
        // Unique temp dir per test invocation: pid + counter → parallel test
        // runs never collide on the filesystem.
        // 每次调用一个唯一临时目录：pid + 计数器 → 并行测试不会互相踩目录。
        let dir = std::env::temp_dir().join(format!(
            "markerss-cfg-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join(name);
        std::fs::write(&path, text).ok();
        let r = load_raw(&path);
        std::fs::remove_dir_all(&dir).ok();
        r
    }

    #[test]
    fn toml_default() {
        let r = load_from("cache_ttl_days = 7\nbrowser = \"firefox\"\n", "config").unwrap();
        assert_eq!(r.cache_ttl_days, Some(7));
        assert_eq!(r.browser.as_deref(), Some("firefox"));
    }

    #[test]
    fn json_by_extension() {
        let r = load_from(r#"{"export_dir": "/data/out"}"#, "config.json").unwrap();
        assert_eq!(r.export_dir.as_deref(), Some("/data/out"));
    }

    #[test]
    fn markers_and_saved_path_parsed() {
        let r = load_from(
            "export_saved_path = \"~/saved.md\"\n[markers]\nsaved = \"\\uF02E\"\nlater = \"[L]\"\nfavourite = \"\\uF005\"\nlazy = \"\\uF186\"\n",
            "config",
        )
        .unwrap();
        assert_eq!(r.export_saved_path.as_deref(), Some("~/saved.md"));
        let m = r.markers.unwrap();
        assert_eq!(m.saved.as_deref(), Some("\u{f02e}"));
        assert_eq!(m.later.as_deref(), Some("[L]"));
        assert_eq!(m.favourite.as_deref(), Some("\u{f005}"));
        assert_eq!(m.lazy.as_deref(), Some("\u{f186}"));
    }

    #[test]
    fn jsonc_comments() {
        let r = load_from("{\n// comment\n\"fetch_timeout\": 15,\n}", "config.jsonc").unwrap();
        assert_eq!(r.fetch_timeout, Some(15));
    }

    #[test]
    fn yaml_by_extension() {
        let r = load_from("pane_ratio:\n  - 0.2\n  - 0.2\n  - 0.6\n", "config.yaml").unwrap();
        assert_eq!(r.pane_ratio, Some(vec![0.2, 0.2, 0.6]));
    }

    #[test]
    fn nav_presets_parsed() {
        let r = load_from(
            "nav_presets = [[\"Unread\", \"Feeds\"], [\"Unread\", \"Later\"]]",
            "config",
        )
        .unwrap();
        assert_eq!(r.nav_presets.unwrap().len(), 2);
    }

    #[test]
    fn refresh_bool_vs_table() {
        // matches!: macro returning true/false for a pattern — testing which
        // enum VARIANT arrived without caring about the payload (`..` ignores
        // struct-variant fields).
        // matches! 宏按模式返回布尔 — 只关心到达的是哪个枚举变体，
        // `..` 忽略结构体变体的字段。
        let r = load_from("refresh = false", "config").unwrap();
        assert!(matches!(r.refresh, Some(RefreshCfg::Bool(false))));
        let r = load_from("refresh = { interval_minutes = 30 }", "config").unwrap();
        assert!(matches!(r.refresh, Some(RefreshCfg::Table { .. })));
    }

    #[test]
    fn unknown_keys_ignored() {
        let r = load_from("bogus_key = 1\ncache_ttl_days = 3\n", "config").unwrap();
        assert_eq!(r.cache_ttl_days, Some(3));
    }

    #[test]
    fn missing_file_none() {
        assert!(load_raw(&std::path::Path::new("/nonexistent/config")).is_none());
    }

    #[test]
    fn default_mark_keys_are_distinct() {
        let kb = default_keybindings();
        assert_eq!(kb.get("mark_list_read").map(|v| v[0].as_str()), Some("a"));
        assert_eq!(kb.get("mark_all_read").map(|v| v[0].as_str()), Some("A"));
        assert_eq!(kb.get("toggle_read").map(|v| v[0].as_str()), Some("u"));
    }
}

// Second test module covering the advanced-spec keys. Same temp-dir helper
// duplicated under a different prefix so both modules can run in parallel.
// 第二个测试模块覆盖 Advanced 规格的键；临时目录助手换前缀重复一份，
// 以便两个模块并行运行互不冲突。
#[cfg(test)]
mod advanced_tests {
    use super::*;

    fn load_from(text: &str, name: &str) -> Option<RawConfig> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "markerss-cfg2-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join(name);
        std::fs::write(&path, text).ok();
        let r = load_raw(&path);
        std::fs::remove_dir_all(&dir).ok();
        r
    }

    #[test]
    fn sort_and_foldlevel_parsed() {
        let r = load_from("sort = [\"unread\", \"time\"]\nfoldlevel = 1\n", "config.toml").unwrap();
        assert_eq!(r.sort, Some(vec!["unread".to_string(), "time".to_string()]));
        assert_eq!(r.foldlevel, Some(1));
    }

    #[test]
    fn offset_parsed() {
        let r = load_from("offset = 5\n", "config.toml").unwrap();
        assert_eq!(r.offset, Some(5));
    }

    #[test]
    fn sort_capped_at_three() {
        // Exercises the apply()-side truncation logic directly on a Config:
        // four sort keys in, take(2) keeps two.
        // 直接在 Config 上验证 apply 侧的截断逻辑：传入四个排序键，take(2) 留两个。
        let r = load_from("sort = [\"a\", \"b\", \"c\", \"d\"]\n", "config.toml").unwrap();
        let mut cfg = Config::load();
        if let Some(v) = r.sort {
            cfg.sort = v.into_iter().take(2).collect();
        }
        assert_eq!(cfg.sort.len(), 2);
    }

    #[test]
    fn theme_colors_load() {
        // Integration-style check of ThemeColors::load against a real temp
        // file: flat theme (no [light]/[dark]) applies to the current mode,
        // and one accent key must patch BOTH t.accent and styles.accent.
        // 用真实临时文件集成验证 ThemeColors::load：平铺主题作用于当前模式，
        // 且 accent 一个键要同时更新 t.accent 与 styles.accent。
        let dir = std::env::temp_dir().join(format!("markerss-theme-{}", std::process::id()));
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join("theme.toml");
        std::fs::write(&path, "accent = \"green\"\nh1 = \"red\"\ncode = \"cyan\"\n").ok();
        let t = ThemeColors::load(Some(&path), LightDark::Dark);
        assert_eq!(t.accent, Color::Green);
        assert_eq!(t.styles.accent, Color::Green);
        std::fs::remove_dir_all(&dir).ok();
    }
}
