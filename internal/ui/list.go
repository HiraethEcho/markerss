package ui

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/lipgloss"

	"markerss/internal/store"
)

var (
	stySel     = lipgloss.NewStyle().Bold(true).Foreground(lipgloss.Color("220"))
	styUnread  = lipgloss.NewStyle().Bold(true)
	styDim     = lipgloss.NewStyle().Foreground(lipgloss.Color("240"))
	styAccent  = lipgloss.NewStyle().Foreground(lipgloss.Color("39"))
	styHeader  = lipgloss.NewStyle().Bold(true).Foreground(lipgloss.Color("39"))
	styWarning = lipgloss.NewStyle().Foreground(lipgloss.Color("203"))
	styTitle   = lipgloss.NewStyle().Bold(true).Foreground(lipgloss.Color("39"))
)

// scopeURLs resolves the feeds for a scope.
func (m *Model) scopeURLs(s scope) []string {
	switch s.kind {
	case scopeUnread, scopeFeeds:
		return m.feedURLs()
	case scopeFeed:
		return []string{s.feedURL}
	case scopeCat:
		var out []string
		for _, f := range m.feeds {
			if f.Category == s.cat || strings.HasPrefix(f.Category, s.cat+"/") {
				out = append(out, f.URL)
			}
		}
		return out
	case scopeTag:
		var out []string
		for _, f := range m.feeds {
			if hasTag(f, s.tag) {
				out = append(out, f.URL)
			}
		}
		return out
	case scopeFav:
		var out []string
		for _, f := range m.feeds {
			if f.Favourite {
				out = append(out, f.URL)
			}
		}
		return out
	case scopeUncat:
		var out []string
		for _, f := range m.feeds {
			if f.Category == "" {
				out = append(out, f.URL)
			}
		}
		return out
	case scopeLater, scopeSaved:
		return nil // filled by Tags & Favorites spec
	}
	return nil
}

// reloadList loads items for a scope and selects the first.
func (m *Model) reloadList(s scope) {
	m.scope = s
	unreadFirst := s.kind != scopeFeed
	items, err := m.store.Items(m.scopeURLs(s), unreadFirst, s.kind == scopeUnread)
	if err != nil {
		m.status = "db: " + err.Error()
		m.items = nil
		return
	}
	m.items = items
	if len(items) == 0 {
		// empty scope → blank article pane
		m.artItem = nil
		m.artMD = nil
		m.listSel = 0
		m.clampScrolls()
		return
	}
	m.listSel = 0
	m.clampScrolls()
}

func (m *Model) scopeTitle() string {
	switch m.scope.kind {
	case scopeUnread:
		return "Unread"
	case scopeCat:
		return m.scope.cat
	case scopeTag:
		return "#" + m.scope.tag
	case scopeFeed:
		if f := m.feedByURL(m.scope.feedURL); f != nil {
			return m.displayName(*f)
		}
		return m.scope.feedURL
	case scopeFeeds:
		return "All Feeds"
	case scopeFav:
		return "Favourite"
	case scopeUncat:
		return "No Category"
	case scopeLater:
		return "Read Later"
	case scopeSaved:
		return "Saved"
	}
	return ""
}

func (m *Model) updateList(k string) {
	switch k {
	case "j", "down":
		if len(m.items) > 0 {
			m.listSel = min(m.listSel+1, len(m.items)-1)
		}
		m.previewArticle(m.listSel)
	case "k", "up":
		m.listSel = max(m.listSel-1, 0)
		m.previewArticle(m.listSel)
	case "n":
		m.jumpUnread(1)
	case "p":
		m.jumpUnread(-1)
	case "a":
		if m.listSel < len(m.items) {
			m.toggleRead(m.items[m.listSel])
		}
	case "A":
		if err := m.store.MarkAllRead(m.scopeURLs(m.scope)); err != nil {
			m.status = "db: " + err.Error()
			return
		}
		m.reloadList(m.scope)
		m.rebuildNav()
		m.status = "marked all read"
	case "o":
		if m.listSel < len(m.items) {
			m.openBrowser(m.items[m.listSel].Link)
		}
	case "e":
		if m.listSel < len(m.items) {
			m.startExport(m.items[m.listSel])
		}
	}
	// h/q/esc/← handled globally (goLeft)
}

