package config

import (
	"os"
	"path/filepath"
	"testing"
)

func TestLoadTOMLBoolRefresh(t *testing.T) {
	dir := t.TempDir()
	p := filepath.Join(dir, "config.toml")
	os.WriteFile(p, []byte(`
cache_ttl_days = 14
refresh = true
pane_ratio = [0.15, 0.2, 0.65]
nav_presets = [["Unread", "Feeds"], ["Unread", "Later"]]
`), 0o644)
	cfg, err := Load(p)
	if err != nil {
		t.Fatal(err)
	}
	if cfg.CacheTTLDays != 14 || !cfg.Refresh.AutoOnStartup {
		t.Errorf("got ttl=%d refresh=%+v", cfg.CacheTTLDays, cfg.Refresh)
	}
	if len(cfg.NavPresets) != 2 || cfg.NavPresets[0][1] != "Feeds" {
		t.Errorf("presets = %v", cfg.NavPresets)
	}
	if cfg.PaneRatio != [3]float64{0.15, 0.2, 0.65} {
		t.Errorf("ratio = %v", cfg.PaneRatio)
	}
	if cfg.FoldLevelValue() != -1 {
		t.Errorf("default foldlevel = %d", cfg.FoldLevelValue())
	}
}

func TestLoadTOMLTableRefresh(t *testing.T) {
	dir := t.TempDir()
	p := filepath.Join(dir, "config.toml")
	os.WriteFile(p, []byte("refresh = { auto_on_startup = false, interval_minutes = 30 }\n"), 0o644)
	cfg, err := Load(p)
	if err != nil {
		t.Fatal(err)
	}
	if cfg.Refresh.AutoOnStartup || cfg.Refresh.IntervalMin != 30 {
		t.Errorf("refresh = %+v", cfg.Refresh)
	}
}

func TestLoadJSONAndYAML(t *testing.T) {
	dir := t.TempDir()
	j := filepath.Join(dir, "config.json")
	os.WriteFile(j, []byte(`{"cache_ttl_days": 7, "refresh": true}`), 0o644)
	cfg, err := Load(j)
	if err != nil || cfg.CacheTTLDays != 7 {
		t.Errorf("json: %v %+v", err, cfg)
	}
	y := filepath.Join(dir, "config.yaml")
	os.WriteFile(y, []byte("cache_ttl_days: 3\nrefresh: {interval_minutes: 15}\n"), 0o644)
	cfg, err = Load(y)
	if err != nil || cfg.CacheTTLDays != 3 || cfg.Refresh.IntervalMin != 15 {
		t.Errorf("yaml: %v %+v", err, cfg)
	}
}

func TestLoadJSONCComments(t *testing.T) {
	dir := t.TempDir()
	p := filepath.Join(dir, "config.jsonc")
	os.WriteFile(p, []byte(`{
  // comment
  "cache_ttl_days": 9, /* block */
  "browser": "firefox"
}`), 0o644)
	cfg, err := Load(p)
	if err != nil {
		t.Fatal(err)
	}
	if cfg.CacheTTLDays != 9 || cfg.Browser != "firefox" {
		t.Errorf("jsonc: %+v", cfg)
	}
}

func TestMissingFileDefaults(t *testing.T) {
	cfg, err := Load(filepath.Join(t.TempDir(), "nope.toml"))
	if err != nil {
		t.Fatal(err)
	}
	if cfg.CacheTTLDays != 30 || cfg.Browser != "xdg-open" {
		t.Errorf("defaults wrong: %+v", cfg)
	}
}

func TestExportDirOverride(t *testing.T) {
	dir := t.TempDir()
	p := filepath.Join(dir, "config.toml")
	os.WriteFile(p, []byte(`export_dir = "~/out/markerss"`), 0o644)
	cfg, _ := Load(p)
	home, _ := os.UserHomeDir()
	if cfg.ExportDirAbs != filepath.Join(home, "out", "markerss") {
		t.Errorf("export dir = %q", cfg.ExportDirAbs)
	}
}

func TestFoldLevelExplicit(t *testing.T) {
	dir := t.TempDir()
	p := filepath.Join(dir, "config.toml")
	os.WriteFile(p, []byte("foldlevel = 1\n"), 0o644)
	cfg, _ := Load(p)
	if cfg.FoldLevelValue() != 1 {
		t.Errorf("foldlevel = %d", cfg.FoldLevelValue())
	}
}

func TestThemeDefaultsAndLoad(t *testing.T) {
	if th := DefaultTheme(); th.Accent != "39" {
		t.Errorf("default accent = %q", th.Accent)
	}
	dir := t.TempDir()
	p := filepath.Join(dir, "dark.toml")
	os.WriteFile(p, []byte("accent = \"220\"\n"), 0o644)
	th, err := LoadTheme(p)
	if err != nil || th.Accent != "220" || th.Dim != "240" {
		t.Errorf("theme: %v %+v", err, th)
	}
	th, err = LoadTheme("")
	if err != nil || th.Accent != "39" {
		t.Errorf("empty theme should default: %v", err)
	}
}
