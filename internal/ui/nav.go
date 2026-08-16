package ui

import (
	"fmt"
	"sort"
	"strings"

	"markerss/internal/feedlist"
)

// rebuildNav reconstructs rows from the current preset + fold state.
func (m *Model) rebuildNav() {
	counts, err := m.store.UnreadCounts(m.feedURLs())
	if err != nil {
		counts = map[string]int{}
	}
	var rows []navRow
	for _, sec := range m.currentPreset() {
		switch sec {
		case "Unread":
			total := 0
			for _, n := range counts {
				total += n
			}
			rows = append(rows, navRow{kind: rowSection, section: "Unread", indent: 0,
				labelCount: total})
		case "Read Later":
			rows = append(rows, navRow{kind: rowSection, section: "Read Later", indent: 0, labelCount: 0})
		case "Saved":
			rows = append(rows, navRow{kind: rowSection, section: "Saved", indent: 0, labelCount: 0})
		case "Favourite":
			key := "Favourite"
			rows = append(rows, navRow{kind: rowSection, section: "Favourite", indent: 0,
				foldable: true, foldKey: key, labelCount: m.favUnread(counts)})
			if !m.folded(key) {
				for i, f := range m.feeds {
					if f.Favourite {
						rows = append(rows, navRow{kind: rowFeed, feedIdx: i, indent: 1,
							containerKey: key})
					}
				}
			}
		case "Categories":
			key := "Categories"
			rows = append(rows, navRow{kind: rowSection, section: "Categories", indent: 0,
				foldable: true, foldKey: key, labelCount: m.categorizedUnread(counts)})
			if !m.folded(key) {
				rows = append(rows, m.categoryRows(counts)...)
			}
			// No Category: sibling section, auto after Categories
			if m.hasUncatFeeds() {
				nk := "No Category"
				rows = append(rows, navRow{kind: rowSection, section: "No Category", indent: 0,
					foldable: true, foldKey: nk, labelCount: m.uncatUnread(counts)})
				if !m.folded(nk) {
					for i, f := range m.feeds {
						if f.Category == "" {
							rows = append(rows, navRow{kind: rowFeed, feedIdx: i, indent: 1,
								containerKey: nk})
						}
					}
				}
			}
		case "Tags":
			key := "Tags"
			rows = append(rows, navRow{kind: rowSection, section: "Tags", indent: 0,
				foldable: true, foldKey: key, labelCount: m.taggedUnread(counts)})
			if !m.folded(key) {
				rows = append(rows, m.tagRows(counts)...)
			}
		case "Feeds":
			key := "Feeds"
			rows = append(rows, navRow{kind: rowSection, section: "Feeds", indent: 0,
				foldable: true, foldKey: key, labelCount: m.totalOf(counts)})
			if !m.folded(key) {
				for i := range m.feeds {
					rows = append(rows, navRow{kind: rowFeed, feedIdx: i, indent: 1,
						containerKey: key})
				}
			}
		}
	}
	m.navRows = rows
	m.clampScrolls()
}

// categoryRows builds the nested category tree + No Category tail.
func (m *Model) categoryRows(counts map[string]int) []navRow {
	// collect distinct category paths
	seen := map[string]bool{}
	var paths []string
	for _, f := range m.feeds {
		if f.Category == "" {
			continue
		}
		if !seen[f.Category] {
			seen[f.Category] = true
			paths = append(paths, f.Category)
		}
	}
	sort.Strings(paths)

	var rows []navRow
	for _, p := range paths {
		rows = append(rows, m.catPathRows(p, counts)...)
	}
	return rows
}

// hasUncatFeeds reports whether any feed lacks a category.
func (m *Model) hasUncatFeeds() bool {
	for _, f := range m.feeds {
		if f.Category == "" {
			return true
		}
	}
	return false
}

// catPathRows emits one row per path segment, respecting folds. A folded
// parent hides deeper segments.
func (m *Model) catPathRows(path string, counts map[string]int) []navRow {
	parts := strings.Split(path, "/")
	var rows []navRow
	prefix := ""
	for i, seg := range parts {
		if i > 0 {
			prefix += "/"
		}
		prefix += seg
		key := "cat:" + prefix
		// skip descendants when an ancestor is folded
		if i > 0 && m.folded("cat:"+strings.Join(parts[:i], "/")) {
			break
		}
		rows = append(rows, navRow{kind: rowCat, cat: prefix, indent: i + 1,
			foldable: m.hasDeeperCats(prefix) || m.catHasFeeds(prefix), foldKey: key,
			labelCount: m.catUnread(prefix, counts)})
		if i == len(parts)-1 && !m.folded(key) {
			for fi, f := range m.feeds {
				if f.Category == prefix {
					rows = append(rows, navRow{kind: rowFeed, feedIdx: fi, indent: i + 2,
						containerKey: key})
				}
			}
		}
	}
	return rows
}

func (m *Model) catHasFeeds(cat string) bool {
	for _, f := range m.feeds {
		if f.Category == cat {
			return true
		}
	}
	return false
}

