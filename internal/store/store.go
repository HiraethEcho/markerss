// Package store persists feed items in SQLite: read flags, fetched
// content, and summaries. Items are keyed (feed_url, guid); read state
// and fetched content survive refresh.
package store

import (
	"database/sql"
	"os"
	"path/filepath"
	"strings"
	"time"

	_ "modernc.org/sqlite"
)

// Item is one feed entry as stored.
type Item struct {
	FeedURL   string
	GUID      string
	Title     string
	Link      string
	Published time.Time
	Summary   string // raw HTML — RSS <description> (refresh-only)
	Body      string // raw HTML — RSS content:encoded / Atom <content>
	Content   string // raw HTML, full article, fetched on demand
	ContentAt time.Time
	Read      bool
}

// Store wraps the SQLite handle.
type Store struct {
	db *sql.DB
}

// Open opens (creating if needed) the DB at path and migrates the schema.
func Open(path string) (*Store, error) {
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return nil, err
	}
	db, err := sql.Open("sqlite", "file:"+path+
		"?_pragma=journal_mode(WAL)&_pragma=busy_timeout(5000)")
	if err != nil {
		return nil, err
	}
	s := &Store{db: db}
	if err := s.migrate(); err != nil {
		db.Close()
		return nil, err
	}
	return s, nil
}

func (s *Store) migrate() error {
	_, err := s.db.Exec(`
CREATE TABLE IF NOT EXISTS items (
	feed_url  TEXT NOT NULL,
	guid      TEXT NOT NULL,
	title     TEXT NOT NULL DEFAULT '',
	link      TEXT NOT NULL DEFAULT '',
	published INTEGER NOT NULL DEFAULT 0,
	summary   TEXT NOT NULL DEFAULT '',
	body      TEXT NOT NULL DEFAULT '',
	content   TEXT NOT NULL DEFAULT '',
	content_at INTEGER NOT NULL DEFAULT 0,
	read      INTEGER NOT NULL DEFAULT 0,
	PRIMARY KEY (feed_url, guid)
);
CREATE INDEX IF NOT EXISTS idx_items_pub ON items(published);
CREATE INDEX IF NOT EXISTS idx_items_read ON items(read);`)
	if err != nil {
		return err
	}
	// migrate old DBs: add body column if missing
	s.db.Exec(`ALTER TABLE items ADD COLUMN body TEXT NOT NULL DEFAULT ''`)
	return nil
}

// Close closes the underlying DB.
func (s *Store) Close() error { return s.db.Close() }

// UpsertItem inserts or updates item metadata. Read state and fetched
// content are preserved on conflict (refresh-only semantics).
func (s *Store) UpsertItem(it Item) error {
	_, err := s.db.Exec(`
INSERT INTO items (feed_url, guid, title, link, published, summary, body)
VALUES (?, ?, ?, ?, ?, ?, ?)
ON CONFLICT(feed_url, guid) DO UPDATE SET
	title = excluded.title,
	link = excluded.link,
	published = excluded.published,
	summary = excluded.summary,
	body = excluded.body`,
		it.FeedURL, it.GUID, it.Title, it.Link, ts(it.Published), it.Summary, it.Body)
	return err
}

// Get returns one item by key, or nil when absent.
func (s *Store) Get(feedURL, guid string) (*Item, error) {
	row := s.db.QueryRow(`
SELECT feed_url, guid, title, link, published, summary, body, content, content_at, read
FROM items WHERE feed_url = ? AND guid = ?`, feedURL, guid)
	return scanItem(row)
}

