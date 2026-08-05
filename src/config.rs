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

pub const DEFAULT_NAV_PRESET: [&str; 6] =
    ["Unread", "Read Later", "Favourite", "Categories", "Tags", "Saved"];

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
    keybindings: Option<String>,
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