func (m *Model) hasDeeperCats(cat string) bool {
	for _, f := range m.feeds {
		if strings.HasPrefix(f.Category, cat+"/") {
			return true
		}
	}
	return false
}

// tagRows lists tags (foldable → feeds carrying them).
func (m *Model) tagRows(counts map[string]int) []navRow {
	var rows []navRow
	for _, tag := range m.allTags() {
		key := "tag:" + tag
		rows = append(rows, navRow{kind: rowTag, tag: tag, indent: 1,
			foldable: true, foldKey: key, labelCount: m.tagUnread(tag, counts)})
		if !m.folded(key) {
			for i, f := range m.feeds {
				if hasTag(f, tag) {
					rows = append(rows, navRow{kind: rowFeed, feedIdx: i, indent: 2,
						containerKey: key})
				}
			}
		}
	}
	return rows
}

func (m *Model) allTags() []string {
	seen := map[string]bool{}
	var out []string
	for _, f := range m.feeds {
		for _, t := range f.Tags {
			if !seen[t] {
				seen[t] = true
				out = append(out, t)
			}
		}
	}
	sort.Strings(out)
	return out
}

func hasTag(f feedlist.Feed, tag string) bool {
	for _, t := range f.Tags {
		if t == tag {
			return true
		}
	}
	return false
}

// categorizedUnread sums unread across feeds that have a category.
func (m *Model) categorizedUnread(counts map[string]int) int {
	n := 0
	for _, f := range m.feeds {
		if f.Category != "" {
			n += counts[f.URL]
		}
	}
	return n
}

// taggedUnread sums unread across feeds carrying at least one tag.
func (m *Model) taggedUnread(counts map[string]int) int {
	n := 0
	for _, f := range m.feeds {
		if len(f.Tags) > 0 {
			n += counts[f.URL]
		}
	}
	return n
}

// favUnread sums unread across favourite feeds.
func (m *Model) favUnread(counts map[string]int) int {
	n := 0
	for _, f := range m.feeds {
		if f.Favourite {
			n += counts[f.URL]
		}
	}
	return n
}

// totalOf sums unread across all feeds.
func (m *Model) totalOf(counts map[string]int) int {
	n := 0
	for _, c := range counts {
		n += c
	}
	return n
}

func (m *Model) uncatUnread(counts map[string]int) int {
	n := 0
	for _, f := range m.feeds {
		if f.Category == "" {
			n += counts[f.URL]
		}
	}
	return n
}

func (m *Model) catUnread(cat string, counts map[string]int) int {
	n := 0
	for _, f := range m.feeds {
		if f.Category == cat || strings.HasPrefix(f.Category, cat+"/") {
			n += counts[f.URL]
		}
	}
	return n
}

func (m *Model) tagUnread(tag string, counts map[string]int) int {
	n := 0
	for _, f := range m.feeds {
		if hasTag(f, tag) {
			n += counts[f.URL]
		}
	}
	return n
}

// toggleFavourite flips the favourite flag of the feed under the cursor.
func (m *Model) toggleFavourite() {
	if m.navSel >= len(m.navRows) || m.navRows[m.navSel].kind != rowFeed {
		m.status = "F: favourite a feed row"
		return
	}
	idx := m.navRows[m.navSel].feedIdx
	url := m.feeds[idx].URL
	m.feeds[idx].Favourite = !m.feeds[idx].Favourite
	if err := m.saveFeeds(); err != nil {
		m.status = "urls: " + err.Error()
		return
	}
	m.rebuildNav()
	// rows shift (favourite feed appears in Favourite section) — relocate
	for i, r := range m.navRows {
		if r.kind == rowFeed && m.feeds[r.feedIdx].URL == url {
			m.navSel = i
			break
		}
	}
	m.status = fmt.Sprintf("%s %s", m.displayName(m.feeds[idx]), onOff(m.feeds[idx].Favourite))
}

func onOff(b bool) string {
	if b {
		return "♥ favourite"
	}
	return "♡ unfavourited"
}

// ---------- rendering ----------

func (m *Model) updateNav(k string) {
	switch k {
	case "j", "down":
		if len(m.navRows) > 0 && m.navSel < len(m.navRows)-1 {
			m.navSel++
		}
		m.previewRow()
	case "k", "up":
		if m.navSel > 0 {
			m.navSel--
		}
		m.previewRow()
	case "N":
		m.inputMode = inputAddURL
		m.inputVal, m.inputCur = "", 0
	case "d":
		if m.navSel < len(m.navRows) && m.navRows[m.navSel].kind == rowFeed {
			idx := m.navRows[m.navSel].feedIdx
			u := m.feeds[idx].URL
			if m.pendDel == u {
				m.deleteFeed(idx)
				return
			}
			m.pendDel = u
			m.status = "press d again to delete " + m.displayName(m.feeds[idx])
		}
	case "M":
		m.renameContext()
	case "T":
		if m.navSel < len(m.navRows) && m.navRows[m.navSel].kind == rowFeed {
			m.editFeed = m.feeds[m.navRows[m.navSel].feedIdx].URL
			m.inputMode = inputEditTags
			f := m.feedByURL(m.editFeed)
			tags := ""
			if f != nil {
				tags = strings.Join(f.Tags, " ")
			}
			m.inputVal = tags
			m.inputCur = len([]rune(tags))
		}
	}
}

