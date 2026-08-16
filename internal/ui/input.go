package ui

import (
	"strconv"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"

	"markerss/internal/feedlist"
	"markerss/internal/opml"
)

var styInput = lipgloss.NewStyle().Foreground(lipgloss.Color("39"))

func (m *Model) inputLabel() string {
	switch m.inputMode {
	case inputAddURL:
		return "Feed URL: "
	case inputAddTitle:
		return "Title (optional, ~ = custom): "
	case inputAddCat:
		return "Category (optional, a/b nests): "
	case inputAddTags:
		return "Tags (space-separated, # optional): "
	case inputRenameCat:
		return "Rename category: "
	case inputRenameTag:
		return "Rename tag: "
	case inputRenameFeed:
		return "Feed custom title (empty = none): "
	case inputEditTags:
		return "Feed tags (space-separated): "
	case inputExport:
		return "Export path: "
	case inputImportOPML:
		return "OPML path: "
	}
	return ""
}

// updateInput handles text entry. Enter commits, esc cancels.
func (m *Model) updateInput(msg tea.KeyMsg) {
	switch msg.String() {
	case "enter":
		m.commitInput()
	case "esc", "ctrl+c":
		m.inputMode = inputNone
	case "backspace":
		runes := []rune(m.inputVal)
		if m.inputCur > 0 {
			m.inputCur--
			m.inputVal = string(runes[:m.inputCur]) + string(runes[m.inputCur+1:])
		}
	case "left":
		if m.inputCur > 0 {
			m.inputCur--
		}
	case "right":
		if m.inputCur < len([]rune(m.inputVal)) {
			m.inputCur++
		}
	default:
		if s := msg.String(); len(s) == 1 {
			runes := []rune(m.inputVal)
			m.inputVal = string(runes[:m.inputCur]) + s + string(runes[m.inputCur:])
			m.inputCur++
		}
	}
}

// commitInput runs the mode's action; errors surface in status.
func (m *Model) commitInput() {
	mode := m.inputMode
	val := strings.TrimSpace(m.inputVal)
	m.inputMode = inputNone
	switch mode {
	case inputAddURL:
		if val == "" {
			m.status = "no URL"
			return
		}
		m.addFlow.url = val
		m.addFlow.title, m.addFlow.cat, m.addFlow.tags = "", "", ""
		m.inputMode = inputAddTitle
		m.inputVal, m.inputCur = "", 0
	case inputAddTitle:
		m.addFlow.title = val
		m.inputMode = inputAddCat
		m.inputVal, m.inputCur = "", 0
	case inputAddCat:
		m.addFlow.cat = val
		m.inputMode = inputAddTags
		m.inputVal, m.inputCur = "", 0
	case inputAddTags:
		m.addFeed(m.addFlow.url, m.addFlow.title, m.addFlow.cat, val)
	case inputRenameCat:
		m.renameCategory(val)
	case inputRenameTag:
		m.renameTag(val)
	case inputRenameFeed:
		m.renameFeedTitle(val)
	case inputEditTags:
		m.editFeedTags(val)
	case inputExport:
		if m.exportIt != nil {
			it := *m.exportIt
			m.exportIt = nil
			m.exportItem(it, val)
		}
	case inputImportOPML:
		m.importOPML(val)
	}
}

func (m *Model) addFeed(url, title, cat, tags string) {
	f := feedlist.Feed{URL: url}
	if title != "" {
		f.Title = strings.TrimPrefix(title, "~")
		f.CustomTitle = strings.HasPrefix(title, "~")
	}
	f.Category = strings.TrimPrefix(cat, "#")
	for _, t := range strings.Fields(tags) {
		t = strings.TrimPrefix(t, "#")
		if t != "" {
			f.Tags = append(f.Tags, t)
		}
	}
	for _, ex := range m.feeds {
		if ex.URL == url {
			m.status = "feed already subscribed"
			return
		}
	}
	m.feeds = append(m.feeds, f)
	if err := m.saveFeeds(); err != nil {
		m.status = "urls: " + err.Error()
		return
	}
	m.rebuildNav()
	m.reloadList(scope{kind: scopeFeed, feedURL: url})
	m.focus = paneList
	m.status = "added " + m.displayName(f)
}

func (m *Model) renameCategory(newName string) {
	old := m.renameCat
	m.renameCat = ""
	if newName == "" || newName == old {
		return
	}
	f := &feedlist.File{Feeds: m.feeds}
	f.RenameCategory(old, newName)
	if err := f.Save(m.urls); err != nil {
		m.status = "urls: " + err.Error()
		return
	}
	m.feeds = f.Feeds
	if m.catFolded[old] {
		m.catFolded[newName] = true
	}
	delete(m.catFolded, old)
	m.rebuildNav()
	if m.scope.kind == scopeCat && m.scope.cat == old {
		m.reloadList(scope{kind: scopeCat, cat: newName})
	}
	m.status = "renamed " + old + " → " + newName
}

func (m *Model) renameTag(newName string) {
	old := m.renameTagNam
	m.renameTagNam = ""
	if newName == "" || newName == old {
		return
	}
	f := &feedlist.File{Feeds: m.feeds}
	f.RenameTag(old, newName)
	if err := f.Save(m.urls); err != nil {
		m.status = "urls: " + err.Error()
		return
	}
	m.feeds = f.Feeds
	if m.tagFolded[old] {
		m.tagFolded[newName] = true
	}
	delete(m.tagFolded, old)
	m.rebuildNav()
	if m.scope.kind == scopeTag && m.scope.tag == old {
		m.reloadList(scope{kind: scopeTag, tag: newName})
	}
	m.status = "renamed #" + old + " → #" + newName
}

func (m *Model) renameFeedTitle(title string) {
	u := m.editFeed
	m.editFeed = ""
	f := m.feedByURL(u)
	if f == nil {
		return
	}
	f.Title = title
	f.CustomTitle = title != ""
	if err := m.saveFeeds(); err != nil {
		m.status = "urls: " + err.Error()
		return
	}
	m.rebuildNav()
	m.status = "renamed feed"
}

func (m *Model) editFeedTags(tags string) {
	u := m.editFeed
	m.editFeed = ""
	f := m.feedByURL(u)
	if f == nil {
		return
	}
	f.Tags = nil
	for _, t := range strings.Fields(tags) {
		t = strings.TrimPrefix(t, "#")
		if t != "" {
			f.Tags = append(f.Tags, t)
		}
	}
	if err := m.saveFeeds(); err != nil {
		m.status = "urls: " + err.Error()
		return
	}
	m.rebuildNav()
	m.status = "tags updated"
}

func (m *Model) importOPML(path string) {
	if path == "" {
		m.status = "no path"
		return
	}
	feeds, err := opml.Import(path)
	if err != nil {
		m.status = "opml import: " + err.Error()
		return
	}
	if len(feeds) == 0 {
		m.status = "opml: no feeds found"
		return
	}
	have := map[string]bool{}
	for _, f := range m.feeds {
		have[f.URL] = true
	}
	added := 0
	for _, f := range feeds {
		if have[f.URL] {
			continue
		}
		have[f.URL] = true
		m.feeds = append(m.feeds, f)
		added++
	}
	if err := m.saveFeeds(); err != nil {
		m.status = "urls: " + err.Error()
		return
	}
	m.rebuildNav()
	m.reloadList(m.scope)
	m.status = "imported " + strconv.Itoa(added) + " feed(s)"
}
