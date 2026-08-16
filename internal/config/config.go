// Package config loads app settings from $XDG_CONFIG_HOME/markerss/config
// (TOML default; JSON / JSONC / YAML by extension), separate from the urls
// subscriptions file. Defaults + XDG fallbacks when keys are absent.
package config

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"

	"github.com/BurntSushi/toml"
	"gopkg.in/yaml.v3"
)

// Refresh controls auto refresh behavior. Accepts either a bare bool
// (auto-on-startup) or a table { auto_on_startup, interval_minutes }.
type Refresh struct {
	AutoOnStartup bool `toml:"auto_on_startup" json:"auto_on_startup" yaml:"auto_on_startup"`
	IntervalMin   int  `toml:"interval_minutes" json:"interval_minutes" yaml:"interval_minutes"`
}

// UnmarshalTOML handles `refresh = true` and `refresh = { interval_minutes = 30 }`.
func (r *Refresh) UnmarshalTOML(v any) error {
	switch t := v.(type) {
	case bool:
		r.AutoOnStartup = t
		return nil
	case map[string]any:
		if b, ok := t["auto_on_startup"].(bool); ok {
			r.AutoOnStartup = b
		}
		if n, ok := t["interval_minutes"].(int64); ok {
			r.IntervalMin = int(n)
		}
		return nil
	}
	return nil
}

// UnmarshalJSON handles the same shapes in JSON configs.
func (r *Refresh) UnmarshalJSON(b []byte) error {
	trim := strings.TrimSpace(string(b))
	if trim == "true" || trim == "false" {
		r.AutoOnStartup = trim == "true"
		return nil
	}
	var m map[string]any
	if err := json.Unmarshal(b, &m); err != nil {
		return err
	}
	if b, ok := m["auto_on_startup"].(bool); ok {
		r.AutoOnStartup = b
	}
	if n, ok := m["interval_minutes"].(float64); ok {
		r.IntervalMin = int(n)
	}
	return nil
}

// UnmarshalYAML handles the same shapes in YAML configs.
func (r *Refresh) UnmarshalYAML(node *yaml.Node) error {
	if node.Kind == yaml.ScalarNode {
		var b bool
		if err := node.Decode(&b); err == nil {
			r.AutoOnStartup = b
		}
		return nil
	}
	var m map[string]any
	if err := node.Decode(&m); err != nil {
		return err
	}
	if b, ok := m["auto_on_startup"].(bool); ok {
		r.AutoOnStartup = b
	}
	if n, ok := m["interval_minutes"].(int); ok {
		r.IntervalMin = n
	}
	return nil
}

// Config is the app settings struct.
type Config struct {
	CacheTTLDays    int               `toml:"cache_ttl_days" json:"cache_ttl_days" yaml:"cache_ttl_days"`
	ExportDir       string            `toml:"export_dir" json:"export_dir" yaml:"export_dir"`
	PaneRatio       [3]float64        `toml:"pane_ratio" json:"pane_ratio" yaml:"pane_ratio"`
	Theme           string            `toml:"theme" json:"theme" yaml:"theme"`
	Browser         string            `toml:"browser" json:"browser" yaml:"browser"`
	Refresh         Refresh           `toml:"refresh" json:"refresh" yaml:"refresh"`
	NavPresets      [][]string        `toml:"nav_presets" json:"nav_presets" yaml:"nav_presets"`
	Images          bool              `toml:"images" json:"images" yaml:"images"`
	FoldLevel       *int              `toml:"foldlevel" json:"foldlevel" yaml:"foldlevel"`
	Sort            []string          `toml:"sort" json:"sort" yaml:"sort"`
	FetchTimeoutSec int               `toml:"fetch_timeout" json:"fetch_timeout" yaml:"fetch_timeout"`
	MaxItemsPerFeed int               `toml:"max_items_per_feed" json:"max_items_per_feed" yaml:"max_items_per_feed"`
	Proxy           string            `toml:"proxy" json:"proxy" yaml:"proxy"`
	Keybindings     map[string]string `toml:"keybindings" json:"keybindings" yaml:"keybindings"`
	DefaultView     string            `toml:"default_view" json:"default_view" yaml:"default_view"`

	// derived
	ExportDirAbs string `toml:"-" json:"-" yaml:"-"`
}

