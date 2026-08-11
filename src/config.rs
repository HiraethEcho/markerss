//! App config: `$XDG_CONFIG_HOME/markerss/config.toml` — format by extension.
//!
//! - `config.toml` → TOML (default)
//! - `.json` → JSON, `.jsonc` → JSON with comments stripped
//! - `.yaml` / `.yml` → YAML
//!
//! Subscriptions live in the separate `urls` file (newsboat format).
//! Unknown keys ignored; defaults when absent; read at startup only.

use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

use crate::xdg;
use ratatui::style::{Color, Modifier, Style};

pub const DEFAULT_NAV_PRESET: [&str; 6] =
    ["Unread", "Read Later", "Favourite", "Categories", "Tags", "Saved"];

/// App colors: markdown element theme + pane accent/dim colors.
/// Loaded from the optional `theme` file (TOML, named colors).
#[derive(Debug, Clone)]
pub struct ThemeColors {
    pub md: the_other_tui_markdown::Theme,
    pub accent: Color,
    pub dim: Color,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            md: the_other_tui_markdown::Theme::default(),
            accent: Color::Yellow,
            dim: Color::DarkGray,
        }
    }
}

impl ThemeColors {
    pub fn load(path: Option<&PathBuf>) -> ThemeColors {
        let mut t = ThemeColors::default();
        let Some(p) = path else { return t };
        let Ok(text) = fs::read_to_string(p) else { return t };
        #[derive(Deserialize, Default)]
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
        }
        let raw: RawTheme = match toml::from_str(&text) {
            Ok(r) => r,
            Err(_) => return t,
        };
        let bold = |c: Color| Style::new().fg(c).add_modifier(Modifier::BOLD);
        if let Some(c) = raw.h1.as_deref().and_then(color_from_str) {
            t.md.h1 = bold(c);
        }
        if let Some(c) = raw.h2.as_deref().and_then(color_from_str) {
            t.md.h2 = Style::new().fg(c).add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
        }
        if let Some(c) = raw.h3.as_deref().and_then(color_from_str) {
            t.md.h3 = bold(c);
        }
        if let Some(c) = raw.code.as_deref().and_then(color_from_str) {
            t.md.inline_code = Style::new().fg(c);
            t.md.code_block = Style::new().fg(c);
        }
        if let Some(c) = raw.quote.as_deref().and_then(color_from_str) {
            t.md.block_quote = Style::new().fg(c).add_modifier(Modifier::ITALIC);
        }
        if let Some(c) = raw.link.as_deref().and_then(color_from_str) {
            t.md.link = Style::new().fg(c).add_modifier(Modifier::UNDERLINED);
        }
        if let Some(c) = raw.accent.as_deref().and_then(color_from_str) {
            t.accent = c;
        }
        if let Some(c) = raw.dim.as_deref().and_then(color_from_str) {
            t.dim = c;
        }
        t
    }
}

