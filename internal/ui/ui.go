// Package ui implements the three-pane bubbletea TUI: preset-driven nav
// tree / item list / article reader. Behavior per DESIGN.md (Config
// section + MVP Layout): config-backed, presets, fold cascade.
package ui

import (
	"fmt"
	"os/exec"
	"path/filepath"
	"strings"
	"sync"
	"time"

	tea "github.com/charmbracelet/bubbletea"

	"markerss/internal/config"
	"markerss/internal/feedlist"
	"markerss/internal/fetch"
	"markerss/internal/opml"
	"markerss/internal/store"
	"markerss/internal/xdg"
)

type pane int

const (
	paneNav pane = iota
	paneList
	paneArticle
)

// scope is the current list view.
type scopeKind int

const (
	scopeUnread scopeKind = iota
	scopeCat
	scopeTag
	scopeFeed
	scopeFeeds
	scopeFav
	scopeUncat
	scopeLater
	scopeSaved
)

type scope struct {
	kind    scopeKind
	cat     string
	tag     string
	feedURL string
}

type navRowKind int

const (
	rowSection navRowKind = iota // virtual leaf or foldable section header
	rowCat
	rowTag
	rowFeed
	rowUncat
)

type navRow struct {
	kind         navRowKind
	section      string // section name (rowSection)
	cat          string // full category path (rowCat)
	tag          string // tag name (rowTag)
	feedIdx      int    // index into feeds (rowFeed)
	indent       int    // display indent
	foldKey      string // this row's fold key when foldable ("" otherwise)
	containerKey string // for feed rows: fold key of containing container
	foldable     bool
	labelCount   int // unread count shown on the row
}

type inputMode int

const (
	inputNone inputMode = iota
	inputAddURL
	inputAddTitle
	inputAddCat
	inputAddTags
	inputRenameCat
	inputRenameTag
	inputRenameFeed
	inputEditTags
	inputExport
	inputImportOPML
)

// Config wires model to storage, subscriptions, paths and app config.
type Config struct {
	Store    *store.Store
	Feeds    []feedlist.Feed
	URLsPath string
	DataDir  string
	Cfg      *config.Config
	Theme    *config.Theme
}

// Model is the bubbletea root model.
type Model struct {
	width, height int
	focus         pane

	store  *store.Store
	client *fetch.Client
	feeds  []feedlist.Feed
	urls   string
	dir    string
	cfg    *config.Config
	theme  *config.Theme

	presetIdx     int
	navRows       []navRow
	navSel        int
	sectionFolded map[string]bool // Favourite / Categories / Tags / Feeds
	catFolded     map[string]bool // full category path
	tagFolded     map[string]bool

	scope   scope
	items   []store.Item
	listSel int

	artItem   *store.Item
	artSel    int
	artMD     []string
	artScroll int
	fetching  bool

	refreshing  bool
	refreshErrs []string

	status string

	helpOpen   bool
	helpScroll int

	fullscreen bool

	inputMode    inputMode
	inputVal     string
	inputCur     int
	addFlow      struct{ url, title, cat, tags string }
	renameCat    string
	renameTagNam string
	editFeed     string
	exportIt     *store.Item

	pendDel string

	spinner int
	pending tea.Cmd
}

// New builds the model from config + storage.
func New(cfg Config) *Model {
	if cfg.Cfg == nil {
		cfg.Cfg = config.Default("")
	}
	if cfg.Theme == nil {
		cfg.Theme = config.DefaultTheme()
	}
	if cfg.DataDir == "" {
		cfg.DataDir = cfg.Cfg.ExportDirAbs
	}
	m := &Model{
		store:         cfg.Store,
		client:        fetch.New(fetch.Options{TimeoutSec: cfg.Cfg.FetchTimeoutSec, Proxy: cfg.Cfg.Proxy, MaxItemsPerFeed: cfg.Cfg.MaxItemsPerFeed}),
		feeds:         cfg.Feeds,
		urls:          cfg.URLsPath,
		dir:           cfg.DataDir,
		cfg:           cfg.Cfg,
		theme:         cfg.Theme,
		sectionFolded: map[string]bool{},
		catFolded:     map[string]bool{},
		tagFolded:     map[string]bool{},
		status:        "",
	}
	m.applyFoldLevel()
	return m
}

