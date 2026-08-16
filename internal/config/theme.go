package config

import (
	"os"
	"path/filepath"
	"strings"

	"github.com/BurntSushi/toml"
)

// Theme defines the color palette, loaded from a standalone TOML file.
type Theme struct {
	Accent    string `toml:"accent"`
	Dim       string `toml:"dim"`
	Header    string `toml:"header"`
	Title     string `toml:"title"`
	Warning   string `toml:"warning"`
	Selection string `toml:"selection"`
}

// DefaultTheme returns the built-in palette (fallback).
func DefaultTheme() *Theme {
	return &Theme{
		Accent:    "39",
		Dim:       "240",
		Header:    "39",
		Title:     "39",
		Warning:   "203",
		Selection: "39",
	}
}

// LoadTheme reads a theme file; missing/empty path → default palette.
func LoadTheme(path string) (*Theme, error) {
	if path == "" {
		return DefaultTheme(), nil
	}
	if strings.HasPrefix(path, "~/") {
		home, _ := os.UserHomeDir()
		path = filepath.Join(home, strings.TrimPrefix(path, "~/"))
	}
	data, err := os.ReadFile(path)
	if err != nil {
		return DefaultTheme(), nil // missing theme → defaults
	}
	t := DefaultTheme()
	if err := toml.Unmarshal(data, t); err != nil {
		return nil, err
	}
	return t, nil
}
