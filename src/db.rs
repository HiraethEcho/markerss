//! SQLite storage — items + read state (rusqlite, bundled).
//!
//! Subscriptions stay in the `urls` file (newsboat format, live-editable);
//! the DB holds feed items and their content. Located at
//! `$XDG_STATE_HOME/markerss/markerss.db`.

use std::path::Path;

use rusqlite::Connection;

use crate::model::Item;

pub struct Db {
    conn: Connection,
}

impl Db {
    pub fn open(path: &Path) -> rusqlite::Result<Db> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).ok();
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS items (
                feed_url TEXT NOT NULL,
                guid     TEXT NOT NULL,
                title    TEXT NOT NULL DEFAULT '',
                url      TEXT NOT NULL DEFAULT '',
                summary  TEXT NOT NULL DEFAULT '',
                content  TEXT NOT NULL DEFAULT '',
                date     TEXT NOT NULL DEFAULT '',
                author   TEXT NOT NULL DEFAULT '',
                read     INTEGER NOT NULL DEFAULT 0,
                read_later INTEGER NOT NULL DEFAULT 0,
                saved    INTEGER NOT NULL DEFAULT 0,
                fetched_at TEXT NOT NULL DEFAULT '',
                PRIMARY KEY (feed_url, guid)
            );
            CREATE INDEX IF NOT EXISTS idx_items_feed ON items(feed_url, read);
            ",
        )?;
        // migrate older DBs (add flag columns if missing)
        for (col, def) in [
            ("read_later", "INTEGER NOT NULL DEFAULT 0"),
            ("saved", "INTEGER NOT NULL DEFAULT 0"),
            ("author", "TEXT NOT NULL DEFAULT ''"),
        ] {
            let has: bool = conn
                .prepare("SELECT 1 FROM pragma_table_info('items') WHERE name = ?1")?
                .query_row([col], |_| Ok(true))
                .unwrap_or(false);
            if !has {
                conn.execute(&format!("ALTER TABLE items ADD COLUMN {col} {def}"), [])?;
            }
        }
        // flag indexes after migration (old DBs lack the columns)
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_items_later ON items(read_later);
             CREATE INDEX IF NOT EXISTS idx_items_saved ON items(saved);",
        )?;
        Ok(Db { conn })
    }

    /// Fetch-mode upsert: insert new items, update metadata of existing ones,
    /// preserve read/content/flags. Returns the guids of newly added items.
    pub fn upsert_fetch(&mut self, feed_url: &str, items: &[Item]) -> rusqlite::Result<Vec<String>> {
        let mut added = Vec::new();
        let tx = self.conn.transaction()?;
        {
            let mut ins = tx.prepare(
                "INSERT OR IGNORE INTO items
                 (feed_url, guid, title, url, summary, content, date, author, read, read_later, saved, fetched_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, 0, 0, ?9)",
            )?;
            let mut upd = tx.prepare(
                "UPDATE items SET title = ?3, url = ?4, summary = ?5, date = ?6, author = ?7,
                        content = CASE WHEN content = '' THEN ?8 ELSE content END,
                        fetched_at = CASE WHEN content = '' THEN ?9 ELSE fetched_at END
                 WHERE feed_url = ?1 AND guid = ?2",
            )?;
            let now = chrono::Utc::now().to_rfc3339();
            for i in items {
                let n = ins.execute(rusqlite::params![
                    feed_url,
                    i.guid,
                    i.title,
                    i.url,
                    i.summary,
                    i.content,
                    i.date,
                    i.author,
                    now,
                ])?;
                if n > 0 {
                    added.push(i.guid.clone());
                }
                upd.execute(rusqlite::params![
                    feed_url, i.guid, i.title, i.url, i.summary, i.date, i.author, i.content, now
                ])?;
            }
        }
        tx.commit()?;
        Ok(added)
    }

    /// Keep `guid`-keyed read flags across a refresh (delete+insert would lose them).
    pub fn replace_feed_items_preserving_read(
        &mut self,
        feed_url: &str,
        items: &[Item],
    ) -> rusqlite::Result<()> {
        let tx = self.conn.transaction()?;
        {
            // capture existing read flags + fetched content BEFORE deleting
            let mut read_guids: std::collections::HashSet<String> = Default::default();
            let mut content_map: std::collections::HashMap<String, String> = Default::default();
            let mut fetched_map: std::collections::HashMap<String, String> = Default::default();
            let mut later_map: std::collections::HashSet<String> = Default::default();
            let mut saved_map: std::collections::HashSet<String> = Default::default();
            // full rows of flagged items — kept even when the feed drops them
            let mut keep_rows: Vec<Item> = Vec::new();
            {
                let mut q = tx.prepare(
                    "SELECT guid, title, url, summary, content, date, author, read, read_later, saved, fetched_at FROM items WHERE feed_url = ?1",
                )?;
                let rows = q.query_map([feed_url], |r| {
                    Ok((
                        r.get::<_, String>(0)?, // guid
                        r.get::<_, String>(1)?, // title
                        r.get::<_, String>(2)?, // url
                        r.get::<_, String>(3)?, // summary
                        r.get::<_, String>(4)?, // content
                        r.get::<_, String>(5)?, // date
                        r.get::<_, String>(6)?, // author
                        r.get::<_, i64>(7)?,   // read
                        r.get::<_, i64>(8)?,   // read_later
                        r.get::<_, i64>(9)?,   // saved
                        r.get::<_, String>(10)?, // fetched_at
                    ))
                })?;
                for row in rows.flatten() {
                    if row.7 != 0 {
                        read_guids.insert(row.0.clone());
                    }
                    if !row.4.is_empty() {
                        content_map.insert(row.0.clone(), row.4.clone());
                        fetched_map.insert(row.0.clone(), row.10);
                    }
                    if row.8 != 0 {
                        later_map.insert(row.0.clone());
                    }
                    if row.9 != 0 {
                        saved_map.insert(row.0.clone());
                    }
                    if row.8 != 0 || row.9 != 0 {
                        keep_rows.push(Item {
                            guid: row.0,
                            title: row.1,
                            url: row.2,
                            summary: row.3,
                            content: row.4,
                            date: row.5,
                            author: row.6,
                            read_later: row.8 != 0,
                            saved: row.9 != 0,
                        });
                    }
                }
            }
            let mut del = tx.prepare("DELETE FROM items WHERE feed_url = ?1")?;
            del.execute([feed_url])?;
            let now = chrono::Utc::now().to_rfc3339();
            let mut ins = tx.prepare(
                "INSERT OR REPLACE INTO items
                 (feed_url, guid, title, url, summary, content, date, author, read, read_later, saved, fetched_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            )?;
            for i in items {
                let read = if read_guids.contains(&i.guid) { 1 } else { 0 };
                let later = if later_map.contains(&i.guid) { 1 } else { 0 };
                let saved = if saved_map.contains(&i.guid) { 1 } else { 0 };
                // keep previously fetched content; otherwise use the feed content
                let content = content_map
                    .get(&i.guid)
                    .cloned()
                    .unwrap_or_else(|| i.content.clone());
                let fetched_at = if content_map.contains_key(&i.guid) {
                    fetched_map.get(&i.guid).cloned().unwrap_or_else(|| now.clone())
                } else {
                    now.clone()
                };
                ins.execute(rusqlite::params![
                    feed_url,
                    i.guid,
                    i.title,
                    i.url,
                    i.summary,
                    content,
                    i.date,
                    i.author,
                    read,
                    later,
                    saved,
                    fetched_at,
                ])?;
            }
            // re-insert flagged items the feed no longer carries (saved /
            // read-later must survive a full refresh)
            for k in &keep_rows {
                if items.iter().any(|i| i.guid == k.guid) {
                    continue;
                }
                ins.execute(rusqlite::params![
                    feed_url,
                    k.guid,
                    k.title,
                    k.url,
                    k.summary,
                    k.content,
                    k.date,
                    k.author,
                    1,
                    k.read_later as i64,
                    k.saved as i64,
                    now,
                ])?;
            }
        }
        tx.commit()
    }

    /// Update an item's content (fetched full article) — preserves read flag.
    pub fn update_item_content(&mut self, feed_url: &str, guid: &str, content: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE items SET content = ?3, fetched_at = ?4 WHERE feed_url = ?1 AND guid = ?2",
            rusqlite::params![feed_url, guid, content, chrono::Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn items_for_feed(&self, feed_url: &str) -> rusqlite::Result<Vec<Item>> {
        let mut stmt = self.conn.prepare(
            "SELECT guid, title, url, summary, content, date, author, read_later, saved
             FROM items WHERE feed_url = ?1",
        )?;
        let rows = stmt.query_map([feed_url], |r| {
            Ok(Item {
                guid: r.get(0)?,
                title: r.get(1)?,
                url: r.get(2)?,
                summary: r.get(3)?,
                content: r.get(4)?,
                date: r.get(5)?,
                author: r.get(6)?,
                read_later: r.get::<_, i64>(7)? != 0,
                saved: r.get::<_, i64>(8)? != 0,
            })
        })?;
        rows.collect()
    }

    /// All (feed_url, item) pairs with a flag set — for virtual nodes.
    pub fn items_with_flag(&self, flag: &str) -> rusqlite::Result<Vec<(String, Item)>> {
        let col = match flag {
            "read_later" => "read_later",
            "saved" => "saved",
            _ => return Ok(Vec::new()),
        };
        let sql = format!(
            "SELECT feed_url, guid, title, url, summary, content, date, author, read_later, saved
             FROM items WHERE {col} = 1"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get(0)?,
                Item {
                    guid: r.get(1)?,
                    title: r.get(2)?,
                    url: r.get(3)?,
                    summary: r.get(4)?,
                    content: r.get(5)?,
                    date: r.get(6)?,
                    author: r.get(7)?,
                    read_later: r.get::<_, i64>(8)? != 0,
                    saved: r.get::<_, i64>(9)? != 0,
                },
            ))
        })?;
        rows.collect()
    }

    pub fn set_flag(&mut self, feed_url: &str, guid: &str, flag: &str, on: bool) -> rusqlite::Result<()> {
        let col = match flag {
            "read_later" | "saved" => flag,
            _ => return Ok(()),
        };
        let sql = format!("UPDATE items SET {col} = ?3 WHERE feed_url = ?1 AND guid = ?2");
        self.conn.execute(&sql, rusqlite::params![feed_url, guid, on as i64])?;
        Ok(())
    }

    pub fn toggle_flag(&mut self, feed_url: &str, guid: &str, flag: &str) -> rusqlite::Result<bool> {
        let col = match flag {
            "read_later" | "saved" => flag,
            _ => return Ok(false),
        };
        let sql = format!("SELECT {col} FROM items WHERE feed_url = ?1 AND guid = ?2");
        let cur: Option<i64> = self
            .conn
            .query_row(&sql, rusqlite::params![feed_url, guid], |r| r.get(0))
            .ok();
        let Some(cur) = cur else {
            return Ok(false); // row gone — nothing to toggle
        };
        self.set_flag(feed_url, guid, flag, cur == 0)?;
        Ok(cur == 0)
    }

    pub fn is_read(&self, feed_url: &str, guid: &str) -> rusqlite::Result<bool> {
        let n: i64 = self.conn.query_row(
            "SELECT read FROM items WHERE feed_url = ?1 AND guid = ?2",
            rusqlite::params![feed_url, guid],
            |r| r.get(0),
        )?;
        Ok(n != 0)
    }

    pub fn set_read(&mut self, feed_url: &str, guid: &str, read: bool) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE items SET read = ?3 WHERE feed_url = ?1 AND guid = ?2",
            rusqlite::params![feed_url, guid, read as i64],
        )?;
        Ok(())
    }

    pub fn toggle_read(&mut self, feed_url: &str, guid: &str) -> rusqlite::Result<bool> {
        let read = self.is_read(feed_url, guid)?;
        self.set_read(feed_url, guid, !read)?;
        Ok(!read)
    }

    pub fn mark_all_read(&mut self, feed_url: &str) -> rusqlite::Result<()> {
        self.conn
            .execute("UPDATE items SET read = 1 WHERE feed_url = ?1", [feed_url])?;
        Ok(())
    }

    pub fn unread_count(&self, feed_url: &str) -> rusqlite::Result<usize> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM items WHERE feed_url = ?1 AND read = 0",
            [feed_url],
            |r| r.get(0),
        )?;
        Ok(n as usize)
    }

    pub fn total_unread(&self) -> rusqlite::Result<usize> {
        let n: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM items WHERE read = 0", [], |r| r.get(0))?;
        Ok(n as usize)
    }

    /// Purge fetched content older than ttl_days (keep item metadata).
    /// Delete all items of a removed feed.
    pub fn remove_feed_items(&mut self, feed_url: &str) -> rusqlite::Result<()> {
        self.conn
            .execute("DELETE FROM items WHERE feed_url = ?1", [feed_url])?;
        Ok(())
    }

    pub fn cleanup_content(&mut self, ttl_days: u64) -> rusqlite::Result<()> {
        if ttl_days == 0 {
            return Ok(());
        }
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(ttl_days as i64))
            .to_rfc3339();
        self.conn.execute(
            "UPDATE items SET content = '' WHERE fetched_at != '' AND fetched_at < ?1 AND saved = 0",
            [cutoff],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Db {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "markerss-db-{}-{}.db",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_file(&path);
        Db::open(&path).unwrap()
    }

    fn item(guid: &str) -> Item {
        Item {
            guid: guid.into(),
            title: format!("t-{guid}"),
            url: format!("https://x.com/{guid}"),
            summary: String::new(),
            content: String::new(),
            date: String::new(),
            author: String::new(),
            read_later: false,
            saved: false,
        }
    }

    #[test]
    fn replace_and_read_back() {
        let mut db = test_db();
        db.replace_feed_items_preserving_read("https://f.com", &[item("a"), item("b")]).unwrap();
        let items = db.items_for_feed("https://f.com").unwrap();
        assert_eq!(items.len(), 2);
        assert!(db.unread_count("https://f.com").unwrap() == 2);
    }

    #[test]
    fn refresh_preserves_read() {
        let mut db = test_db();
        db.replace_feed_items_preserving_read("https://f.com", &[item("a")]).unwrap();
        db.set_read("https://f.com", "a", true).unwrap();
        db.replace_feed_items_preserving_read("https://f.com", &[item("a"), item("b")]).unwrap();
        assert!(db.is_read("https://f.com", "a").unwrap());
        assert!(!db.is_read("https://f.com", "b").unwrap());
    }

    #[test]
    fn toggle_and_mark_all() {
        let mut db = test_db();
        db.replace_feed_items_preserving_read("https://f.com", &[item("a"), item("b")]).unwrap();
        assert!(db.toggle_read("https://f.com", "a").unwrap());
        assert!(!db.toggle_read("https://f.com", "a").unwrap());
        db.mark_all_read("https://f.com").unwrap();
        assert_eq!(db.unread_count("https://f.com").unwrap(), 0);
        assert_eq!(db.total_unread().unwrap(), 0);
    }

    #[test]
    fn update_content_keeps_read() {
        let mut db = test_db();
        db.replace_feed_items_preserving_read("https://f.com", &[item("a")]).unwrap();
        db.set_read("https://f.com", "a", true).unwrap();
        db.update_item_content("https://f.com", "a", "<p>full</p>").unwrap();
        let items = db.items_for_feed("https://f.com").unwrap();
        assert_eq!(items[0].content, "<p>full</p>");
        assert!(db.is_read("https://f.com", "a").unwrap());
    }
}