// applyFoldLevel folds the Categories tree to the configured depth
// (0 = all folded, -1 = all open).
func (m *Model) applyFoldLevel() {
	lvl := m.cfg.FoldLevelValue()
	if lvl < 0 {
		return
	}
	for _, f := range m.feeds {
		if f.Category == "" {
			continue
		}
		parts := strings.Split(f.Category, "/")
		for i := 1; i < len(parts); i++ {
			m.catFolded[strings.Join(parts[:i], "/")] = true
		}
		if len(parts) > lvl {
			m.catFolded[f.Category] = true
		}
	}
}

// ---------- messages ----------

type refreshDoneMsg struct {
	errs []string
	full bool
}
type articleDoneMsg struct {
	item *store.Item
	html string
	err  error
}
type spinnerTickMsg struct{}
type intervalTickMsg struct{}

// Init shows cached state immediately, then starts background refresh
// (per config auto_on_startup) and the interval ticker.
func (m *Model) Init() tea.Cmd {
	m.rebuildNav()
	m.applyDefaultView()
	m.previewArticle(m.listSel)
	var cmds []tea.Cmd
	if m.cfg.Refresh.AutoOnStartup {
		m.refreshing = true
		cmds = append(cmds, m.refreshCmd(m.feedURLs(), false))
	}
	if m.cfg.Refresh.IntervalMin > 0 {
		cmds = append(cmds, m.intervalCmd())
	}
	cmds = append(cmds, m.spinnerCmd())
	return tea.Batch(cmds...)
}

func (m *Model) spinnerCmd() tea.Cmd {
	return tea.Tick(100*time.Millisecond, func(time.Time) tea.Msg { return spinnerTickMsg{} })
}

func (m *Model) intervalCmd() tea.Cmd {
	d := time.Duration(m.cfg.Refresh.IntervalMin) * time.Minute
	return tea.Tick(d, func(time.Time) tea.Msg { return intervalTickMsg{} })
}

// applyDefaultView sets the startup scope from config.
func (m *Model) applyDefaultView() {
	v := m.cfg.DefaultView
	switch {
	case strings.HasPrefix(v, "Feed:"):
		u := strings.TrimPrefix(v, "Feed:")
		m.reloadList(scope{kind: scopeFeed, feedURL: u})
	case strings.HasPrefix(v, "Category:"):
		c := strings.TrimPrefix(v, "Category:")
		m.reloadList(scope{kind: scopeCat, cat: c})
	default:
		m.reloadList(scope{kind: scopeUnread})
	}
}

// refreshCmd fetches feeds (summary only) in parallel goroutines.
// full=true rebuilds the list snapshot on completion; partial merges.
func (m *Model) refreshCmd(urls []string, full bool) tea.Cmd {
	cl := m.client
	st := m.store
	maxPer := m.cfg.MaxItemsPerFeed
	return func() tea.Msg {
		var (
			mu   sync.Mutex
			errs []string
			wg   sync.WaitGroup
		)
		sem := make(chan struct{}, 8)
		for _, u := range urls {
			u := u
			wg.Add(1)
			go func() {
				defer wg.Done()
				sem <- struct{}{}
				defer func() { <-sem }()
				items, err := cl.Refresh(u)
				if err != nil {
					mu.Lock()
					errs = append(errs, u+": "+err.Error())
					mu.Unlock()
					return
				}
				for i := range items {
					if err := st.UpsertItem(items[i]); err != nil {
						mu.Lock()
						errs = append(errs, u+": "+err.Error())
						mu.Unlock()
						break
					}
				}
				if err := st.PruneFeed(u, maxPer); err != nil {
					mu.Lock()
					errs = append(errs, u+": "+err.Error())
					mu.Unlock()
				}
			}()
		}
		wg.Wait()
		return refreshDoneMsg{errs: errs, full: full}
	}
}

// fetchArticleCmd fetches full content for one item.
func (m *Model) fetchArticleCmd(it *store.Item) tea.Cmd {
	cl := m.client
	return func() tea.Msg {
		html, err := cl.Article(it.Link)
		return articleDoneMsg{item: it, html: html, err: err}
	}
}

// ---------- update ----------

