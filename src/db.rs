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
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS items (
                feed_url TEXT NOT NULL,
                guid     TEXT NOT NULL,
                title    TEXT NOT NULL DEFAULT '',
                url      TEXT NOT NULL DEFAULT '',
                summary  TEXT NOT NULL DEFAULT '',
                content  TEXT NOT NULL DEFAULT '',
                date     TEXT NOT NULL DEFAULT '',
                read     INTEGER NOT NULL DEFAULT 0,
                fetched_at TEXT NOT NULL DEFAULT '',
                PRIMARY KEY (feed_url, guid)
            );
            CREATE INDEX IF NOT EXISTS idx_items_feed ON items(feed_url, read);
            ",
        )?;
        Ok(Db { conn })
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
            {
                let mut q = tx.prepare("SELECT guid, read, content FROM items WHERE feed_url = ?1")?;
                let rows = q.query_map([feed_url], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?))
                })?;
                for row in rows.flatten() {
                    if row.1 != 0 {
                        read_guids.insert(row.0.clone());
                    }
                    if !row.2.is_empty() {
                        content_map.insert(row.0, row.2);
                    }
                }
            }
            let mut del = tx.prepare("DELETE FROM items WHERE feed_url = ?1")?;
            del.execute([feed_url])?;
            let mut ins = tx.prepare(
                "INSERT OR REPLACE INTO items
                 (feed_url, guid, title, url, summary, content, date, read)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?;
            for i in items {
                let read = if read_guids.contains(&i.guid) { 1 } else { 0 };
                // keep previously fetched content; new items are summary-only
                let content = content_map.get(&i.guid).cloned().unwrap_or_default();
                ins.execute(rusqlite::params![
                    feed_url,
                    i.guid,
                    i.title,
                    i.url,
                    i.summary,
                    content,
                    i.date,
                    read
                ])?;
            }
        }
        tx.commit()
    }

    /// Update an item's content (fetched full article) — preserves read flag.
    pub fn update_item_content(&mut self, feed_url: &str, guid: &str, content: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE items SET content = ?3 WHERE feed_url = ?1 AND guid = ?2",
            rusqlite::params![feed_url, guid, content],
        )?;
        Ok(())
    }

    pub fn items_for_feed(&self, feed_url: &str) -> rusqlite::Result<Vec<Item>> {
        let mut stmt = self
            .conn
            .prepare("SELECT guid, title, url, summary, content, date FROM items WHERE feed_url = ?1")?;
        let rows = stmt.query_map([feed_url], |r| {
            Ok(Item {
                guid: r.get(0)?,
                title: r.get(1)?,
                url: r.get(2)?,
                summary: r.get(3)?,
                content: r.get(4)?,
                date: r.get(5)?,
            })
        })?;
        rows.collect()
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
    pub fn cleanup_content(&mut self, ttl_days: u64) -> rusqlite::Result<()> {
        if ttl_days == 0 {
            return Ok(());
        }
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(ttl_days as i64))
            .to_rfc3339();
        self.conn.execute(
            "UPDATE items SET content = '' WHERE fetched_at != '' AND fetched_at < ?1",
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