/// Named color → ratatui Color (16-color palette).
pub fn color_from_str(s: &str) -> Option<Color> {
    Some(match s.to_ascii_lowercase().as_str() {
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

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct RawConfig {
    cache_ttl_days: Option<u64>,
    export_dir: Option<String>,
    browser: Option<String>,
    refresh: Option<RefreshCfg>,
    fetch_timeout: Option<u64>,
    max_items_per_feed: Option<usize>,
    theme: Option<String>,
    pane_ratio: Option<Vec<f64>>,
    nav_presets: Option<Vec<Vec<String>>>,
    default_view: Option<String>,
    images: Option<bool>,
    proxy: Option<String>,
    keybindings: Option<std::collections::HashMap<String, String>>,
    sort: Option<Vec<String>>,
    foldlevel: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum RefreshCfg {
    Bool(bool),
    Table { interval_minutes: Option<u64> },
}



#[derive(Debug, Clone)]
pub struct Config {
    pub config_dir: PathBuf,
    pub urls_path: PathBuf,
    pub db_path: PathBuf,
    pub cache_ttl_days: u64,
    pub export_dir: PathBuf,
    pub browser: Option<String>,
    pub refresh_on_startup: bool,
    pub refresh_interval_minutes: Option<u64>,
    pub fetch_timeout: u64,
    pub max_items_per_feed: Option<usize>,
    pub theme_path: Option<PathBuf>,
    pub pane_ratio: [f64; 3],
    pub nav_presets: Vec<Vec<String>>,
    pub default_view: Option<String>,
    pub images: bool,
    pub proxy: Option<String>,
    pub sort: Vec<String>,
    pub foldlevel: Option<usize>,
    pub keybindings: std::collections::HashMap<String, String>,
}

impl Config {
    pub fn load() -> Config {
        let config_dir = xdg::config_home().join("markerss");
        let cache_dir = xdg::cache_home().join("markerss");
        let data_dir = xdg::data_home().join("markerss");

        let mut cfg = Config {
            urls_path: config_dir.join("urls"),
            db_path: cache_dir.join("markerss.db"),
            cache_ttl_days: 14,
            export_dir: data_dir,
            browser: None,
            refresh_on_startup: true,
            refresh_interval_minutes: None,
            fetch_timeout: 30,
            max_items_per_feed: None,
            theme_path: None,
            pane_ratio: [0.15, 0.15, 0.7],
            nav_presets: vec![DEFAULT_NAV_PRESET.iter().map(|s| s.to_string()).collect()],
            default_view: None,
            images: false,
            proxy: None,
            sort: Vec::new(),
            foldlevel: None,
            keybindings: std::collections::HashMap::new(),
            config_dir: config_dir.clone(),
        };

        if let Some(raw) = load_raw(&config_dir.join("config.toml")) {
            cfg.apply(raw);
        }
        cfg
    }

    fn apply(&mut self, raw: RawConfig) {
        if let Some(v) = raw.cache_ttl_days {
            self.cache_ttl_days = v;
        }
        if let Some(v) = raw.export_dir {
            self.export_dir = PathBuf::from(v);
        }
        self.browser = raw.browser;
        if let Some(v) = raw.refresh {
            match v {
                RefreshCfg::Bool(b) => self.refresh_on_startup = b,
                RefreshCfg::Table { interval_minutes } => {
                    self.refresh_interval_minutes = interval_minutes
                }
            }
        }
        if let Some(v) = raw.fetch_timeout {
            self.fetch_timeout = v;
        }
        self.max_items_per_feed = raw.max_items_per_feed;
        if let Some(v) = raw.theme {
            self.theme_path = Some(PathBuf::from(v));
        }
        if let Some(v) = raw.sort {
            self.sort = v.into_iter().take(3).collect();
        }
        self.foldlevel = raw.foldlevel;
        if let Some(k) = raw.keybindings {
            self.keybindings = k;
        }
        if let Some(v) = raw.pane_ratio {
            if v.len() == 3 {
                self.pane_ratio = [v[0], v[1], v[2]];
            }
        }
        if let Some(v) = raw.nav_presets {
            if !v.is_empty() {
                self.nav_presets = v;
            }
        }
        self.default_view = raw.default_view;
        if let Some(v) = raw.images {
            self.images = v;
        }
        self.proxy = raw.proxy;
    }
}

/// Parse the config file by extension; `None` when unreadable/absent.
fn load_raw(path: &std::path::Path) -> Option<RawConfig> {
    let text = fs::read_to_string(path).ok()?;
    let name = path.file_name()?.to_string_lossy().to_string();
    let parsed: Option<RawConfig> = match name.rsplit_once('.') {
        Some((_, ext)) => match ext.to_ascii_lowercase().as_str() {
            "json" => serde_json::from_str(&text).ok(),
            "jsonc" => {
                let cleaned = strip_jsonc(&text);
                let re = regex::Regex::new(r",\s*([}\]])$").unwrap();
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
fn strip_jsonc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_str = false;
    let mut in_line = false;
    let mut in_block = false;
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
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
                if let Some(n) = next {
                    out.push(n);
                }
                i += 2;
                continue;
            }
            if c == '"' {
                in_str = false;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn load_from(text: &str, name: &str) -> Option<RawConfig> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
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
}

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
    fn sort_capped_at_three() {
        let r = load_from("sort = [\"a\", \"b\", \"c\", \"d\"]\n", "config.toml").unwrap();
        let mut cfg = Config::load();
        if let Some(v) = r.sort {
            cfg.sort = v.into_iter().take(3).collect();
        }
        assert_eq!(cfg.sort.len(), 3);
    }

    #[test]
    fn theme_colors_load() {
        let dir = std::env::temp_dir().join(format!("markerss-theme-{}", std::process::id()));
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join("theme.toml");
        std::fs::write(&path, "accent = \"green\"\nh1 = \"red\"\ncode = \"cyan\"\n").ok();
        let t = ThemeColors::load(Some(&path));
        assert_eq!(t.accent, Color::Green);
        assert_eq!(t.md.h1.fg, Some(Color::Red));
        assert_eq!(t.md.inline_code.fg, Some(Color::Cyan));
        std::fs::remove_dir_all(&dir).ok();
    }
}