func (m *Model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width, m.height = msg.Width, msg.Height
		m.clampScrolls()
		return m, nil
	case tea.KeyMsg:
		if m.helpOpen {
			m.updateHelp(msg)
			return m, nil
		}
		if m.inputMode != inputNone {
			m.updateInput(msg)
			return m, nil
		}
		cmd := m.updateKeys(msg)
		if cmd == nil {
			cmd = m.pending
		}
		m.pending = nil
		return m, cmd
	case refreshDoneMsg:
		m.refreshing = false
		m.refreshErrs = msg.errs
		m.rebuildNav()
		if msg.full || m.scope.kind == scopeUnread {
			// full refresh rebuilds the snapshot; unread view drops read items
			m.reloadList(m.scope)
		} else {
			m.mergeNewItems()
		}
		if m.scope.kind == scopeUnread && m.focus != paneArticle {
			m.previewArticle(m.listSel)
		}
		m.clampScrolls()
		if len(msg.errs) > 0 {
			m.status = fmt.Sprintf("refresh: %d feed(s) failed — R to retry", len(msg.errs))
		} else if msg.full {
			m.status = "full refresh done"
		} else {
			m.status = "refreshed"
		}
		return m, nil
	case articleDoneMsg:
		m.fetching = false
		if msg.err != nil {
			m.status = "fetch failed: " + msg.err.Error()
			return m, nil
		}
		if err := m.store.SaveContent(msg.item.FeedURL, msg.item.GUID, msg.html); err != nil {
			m.status = "store: " + err.Error()
			return m, nil
		}
		if it, err := m.store.Get(msg.item.FeedURL, msg.item.GUID); err == nil {
			m.artItem = it
			m.buildArticle(true)
		}
		m.status = "full article fetched"
		return m, nil
	case spinnerTickMsg:
		m.spinner++
		if m.refreshing {
			return m, m.spinnerCmd()
		}
		return m, nil
	case intervalTickMsg:
		if m.cfg.Refresh.IntervalMin > 0 {
			m.refreshing = true
			return m, tea.Batch(m.refreshCmd(m.scopeURLs(m.scope), false), m.intervalCmd())
		}
		return m, nil
	}
	return m, nil
}

// mergeNewItems keeps current list order and prepends newly-seen items
// (partial refresh semantics: append-only, no reorder).
func (m *Model) mergeNewItems() {
	seen := map[string]bool{}
	for _, it := range m.items {
		seen[it.FeedURL+"|"+it.GUID] = true
	}
	all, err := m.store.Items(m.scopeURLs(m.scope), m.scope.kind != scopeFeed, false)
	if err != nil {
		return
	}
	var fresh []store.Item
	for _, it := range all {
		if !seen[it.FeedURL+"|"+it.GUID] {
			fresh = append(fresh, it)
		}
	}
	if len(fresh) == 0 {
		return
	}
	// new unread go to the top, newest first
	m.items = append(fresh, m.items...)
	m.clampScrolls()
}

// updateKeys dispatches global keys; returns a cmd (may be tea.Quit).
func (m *Model) updateKeys(msg tea.KeyMsg) tea.Cmd {
	k := msg.String()
	// keybinding remap
	if mapped, ok := m.cfg.Keybindings[k]; ok {
		k = mapped
	}
	switch k {
	case "ctrl+c":
		return tea.Quit
	case "Q":
		return tea.Quit
	case "?":
		m.helpOpen = true
		m.helpScroll = 0
		return nil
	}
	switch k {
	case "h", "q", "esc", "left":
		m.goLeft()
	case "l", "enter", "right":
		m.goRight()
	case "tab":
		m.focus = (m.focus + 1) % 3
	case "shift+tab":
		m.focus = (m.focus + 2) % 3
	case "r":
		return m.startRefresh(false)
	case "R":
		return m.startRefresh(true)
	case "t":
		m.cyclePreset()
	case "i":
		m.inputMode = inputImportOPML
		m.inputVal = filepath.Join(xdg.DataHome(), "markerss", "subscriptions.opml")
		m.inputCur = len([]rune(m.inputVal))
	case "x":
		m.exportOPML()
	case "F":
		if m.focus == paneArticle {
			m.fullscreen = !m.fullscreen
		} else if m.focus == paneNav {
			m.toggleFavourite()
		}
	case "L", "S":
		m.status = k + ": read-later/saved land with Tags & Favorites spec"
	default:
		switch m.focus {
		case paneNav:
			m.updateNav(k)
		case paneList:
			m.updateList(k)
		case paneArticle:
			m.updateArticle(k)
		}
	}
	return nil
}

func (m *Model) startRefresh(full bool) tea.Cmd {
	if m.refreshing {
		return nil
	}
	m.refreshing = true
	var urls []string
	if full {
		urls = m.feedURLs()
	} else {
		urls = m.scopeURLs(m.scope)
	}
	if len(urls) == 0 {
		m.refreshing = false
		m.status = "no feeds in scope"
		return nil
	}
	m.status = "refreshing…"
	return m.refreshCmd(urls, full)
}

