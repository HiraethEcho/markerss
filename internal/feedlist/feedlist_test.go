package feedlist

import (
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"
)

func TestParse(t *testing.T) {
	tests := []struct {
		name  string
		input string
		want  []Feed
	}{
		{
			name:  "category and tags",
			input: `https://a.test/rss "My Feed" tech #go #rss`,
			want: []Feed{{URL: "https://a.test/rss", Title: "My Feed",
				Category: "tech", Tags: []string{"go", "rss"}}},
		},
		{
			name:  "custom title tilde",
			input: `https://a.test/rss "~My Display" blog`,
			want:  []Feed{{URL: "https://a.test/rss", Title: "My Display", CustomTitle: true, Category: "blog"}},
		},
		{
			name:  "no category only tags",
			input: `https://a.test/rss #mine`,
			want:  []Feed{{URL: "https://a.test/rss", Tags: []string{"mine"}}},
		},
		{
			name:  "favourite marker",
			input: `https://a.test/rss "A" !favourite`,
			want:  []Feed{{URL: "https://a.test/rss", Title: "A", Favourite: true}},
		},
		{
			name:  "nested category",
			input: `https://a.test/rss tech/go`,
			want:  []Feed{{URL: "https://a.test/rss", Category: "tech/go"}},
		},
		{
			name: "comment and blank lines",
			input: "# comment\n" +
				"\n" +
				`https://a.test/rss "F" x` + "\n" +
				"# trailing\n",
			want: []Feed{{URL: "https://a.test/rss", Title: "F", Category: "x"}},
		},
		{
			name:  "quoted title with spaces",
			input: `https://a.test/rss "Go Blog" programming #go`,
			want:  []Feed{{URL: "https://a.test/rss", Title: "Go Blog", Category: "programming", Tags: []string{"go"}}},
		},
		{
			name:  "multiple feeds",
			input: `https://a.com/f "A" x` + "\n" + `https://b.com/f`,
			want: []Feed{
				{URL: "https://a.com/f", Title: "A", Category: "x"},
				{URL: "https://b.com/f"},
			},
		},
		{
			name:  "real-world line",
			input: `https://memex.keinmal.top/index.xml "memex" blog #mine !favourite`,
			want:  []Feed{{URL: "https://memex.keinmal.top/index.xml", Title: "memex", Category: "blog", Tags: []string{"mine"}, Favourite: true}},
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := Parse(tt.input)
			if !reflect.DeepEqual(got, tt.want) {
				t.Errorf("Parse() = %+v, want %+v", got, tt.want)
			}
		})
	}
}

func TestSaveRoundTrip(t *testing.T) {
	feeds := []Feed{
		{URL: "https://a.test/rss", Title: "Alpha", CustomTitle: true, Category: "tech", Tags: []string{"go"}, Favourite: true},
		{URL: "https://b.test/feed", Title: "Beta", Category: "tech/go"},
		{URL: "https://c.test/x"},
	}
	f := &File{Feeds: feeds}
	path := filepath.Join(t.TempDir(), "urls")
	if err := f.Save(path); err != nil {
		t.Fatal(err)
	}
	data, _ := os.ReadFile(path)
	got := string(data)
	for _, want := range []string{
		`https://a.test/rss "~Alpha" tech #go !favourite`,
		`https://b.test/feed "Beta" tech/go`,
		`https://c.test/x`,
	} {
		if !strings.Contains(got, want) {
			t.Errorf("missing %q in %q", want, got)
		}
	}
	back := Parse(got)
	if !reflect.DeepEqual(back, feeds) {
		t.Errorf("round-trip:\n got %+v\nwant %+v", back, feeds)
	}
}

func TestCRUD(t *testing.T) {
	f := &File{Feeds: []Feed{
		{URL: "a", Title: "A", Category: "tech", Tags: []string{"go"}},
		{URL: "b", Category: "tech"},
		{URL: "c", Category: "tech/go"},
		{URL: "d"},
	}}
	if n := f.RenameCategory("tech", "dev"); n != 2 {
		t.Errorf("rename cat touches %d, want 2", n)
	}
	if n := f.RenameTag("go", "golang"); n != 1 {
		t.Errorf("rename tag touches %d, want 1", n)
	}
	if f.Feeds[1].Category != "dev" || f.Feeds[0].Tags[0] != "golang" {
		t.Errorf("rename broken: %+v", f.Feeds)
	}
	if !f.RemoveURL("b") || f.RemoveURL("nope") {
		t.Error("RemoveURL behavior wrong")
	}
	tops := f.TopCategories()
	if !reflect.DeepEqual(tops, []string{"dev", "tech"}) {
		t.Errorf("TopCategories = %v", tops)
	}
	tags := f.AllTags()
	if !reflect.DeepEqual(tags, []string{"golang"}) {
		t.Errorf("AllTags = %v", tags)
	}
}

func TestTopCategory(t *testing.T) {
	if TopCategory("tech/go/extra") != "tech" || TopCategory("flat") != "flat" {
		t.Error("TopCategory split wrong")
	}
}

func TestSaveMissingDir(t *testing.T) {
	f := &File{Feeds: []Feed{{URL: "a"}}}
	if err := f.Save("/nonexistent/dir/urls"); err == nil {
		t.Error("save to missing dir should fail")
	}
}
