//! App config: `$XDG_CONFIG_HOME/markerss/config` — key = value lines.
//!
//! Subscriptions live in the separate `urls` file (newsboat format).

use std::fs;
use std::path::PathBuf;

use crate::xdg;

#[derive(Debug, Clone)]
pub struct Config {
    pub config_dir: PathBuf,
    pub urls_path: PathBuf,
    pub db_path: PathBuf,
    /// Content cleanup TTL in days; older fetched content purged at startup.
    pub cache_ttl_days: u64,
    /// Export base dir; default `$XDG_DATA_HOME/markerss`.
    pub export_dir: PathBuf,
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
            config_dir,
        };

        let raw = fs::read_to_string(cfg.config_dir.join("config")).unwrap_or_default();
        for line in raw.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                match (k.trim(), v.trim()) {
                    ("cache_ttl_days", v) => {
                        cfg.cache_ttl_days = v.parse().unwrap_or(cfg.cache_ttl_days)
                    }
                    ("export_dir", v) => {
                        cfg.export_dir = PathBuf::from(v.trim_matches('"'));
                    }
                    _ => {}
                }
            }
        }
        cfg
    }
}
