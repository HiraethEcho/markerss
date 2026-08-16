package ui

import (
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"

	tea "github.com/charmbracelet/bubbletea"

	"markerss/internal/config"
	"markerss/internal/feedlist"
	"markerss/internal/store"
)

const testRSS = `<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0"><channel><title>Test Feed</title>
<item><title>First Post</title><link>http://x.test/1</link><guid>g1</guid>
<description>&lt;p&gt;Hello &lt;b&gt;world&lt;/b&gt;&lt;/p&gt;</description>
<pubDate>Mon, 01 Jan 2024 12:00:00 GMT</pubDate></item>
<item><title>Second Post</title><link>http://x.test/2</link><guid>g2</guid>
<description>&lt;p&gt;Second summary&lt;/p&gt;</description></item>
</channel></rss>`

func newTestModel(t *testing.T) *Model {
	t.Helper()
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/rss+xml")
		w.Write([]byte(testRSS))
	}))
	t.Cleanup(srv.Close)
	st, err := store.Open(filepath.Join(t.TempDir(), "m.db"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { st.Close() })
	feeds := []feedlist.Feed{
		{URL: srv.URL, Title: "Test Feed", Category: "tech/go", Tags: []string{"mine"}},
		{URL: "https://other.test/rss", Title: "Other", Category: "tech", Favourite: true},
	}
	m := New(Config{
		Store:    st,
		Feeds:    feeds,
		URLsPath: filepath.Join(t.TempDir(), "urls"),
		Cfg:      config.Default(""),
		DataDir:  t.TempDir(),
	})
	m.width, m.height = 120, 40
	return m
}

func refresh(t *testing.T, m *Model) *Model {
	t.Helper()
	var mm tea.Model = m
	for _, msg := range execCmd(m.Init()) {
		if _, ok := msg.(intervalTickMsg); ok {
			continue
		}
		mm, _ = mm.Update(msg)
	}
	return mm.(*Model)
}

func execCmd(cmd tea.Cmd) []tea.Msg {
	if cmd == nil {
		return nil
	}
	msg := cmd()
	if b, ok := msg.(tea.BatchMsg); ok {
		var out []tea.Msg
		for _, sub := range b {
			out = append(out, execCmd(sub)...)
		}
		return out
	}
	return []tea.Msg{msg}
}

func key(s string) tea.KeyMsg {
	switch s {
	case "enter":
		return tea.KeyMsg{Type: tea.KeyEnter}
	case "up":
		return tea.KeyMsg{Type: tea.KeyUp}
	case "down":
		return tea.KeyMsg{Type: tea.KeyDown}
	case "left":
		return tea.KeyMsg{Type: tea.KeyLeft}
	case "right":
		return tea.KeyMsg{Type: tea.KeyRight}
	}
	return tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune(s)}
}

// findRow returns the nav row index for a section/cat/tag name.
func (m *Model) findRow(pred func(navRow) bool) int {
	for i, r := range m.navRows {
		if pred(r) {
			return i
		}
	}
	return -1
}

func TestRefreshPopulatesUnread(t *testing.T) {
	m := refresh(t, newTestModel(t))
	if m.scope.kind != scopeUnread {
		t.Errorf("default view should be Unread, got %+v", m.scope)
	}
	if len(m.items) != 2 {
		t.Fatalf("want 2 unread items, got %d", len(m.items))
	}
	if m.items[0].GUID != "g1" || m.items[1].GUID != "g2" {
		t.Errorf("ordering wrong: %s, %s", m.items[0].GUID, m.items[1].GUID)
	}
	// default preset sections present
	for _, sec := range []string{"Unread", "Read Later", "Favourite", "Categories", "Tags", "Saved"} {
		if m.findRow(func(r navRow) bool { return r.kind == rowSection && r.section == sec }) < 0 {
			t.Errorf("section %s missing", sec)
		}
	}
}

func TestNestedCategoryRows(t *testing.T) {
	m := refresh(t, newTestModel(t))
	// Categories section: cat rows tech, tech/go (nested), No Category absent
	catIdx := m.findRow(func(r navRow) bool { return r.kind == rowSection && r.section == "Categories" })
	if catIdx < 0 {
		t.Fatal("Categories missing")
	}
	techIdx := m.findRow(func(r navRow) bool { return r.kind == rowCat && r.cat == "tech" })
	goIdx := m.findRow(func(r navRow) bool { return r.kind == rowCat && r.cat == "tech/go" })
	if techIdx < 0 || goIdx < 0 {
		t.Fatalf("nested cats missing: tech=%d go=%d", techIdx, goIdx)
	}
	if !(catIdx < techIdx && techIdx < goIdx) {
		t.Errorf("order wrong: cat=%d tech=%d go=%d", catIdx, techIdx, goIdx)
	}
}

