package store

import (
	"path/filepath"
	"testing"
	"time"
)

func TestOpenAndUpsert(t *testing.T) {
	s, err := Open(filepath.Join(t.TempDir(), "m.db"))
	if err != nil {
		t.Fatal(err)
	}
	defer s.Close()

	it := Item{
		FeedURL: "https://f.test/rss", GUID: "g1", Title: "T1",
		Link: "https://f.test/1", Published: time.Now(), Summary: "<p>s</p>",
	}
	if err := s.UpsertItem(it); err != nil {
		t.Fatal(err)
	}
	if err := s.UpsertItem(it); err != nil { // idempotent
		t.Fatal(err)
	}
	got, err := s.Get(it.FeedURL, it.GUID)
	if err != nil {
		t.Fatal(err)
	}
	if got.Title != "T1" || got.Read {
		t.Errorf("unexpected: %+v", got)
	}
}

func TestPreserveReadAndContentOnRefresh(t *testing.T) {
	s, err := Open(filepath.Join(t.TempDir(), "m.db"))
	if err != nil {
		t.Fatal(err)
	}
	defer s.Close()

	it := Item{FeedURL: "u", GUID: "g", Title: "old"}
	if err := s.UpsertItem(it); err != nil {
		t.Fatal(err)
	}
	if err := s.MarkRead("u", "g", true); err != nil {
		t.Fatal(err)
	}
	if err := s.SaveContent("u", "g", "<article>html</article>"); err != nil {
		t.Fatal(err)
	}
	// refresh with new title
	if err := s.UpsertItem(Item{FeedURL: "u", GUID: "g", Title: "new"}); err != nil {
		t.Fatal(err)
	}
	got, _ := s.Get("u", "g")
	if got.Title != "new" {
		t.Errorf("title should update: %q", got.Title)
	}
	if !got.Read {
		t.Error("read flag must survive refresh")
	}
	if got.Content != "<article>html</article>" {
		t.Error("content must survive refresh")
	}
}

func TestItemsOrderingAndFilter(t *testing.T) {
	s, _ := Open(filepath.Join(t.TempDir(), "m.db"))
	defer s.Close()
	old := time.Now().Add(-time.Hour)
	now := time.Now()
	s.UpsertItem(Item{FeedURL: "u1", GUID: "a", Published: old})
	s.UpsertItem(Item{FeedURL: "u1", GUID: "b", Published: now})
	s.UpsertItem(Item{FeedURL: "u2", GUID: "c", Published: now.Add(-time.Second)})

	items, err := s.Items([]string{"u1", "u2"}, true, false)
	if err != nil {
		t.Fatal(err)
	}
	if len(items) != 3 {
		t.Fatalf("want 3 items, got %d", len(items))
	}
	if items[0].GUID != "b" {
		t.Errorf("newest first: got %s", items[0].GUID)
	}

	only, _ := s.Items([]string{"u2"}, false, false)
	if len(only) != 1 || only[0].GUID != "c" {
		t.Errorf("feed filter broken: %+v", only)
	}

	s.MarkRead("u1", "a", true)
	unread, _ := s.Items([]string{"u1", "u2"}, true, false)
	if unread[0].Read {
		t.Error("unread should sort first")
	}
}

func TestUnreadCountsAndMarkAll(t *testing.T) {
	s, _ := Open(filepath.Join(t.TempDir(), "m.db"))
	defer s.Close()
	s.UpsertItem(Item{FeedURL: "u1", GUID: "a"})
	s.UpsertItem(Item{FeedURL: "u1", GUID: "b"})
	s.UpsertItem(Item{FeedURL: "u2", GUID: "c"})

	counts, err := s.UnreadCounts([]string{"u1", "u2"})
	if err != nil {
		t.Fatal(err)
	}
	if counts["u1"] != 2 || counts["u2"] != 1 {
		t.Errorf("counts wrong: %+v", counts)
	}
	if err := s.MarkAllRead([]string{"u1"}); err != nil {
		t.Fatal(err)
	}
	counts, _ = s.UnreadCounts([]string{"u1", "u2"})
	if counts["u1"] != 0 || counts["u2"] != 1 {
		t.Errorf("after mark-all wrong: %+v", counts)
	}
}

func TestPurgeExpired(t *testing.T) {
	s, _ := Open(filepath.Join(t.TempDir(), "m.db"))
	defer s.Close()
	s.UpsertItem(Item{FeedURL: "u", GUID: "a"})
	s.SaveContent("u", "a", "x")
	// content_at is now; purge with negative cutoff must clear
	if err := s.PurgeExpired(1); err != nil {
		t.Fatal(err)
	}
	// freshly saved → not expired
	got, _ := s.Get("u", "a")
	if got.Content != "x" {
		t.Errorf("fresh content should survive: %q", got.Content)
	}
}

func TestPruneFeed(t *testing.T) {
	s, _ := Open(filepath.Join(t.TempDir(), "m.db"))
	defer s.Close()
	now := time.Now()
	for i := 0; i < 5; i++ {
		s.UpsertItem(Item{FeedURL: "u", GUID: itoa(i), Published: now.Add(time.Duration(i) * time.Minute)})
	}
	s.UpsertItem(Item{FeedURL: "other", GUID: "keep"})
	if err := s.PruneFeed("u", 3); err != nil {
		t.Fatal(err)
	}
	items, _ := s.Items([]string{"u"}, false, false)
	if len(items) != 3 {
		t.Fatalf("want 3 kept, got %d", len(items))
	}
	// newest three survive
	want := map[string]bool{"2": true, "3": true, "4": true}
	for _, it := range items {
		if !want[it.GUID] {
			t.Errorf("unexpected kept guid %s", it.GUID)
		}
	}
	other, _ := s.Items([]string{"other"}, false, false)
	if len(other) != 1 {
		t.Error("other feed must be untouched")
	}
}

func itoa(i int) string {
	if i == 0 {
		return "0"
	}
	b := []byte{}
	for i > 0 {
		b = append([]byte{byte('0' + i%10)}, b...)
		i /= 10
	}
	return string(b)
}
