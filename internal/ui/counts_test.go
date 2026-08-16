package ui

import (
	"testing"

	"markerss/internal/feedlist"
)

func TestSectionCountsConsistent(t *testing.T) {
	m := refresh(t, newTestModel(t))
	m.feeds = append(m.feeds, feedlist.Feed{URL: "https://nocat.test/rss", Title: "Uncat"})
	m.rebuildNav()
	counts := map[string]int{}
	// mark one feed read to vary counts
	st := m.store
	if err := st.MarkRead(m.feeds[0].URL, "g1", true); err != nil {
		t.Fatal(err)
	}
	m.rebuildNav()
	for _, sec := range []string{"Unread", "Categories", "Tags", "Favourite", "No Category"} {
		idx := m.findRow(func(r navRow) bool { return r.kind == rowSection && r.section == sec })
		if idx < 0 {
			t.Errorf("section %s missing", sec)
			continue
		}
		t.Logf("%s count=%d", sec, m.navRows[idx].labelCount)
	}
	_ = counts
	// consistency: Unread = Categories + No Category (all feeds categorized or not)
	var unread, cat, uncat int
	for _, r := range m.navRows {
		if r.kind == rowSection {
			switch r.section {
			case "Unread":
				unread = r.labelCount
			case "Categories":
				cat = r.labelCount
			case "No Category":
				uncat = r.labelCount
			}
		}
	}
	if cat+uncat != unread {
		t.Errorf("Unread(%d) != Categories(%d)+NoCategory(%d)", unread, cat, uncat)
	}
}