// Items lists items from the given feeds. unreadFirst puts unread on top
// (All Unread view); unreadOnly filters to unread.
func (s *Store) Items(feedURLs []string, unreadFirst, unreadOnly bool) ([]Item, error) {
	if len(feedURLs) == 0 {
		return nil, nil
	}
	q := `SELECT feed_url, guid, title, link, published, summary, body, content, content_at, read
FROM items WHERE feed_url IN (` + placeholders(len(feedURLs)) + `)`
	args := urlsToAny(feedURLs)
	if unreadOnly {
		q += " AND read = 0"
	}
	q += " ORDER BY read ASC, published DESC, rowid DESC"
	if !unreadFirst {
		q = strings.Replace(q, "read ASC, ", "", 1)
	}
	rows, err := s.db.Query(q, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var items []Item
	for rows.Next() {
		it, err := scanItem(rows)
		if err != nil {
			return nil, err
		}
		items = append(items, *it)
	}
	return items, rows.Err()
}

type scanner interface {
	Scan(dest ...any) error
}

func scanItem(s scanner) (*Item, error) {
	var it Item
	var pub, cat int64
	err := s.Scan(&it.FeedURL, &it.GUID, &it.Title, &it.Link, &pub,
		&it.Summary, &it.Body, &it.Content, &cat, &it.Read)
	if err != nil {
		return nil, err
	}
	it.Published = time.Unix(pub, 0)
	it.ContentAt = time.Unix(cat, 0)
	return &it, nil
}

// UnreadCounts returns unread item count per feed URL.
func (s *Store) UnreadCounts(feedURLs []string) (map[string]int, error) {
	out := map[string]int{}
	if len(feedURLs) == 0 {
		return out, nil
	}
	rows, err := s.db.Query(
		`SELECT feed_url, COUNT(*) FROM items WHERE feed_url IN (`+
			placeholders(len(feedURLs))+`) AND read = 0 GROUP BY feed_url`,
		urlsToAny(feedURLs)...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	for rows.Next() {
		var u string
		var n int
		if err := rows.Scan(&u, &n); err != nil {
			return nil, err
		}
		out[u] = n
	}
	return out, rows.Err()
}

// MarkRead sets the read flag for one item.
func (s *Store) MarkRead(feedURL, guid string, read bool) error {
	_, err := s.db.Exec(`UPDATE items SET read = ? WHERE feed_url = ? AND guid = ?`,
		boolInt(read), feedURL, guid)
	return err
}

// MarkAllRead marks every item of the given feeds read.
func (s *Store) MarkAllRead(feedURLs []string) error {
	if len(feedURLs) == 0 {
		return nil
	}
	_, err := s.db.Exec(`UPDATE items SET read = 1 WHERE feed_url IN (`+
		placeholders(len(feedURLs))+`)`, urlsToAny(feedURLs)...)
	return err
}

// SaveContent stores fetched full-article HTML for one item.
func (s *Store) SaveContent(feedURL, guid, html string) error {
	_, err := s.db.Exec(
		`UPDATE items SET content = ?, content_at = ? WHERE feed_url = ? AND guid = ?`,
		html, time.Now().Unix(), feedURL, guid)
	return err
}

// PurgeExpired clears fetched content older than days (TTL cleanup).
func (s *Store) PurgeExpired(days int) error {
	if days <= 0 {
		return nil
	}
	cutoff := time.Now().Add(-time.Duration(days) * 24 * time.Hour).Unix()
	_, err := s.db.Exec(
		`UPDATE items SET content = '', content_at = 0 WHERE content != '' AND content_at < ?`,
		cutoff)
	return err
}

// PruneFeed deletes items of a feed beyond the newest maxN (by
// published, then rowid). maxN <= 0 keeps everything.
func (s *Store) PruneFeed(feedURL string, maxN int) error {
	if maxN <= 0 {
		return nil
	}
	_, err := s.db.Exec(`DELETE FROM items WHERE feed_url = ? AND rowid NOT IN (
		SELECT rowid FROM items WHERE feed_url = ?
		ORDER BY published DESC, rowid DESC LIMIT ?
	)`, feedURL, feedURL, maxN)
	return err
}

func placeholders(n int) string {
	return strings.TrimSuffix(strings.Repeat("?,", n), ",")
}

func urlsToAny(urls []string) []any {
	out := make([]any, len(urls))
	for i, u := range urls {
		out[i] = u
	}
	return out
}

func ts(t time.Time) int64 {
	if t.IsZero() {
		return 0
	}
	return t.Unix()
}

func boolInt(b bool) int {
	if b {
		return 1
	}
	return 0
}
