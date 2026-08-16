// Package export writes items as markdown files with YAML frontmatter.
// Markdown is generated here, at export time — never stored.
package export

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	"markerss/internal/markdown"
)

// Meta is the frontmatter payload for one export.
type Meta struct {
	Title    string
	Link     string
	Date     time.Time
	Feed     string
	Category string
}

// Write writes md to baseDir[/category]/<slug>.md and returns the path.
func Write(md string, meta Meta, baseDir string) (string, error) {
	dir := baseDir
	if meta.Category != "" {
		dir = filepath.Join(dir, safePath(meta.Category))
	}
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return "", err
	}
	path := filepath.Join(dir, markdown.Slug(meta.Title)+".md")
	if err := writeFile(path, md, meta); err != nil {
		return "", err
	}
	return path, nil
}

// WriteTo writes md with frontmatter to an exact path.
func WriteTo(path string, md string, meta Meta) (string, error) {
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return "", err
	}
	if err := writeFile(path, md, meta); err != nil {
		return "", err
	}
	return path, nil
}

func writeFile(path, md string, meta Meta) error {
	var fm strings.Builder
	fm.WriteString("---\n")
	fmt.Fprintf(&fm, "title: %q\n", meta.Title)
	fmt.Fprintf(&fm, "link: %q\n", meta.Link)
	if !meta.Date.IsZero() {
		fmt.Fprintf(&fm, "date: %s\n", meta.Date.Format(time.RFC3339))
	}
	fmt.Fprintf(&fm, "feed: %q\n", meta.Feed)
	fm.WriteString("---\n\n")

	body := strings.TrimSpace(md) + "\n"
	return os.WriteFile(path, []byte(fm.String()+body), 0o644)
}

// safePath neutralizes path separators and traversal in category names.
func safePath(s string) string {
	s = strings.NewReplacer("/", "-", "\\", "-").Replace(s)
	s = strings.TrimSpace(strings.Trim(s, "."))
	if s == "" {
		return "misc"
	}
	return s
}