func TestFoldCascade(t *testing.T) {
	m := refresh(t, newTestModel(t))
	// collapse Categories section: h on Categories header (foldable, unfolded)
	ci := m.findRow(func(r navRow) bool { return r.kind == rowSection && r.section == "Categories" })
	m.navSel = ci
	m.updateKeys(key("h"))
	if !m.sectionFolded["Categories"] {
		t.Error("h on section should fold it")
	}
	// l on folded → expand + jump to first child
	m.updateKeys(key("l"))
	if m.sectionFolded["Categories"] {
		t.Error("l on folded should expand")
	}
	if m.navRows[m.navSel].kind != rowCat {
		t.Errorf("cursor should jump to first child, got %+v", m.navRows[m.navSel])
	}
}

func TestGoRightToScopes(t *testing.T) {
	m := refresh(t, newTestModel(t))
	// Unread row right → list
	ui := m.findRow(func(r navRow) bool { return r.kind == rowSection && r.section == "Unread" })
	m.navSel = ui
	m.updateKeys(key("enter"))
	if m.focus != paneList || m.scope.kind != scopeUnread {
		t.Errorf("unread right: focus=%d scope=%+v", m.focus, m.scope)
	}
	// back to nav, feed row right → feed scope
	m.updateKeys(key("q"))
	ti := m.findRow(func(r navRow) bool { return r.kind == rowFeed && m.feeds[r.feedIdx].Title == "Test Feed" })
	if ti < 0 {
		t.Fatal("Test Feed row missing")
	}
	m.navSel = ti
	m.updateKeys(key("enter"))
	if m.scope.kind != scopeFeed || m.scope.feedURL != m.feeds[0].URL {
		t.Errorf("feed scope = %+v", m.scope)
	}
}

func TestOpenArticleMarksRead(t *testing.T) {
	m := refresh(t, newTestModel(t))
	m.navSel = m.findRow(func(r navRow) bool { return r.kind == rowSection && r.section == "Unread" })
	m.updateKeys(key("enter"))
	m.updateKeys(key("enter")) // open g1
	if m.focus != paneArticle || m.artItem == nil || m.artItem.GUID != "g1" {
		t.Fatalf("article = %+v focus=%d", m.artItem, m.focus)
	}
	got, _ := m.store.Get(m.artItem.FeedURL, m.artItem.GUID)
	if !got.Read {
		t.Error("open must mark read")
	}
}

func TestFavouriteToggle(t *testing.T) {
	m := refresh(t, newTestModel(t))
	ti := m.findRow(func(r navRow) bool { return r.kind == rowFeed && m.feeds[r.feedIdx].Title == "Test Feed" })
	m.navSel = ti
	m.updateKeys(key("F"))
	if !m.feeds[0].Favourite {
		t.Error("F should favourite the feed")
	}
	m.updateKeys(key("F"))
	if m.feeds[0].Favourite {
		t.Error("F again should unfavourite")
	}
}

func TestPresetCycle(t *testing.T) {
	cfg := config.Default("")
	cfg.NavPresets = [][]string{{"Unread", "Feeds"}, {"Unread", "Categories"}}
	m := newTestModel(t)
	m.cfg = cfg
	m.rebuildNav()
	if len(m.navRows) != 4 { // Unread + Feeds + 2 feed rows
		t.Fatalf("preset 1 rows = %d, want 4", len(m.navRows))
	}
	m.updateKeys(key("t"))
	if m.navRows[1].section != "Categories" {
		t.Errorf("after t, second section = %q", m.navRows[1].section)
	}
}

func TestUnreadJump(t *testing.T) {
	m := refresh(t, newTestModel(t))
	m.navSel = m.findRow(func(r navRow) bool { return r.kind == rowSection && r.section == "Unread" })
	m.updateKeys(key("enter"))
	// mark g1 read via a, then n should jump to g2 (unread)
	m.updateKeys(key("a"))
	if !m.items[0].Read {
		t.Error("a should toggle read")
	}
	m.updateKeys(key("n"))
	if m.items[m.listSel].GUID != "g2" {
		t.Errorf("n should jump to next unread, at %s", m.items[m.listSel].GUID)
	}
}

func TestExportPrompt(t *testing.T) {
	m := refresh(t, newTestModel(t))
	m.navSel = m.findRow(func(r navRow) bool { return r.kind == rowSection && r.section == "Unread" })
	m.updateKeys(key("enter"))
	m.updateKeys(key("enter")) // open g1
	m.updateKeys(key("e"))     // prompt, prefilled
	if m.inputMode != inputExport {
		t.Fatal("e should open export prompt")
	}
	m.Update(key("enter")) // accept default (routed via input mode)
	if !strings.HasPrefix(m.status, "exported") {
		t.Fatalf("export failed: %s", m.status)
	}
	if _, err := os.Stat(filepath.Join(m.dir, "tech", "go", "first-post.md")); err != nil {
		t.Errorf("export file missing: %v", err)
	}
}

