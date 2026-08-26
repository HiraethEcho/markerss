//! XDG directory resolution for markerss, per XDG Base Directory spec.
//! 按 XDG Base Directory 规范解析 markerss 的各目录。
//! `dirs` crate already applies the spec fallbacks when vars are unset:
//! `~/.config`, `~/.cache`, `~/.local/state`, `~/.local/share`.
//! 环境变量未设置时，`dirs` crate 已按规范回退到这些默认路径。

// PathBuf: an owned, growable path — the path equivalent of String.
// Its borrowed view is `&Path` / `&str`-like `&OsStr`.
// PathBuf：拥有所有权的路径类型，相当于路径版的 String；
// 其借用视图是 &Path（类似 &str 之于 String）。
use std::path::PathBuf;

/// Home directory, or "." as a last-resort fallback.
/// 主目录；实在拿不到就退化为当前目录 "."。
fn home() -> PathBuf {
    // dirs::home_dir() returns Option<PathBuf>:
    //   Some(path) if found, None otherwise. Like a pointer that may be NULL,
    //   but the compiler forces you to handle the None case before use —
    //   no null-pointer dereferences, ever.
    // dirs::home_dir() 返回 Option<PathBuf>：Some(有值) 或 None(无值)。
    // 相当于可能为 NULL 的指针，但编译器强制先处理 None 才能使用，
    // 从根源上杜绝空指针解引用。
    //
    // unwrap_or_else takes a CLOSURE (`|| ...`) run only when None:
    // lazy default — cheaper than computing a value we might not need.
    // unwrap_or_else 接收一个闭包（`|| ...`），只在 None 时才执行：
    // 惰性求值，避免白算一个用不上的默认值。
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

/// Config dir: `$XDG_CONFIG_HOME/markerss` or `~/.config/markerss`.
/// 配置目录：$XDG_CONFIG_HOME/markerss 或 ~/.config/markerss。
pub fn config_home() -> PathBuf {
    // .join() appends a path component with the right separator (/ on Unix).
    // .join() 用正确的分隔符拼接路径（Unix 上是 /）。
    dirs::config_dir().unwrap_or_else(|| home().join(".config"))
}

/// Cache dir: `~/.cache/markerss` (safe to delete anytime).
/// 缓存目录：~/.cache/markerss（可随时删除）。
pub fn cache_home() -> PathBuf {
    dirs::cache_dir().unwrap_or_else(|| home().join(".cache"))
}

/// Data dir: `~/.local/share/markerss` (persistent app data).
/// 数据目录：~/.local/share/markerss（持久化应用数据）。
pub fn data_home() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| home().join(".local/share"))
}
