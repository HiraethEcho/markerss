package ui

import (
	"testing"

	"markerss/internal/feedlist"
)

func TestFoldStayAndNoCategory(t *testing.T) {
	m := newTestModel(t)
	m.feeds = append(m.feeds, feedlist.Feed{URL: "https://nocat.test/rss", Title: "NoCat Feed"})
	m.rebuildNav()
	// find No Category section row
	nc := m.findRow(func(r navRow) bool { return r.kind == rowSection && r.section == "No Category" })
	ci := m.findRow(func(r navRow) bool { return r.kind == rowSection && r.section == "Categories" })
	if nc < 0 || ci < 0 {
		t.Fatalf("No Category / Categories missing: nc=%d ci=%d", nc, ci)
	}
	if nc <= ci {
		t.Errorf("No Category must come after Categories subtree: ci=%d nc=%d", ci, nc)
	}
	r := m.navRows[nc]
	if r.indent != 0 || !r.foldable || r.kind != rowSection {
		t.Errorf("No Category must be a top-level foldable section: %+v", r)
	}
	// fold Categories → No Category is the very next row
	m.navSel = ci
	m.updateKeys(key("h"))
	ci2 := m.findRow(func(rr navRow) bool { return rr.kind == rowSection && rr.section == "Categories" })
	nc2 := m.findRow(func(rr navRow) bool { return rr.kind == rowSection && rr.section == "No Category" })
	if nc2 != ci2+1 {
		t.Errorf("with Categories folded, No Category should be next row: ci=%d nc=%d", ci2, nc2)
	}
	// fold everything: Categories, No Category, then h on folded Categories stays
	m.navSel = ci
	m.updateKeys(key("h"))
	if !m.sectionFolded["Categories"] {
		t.Error("h folds Categories")
	}
	m.updateKeys(key("h")) // folded top → stays
	if m.navSel != ci {
		t.Errorf("h on folded top section must stay, moved to %d", m.navSel)
	}
	// h on No Category folds it; folded top section stays on further h
	nc = m.findRow(func(rr navRow) bool { return rr.kind == rowSection && rr.section == "No Category" })
	m.navSel = nc
	m.updateKeys(key("h"))
	if !m.sectionFolded["No Category"] {
		t.Error("h folds No Category")
	}
	nc = m.findRow(func(rr navRow) bool { return rr.kind == rowSection && rr.section == "No Category" })
	m.navSel = nc
	m.updateKeys(key("h"))
	if m.navSel != nc {
		t.Errorf("h on folded No Category must stay, moved to %d", m.navSel)
	}
	// expand No Category → first child is a feed
	m.updateKeys(key("l"))
	if m.sectionFolded["No Category"] {
		t.Error("l expands No Category")
	}
	if m.navRows[m.navSel].kind != rowFeed {
		t.Errorf("expand should jump to first feed, got %+v", m.navRows[m.navSel])
	}
}
