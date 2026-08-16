package markdown

import (
	"strings"
	"testing"
)

func TestHTMLToMD(t *testing.T) {
	in := `<p>Hello <b>world</b> with <a href="https://x.test">link</a> and <sub>sub</sub>/<sup>sup</sup></p>`
	md := HTMLToMD(in)
	if !strings.Contains(md, "**world**") {
		t.Errorf("expected bold, got %q", md)
	}
	if !strings.Contains(md, "[link](https://x.test)") {
		t.Errorf("expected link markdown, got %q", md)
	}
	if !strings.Contains(md, "~sub~") || !strings.Contains(md, "^sup^") {
		t.Errorf("expected sub/sup markers, got %q", md)
	}
}

func TestHTMLToMDEmpty(t *testing.T) {
	if HTMLToMD("  ") != "" {
		t.Error("whitespace should yield empty")
	}
	if HTMLToMD("") != "" {
		t.Error("empty should yield empty")
	}
}

func TestRenderInlineLinkHidesURL(t *testing.T) {
	lines := Render("[click me](https://example.com)", 100)
	got := strings.Join(lines, "\n")
	if strings.Contains(got, "example.com") {
		t.Errorf("URL must not render: %q", got)
	}
	if !strings.Contains(got, "click me") {
		t.Errorf("alt text must render: %q", got)
	}
}

func TestRenderImagePlaceholder(t *testing.T) {
	got := strings.Join(Render("![photo](https://x.test/a.png)", 80), "\n")
	if !strings.Contains(got, "[img]") {
		t.Errorf("expected [img] placeholder, got %q", got)
	}
}

func TestRenderHeading(t *testing.T) {
	got := strings.Join(Render("# Big News", 80), "\n")
	if !strings.Contains(got, "Big News") {
		t.Errorf("heading text missing: %q", got)
	}
}

func TestRenderWraps(t *testing.T) {
	long := strings.Repeat("word ", 60)
	lines := Render(long, 40)
	for _, l := range lines {
		if l == "" {
			continue
		}
		if width(l) > 40 {
			t.Errorf("line %q wider than 40", l)
		}
	}
}

func TestSlug(t *testing.T) {
	cases := map[string]string{
		"Hello, World!": "hello-world",
		"   ":           "untitled",
		"Café & Nächt":  "café-nächt",
		"---dash---":    "dash",
	}
	for in, want := range cases {
		if got := Slug(in); got != want {
			t.Errorf("Slug(%q) = %q, want %q", in, got, want)
		}
	}
}

// width strips ANSI escapes for width assertions.
func width(s string) int {
	return len([]rune(s))
}
