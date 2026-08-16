// Package xdg resolves XDG base directories per the XDG Base Directory
// Specification, with home fallbacks when env vars are unset.
package xdg

import (
	"os"
	"path/filepath"
)

// ConfigHome returns $XDG_CONFIG_HOME or ~/.config.
func ConfigHome() string {
	return dir("XDG_CONFIG_HOME", ".config")
}

// CacheHome returns $XDG_CACHE_HOME or ~/.cache.
func CacheHome() string {
	return dir("XDG_CACHE_HOME", ".cache")
}

// StateHome returns $XDG_STATE_HOME or ~/.local/state.
func StateHome() string {
	return dir("XDG_STATE_HOME", ".local/state")
}

// DataHome returns $XDG_DATA_HOME or ~/.local/share.
func DataHome() string {
	return dir("XDG_DATA_HOME", ".local/share")
}

func dir(env, fallback string) string {
	if v := os.Getenv(env); v != "" {
		return v
	}
	home, err := os.UserHomeDir()
	if err != nil {
		return ""
	}
	return filepath.Join(home, fallback)
}