// goLeft: article → list → nav; in nav: fold cascade.
func (m *Model) goLeft() {
	if m.focus == paneArticle {
		if m.fullscreen {
			m.fullscreen = false
		}
		m.focus = paneList
		m.buildArticle(false) // list mode: summary only
		return
	}
	if m.focus == paneList {
		m.focus = paneNav
		return
	}
	// nav fold cascade
	if m.navSel >= len(m.navRows) {
		return
	}
	r := m.navRows[m.navSel]
	switch {
	case r.kind == rowFeed && r.containerKey != "":
		m.foldKey(r.containerKey)
	case r.foldable && !m.folded(r.foldKey):
		m.foldKey(r.foldKey)
	case r.foldable && m.folded(r.foldKey):
		// folded → fold parent (top stays)
		if p := m.parentFoldKey(r); p != "" {
			m.foldKey(p)
		}
	default:
		// leaf cat/tag/uncat with nothing to fold → fold its container
		if p := m.parentFoldKey(r); p != "" {
			m.foldKey(p)
		}
	}
}

// goRight: expand → list → article → fetch full.
func (m *Model) goRight() {
	switch m.focus {
	case paneList:
		m.openArticle(m.listSel)
	case paneArticle:
		m.fetchFull()
	case paneNav:
		if m.navSel >= len(m.navRows) {
			return
		}
		r := m.navRows[m.navSel]
		switch {
		case r.foldable && m.folded(r.foldKey):
			m.unfoldKey(r.foldKey, true)
		case r.kind == rowCat:
			if m.folded(r.foldKey) {
				m.unfoldKey(r.foldKey, true)
			} else {
				m.reloadList(scope{kind: scopeCat, cat: r.cat})
				m.previewArticle(m.listSel)
				m.focus = paneList
			}
		case r.kind == rowTag:
			if m.folded(r.foldKey) {
				m.unfoldKey(r.foldKey, true)
			} else {
				m.reloadList(scope{kind: scopeTag, tag: r.tag})
				m.previewArticle(m.listSel)
				m.focus = paneList
			}
		case r.kind == rowFeed:
			m.reloadList(scope{kind: scopeFeed, feedURL: m.feeds[r.feedIdx].URL})
			m.previewArticle(m.listSel)
			m.focus = paneList
		case r.kind == rowUncat:
			m.reloadList(scope{kind: scopeUncat})
			m.previewArticle(m.listSel)
			m.focus = paneList
		case r.kind == rowSection && r.foldable:
			// expanded section → aggregate list
			m.reloadList(m.sectionScope(r.section))
			m.previewArticle(m.listSel)
			m.focus = paneList
		case r.kind == rowSection:
			// leaf virtual → list
			m.reloadList(m.sectionScope(r.section))
			m.previewArticle(m.listSel)
			m.focus = paneList
		}
	}
}

// sectionScope maps a section name to a list scope.
func (m *Model) sectionScope(sec string) scope {
	switch sec {
	case "Unread":
		return scope{kind: scopeUnread}
	case "Read Later":
		return scope{kind: scopeLater}
	case "Saved":
		return scope{kind: scopeSaved}
	case "Favourite":
		return scope{kind: scopeFav}
	case "Categories":
		return scope{kind: scopeFeeds}
	case "Tags":
		return scope{kind: scopeFeeds}
	case "Feeds":
		return scope{kind: scopeFeeds}
	case "No Category":
		return scope{kind: scopeUncat}
	}
	return scope{kind: scopeUnread}
}

func (m *Model) folded(key string) bool {
	if key == "" {
		return false
	}
	switch {
	case strings.HasPrefix(key, "cat:"):
		return m.catFolded[strings.TrimPrefix(key, "cat:")]
	case strings.HasPrefix(key, "tag:"):
		return m.tagFolded[strings.TrimPrefix(key, "tag:")]
	default:
		return m.sectionFolded[key]
	}
}

func (m *Model) foldKey(key string) {
	if key == "" {
		return
	}
	switch {
	case strings.HasPrefix(key, "cat:"):
		m.catFolded[strings.TrimPrefix(key, "cat:")] = true
	case strings.HasPrefix(key, "tag:"):
		m.tagFolded[strings.TrimPrefix(key, "tag:")] = true
	default:
		m.sectionFolded[key] = true
	}
	m.rebuildNav()
	m.moveToKey(key)
}