#[cfg(test)]
mod flag_tests {
    use super::*;

    fn db() -> Db {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!("markerss-flags-{}-{}.db", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::remove_file(&path);
        Db::open(&path).unwrap()
    }

    fn item(guid: &str) -> Item {
        Item { guid: guid.into(), title: String::new(), url: String::new(), summary: String::new(), content: String::new(), date: String::new(), author: String::new(), read_later: false, saved: false }
    }

    #[test]
    fn flags_toggle_and_persist() {
        let mut d = db();
        d.replace_feed_items_preserving_read("f", &[item("a"), item("b")]).unwrap();
        assert!(d.toggle_flag("f", "a", "read_later").unwrap());
        assert!(d.toggle_flag("f", "b", "saved").unwrap());
        let later = d.items_with_flag("read_later").unwrap();
        assert_eq!(later.len(), 1);
        assert_eq!(later[0].0, "f");
        assert!(later[0].1.read_later);
        let saved = d.items_with_flag("saved").unwrap();
        assert_eq!(saved.len(), 1);
        // refresh preserves flags
        d.replace_feed_items_preserving_read("f", &[item("a"), item("b"), item("c")]).unwrap();
        assert_eq!(d.items_with_flag("read_later").unwrap().len(), 1);
        assert_eq!(d.items_with_flag("saved").unwrap().len(), 1);
        // toggle off
        assert!(!d.toggle_flag("f", "a", "read_later").unwrap());
        assert_eq!(d.items_with_flag("read_later").unwrap().len(), 0);
    }

    #[test]
    fn old_db_migrates_columns() {
        // create a pre-flag db, reopen → columns added
        let path = std::env::temp_dir().join(format!("markerss-migrate-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let c = rusqlite::Connection::open(&path).unwrap();
            c.execute_batch("CREATE TABLE items (feed_url TEXT NOT NULL, guid TEXT NOT NULL, title TEXT NOT NULL DEFAULT '', url TEXT NOT NULL DEFAULT '', summary TEXT NOT NULL DEFAULT '', content TEXT NOT NULL DEFAULT '', date TEXT NOT NULL DEFAULT '', read INTEGER NOT NULL DEFAULT 0, fetched_at TEXT NOT NULL DEFAULT '', PRIMARY KEY (feed_url, guid));").unwrap();
        }
        let mut d = Db::open(&path).unwrap();
        d.replace_feed_items_preserving_read("f", &[item("x")]).unwrap();
        assert_eq!(d.items_with_flag("saved").unwrap().len(), 0);
        std::fs::remove_file(&path).ok();
    }
}

#[cfg(test)]
mod upsert_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);