// jumpUnread marks current read, moves to next/prev unread (no reorder).
func (m *Model) jumpUnread(dir int) {
	if len(m.items) == 0 {
		return
	}
	if !m.items[m.listSel].Read {
		m.toggleRead(m.items[m.listSel])
	}
	i := m.listSel + dir
	for i >= 0 && i < len(m.items) {
		if !m.items[i].Read {
			m.listSel = i
			m.previewArticle(i)
			return
		}
		i += dir
	}
	m.status = "no more unread"
}

func (m *Model) toggleRead(it store.Item) {
	if err := m.store.MarkRead(it.FeedURL, it.GUID, !it.Read); err != nil {
		m.status = "db: " + err.Error()
		return
	}
	for i := range m.items {
		if m.items[i].GUID == it.GUID && m.items[i].FeedURL == it.FeedURL {
			m.items[i].Read = !m.items[i].Read
		}
	}
	if m.artItem != nil && m.artItem.GUID == it.GUID {
		m.artItem.Read = !m.artItem.Read
	}
	m.rebuildNav()
}

func (m *Model) unreadInView() int {
	n := 0
	for _, it := range m.items {
		if !it.Read {
			n++
		}
	}
	return n
}

// listView renders items; window derives from the cursor position.
func (m *Model) listView(w, h int) string {
	if m.fullscreen {
		return paneStyle(w, h, false, "List").Render("")
	}
	inner := h - 2
	itemRows := inner - 1 // header line included in the pane
	unread := m.unreadInView()
	header := fmt.Sprintf("%s · %d unread", m.scopeTitle(), unread)
	var lines []string
	if len(m.items) == 0 {
		lines = append(lines, styDim.Render("no items — r to refresh"))
	} else {
		start := 0
		if len(m.items) > itemRows {
			start = max(0, min(m.listSel-itemRows/2, len(m.items)-itemRows))
		}
		end := min(start+itemRows, len(m.items))
		for i := start; i < end; i++ {
			it := m.items[i]
			mark := " "
			if !it.Read {
				mark = "•"
			}
			title := fmt.Sprintf("%s %s", mark, truncate(it.Title, w-6))
			if i == m.listSel {
				title = stySel.Render(title)
			} else if !it.Read {
				title = styUnread.Render(title)
			}
			lines = append(lines, truncateW(title, w-2))
		}
	}
	body := strings.Join(lines, "\n")
	if len(lines) < itemRows {
		body += strings.Repeat("\n", itemRows-len(lines))
	}
	body = headerLine(header, w-2) + "\n" + body
	return paneStyle(w, h, m.focus == paneList, "List").Render(body)
}

func headerLine(title string, w int) string {
	return styHeader.Render(truncateW(title, w))
}

func truncate(s string, n int) string {
	if n <= 0 {
		return ""
	}
	if len(s) <= n {
		return s
	}
	return s[:n-1] + "…"
}

// truncateW cuts s to max display width n, ANSI-aware. When cutting, it
// closes open styles with a reset and appends "…" when it fits.
func truncateW(s string, n int) string {
	if n <= 0 {
		return ""
	}
	out := make([]rune, 0, len(s))
	w := 0
	inEsc := false
	lastW := 0
	cut := false
	for _, r := range s {
		if r == '\x1b' {
			inEsc = true
			out = append(out, r)
			continue
		}
		if inEsc {
			out = append(out, r)
			if (r >= 'a' && r <= 'z') || (r >= 'A' && r <= 'Z') {
				inEsc = false
			}
			continue
		}
		rw := runeWidth(r)
		if w+rw > n {
			if w+1 <= n {
				out = append(out, '…')
			} else if w > 0 {
				// drop last rune to make room for …
				out = out[:len(out)-1]
				w -= lastW
				out = append(out, '…')
			}
			cut = true
			break
		}
		w += rw
		lastW = rw
		out = append(out, r)
	}
	if cut && strings.ContainsRune(s, '\x1b') {
		out = append(out, []rune("\x1b[0m")...)
	}
	return string(out)
}

// runeWidth approximates terminal display width (CJK = 2).
func runeWidth(r rune) int {
	if r == '\t' || r == '\n' {
		return 0
	}
	if r >= 0x2E80 && r <= 0x9FFF || r >= 0xF900 && r <= 0xFAFF || r >= 0xFF00 && r <= 0xFF60 {
		return 2
	}
	return 1
}

// blankLines returns height rows of width spaces (full-screen overlay base).
func blankLines(w, h int) []string {
	lines := make([]string, h)
	for i := range lines {
		lines[i] = padRight("", w)
	}
	return lines
}