func TestLayoutExactHeight(t *testing.T) {
	m := refresh(t, newTestModel(t))
	m.width, m.height = 100, 25
	v := m.View()
	if got := len(strings.Split(v, "\n")); got != m.height {
		t.Errorf("View = %d lines, want %d", got, m.height)
	}
	if !strings.Contains(strings.Split(v, "\n")[0], "╭") {
		t.Error("top border must be on first line")
	}
}

func TestPaneRatio(t *testing.T) {
	m := newTestModel(t)
	m.cfg.PaneRatio = [3]float64{0.2, 0.3, 0.5}
	navW, listW, artW := m.paneWidths(100)
	if navW != 20 || listW != 30 || artW != 50 {
		t.Errorf("widths = %d/%d/%d", navW, listW, artW)
	}
}

func TestStartupShowsCacheBeforeFetch(t *testing.T) {
	st, err := store.Open(filepath.Join(t.TempDir(), "m.db"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { st.Close() })
	url := "http://127.0.0.1:1/feed.xml" // unreachable
	if err := st.UpsertItem(store.Item{FeedURL: url, GUID: "c1", Title: "Cached Post"}); err != nil {
		t.Fatal(err)
	}
	m := New(Config{
		Store:    st,
		Feeds:    []feedlist.Feed{{URL: url, Category: "tech"}},
		URLsPath: filepath.Join(t.TempDir(), "urls"),
		Cfg:      config.Default(""),
		DataDir:  t.TempDir(),
	})
	m.width, m.height = 120, 40
	_ = m.Init()
	if len(m.items) != 1 || m.items[0].Title != "Cached Post" {
		t.Errorf("cache must be visible before fetch: %+v", m.items)
	}
	if !strings.Contains(m.View(), "Cached Post") {
		t.Error("cached item must render")
	}
}

func TestOverlayFullScreen(t *testing.T) {
	m := refresh(t, newTestModel(t))
	m.width, m.height = 100, 30
	// input overlay: floating box, background preserved
	m.inputMode = inputAddURL
	m.inputVal = "https://x.test/feed"
	m.inputCur = len(m.inputVal)
	v := m.View()
	if got := len(strings.Split(v, "\n")); got != m.height {
		t.Errorf("view = %d lines, want %d", got, m.height)
	}
	if !strings.Contains(v, "Feed URL:") {
		t.Error("input label missing")
	}
	if !strings.Contains(v, "Unread") {
		t.Error("background nav must stay visible behind input box")
	}
	// help overlay: floating box, background preserved
	m.inputMode = inputNone
	m.helpOpen = true
	v = m.View()
	if got := len(strings.Split(v, "\n")); got != m.height {
		t.Errorf("view = %d lines, want %d", got, m.height)
	}
	if !strings.Contains(v, "markerss — keys") {
		t.Error("help title missing")
	}
	if !strings.Contains(v, "Unread") {
		t.Error("background must stay visible behind help box")
	}
	m.helpOpen = false
}

func TestAddFeedFlow(t *testing.T) {
	m := refresh(t, newTestModel(t))
	m.updateKeys(key("N"))
	if m.inputMode != inputAddURL {
		t.Fatal("N should open URL input")
	}
	typeText := func(s string) {
		for _, r := range s {
			m.updateInput(key(string(r)))
		}
	}
	typeText("https://new.test/feed")
	m.updateInput(key("enter"))
	if m.inputMode != inputAddTitle {
		t.Fatalf("expected title prompt, got %d", m.inputMode)
	}
	typeText("New Feed")
	m.updateInput(key("enter"))
	typeText("news")
	m.updateInput(key("enter"))
	typeText("#tag1 tag2")
	m.updateInput(key("enter"))
	if m.inputMode != inputNone {
		t.Errorf("input should close, got %d", m.inputMode)
	}
	if m.feedByURL("https://new.test/feed") == nil {
		t.Fatal("feed not added")
	}
	f := m.feedByURL("https://new.test/feed")
	if f.Category != "news" || len(f.Tags) != 2 || f.Tags[0] != "tag1" {
		t.Errorf("feed = %+v", f)
	}
	data, err := os.ReadFile(m.urls)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(data), `"New Feed" news #tag1 #tag2`) {
		t.Errorf("urls file not updated: %s", data)
	}
}

func TestEmptyFeedsState(t *testing.T) {
	st, err := store.Open(filepath.Join(t.TempDir(), "m.db"))
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()
	m := New(Config{Store: st, URLsPath: filepath.Join(t.TempDir(), "urls"),
		Cfg: config.Default(""), DataDir: t.TempDir()})
	m.width, m.height = 120, 40
	m.rebuildNav()
	if !strings.Contains(m.View(), "no feeds") {
		t.Error("empty nav should hint add feed")
	}
}
