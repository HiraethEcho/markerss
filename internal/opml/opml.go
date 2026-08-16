// Package opml imports/exports subscriptions in OPML 2.0, interoperable
// with newsboat. Categories become nested outlines; feeds carry
// xmlUrl; "~" title prefix preserved as custom display name.
package opml

import (
	"encoding/xml"
	"fmt"
	"os"
	"strings"

	"markerss/internal/feedlist"
)

type doc struct {
	XMLName xml.Name `xml:"opml"`
	Version string   `xml:"version,attr"`
	Head    head     `xml:"head"`
	Body    body     `xml:"body"`
}

type head struct {
	Title string `xml:"title"`
}

type body struct {
	Outlines []outline `xml:"outline"`
}

type outline struct {
	Text     string    `xml:"text,attr"`
	Title    string    `xml:"title,attr"`
	Type     string    `xml:"type,attr"`
	XMLURL   string    `xml:"xmlUrl,attr"`
	Outlines []outline `xml:"outline"`
}

// FeedName renders a feed's display name in OPML (newsboat convention:
// custom names carry a leading "~").
func FeedName(f feedlist.Feed) string {
	if f.Title == "" {
		return f.URL
	}
	if f.CustomTitle {
		return "~" + f.Title
	}
	return f.Title
}

// Export writes feeds to path as OPML, grouped by category (nested
// folders for nested categories).
func Export(path string, feeds []feedlist.Feed) error {
	d := doc{Version: "2.0", Head: head{Title: "markerss subscriptions"}}
	byCat := map[string][]feedlist.Feed{}
	var order []string
	for _, f := range feeds {
		cat := f.Category
		if _, ok := byCat[cat]; !ok {
			order = append(order, cat)
		}
		byCat[cat] = append(byCat[cat], f)
	}
	for _, cat := range order {
		if cat == "" {
			for _, f := range byCat[cat] {
				d.Body.Outlines = append(d.Body.Outlines, feedOutline(f))
			}
			continue
		}
		// nested category → nested folder
		d.Body.Outlines = append(d.Body.Outlines, catOutline(cat, byCat[cat]))
	}
	out, err := xml.MarshalIndent(d, "", "  ")
	if err != nil {
		return err
	}
	content := xml.Header + string(out) + "\n"
	return os.WriteFile(path, []byte(content), 0o644)
}

func feedOutline(f feedlist.Feed) outline {
	name := FeedName(f)
	return outline{
		Text:   name,
		Title:  name,
		Type:   "rss",
		XMLURL: f.URL,
	}
}

// catOutline builds nested folder outlines for a category path (a/b/c).
func catOutline(cat string, feeds []feedlist.Feed) outline {
	parts := strings.Split(cat, "/")
	var build func(level int) outline
	build = func(level int) outline {
		name := parts[level]
		o := outline{Text: name, Title: name}
		if level == len(parts)-1 {
			for _, f := range feeds {
				o.Outlines = append(o.Outlines, feedOutline(f))
			}
			return o
		}
		o.Outlines = append(o.Outlines, build(level+1))
		return o
	}
	return build(0)
}

// Import reads an OPML file. Folder outlines become nested categories
// (folder path joined with "/").
func Import(path string) ([]feedlist.Feed, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	var d doc
	if err := xml.Unmarshal(data, &d); err != nil {
		return nil, fmt.Errorf("%s: %w", path, err)
	}
	var feeds []feedlist.Feed
	var walk func(os []outline, tags []string) error
	walk = func(os []outline, tags []string) error {
		for _, o := range os {
			if o.XMLURL != "" {
				f := feedlist.Feed{
					URL:      o.XMLURL,
					Category: strings.Join(tags, "/"),
				}
				name := o.Title
				if name == "" {
					name = o.Text
				}
				if strings.HasPrefix(name, "~") {
					f.CustomTitle = true
					name = strings.TrimPrefix(name, "~")
				}
				if name != "" && name != f.URL {
					f.Title = name
				}
				feeds = append(feeds, f)
				continue
			}
			cat := o.Title
			if cat == "" {
				cat = o.Text
			}
			nt := tags
			if cat != "" {
				nt = append(append([]string(nil), tags...), cat)
			}
			if err := walk(o.Outlines, nt); err != nil {
				return err
			}
		}
		return nil
	}
	if err := walk(d.Body.Outlines, nil); err != nil {
		return nil, err
	}
	return feeds, nil
}
