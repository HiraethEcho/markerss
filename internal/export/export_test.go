package export

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestWrite(t *testing.T) {
	base := t.TempDir()
	path, err := Write("**bold** body", Meta{
		Title:    "Hello World!",
		Link:     "https://x.test/1",
		Date:     time.Date(2026, 8, 1, 12, 0, 0, 0, time.UTC),
		Feed:     "My Feed",
		Category: "tech",
	}, base)
	if err != nil {
		t.Fatal(err)
	}
	if want := filepath.Join(base, "tech", "hello-world.md"); path != want {
		t.Errorf("path = %s, want %s", path, want)
	}
	data, _ := os.ReadFile(path)
	s := string(data)
	for _, want := range []string{
		"---", `title: "Hello World!"`, `link: "https://x.test/1"`,
		"date: 2026-08-01T12:00:00Z", `feed: "My Feed"`, "**bold** body",
	} {
		if !strings.Contains(s, want) {
			t.Errorf("missing %q in export", want)
		}
	}
}

func TestWriteUncategorizedAndNoDate(t *testing.T) {
	base := t.TempDir()
	path, err := Write("body", Meta{Title: "Untagged Item", Link: "https://x.test/2"}, base)
	if err != nil {
		t.Fatal(err)
	}
	if want := filepath.Join(base, "untagged-item.md"); path != want {
		t.Errorf("path = %s, want %s", path, want)
	}
	data, _ := os.ReadFile(path)
	if strings.Contains(string(data), "date:") {
		t.Error("zero date must be omitted")
	}
}

func TestWriteCategoryTraversal(t *testing.T) {
	base := t.TempDir()
	path, err := Write("body", Meta{Title: "T", Category: "../evil"}, base)
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(path, "..") {
		t.Errorf("category must not traverse: %s", path)
	}
}
