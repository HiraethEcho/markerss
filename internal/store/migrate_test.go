package store

import (
	"database/sql"
	"path/filepath"
	"testing"

	_ "modernc.org/sqlite"
)

// old-schema DB (no body column) must migrate on Open.
func TestMigrateAddsBodyColumn(t *testing.T) {
	p := filepath.Join(t.TempDir(), "old.db")
	db, err := sql.Open("sqlite", "file:"+p)
	if err != nil {
		t.Fatal(err)
	}
	_, err = db.Exec(`CREATE TABLE items (
		feed_url TEXT NOT NULL, guid TEXT NOT NULL, title TEXT NOT NULL DEFAULT '',
		link TEXT NOT NULL DEFAULT '', published INTEGER NOT NULL DEFAULT 0,
		summary TEXT NOT NULL DEFAULT '', content TEXT NOT NULL DEFAULT '',
		content_at INTEGER NOT NULL DEFAULT 0, read INTEGER NOT NULL DEFAULT 0,
		PRIMARY KEY (feed_url, guid))`)
	if err != nil {
		t.Fatal(err)
	}
	db.Close()

	s, err := Open(p)
	if err != nil {
		t.Fatal(err)
	}
	defer s.Close()
	it := Item{FeedURL: "u", GUID: "g", Title: "T", Summary: "s", Body: "b"}
	if err := s.UpsertItem(it); err != nil {
		t.Fatal(err)
	}
	got, err := s.Get("u", "g")
	if err != nil || got.Body != "b" {
		t.Errorf("body after migrate: %+v err=%v", got, err)
	}
}
