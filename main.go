// markerss — TUI RSS reader (Go branch).
//
// Three-pane reader: preset-driven nav / item list / article. Settings in
// $XDG_CONFIG_HOME/markerss/config.toml; subscriptions in a newsboat-format
// urls file; items persist in SQLite; export writes markdown.
package main

import (
	"fmt"
	"os"
	"path/filepath"

	tea "github.com/charmbracelet/bubbletea"

	"markerss/internal/config"
	"markerss/internal/feedlist"
	"markerss/internal/store"
	"markerss/internal/ui"
	"markerss/internal/xdg"
)

func main() {
	cfgDir := filepath.Join(xdg.ConfigHome(), "markerss")
	cacheDir := filepath.Join(xdg.CacheHome(), "markerss")
	for _, d := range []string{cfgDir, cacheDir} {
		if err := os.MkdirAll(d, 0o755); err != nil {
			fmt.Fprintln(os.Stderr, "markerss:", err)
			os.Exit(1)
		}
	}

	// app config (defaults + XDG fallbacks; restart to apply)
	cfg, err := config.Load(filepath.Join(cfgDir, "config.toml"))
	if err != nil {
		fmt.Fprintln(os.Stderr, "markerss: config:", err)
		os.Exit(1)
	}
	theme, err := config.LoadTheme(cfg.Theme)
	if err != nil {
		fmt.Fprintln(os.Stderr, "markerss: theme:", err)
		os.Exit(1)
	}

	db, err := store.Open(filepath.Join(cacheDir, "markerss.db"))
	if err != nil {
		fmt.Fprintln(os.Stderr, "markerss:", err)
		os.Exit(1)
	}
	defer db.Close()

	// startup TTL purge of fetched content
	if err := db.PurgeExpired(cfg.CacheTTLDays); err != nil {
		fmt.Fprintln(os.Stderr, "markerss: purge:", err)
		os.Exit(1)
	}

	urlsPath := filepath.Join(cfgDir, "urls")
	feeds := []feedlist.Feed{}
	if fl, ok := feedlist.LoadFile(urlsPath); ok {
		feeds = fl.Feeds
	}

	m := ui.New(ui.Config{
		Store:    db,
		Feeds:    feeds,
		URLsPath: urlsPath,
		Cfg:      cfg,
		Theme:    theme,
	})
	p := tea.NewProgram(m, tea.WithAltScreen())
	if _, err := p.Run(); err != nil {
		fmt.Fprintln(os.Stderr, "markerss:", err)
		os.Exit(1)
	}
}
