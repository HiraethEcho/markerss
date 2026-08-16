// Package feedlist parses and writes newsboat-format subscription
// files (urls).
//
// Line format: URL "Title" category #tag1 #tag2 [!favourite]
//   - Title is optional and must be quoted (first token after URL). A "~"
//     prefix marks it as a custom display name.
//   - The first bare token is the category (single; "/" nests, e.g. tech/go).
//   - "#tag" tokens are feed tags (multi, optional).
//   - "!favourite" marks the feed as a favourite (feed-level flag).
package feedlist

import (
	"bufio"
	"os"
	"strings"
)

// Feed is one subscription line.
type Feed struct {
	URL         string
	Title       string   // display title; empty when no title given
	CustomTitle bool     // true when title had "~" prefix
	Category    string   // tree placement (single; may nest via "/")
	Tags        []string // #-prefixed tags
	Favourite   bool     // !favourite marker
}

// File is a parsed urls file.
type File struct {
	Feeds []Feed
}

// Parse parses urls-format content.
func Parse(src string) []Feed {
	var feeds []Feed
	sc := bufio.NewScanner(strings.NewReader(src))
	for sc.Scan() {
		line := strings.TrimSpace(sc.Text())
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		toks := split(line)
		if len(toks) == 0 {
			continue
		}
		f := Feed{URL: toks[0]}
		rest := toks[1:]
		// quoted title must be the first token
		if len(rest) > 0 && strings.HasPrefix(rest[0], `"`) {
			f.Title = strings.Trim(rest[0], `"`)
			if strings.HasPrefix(f.Title, "~") {
				f.CustomTitle = true
				f.Title = strings.TrimPrefix(f.Title, "~")
			}
			rest = rest[1:]
		}
		for _, tok := range rest {
			// mid-line quoted token: strip quotes, treat as bare word
			if strings.HasPrefix(tok, `"`) {
				tok = strings.Trim(tok, `"`)
			}
			switch {
			case tok == "!favourite":
				f.Favourite = true
			case strings.HasPrefix(tok, "#"):
				tag := strings.TrimPrefix(tok, "#")
				if tag != "" {
					f.Tags = append(f.Tags, tag)
				}
			case strings.HasPrefix(tok, "!"):
				// unknown marker, ignore
			case f.Category == "":
				f.Category = tok
			default:
				// extra bare token after category → treat as tag (permissive)
				f.Tags = append(f.Tags, tok)
			}
		}
		feeds = append(feeds, f)
	}
	return feeds
}

// split tokenizes a line, keeping a quoted title as a single token.
func split(line string) []string {
	var toks []string
	var cur strings.Builder
	inQuote := false
	for _, r := range line {
		switch {
		case r == '"':
			inQuote = !inQuote
			cur.WriteRune(r)
		case r == ' ' || r == '\t':
			if inQuote {
				cur.WriteRune(r)
			} else if cur.Len() > 0 {
				toks = append(toks, cur.String())
				cur.Reset()
			}
		default:
			cur.WriteRune(r)
		}
	}
	if cur.Len() > 0 {
		toks = append(toks, cur.String())
	}
	return toks
}

// LoadFile reads and parses an urls file. Missing file → nil, false.
func LoadFile(path string) (*File, bool) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, false
	}
	return &File{Feeds: Parse(string(data))}, true
}

// Save writes feeds back to newsboat urls format.
func (f *File) Save(path string) error {
	var b strings.Builder
	for _, feed := range f.Feeds {
		b.WriteString(feed.URL)
		if feed.Title != "" {
			t := feed.Title
			if feed.CustomTitle {
				t = "~" + t
			}
			b.WriteString(` "` + strings.ReplaceAll(t, `"`, `\"`) + `"`)
		}
		if feed.Category != "" {
			b.WriteString(" " + feed.Category)
		}
		for _, tag := range feed.Tags {
			b.WriteString(" #" + tag)
		}
		if feed.Favourite {
			b.WriteString(" !favourite")
		}
		b.WriteString("\n")
	}
	return os.WriteFile(path, []byte(b.String()), 0o644)
}

// Add appends a feed.
func (f *File) Add(feed Feed) { f.Feeds = append(f.Feeds, feed) }

// RemoveURL deletes the first feed with the given URL; false when absent.
func (f *File) RemoveURL(url string) bool {
	for i, feed := range f.Feeds {
		if feed.URL == url {
			f.Feeds = append(f.Feeds[:i], f.Feeds[i+1:]...)
			return true
		}
	}
	return false
}

// RenameCategory renames the category where it equals old; returns count.
func (f *File) RenameCategory(old, new string) int {
	n := 0
	for i := range f.Feeds {
		if f.Feeds[i].Category == old {
			f.Feeds[i].Category = new
			n++
		}
	}
	return n
}

// RenameTag renames a tag across feeds; returns count.
func (f *File) RenameTag(old, new string) int {
	n := 0
	for i := range f.Feeds {
		for j, tag := range f.Feeds[i].Tags {
			if tag == old {
				f.Feeds[i].Tags[j] = new
				n++
			}
		}
	}
	return n
}

// TopCategories returns top-level category names (first path segment) in
// first-appearance order.
func (f *File) TopCategories() []string {
	seen := map[string]bool{}
	var out []string
	for _, feed := range f.Feeds {
		if feed.Category == "" {
			continue
		}
		top := TopCategory(feed.Category)
		if !seen[top] {
			seen[top] = true
			out = append(out, top)
		}
	}
	return out
}

// TopCategory returns the first segment of a nested category path.
func TopCategory(cat string) string {
	if i := strings.IndexByte(cat, '/'); i >= 0 {
		return cat[:i]
	}
	return cat
}

// AllTags returns unique tags in first-appearance order.
func (f *File) AllTags() []string {
	seen := map[string]bool{}
	var out []string
	for _, feed := range f.Feeds {
		for _, tag := range feed.Tags {
			if !seen[tag] {
				seen[tag] = true
				out = append(out, tag)
			}
		}
	}
	return out
}
