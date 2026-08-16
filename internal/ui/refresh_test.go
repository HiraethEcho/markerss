package ui

import (
	"testing"
)

func TestRefreshDropsReadFromUnread(t *testing.T) {
	m := refresh(t, newTestModel(t)) // 2 unread items
	m.navSel = m.findRow(func(r navRow) bool { return r.kind == rowSection && r.section == "Unread" })
	m.updateKeys(key("enter")) // list
	// mark first read
	m.updateKeys(key("a"))
	if m.items[0].Read != true {
		t.Fatal("a should mark read")
	}
	if len(m.items) != 2 {
		t.Fatalf("read item stays until refresh: %d", len(m.items))
	}
	// partial refresh (r) → unread list drops the read item
	cmd := m.startRefresh(false)
	for _, msg := range execCmd(cmd) {
		m.Update(msg)
	}
	if len(m.items) != 1 {
		t.Errorf("after r, unread list should have 1 item, got %d", len(m.items))
	}
	if m.items[0].Read {
		t.Errorf("remaining item should be unread")
	}
}