    fn db() -> Db {
        let p = std::env::temp_dir().join(format!("markerss-upsert-{}-{}.db", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::remove_file(&p);
        Db::open(&p).unwrap()
    }

    fn item(guid: &str, title: &str) -> Item {
        Item { guid: guid.into(), title: title.into(), url: String::new(), summary: String::new(), content: String::new(), date: "2026-01-01".into(), author: String::new(), read_later: false, saved: false }
    }

    #[test]
    fn upsert_adds_new_keeps_existing() {
        let mut d = db();
        d.replace_feed_items_preserving_read("f", &[item("a", "old title")]).unwrap();
        d.set_read("f", "a", true).unwrap();
        d.set_flag("f", "a", "saved", true).unwrap();
        d.update_item_content("f", "a", "<p>content</p>").unwrap();
        // fetch: new guid b added, a updated metadata only
        let added = d.upsert_fetch("f", &[item("a", "new title"), item("b", "new")]).unwrap();
        assert_eq!(added, vec!["b"]);
        let items = d.items_for_feed("f").unwrap();
        assert_eq!(items.len(), 2);
        let a = items.iter().find(|i| i.guid == "a").unwrap();
        assert_eq!(a.title, "new title"); // metadata updated
        assert!(d.is_read("f", "a").unwrap()); // read preserved
        assert_eq!(d.items_with_flag("saved").unwrap().len(), 1); // flag preserved
        let a2 = items.iter().find(|i| i.guid == "a").unwrap();
        assert_eq!(a2.content, "<p>content</p>"); // content preserved
        let b = items.iter().find(|i| i.guid == "b").unwrap();
        assert!(!d.is_read("f", "b").unwrap()); // new item unread
    }

    #[test]
    fn ttl_purge_clears_only_old_non_saved_content() {
        let mut d = db();
        d.replace_feed_items_preserving_read("f", &[item("a", "A")]).unwrap();
        d.update_item_content("f", "a", "<p>fresh</p>").unwrap();
        d.upsert_fetch("f", &[item("b", "B")]).unwrap();
        d.update_item_content("f", "b", "<p>old</p>").unwrap();
        // age b's content beyond the TTL by backdating fetched_at
        let old = (chrono::Utc::now() - chrono::Duration::days(99)).to_rfc3339();
        d.conn.execute("UPDATE items SET fetched_at = ?1 WHERE guid = 'b'", [old]).unwrap();
        // saved items are exempt
        d.set_flag("f", "a", "saved", true).unwrap();
        d.cleanup_content(30).unwrap();
        let items = d.items_for_feed("f").unwrap();
        let a = items.iter().find(|i| i.guid == "a").unwrap();
        let b = items.iter().find(|i| i.guid == "b").unwrap();
        assert_eq!(a.content, "<p>fresh</p>"); // saved + fresh → kept
        assert_eq!(b.content, ""); // old non-saved → purged
        // item rows stay; only content is cleared
        assert_eq!(items.len(), 2);
    }
}
