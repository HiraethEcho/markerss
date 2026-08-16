package ui

import (
	"strings"
	"testing"
)

func TestEmptyScopeBlankArticle(t *testing.T) {
	m := refresh(t, newTestModel(t))
	// open first item → article populated
	m.navSel = m.findRow(func(r navRow) bool { return r.kind == rowSection && r.section == "Unread" })
	m.updateKeys(key("enter"))
	m.updateKeys(key("enter"))
	if len(m.artMD) == 0 {
		t.Fatal("article should be populated")
	}
	m.updateKeys(key("q")) // article → list
	m.updateKeys(key("q")) // list → nav
	// Saved scope → empty list → blank article (no stale content, no placeholder)
	m.navSel = m.findRow(func(r navRow) bool { return r.kind == rowSection && r.section == "Saved" })
	m.updateKeys(key("enter"))
	if len(m.items) != 0 {
		t.Fatalf("Saved should be empty, got %d", len(m.items))
	}
	if m.artItem != nil || m.artMD != nil {
		t.Error("stale article must be cleared on empty scope")
	}
	v := m.articleView(60, 30)
	if strings.Contains(v, "select an item") || strings.Contains(v, "First Post") {
		t.Errorf("article pane should be blank: %q", v)
	}
	if !strings.Contains(v, "╭") {
		t.Error("blank pane must still show border")
	}
	// Read Later too
	m.navSel = m.findRow(func(r navRow) bool { return r.kind == rowSection && r.section == "Read Later" })
	m.updateKeys(key("enter"))
	if m.artItem != nil {
		t.Error("Read Later empty scope must clear article")
	}
}