// renameContext: M on cat → rename category; tag → rename tag; feed → rename title.
func (m *Model) renameContext() {
	if m.navSel >= len(m.navRows) {
		return
	}
	r := m.navRows[m.navSel]
	switch r.kind {
	case rowCat:
		m.renameCat = r.cat
		m.inputMode = inputRenameCat
		m.inputVal = r.cat
		m.inputCur = len([]rune(r.cat))
	case rowTag:
		m.renameTagNam = r.tag
		m.inputMode = inputRenameTag
		m.inputVal = r.tag
		m.inputCur = len([]rune(r.tag))
	case rowFeed:
		f := m.feeds[r.feedIdx]
		m.editFeed = f.URL
		m.inputMode = inputRenameFeed
		m.inputVal = f.Title
		m.inputCur = len([]rune(f.Title))
	}
}

func (m *Model) deleteFeed(idx int) {
	m.pendDel = ""
	u := m.feeds[idx].URL
	name := m.displayName(m.feeds[idx])
	m.feeds = append(m.feeds[:idx], m.feeds[idx+1:]...)
	if err := m.saveFeeds(); err != nil {
		m.status = "urls: " + err.Error()
		return
	}
	m.rebuildNav()
	if m.scope.kind == scopeFeed && m.scope.feedURL == u {
		m.reloadList(scope{kind: scopeUnread})
	}
	m.status = "deleted " + name
}

// previewRow live-previews the hovered row's scope + first item.
func (m *Model) previewRow() {
	if m.navSel < len(m.navRows) {
		r := m.navRows[m.navSel]
		m.reloadList(m.scopeFromRow(r))
		m.previewArticle(m.listSel)
	}
}

// scopeFromRow derives the list scope for a nav row.
func (m *Model) scopeFromRow(r navRow) scope {
	switch r.kind {
	case rowSection:
		return m.sectionScope(r.section)
	case rowCat:
		return scope{kind: scopeCat, cat: r.cat}
	case rowTag:
		return scope{kind: scopeTag, tag: r.tag}
	case rowFeed:
		return scope{kind: scopeFeed, feedURL: m.feeds[r.feedIdx].URL}
	case rowUncat:
		return scope{kind: scopeUncat}
	}
	return scope{kind: scopeUnread}
}

// navView renders the tree with a cursor window.
func (m *Model) navView(w, h int) string {
	if m.fullscreen {
		return paneStyle(w, h, false, "Nav").Render("")
	}
	inner := h - 2
	var all []string
	for i, r := range m.navRows {
		text := m.rowText(r)
		if i == m.navSel {
			text = stySel.Render(text)
		} else if r.kind == rowSection || r.kind == rowUncat {
			text = styHeader.Render(text)
		}
		all = append(all, truncateW(text, w-2))
	}
	if len(m.feeds) == 0 {
		all = append(all, styDim.Render("no feeds — press N"))
	}
	start := 0
	if len(all) > inner {
		start = max(0, min(m.navSel-inner/2, len(all)-inner))
	}
	end := min(start+inner, len(all))
	lines := make([]string, 0, inner)
	lines = append(lines, all[start:end]...)
	if len(lines) < inner {
		lines = append(lines, make([]string, inner-len(lines))...)
	}
	return paneStyle(w, h, m.focus == paneNav, "Nav").Render(strings.Join(lines, "\n"))
}

func (m *Model) rowText(r navRow) string {
	pad := strings.Repeat("  ", r.indent)
	switch r.kind {
	case rowSection:
		if r.foldable {
			mark := "▾"
			if m.folded(r.foldKey) {
				mark = "▸"
			}
			return fmt.Sprintf("%s%s %s (%d)", pad, mark, r.section, r.labelCount)
		}
		return fmt.Sprintf("%s%s (%d)", pad, r.section, r.labelCount)
	case rowCat:
		mark := "▾"
		if m.folded(r.foldKey) {
			mark = "▸"
		}
		seg := r.cat
		if i := strings.LastIndexByte(r.cat, '/'); i >= 0 {
			seg = r.cat[i+1:]
		}
		return fmt.Sprintf("%s%s %s (%d)", pad, mark, seg, r.labelCount)
	case rowTag:
		mark := "▾"
		if m.folded(r.foldKey) {
			mark = "▸"
		}
		return fmt.Sprintf("%s%s #%s (%d)", pad, mark, r.tag, r.labelCount)
	case rowUncat:
		mark := "▾"
		if m.folded(r.foldKey) {
			mark = "▸"
		}
		return fmt.Sprintf("%s%s No Category (%d)", pad, mark, r.labelCount)
	case rowFeed:
		f := m.feeds[r.feedIdx]
		return fmt.Sprintf("%s%s (%d)", pad, m.displayName(f), m.feedUnread(f.URL))
	}
	return ""
}

func (m *Model) feedUnread(url string) int {
	counts, err := m.store.UnreadCounts([]string{url})
	if err != nil {
		return 0
	}
	return counts[url]
}
