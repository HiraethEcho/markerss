package ui

import (
	"fmt"
	"path/filepath"
	"strings"

	"markerss/internal/export"
	"markerss/internal/markdown"
	"markerss/internal/store"
)

// previewArticle shows an item's header (title/meta/summary excerpt)
// without marking it read — list-pane live preview (list mode).
func (m *Model) previewArticle(i int) {
	if i < 0 || i >= len(m.items) {
		return
	}
	it := m.items[i]
	m.artItem = &it
	m.artSel = i
	m.artScroll = 0
	m.fetching = false
	m.buildArticle(false)
}

// openArticle opens item i: marks read, focuses article pane, shows body.
func (m *Model) openArticle(i int) {
	if i < 0 || i >= len(m.items) {
		return
	}
	it := m.items[i]
	if !it.Read {
		if err := m.store.MarkRead(it.FeedURL, it.GUID, true); err == nil {
			it.Read = true
			m.items[i] = it
			m.rebuildNav()
		}
	}
	m.artItem = &it
	m.artSel = i
	m.artScroll = 0
	m.fetching = false
	m.focus = paneArticle
	m.buildArticle(true)
}

// buildArticle renders the article pane. open=true shows the RSS body +
// fetched content; open=false is the summary-only list preview.
func (m *Model) buildArticle(open bool) {
	it := m.artItem
	if it == nil {
		m.artMD = nil
		return
	}
	artW := m.articleWidth() - 4
	var lines []string
	lines = append(lines, styTitle.Render(it.Title))
	feedName := ""
	if f := m.feedByURL(it.FeedURL); f != nil {
		feedName = m.displayName(*f)
	}
	date := "—"
	if !it.Published.IsZero() {
		date = it.Published.Format("2006-01-02 15:04")
	}
	lines = append(lines, styDim.Render(fmt.Sprintf("%s · %s", feedName, date)))
	if it.Link != "" {
		lines = append(lines, styDim.Render(it.Link))
	}
	if !open {
		// list mode: summary excerpt only
		if s := markdown.HTMLToMD(it.Summary); s != "" {
			lines = append(lines, "")
			lines = append(lines, markdown.Render(s, artW)...)
		}
		m.artMD = lines
		m.clampScrolls()
		return
	}
	// article mode: summary in header; body = fetched content > RSS body
	// (content:encoded / atom content) > fetch hint
	if s := markdown.HTMLToMD(it.Summary); s != "" {
		lines = append(lines, "", styDim.Render("─ summary ─"))
		lines = append(lines, markdown.Render(s, artW)...)
	}
	switch {
	case it.Content != "":
		if c := markdown.HTMLToMD(it.Content); c != "" {
			lines = append(lines, "", styDim.Render("─ content ─"))
			lines = append(lines, markdown.Render(c, artW)...)
		}
	case it.Body != "":
		if b := markdown.HTMLToMD(it.Body); b != "" {
			lines = append(lines, "", styDim.Render("─ body ─"))
			lines = append(lines, markdown.Render(b, artW)...)
		}
	case m.fetching:
		lines = append(lines, "", "", styDim.Render("fetching full article…"))
	default:
		lines = append(lines, "", "",
			styDim.Render("no body in feed — press enter to fetch full article"))
	}
	m.artMD = lines
	m.clampScrolls()
}

func (m *Model) updateArticle(k string) {
	switch k {
	case "j", "down":
		m.artScroll++
	case "k", "up":
		m.artScroll--
	case "ctrl+u", "pgup":
		m.artScroll -= max(1, (m.height-1)/2)
	case "ctrl+d", "pgdown":
		m.artScroll += max(1, (m.height-1)/2)
	case "n":
		m.openArticle(m.artSel + 1)
	case "p":
		m.openArticle(m.artSel - 1)
	case "a":
		if m.artItem != nil {
			m.toggleRead(*m.artItem)
		}
	case "o":
		if m.artItem != nil {
			m.openBrowser(m.artItem.Link)
		}
	case "e":
		if m.artItem != nil {
			m.startExport(*m.artItem)
		}
	}
	// enter/l/→, h/q/esc/← handled globally (goRight / goLeft)
	m.clampScrolls()
}

// fetchFull fetches article content on explicit enter only.
func (m *Model) fetchFull() {
	it := m.artItem
	if it == nil || m.fetching {
		return
	}
	if it.Content != "" {
		m.status = "already fetched"
		return
	}
	if it.Link == "" {
		m.status = "no link"
		return
	}
	m.fetching = true
	m.buildArticle(true)
	m.status = "fetching full article…"
	m.pending = m.fetchArticleCmd(it)
}

// startExport opens the export prompt with the default path prefilled.
func (m *Model) startExport(it store.Item) {
	def := m.defaultExportPath(it)
	m.exportIt = &it
	m.inputMode = inputExport
	m.inputVal = def
	m.inputCur = len([]rune(def))
	m.status = "e: enter accepts, type to change path"
}

func (m *Model) defaultExportPath(it store.Item) string {
	cat := m.categoryOf(it.FeedURL)
	slug := markdown.Slug(it.Title)
	if cat != "" {
		return filepath.Join(m.dir, cat, slug+".md")
	}
	return filepath.Join(m.dir, slug+".md")
}

// exportItem writes the item as markdown with YAML frontmatter. path is
// either a full .md path (used as-is) or a base dir (category/slug
// appended by export.Write).
func (m *Model) exportItem(it store.Item, path string) {
	md := markdown.HTMLToMD(it.Content)
	if strings.TrimSpace(md) == "" {
		md = markdown.HTMLToMD(it.Body)
	}
	if strings.TrimSpace(md) == "" {
		md = markdown.HTMLToMD(it.Summary)
	}
	feedName := ""
	if f := m.feedByURL(it.FeedURL); f != nil {
		feedName = m.displayName(*f)
	}
	meta := export.Meta{
		Title:    it.Title,
		Link:     it.Link,
		Date:     it.Published,
		Feed:     feedName,
		Category: m.categoryOf(it.FeedURL),
	}
	var p string
	var err error
	if strings.HasSuffix(path, ".md") {
		p, err = export.WriteTo(path, md, meta)
	} else {
		p, err = export.Write(md, meta, path)
	}
	if err != nil {
		m.status = "export: " + err.Error()
		return
	}
	m.status = "exported → " + p
}

// articleView renders the article pane slice around the scroll offset.
func (m *Model) articleView(w, h int) string {
	if len(m.items) == 0 || m.artItem == nil {
		// empty list → blank article pane
		return paneStyle(w, h, m.focus == paneArticle, "Article").Render("")
	}
	inner := h - 2
	if len(m.artMD) == 0 {
		m.buildArticle(m.focus == paneArticle)
	}
	lines := m.artMD
	if m.artScroll < 0 {
		m.artScroll = 0
	}
	start := m.artScroll
	if start > len(lines) {
		start = len(lines)
	}
	end := min(start+inner, len(lines))
	var parts []string
	for _, l := range lines[start:end] {
		parts = append(parts, truncateW(l, w-2))
	}
	body := strings.Join(parts, "\n")
	if end-start < inner {
		body += strings.Repeat("\n", inner-(end-start))
	}
	return paneStyle(w, h, m.focus == paneArticle, "Article").Render(body)
}