// Default returns defaults with XDG-derived paths.
func Default(cfgDir string) *Config {
	c := &Config{
		CacheTTLDays:    30,
		PaneRatio:       [3]float64{0.15, 0.15, 0.70},
		Browser:         "xdg-open",
		Refresh:         Refresh{AutoOnStartup: true},
		FetchTimeoutSec: 30,
	}
	fl := -1
	c.FoldLevel = &fl
	c.ExportDirAbs = filepath.Join(os.Getenv("XDG_DATA_HOME"), "markerss")
	if c.ExportDirAbs == "" || !filepath.IsAbs(c.ExportDirAbs) {
		home, _ := os.UserHomeDir()
		c.ExportDirAbs = filepath.Join(home, ".local", "share", "markerss")
	}
	_ = cfgDir
	return c
}

// Load reads the config file at path. Missing file → defaults.
// Format detected by extension: .json/.jsonc → JSON (comments stripped),
// .yaml/.yml → YAML, anything else → TOML.
func Load(path string) (*Config, error) {
	cfg := Default(filepath.Dir(path))
	data, err := os.ReadFile(path)
	if err != nil {
		if os.IsNotExist(err) {
			return cfg, nil
		}
		return nil, err
	}
	switch strings.ToLower(filepath.Ext(path)) {
	case ".json", ".jsonc":
		err = json.Unmarshal(stripJSONC(data), cfg)
	case ".yaml", ".yml":
		err = yaml.Unmarshal(data, cfg)
	default: // TOML
		err = toml.Unmarshal(data, cfg)
	}
	if err != nil {
		return nil, err
	}
	cfg.applyDefaults()
	return cfg, nil
}

func (c *Config) applyDefaults() {
	d := Default("")
	if c.CacheTTLDays == 0 {
		c.CacheTTLDays = d.CacheTTLDays
	}
	if c.PaneRatio == [3]float64{} {
		c.PaneRatio = d.PaneRatio
	}
	if c.Browser == "" {
		c.Browser = d.Browser
	}
	if c.FetchTimeoutSec == 0 {
		c.FetchTimeoutSec = d.FetchTimeoutSec
	}
	if c.FoldLevel == nil {
		f := -1 // all open
		c.FoldLevel = &f
	}
	if c.ExportDir != "" {
		c.ExportDirAbs = expandHome(c.ExportDir)
	}
}

// FoldLevelValue returns the fold level (-1 = all open).
func (c *Config) FoldLevelValue() int {
	if c.FoldLevel == nil {
		return -1
	}
	return *c.FoldLevel
}

func expandHome(p string) string {
	if p == "~" || strings.HasPrefix(p, "~/") {
		home, _ := os.UserHomeDir()
		return filepath.Join(home, strings.TrimPrefix(p, "~/"))
	}
	return p
}

// stripJSONC removes // and /* */ comments (crude but sufficient).
func stripJSONC(b []byte) []byte {
	out := make([]byte, 0, len(b))
	inStr := false
	esc := false
	for i := 0; i < len(b); i++ {
		c := b[i]
		if inStr {
			out = append(out, c)
			if esc {
				esc = false
			} else if c == '\\' {
				esc = true
			} else if c == '"' {
				inStr = false
			}
			continue
		}
		if c == '"' {
			inStr = true
			out = append(out, c)
			continue
		}
		if c == '/' && i+1 < len(b) && b[i+1] == '/' {
			for i < len(b) && b[i] != '\n' {
				i++
			}
			if i < len(b) {
				out = append(out, '\n')
			}
			continue
		}
		if c == '/' && i+1 < len(b) && b[i+1] == '*' {
			i += 2
			for i+1 < len(b) && !(b[i] == '*' && b[i+1] == '/') {
				i++
			}
			i++
			continue
		}
		out = append(out, c)
	}
	return out
}
