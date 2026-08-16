package opml

import (
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	"markerss/internal/feedlist"
)

func TestRoundTrip(t *testing.T) {
	feeds := []feedlist.Feed{
		{URL: "https://a.test/rss", Title: "Alpha", CustomTitle: true, Category: "tech"},
		{URL: "https://b.test/feed", Title: "Beta", Category: "tech"},
		{URL: "https://c.test/x"},
	}
	path := filepath.Join(t.TempDir(), "subs.opml")
	if err := Export(path, feeds); err != nil {
		t.Fatal(err)
	}
	data, _ := os.ReadFile(path)
	s := string(data)
	for _, want := range []string{"opml", "xmlUrl", "~Alpha", "tech"} {
		if !strings.Contains(s, want) {
			t.Errorf("missing %q in exported OPML", want)
		}
	}
	back, err := Import(path)
	if err != nil {
		t.Fatal(err)
	}
	if len(back) != 3 {
		t.Fatalf("want 3 feeds, got %d: %+v", len(back), back)
	}
	want := []feedlist.Feed{
		{URL: "https://a.test/rss", Title: "Alpha", CustomTitle: true, Category: "tech"},
		{URL: "https://b.test/feed", Title: "Beta", Category: "tech"},
		{URL: "https://c.test/x"},
	}
	if !reflect.DeepEqual(back, want) {
		t.Errorf("round-trip mismatch:\n got %+v\nwant %+v", back, want)
	}
}

func TestNestedCategory(t *testing.T) {
	feeds := []feedlist.Feed{
		{URL: "https://a.test/rss", Title: "A", Category: "tech/go"},
		{URL: "https://b.test/rss", Title: "B", Category: "tech"},
	}
	path := filepath.Join(t.TempDir(), "subs.opml")
	if err := Export(path, feeds); err != nil {
		t.Fatal(err)
	}
	data, _ := os.ReadFile(path)
	if !strings.Contains(string(data), "go") {
		t.Errorf("nested folder missing: %s", data)
	}
	back, err := Import(path)
	if err != nil {
		t.Fatal(err)
	}
	if len(back) != 2 || back[0].Category != "tech/go" || back[1].Category != "tech" {
		t.Errorf("nested import wrong: %+v", back)
	}
}

func TestImportMissingFile(t *testing.T) {
	if _, err := Import("/nonexistent/x.opml"); err == nil {
		t.Error("import of missing file should fail")
	}
}
