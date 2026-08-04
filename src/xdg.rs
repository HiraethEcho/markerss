//! XDG directory resolution for markerss, per XDG Base Directory spec.
//! `dirs` crate already applies the spec fallbacks when vars are unset:
//! `~/.config`, `~/.cache`, `~/.local/state`, `~/.local/share`.

use std::path::PathBuf;

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

pub fn config_home() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| home().join(".config"))
}

pub fn cache_home() -> PathBuf {
    dirs::cache_dir().unwrap_or_else(|| home().join(".cache"))
}

pub fn data_home() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| home().join(".local/share"))
}