// unfoldKey expands and optionally jumps the cursor to the first child.
func (m *Model) unfoldKey(key string, jumpChild bool) {
	switch {
	case strings.HasPrefix(key, "cat:"):
		delete(m.catFolded, strings.TrimPrefix(key, "cat:"))
	case strings.HasPrefix(key, "tag:"):
		delete(m.tagFolded, strings.TrimPrefix(key, "tag:"))
	default:
		delete(m.sectionFolded, key)
	}
	m.rebuildNav()
	if jumpChild {
		m.moveToFirstChild(key)
	}
}

// parentFoldKey finds the containing container's fold key. A folded
// top-level section stays (no parent); nested rows fold to their section
// or enclosing category/tag.
func (m *Model) parentFoldKey(r navRow) string {
	if r.kind == rowSection || r.kind == rowUncat && r.foldKey == "" {
		return "" // top stays
	}
	for i := m.navSel - 1; i >= 0; i-- {
		p := m.navRows[i]
		if p.kind == rowSection && p.foldable && p.foldKey != "" {
			return p.foldKey
		}
		if p.foldable && p.foldKey != "" && p.kind != rowSection && p.indent < r.indent {
			return p.foldKey
		}
	}
	return ""
}

func (m *Model) moveToKey(key string) {
	for i, r := range m.navRows {
		if r.foldKey == key || r.containerKey == key {
			m.navSel = i
			return
		}
	}
}

func (m *Model) moveToFirstChild(key string) {
	for i, r := range m.navRows {
		if r.foldKey == key {
			if i+1 < len(m.navRows) {
				m.navSel = i + 1
			}
			return
		}
	}
}

func (m *Model) cyclePreset() {
	presets := m.presets()
	if len(presets) < 2 {
		return
	}
	m.presetIdx = (m.presetIdx + 1) % len(presets)
	m.rebuildNav()
	m.status = fmt.Sprintf("preset %d/%d", m.presetIdx+1, len(presets))
}

// presets returns configured presets or the default full one.
func (m *Model) presets() [][]string {
	if len(m.cfg.NavPresets) > 0 {
		return m.cfg.NavPresets
	}
	return [][]string{{"Unread", "Read Later", "Favourite", "Categories", "Tags", "Saved"}}
}

func (m *Model) currentPreset() []string {
	ps := m.presets()
	if m.presetIdx >= len(ps) {
		m.presetIdx = 0
	}
	return ps[m.presetIdx]
}

func (m *Model) exportOPML() {
	path := filepath.Join(xdg.DataHome(), "markerss", "subscriptions.opml")
	if err := opml.Export(path, m.feeds); err != nil {
		m.status = "opml export: " + err.Error()
		return
	}
	m.status = "exported → " + path
}

func (m *Model) saveFeeds() error {
	f := &feedlist.File{Feeds: m.feeds}
	return f.Save(m.urls)
}

func (m *Model) feedURLs() []string {
	out := make([]string, 0, len(m.feeds))
	for _, f := range m.feeds {
		out = append(out, f.URL)
	}
	return out
}

func (m *Model) feedByURL(u string) *feedlist.Feed {
	for i := range m.feeds {
		if m.feeds[i].URL == u {
			return &m.feeds[i]
		}
	}
	return nil
}

func (m *Model) displayName(f feedlist.Feed) string {
	if f.Title != "" {
		return f.Title
	}
	return f.URL
}

func (m *Model) categoryOf(feedURL string) string {
	if f := m.feedByURL(feedURL); f != nil {
		return f.Category
	}
	return ""
}

func (m *Model) openBrowser(url string) {
	if url == "" {
		return
	}
	bin := m.cfg.Browser
	if bin == "" {
		bin = "xdg-open"
	}
	if err := exec.Command(bin, url).Start(); err != nil {
		m.status = "open: " + err.Error()
		return
	}
	m.status = "opened in browser"
}

func (m *Model) clampScrolls() {
	if m.artItem != nil && m.artScroll > len(m.artMD)-1 {
		m.artScroll = max(0, len(m.artMD)-1)
	}
	if m.navSel >= len(m.navRows) {
		m.navSel = max(0, len(m.navRows)-1)
	}
	if m.listSel >= len(m.items) {
		m.listSel = max(0, len(m.items)-1)
	}
	if m.artScroll < 0 {
		m.artScroll = 0
	}
}

func max(a, b int) int {
	if a > b {
		return a
	}
	return b
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}
